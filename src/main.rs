//! reel2mp3 — a dashboard for turning Instagram reels into MP3s.
//!
//! Links are pasted into the staging panel, reviewed there, then started as a
//! batch. Each job runs yt-dlp, keeps the audio and throws the video away.

mod graph;
mod job;
mod theme;
mod ui;

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::TableState;

use job::Msg;

const TICK: Duration = Duration::from_millis(100);
/// 100ms samples; the waveforms show the most recent 15 seconds of them.
const NET_HISTORY: usize = 1800;
pub const GRAPH_WINDOW: usize = 150;
const JOB_HISTORY: usize = 96;
const COOKIE_CHOICES: [Option<&str>; 5] = [
    None,
    Some("chromium"),
    Some("firefox"),
    Some("chrome"),
    Some("brave"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Queued,
    Fetching,
    Downloading,
    Converting,
    Done,
    Failed,
    Canceled,
}

impl Status {
    pub fn active(self) -> bool {
        matches!(self, Self::Fetching | Self::Downloading | Self::Converting)
    }
}

/// A link waiting in the staging panel, not yet handed to yt-dlp.
pub struct Staged {
    pub url: String,
    pub dup: bool,
}

pub struct Job {
    pub id: usize,
    pub url: String,
    pub status: Status,
    pub title: Option<String>,
    pub uploader: Option<String>,
    pub duration: Option<f64>,
    pub done: u64,
    pub last_done: u64,
    pub total: Option<u64>,
    pub speed: f64,
    pub rate: f64,
    pub eta: Option<u64>,
    pub file: Option<PathBuf>,
    pub size: Option<u64>,
    pub error: Option<String>,
    pub log: VecDeque<String>,
    pub hist: VecDeque<f64>,
    pub started: Option<Instant>,
    pub elapsed: Option<Duration>,
    /// Bytes actually crossed the wire (false when yt-dlp reused a local file).
    pub got_bytes: bool,
    pub cached: bool,
}

impl Job {
    fn new(id: usize, url: String) -> Self {
        Self {
            id,
            url,
            status: Status::Queued,
            title: None,
            uploader: None,
            duration: None,
            done: 0,
            last_done: 0,
            total: None,
            speed: 0.0,
            rate: 0.0,
            eta: None,
            file: None,
            size: None,
            error: None,
            log: VecDeque::new(),
            hist: VecDeque::new(),
            started: None,
            elapsed: None,
            got_bytes: false,
            cached: false,
        }
    }

    /// Download completion, 0..1. None while the total size is unknown.
    pub fn frac(&self) -> Option<f64> {
        match self.status {
            Status::Done => Some(1.0),
            _ => self
                .total
                .filter(|t| *t > 0)
                .map(|t| (self.done as f64 / t as f64).clamp(0.0, 1.0)),
        }
    }

    /// Display name: the reel's title until yt-dlp reports one, then the URL.
    pub fn label(&self) -> &str {
        self.title
            .as_deref()
            .unwrap_or_else(|| short_url(&self.url))
    }

    fn reset(&mut self) {
        let url = std::mem::take(&mut self.url);
        let id = self.id;
        *self = Job::new(id, url);
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Focus {
    Add,
    Queue,
    Input,
}

pub struct Flash {
    pub text: String,
    pub color: Color,
    pub at: Instant,
}

pub struct App {
    pub jobs: Vec<Job>,
    pub staged: Vec<Staged>,
    pub staged_sel: usize,
    pub next_id: usize,
    pub table: TableState,
    pub focus: Focus,
    pub input: String,
    pub cookies: usize,
    pub workers: usize,
    pub out_dir: PathBuf,
    pub net_hist: VecDeque<f64>,
    /// Smoothed transfer rate: what the waveform plots.
    pub flow_hist: VecDeque<f64>,
    /// Busy workers over time, the load waveform under it.
    pub load_hist: VecDeque<f64>,
    flow: f64,
    load: f64,
    pub net_peak: f64,
    pub scale: f64,
    pub session_bytes: u64,
    pub library: (usize, u64),
    pub flash: Option<Flash>,
    pub started: Instant,
    pub tick: u64,
    /// Clickable regions, refreshed every frame by the renderer.
    pub hit_start: Rect,
    pub hit_staged: Rect,
    pub hit_rows: Rect,
    quit_armed: Option<Instant>,
    should_quit: bool,
    children: HashMap<usize, Arc<Mutex<Child>>>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl App {
    fn new(out_dir: PathBuf, cookies: usize) -> Self {
        let (tx, rx) = channel();
        let mut app = Self {
            jobs: Vec::new(),
            staged: Vec::new(),
            staged_sel: 0,
            next_id: 0,
            table: TableState::default().with_selected(0),
            focus: Focus::Add,
            input: String::new(),
            cookies,
            workers: 2,
            out_dir,
            net_hist: VecDeque::from(vec![0.0; NET_HISTORY]),
            flow_hist: VecDeque::from(vec![0.0; NET_HISTORY]),
            load_hist: VecDeque::from(vec![0.0; NET_HISTORY]),
            flow: 0.0,
            load: 0.0,
            net_peak: 0.0,
            scale: 512.0 * 1024.0,
            session_bytes: 0,
            library: (0, 0),
            flash: None,
            started: Instant::now(),
            tick: 0,
            hit_start: Rect::ZERO,
            hit_staged: Rect::ZERO,
            hit_rows: Rect::ZERO,
            quit_armed: None,
            should_quit: false,
            children: HashMap::new(),
            tx,
            rx,
        };
        app.scan_library();
        app
    }

    pub fn cookie_name(&self) -> &'static str {
        COOKIE_CHOICES[self.cookies].unwrap_or("none")
    }

    pub fn selected(&self) -> Option<&Job> {
        self.jobs.get(self.table.selected().unwrap_or(0))
    }

    pub fn ready_count(&self) -> usize {
        self.staged.iter().filter(|s| !s.dup).count()
    }

    fn flash(&mut self, text: impl Into<String>, color: Color) {
        self.flash = Some(Flash {
            text: text.into(),
            color,
            at: Instant::now(),
        });
    }

    fn scan_library(&mut self) {
        let mut count = 0;
        let mut bytes = 0;
        if let Ok(entries) = std::fs::read_dir(&self.out_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().is_some_and(|e| e == "mp3") {
                    count += 1;
                    bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }
        self.library = (count, bytes);
    }

    // ---- staging ---------------------------------------------------------

    fn stage(&mut self, text: &str) {
        let mut added = 0;
        let mut repeats = 0;
        for url in extract_urls(text) {
            let key = normalize(&url);
            if self.staged.iter().any(|s| normalize(&s.url) == key) {
                repeats += 1;
                continue;
            }
            let dup = self.jobs.iter().any(|j| normalize(&j.url) == key);
            self.staged.push(Staged { url, dup });
            added += 1;
        }
        match (added, repeats) {
            (0, 0) => self.flash("no links found in that text", theme::WARN),
            (0, r) => self.flash(format!("already staged ({r})"), theme::WARN),
            (a, _) => self.flash(format!("+{a} staged — press enter to start"), theme::OK),
        }
        self.staged_sel = self.staged_sel.min(self.staged.len().saturating_sub(1));
    }

    fn start_staged(&mut self) {
        if self.staged.is_empty() {
            self.flash("nothing staged — paste some links first", theme::WARN);
            return;
        }
        let skipped = self.staged.len() - self.ready_count();
        let urls: Vec<String> = self
            .staged
            .drain(..)
            .filter(|s| !s.dup)
            .map(|s| s.url)
            .collect();
        if urls.is_empty() {
            self.flash("those are already in the queue", theme::WARN);
            return;
        }
        let started = urls.len();
        for url in urls {
            let id = self.next_id;
            self.next_id += 1;
            self.jobs.push(Job::new(id, url));
        }
        self.staged_sel = 0;
        let note = if skipped > 0 {
            format!("started {started} · skipped {skipped} already queued")
        } else {
            format!("started {started}")
        };
        self.flash(note, theme::ACCENT);
        if self.table.selected().is_none() {
            self.table.select(Some(0));
        }
    }

    fn drop_staged(&mut self) {
        if self.staged.is_empty() {
            return;
        }
        let i = self.staged_sel.min(self.staged.len() - 1);
        self.staged.remove(i);
        self.staged_sel = i.min(self.staged.len().saturating_sub(1));
    }

    // ---- queue -----------------------------------------------------------

    /// Start queued jobs until the worker limit is reached.
    fn schedule(&mut self) {
        loop {
            let active = self.jobs.iter().filter(|j| j.status.active()).count();
            if active >= self.workers {
                return;
            }
            let Some(idx) = self.jobs.iter().position(|j| j.status == Status::Queued) else {
                return;
            };
            let (id, url) = (self.jobs[idx].id, self.jobs[idx].url.clone());
            let cookies = COOKIE_CHOICES[self.cookies];
            match job::spawn(id, &url, &self.out_dir, cookies, self.tx.clone()) {
                Ok(child) => {
                    self.children.insert(id, child);
                    let j = &mut self.jobs[idx];
                    j.status = Status::Fetching;
                    j.started = Some(Instant::now());
                }
                Err(e) => {
                    let j = &mut self.jobs[idx];
                    j.status = Status::Failed;
                    j.error = Some(format!("could not start yt-dlp: {e}"));
                }
            }
        }
    }

    fn job_mut(&mut self, id: usize) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Meta {
                id,
                uploader,
                duration,
                title,
            } => {
                if let Some(j) = self.job_mut(id) {
                    j.uploader = uploader;
                    j.duration = duration;
                    j.title = title;
                }
            }
            Msg::Progress {
                id,
                done,
                total,
                speed,
                eta,
            } => {
                if let Some(j) = self.job_mut(id) {
                    if j.status != Status::Converting {
                        j.status = Status::Downloading;
                    }
                    j.done = done;
                    j.got_bytes |= done > 0;
                    j.total = total.or(j.total);
                    j.speed = speed.unwrap_or(0.0);
                    j.eta = eta;
                }
            }
            Msg::Post { id, status } => {
                if let Some(j) = self.job_mut(id)
                    && status != "finished"
                {
                    j.status = Status::Converting;
                    j.speed = 0.0;
                    j.eta = None;
                }
            }
            Msg::File { id, path } => {
                let size = std::fs::metadata(&path).map(|m| m.len()).ok();
                if let Some(j) = self.job_mut(id) {
                    j.size = size;
                    j.file = Some(path);
                }
            }
            Msg::Error { id, msg } => {
                if let Some(j) = self.job_mut(id) {
                    j.error = Some(msg);
                }
            }
            Msg::Log { id, line } => {
                if let Some(j) = self.job_mut(id) {
                    if j.log.len() == 8 {
                        j.log.pop_front();
                    }
                    j.log.push_back(line);
                }
            }
            Msg::Exit { id, code } => {
                self.children.remove(&id);
                let mut finished_bytes = 0;
                if let Some(j) = self.job_mut(id) {
                    j.speed = 0.0;
                    j.rate = 0.0;
                    j.eta = None;
                    j.elapsed = j.started.map(|s| s.elapsed());
                    if j.status == Status::Canceled {
                        // left as canceled on purpose
                    } else if code == Some(0) {
                        j.status = Status::Done;
                        j.cached = !j.got_bytes;
                        finished_bytes = j.size.unwrap_or(0);
                        if j.file.is_none() {
                            j.error = Some("already saved earlier — nothing to download".into());
                        }
                    } else {
                        j.status = Status::Failed;
                        if j.error.is_none() {
                            j.error = Some(match code {
                                Some(c) => format!("yt-dlp exited with code {c}"),
                                None => "yt-dlp was terminated".into(),
                            });
                        }
                    }
                }
                if finished_bytes > 0 {
                    self.session_bytes += finished_bytes;
                    self.scan_library();
                }
            }
        }
    }

    fn cancel_or_remove(&mut self) {
        let Some(idx) = self.table.selected() else {
            return;
        };
        let Some(j) = self.jobs.get_mut(idx) else {
            return;
        };
        if j.status.active() {
            j.status = Status::Canceled;
            j.error = Some("canceled".into());
            let id = j.id;
            if let Some(child) = self.children.get(&id) {
                job::kill(child);
            }
            self.flash("canceled", theme::WARN);
        } else {
            self.jobs.remove(idx);
            let len = self.jobs.len();
            self.table.select(if len == 0 {
                Some(0)
            } else {
                Some(idx.min(len - 1))
            });
        }
    }

    fn retry(&mut self, all: bool) {
        let targets: Vec<usize> = if all {
            self.jobs
                .iter()
                .enumerate()
                .filter(|(_, j)| matches!(j.status, Status::Failed | Status::Canceled))
                .map(|(i, _)| i)
                .collect()
        } else {
            self.table
                .selected()
                .filter(|i| {
                    self.jobs
                        .get(*i)
                        .is_some_and(|j| !j.status.active() && j.status != Status::Queued)
                })
                .into_iter()
                .collect()
        };
        if targets.is_empty() {
            self.flash("nothing to retry", theme::WARN);
            return;
        }
        for i in &targets {
            self.jobs[*i].reset();
        }
        self.flash(format!("retrying {}", targets.len()), theme::CYAN);
    }

    fn clear_finished(&mut self) {
        let before = self.jobs.len();
        self.jobs.retain(|j| j.status != Status::Done);
        let removed = before - self.jobs.len();
        let len = self.jobs.len();
        self.table.select(Some(
            self.table
                .selected()
                .unwrap_or(0)
                .min(len.saturating_sub(1)),
        ));
        self.flash(format!("cleared {removed}"), theme::DIM);
    }

    fn move_sel(&mut self, delta: isize) {
        match self.focus {
            Focus::Add => {
                if self.staged.is_empty() {
                    return;
                }
                let next =
                    (self.staged_sel as isize + delta).clamp(0, self.staged.len() as isize - 1);
                self.staged_sel = next as usize;
            }
            _ => {
                if self.jobs.is_empty() {
                    return;
                }
                let cur = self.table.selected().unwrap_or(0) as isize;
                let next = (cur + delta).clamp(0, self.jobs.len() as isize - 1);
                self.table.select(Some(next as usize));
            }
        }
    }

    fn open(&mut self, path: &Path) {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        match cmd.spawn() {
            Ok(_) => self.flash("opening…", theme::CYAN),
            Err(e) => self.flash(format!("could not open: {e}"), theme::ERR),
        }
    }

    fn play_selected(&mut self) {
        match self.selected().and_then(|j| j.file.clone()) {
            Some(file) => self.open(&file),
            None => self.flash("no file for that row yet", theme::WARN),
        }
    }

    fn paste_clipboard(&mut self) {
        let text = ["wl-paste", "xclip", "xsel"].iter().find_map(|tool| {
            let args: &[&str] = match *tool {
                "wl-paste" => &["--no-newline"],
                "xclip" => &["-o", "-selection", "clipboard"],
                _ => &["-b", "-o"],
            };
            Command::new(tool)
                .args(args)
                .stderr(Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        });
        match text {
            Some(t) => self.stage(&t),
            None => self.flash("clipboard unavailable", theme::ERR),
        }
    }

    fn quit(&mut self) {
        let active = self.jobs.iter().filter(|j| j.status.active()).count();
        let armed = self
            .quit_armed
            .is_some_and(|t| t.elapsed() < Duration::from_secs(3));
        if active > 0 && !armed {
            self.quit_armed = Some(Instant::now());
            self.flash(
                format!("{active} still running — press q again to quit"),
                theme::WARN,
            );
            return;
        }
        for child in self.children.values() {
            job::kill(child);
        }
        self.should_quit = true;
    }

    // ---- per-tick bookkeeping -------------------------------------------

    fn on_tick(&mut self, dt: f64) {
        self.tick += 1;
        // Measure throughput from bytes actually arrived, rather than trusting
        // yt-dlp's own sampling, which reports in bursts.
        let mut total = 0.0;
        for j in &mut self.jobs {
            let inst = if j.done > j.last_done {
                (j.done - j.last_done) as f64 / dt
            } else {
                0.0
            };
            j.last_done = j.done;
            j.rate = j.rate * 0.5 + inst * 0.5;
            total += inst;
            if j.hist.len() == JOB_HISTORY {
                j.hist.pop_front();
            }
            j.hist.push_back(j.rate);
        }
        if self.net_hist.len() == NET_HISTORY {
            self.net_hist.pop_front();
        }
        self.net_hist.push_back(total);

        // A reel's bytes land inside a single tick, so the raw series is all
        // spikes. Low-pass it into a flow rate — the area under the curve is
        // still the bytes moved, but it reads as a wave instead of a needle.
        self.flow = self.flow * 0.95 + total * 0.05;
        let active = self.jobs.iter().filter(|j| j.status.active()).count() as f64;
        self.load = self.load * 0.75 + active * 0.25;
        for (hist, value) in [
            (&mut self.flow_hist, self.flow),
            (&mut self.load_hist, self.load),
        ] {
            if hist.len() == NET_HISTORY {
                hist.pop_front();
            }
            hist.push_back(value);
        }

        self.net_peak = self.net_peak.max(self.flow);

        // Ease the graph ceiling toward the busiest moment on screen, so the
        // curve fills the panel instead of hugging the floor after one spike.
        let window: Vec<f64> = self
            .flow_hist
            .iter()
            .rev()
            .take(GRAPH_WINDOW)
            .copied()
            .collect();
        let target = (window.iter().copied().fold(0.0_f64, f64::max) * 1.25).max(256.0 * 1024.0);
        self.scale += (target - self.scale) * 0.08;

        if self
            .flash
            .as_ref()
            .is_some_and(|f| f.at.elapsed() > Duration::from_secs(4))
        {
            self.flash = None;
        }
    }

    pub fn net_speed(&self) -> f64 {
        self.flow
    }

    pub fn load(&self) -> f64 {
        self.load
    }

    // ---- input -----------------------------------------------------------

    fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(key.code, KeyCode::Char('c')) {
            for child in self.children.values() {
                job::kill(child);
            }
            self.should_quit = true;
            return;
        }
        if ctrl && matches!(key.code, KeyCode::Char('v')) {
            self.paste_clipboard();
            return;
        }

        if self.focus == Focus::Input {
            match key.code {
                KeyCode::Esc => {
                    self.focus = Focus::Add;
                    self.input.clear();
                }
                KeyCode::Enter => {
                    let text = std::mem::take(&mut self.input);
                    self.stage(&text);
                    self.focus = Focus::Add;
                }
                KeyCode::Backspace => {
                    self.input.pop();
                }
                KeyCode::Char('u') if ctrl => self.input.clear(),
                KeyCode::Char(c) => self.input.push(c),
                _ => {}
            }
            return;
        }

        match key.code {
            // panel-specific
            KeyCode::Enter if self.focus == Focus::Add || !self.staged.is_empty() => {
                self.start_staged()
            }
            KeyCode::Enter => self.play_selected(),
            KeyCode::Char('s') => self.start_staged(),
            KeyCode::Char('d') | KeyCode::Delete | KeyCode::Backspace
                if self.focus == Focus::Add =>
            {
                self.drop_staged()
            }
            KeyCode::Char('x') if self.focus == Focus::Add => {
                self.staged.clear();
                self.flash("staging cleared", theme::DIM);
            }
            KeyCode::Char('d') | KeyCode::Delete => self.cancel_or_remove(),
            KeyCode::Char('x') => self.clear_finished(),
            KeyCode::Char('r') => self.retry(false),
            KeyCode::Char('R') => self.retry(true),
            KeyCode::Char('p') => self.play_selected(),

            // shared
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = if self.focus == Focus::Add {
                    Focus::Queue
                } else {
                    Focus::Add
                };
            }
            KeyCode::Char('a') | KeyCode::Char('i') | KeyCode::Char('/') => {
                self.focus = Focus::Input;
            }
            KeyCode::Char('v') => self.paste_clipboard(),
            KeyCode::Up | KeyCode::Char('k') => self.move_sel(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_sel(1),
            KeyCode::PageUp => self.move_sel(-10),
            KeyCode::PageDown => self.move_sel(10),
            KeyCode::Home => self.move_sel(-9999),
            KeyCode::End => self.move_sel(9999),
            KeyCode::Char('q') | KeyCode::Esc => self.quit(),
            KeyCode::Char('o') => {
                let dir = self.out_dir.clone();
                self.open(&dir);
            }
            KeyCode::Char('c') => {
                self.cookies = (self.cookies + 1) % COOKIE_CHOICES.len();
                let name = self.cookie_name();
                self.flash(format!("cookies: {name}"), theme::VIOLET);
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.workers = (self.workers + 1).min(6);
                let n = self.workers;
                self.flash(format!("{n} workers"), theme::CYAN);
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                self.workers = self.workers.saturating_sub(1).max(1);
                let n = self.workers;
                self.flash(format!("{n} workers"), theme::CYAN);
            }
            _ => {}
        }
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        let at = |r: Rect| {
            r.width > 0
                && m.column >= r.x
                && m.column < r.x + r.width
                && m.row >= r.y
                && m.row < r.y + r.height
        };
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if at(self.hit_start) {
                    self.start_staged();
                } else if at(self.hit_staged) {
                    self.focus = Focus::Add;
                    let i = (m.row - self.hit_staged.y) as usize;
                    if i < self.staged.len() {
                        self.staged_sel = i;
                    }
                } else if at(self.hit_rows) {
                    self.focus = Focus::Queue;
                    let i = self.table.offset() + (m.row - self.hit_rows.y) as usize;
                    if i < self.jobs.len() {
                        self.table.select(Some(i));
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if at(self.hit_staged) {
                    self.focus = Focus::Add;
                }
                if at(self.hit_rows) {
                    self.focus = Focus::Queue;
                }
                self.move_sel(2);
            }
            MouseEventKind::ScrollUp => {
                if at(self.hit_staged) {
                    self.focus = Focus::Add;
                }
                if at(self.hit_rows) {
                    self.focus = Focus::Queue;
                }
                self.move_sel(-2);
            }
            _ => {}
        }
    }
}

// ---- helpers -------------------------------------------------------------

pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

pub fn human_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec < 1024.0 {
        return "—".into();
    }
    format!("{}/s", human_bytes(bytes_per_sec as u64))
}

pub fn human_secs(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}

pub fn short_url(url: &str) -> &str {
    let s = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.");
    s.split('?').next().unwrap_or(s).trim_end_matches('/')
}

pub fn home_relative(path: &Path) -> String {
    match std::env::var_os("HOME").map(PathBuf::from) {
        Some(home) => match path.strip_prefix(&home) {
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        },
        None => path.display().to_string(),
    }
}

/// Local wall clock, without pulling in a date crate.
pub fn clock() -> String {
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&now, &mut tm) };
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}

fn extract_urls(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|t| t.starts_with("http://") || t.starts_with("https://"))
        .map(|t| {
            t.trim_end_matches([')', ']', '}', ',', '.', ';', '"', '\'', '>'])
                .to_string()
        })
        .filter(|t| t.len() > 12)
        .collect()
}

/// Identity for duplicate checks: scheme, www, query and trailing slash dropped.
fn normalize(url: &str) -> String {
    short_url(url).trim_end_matches('/').to_lowercase()
}

fn main() -> std::io::Result<()> {
    let mut out_dir = std::env::current_dir()?;
    let mut cookies = 0;
    let mut urls: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-c" | "--cookies" => {
                let name = args.next().unwrap_or_default();
                match COOKIE_CHOICES
                    .iter()
                    .position(|c| *c == Some(name.as_str()))
                {
                    Some(i) => cookies = i,
                    None => {
                        eprintln!("unknown browser: {name}");
                        std::process::exit(2);
                    }
                }
            }
            "-o" | "--out" => {
                out_dir = PathBuf::from(args.next().unwrap_or_default());
            }
            "-h" | "--help" => {
                println!(
                    "reel2mp3 — Instagram reels to MP3\n\n\
                     usage: reel2mp3 [--cookies BROWSER] [--out DIR] [URL…]\n\n\
                     browsers: chromium, firefox, chrome, brave"
                );
                return Ok(());
            }
            other => urls.push(other.to_string()),
        }
    }

    if Command::new("yt-dlp").arg("--version").output().is_err() {
        eprintln!("yt-dlp not found — install it with: sudo pacman -S yt-dlp");
        std::process::exit(1);
    }
    std::fs::create_dir_all(&out_dir)?;

    let mut app = App::new(out_dir.canonicalize().unwrap_or(out_dir), cookies);
    if !urls.is_empty() {
        app.stage(&urls.join(" "));
    }

    let mut terminal = ratatui::init();
    execute!(std::io::stdout(), EnableBracketedPaste, EnableMouseCapture)?;
    let result = run(&mut terminal, &mut app);
    let _ = execute!(
        std::io::stdout(),
        DisableBracketedPaste,
        DisableMouseCapture
    );
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        let timeout = TICK.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => app.on_key(key),
                Event::Mouse(m) => app.on_mouse(m),
                Event::Paste(text) => {
                    if app.focus == Focus::Input && extract_urls(&text).is_empty() {
                        app.input.push_str(text.trim());
                    } else {
                        app.stage(&text);
                    }
                }
                _ => {}
            }
        }

        while let Ok(msg) = app.rx.try_recv() {
            app.handle(msg);
        }
        let elapsed = last_tick.elapsed();
        if elapsed >= TICK {
            app.on_tick(elapsed.as_secs_f64());
            last_tick = Instant::now();
        }
        app.schedule();

        if app.should_quit {
            return Ok(());
        }
    }
}
