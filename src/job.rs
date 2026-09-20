//! Runs one yt-dlp process per job and turns its output into messages.
//!
//! yt-dlp is asked to emit tagged lines (`@DL`, `@PP`, `@META`, `@FILE`) via
//! --progress-template and --print, which is far steadier to parse than its
//! human-facing output. Progress lands on stdout or stderr depending on the
//! flags in play, so both streams are read and parsed the same way.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug)]
pub enum Msg {
    Meta {
        id: usize,
        uploader: Option<String>,
        duration: Option<f64>,
        title: Option<String>,
    },
    Progress {
        id: usize,
        done: u64,
        total: Option<u64>,
        speed: Option<f64>,
        eta: Option<u64>,
    },
    Post {
        id: usize,
        status: String,
    },
    File {
        id: usize,
        path: PathBuf,
    },
    Error {
        id: usize,
        msg: String,
    },
    Log {
        id: usize,
        line: String,
    },
    Exit {
        id: usize,
        code: Option<i32>,
    },
}

const DL_TEMPLATE: &str = "download:@DL %(progress.downloaded_bytes)s %(progress.total_bytes)s \
                           %(progress.total_bytes_estimate)s %(progress.speed)s %(progress.eta)s";
const PP_TEMPLATE: &str = "postprocess:@PP %(progress.status)s";
const META_TEMPLATE: &str = "before_dl:@META %(uploader)s\t%(duration)s\t%(title)s";
const FILE_TEMPLATE: &str = "after_move:@FILE %(filepath)s";

pub fn spawn(
    id: usize,
    url: &str,
    out_dir: &Path,
    cookies: Option<&str>,
    tx: Sender<Msg>,
) -> std::io::Result<Arc<Mutex<Child>>> {
    let mut cmd = Command::new("yt-dlp");
    cmd.args([
        "--newline",
        "--progress",
        "--color",
        "never",
        "--no-warnings",
    ])
    .args(["-f", "bestaudio/best"])
    .args(["-x", "--audio-format", "mp3", "--audio-quality", "0"])
    .arg("-P")
    .arg(out_dir)
    .args(["-o", "%(title).80B [%(id)s].%(ext)s"])
    .args(["--progress-template", DL_TEMPLATE])
    .args(["--progress-template", PP_TEMPLATE])
    .args(["--print", META_TEMPLATE])
    .args(["--print", FILE_TEMPLATE]);
    if let Some(browser) = cookies {
        cmd.args(["--cookies-from-browser", browser]);
    }
    cmd.arg("--").arg(url);

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Own process group: one kill(-pgid) also stops the ffmpeg it spawns, and
    // terminal signals stay with the TUI.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child = Arc::new(Mutex::new(child));

    let mut readers = Vec::new();
    for stream in [
        stdout.map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        stderr.map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        readers.push(thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if let Some(msg) = parse(id, line.trim_end())
                    && tx.send(msg).is_err()
                {
                    break;
                }
            }
        }));
    }

    // Reap the process once both streams hit EOF, so `kill` never races `wait`.
    let waiter = Arc::clone(&child);
    thread::spawn(move || {
        for reader in readers {
            let _ = reader.join();
        }
        let code = waiter.lock().ok().and_then(|mut c| c.wait().ok());
        let _ = tx.send(Msg::Exit {
            id,
            code: code.and_then(|s| s.code()),
        });
    });

    Ok(child)
}

/// Terminate the whole process group.
pub fn kill(child: &Arc<Mutex<Child>>) {
    #[cfg(unix)]
    if let Ok(c) = child.lock() {
        unsafe {
            libc::kill(-(c.id() as i32), libc::SIGTERM);
        }
    }
    #[cfg(not(unix))]
    if let Ok(mut c) = child.lock() {
        let _ = c.kill();
    }
}

fn num(field: Option<&str>) -> Option<f64> {
    match field {
        Some("NA") | Some("None") | Some("") | None => None,
        Some(v) => v.parse::<f64>().ok(),
    }
}

fn text(field: Option<&str>) -> Option<String> {
    match field {
        Some("NA") | Some("None") | Some("") | None => None,
        Some(v) => Some(v.to_string()),
    }
}

fn parse(id: usize, line: &str) -> Option<Msg> {
    if let Some(rest) = line.strip_prefix("@DL ") {
        let f: Vec<&str> = rest.split_whitespace().collect();
        let total = num(f.get(1).copied()).or_else(|| num(f.get(2).copied()));
        return Some(Msg::Progress {
            id,
            done: num(f.first().copied()).unwrap_or(0.0) as u64,
            total: total.map(|t| t as u64),
            speed: num(f.get(3).copied()),
            eta: num(f.get(4).copied()).map(|e| e as u64),
        });
    }
    if let Some(rest) = line.strip_prefix("@PP ") {
        return Some(Msg::Post {
            id,
            status: rest.split_whitespace().next().unwrap_or("").to_string(),
        });
    }
    if let Some(rest) = line.strip_prefix("@META ") {
        let mut f = rest.split('\t');
        return Some(Msg::Meta {
            id,
            uploader: text(f.next()),
            duration: num(f.next()),
            title: text(f.next()),
        });
    }
    if let Some(rest) = line.strip_prefix("@FILE ") {
        return Some(Msg::File {
            id,
            path: PathBuf::from(rest),
        });
    }
    if let Some(rest) = line.strip_prefix("ERROR: ") {
        return Some(Msg::Error {
            id,
            msg: clean_error(rest),
        });
    }
    if line.is_empty() {
        return None;
    }
    Some(Msg::Log {
        id,
        line: line.to_string(),
    })
}

/// Strip yt-dlp's "[Instagram] abc123: " prefixes and the advice tail.
fn clean_error(msg: &str) -> String {
    let mut out = msg.trim();
    // "[Extractor] ..." — drop the tag, then the video id that may follow it.
    if let Some(rest) = out.strip_prefix('[').and_then(|r| r.split_once("] ")) {
        out = rest.1;
        if let Some((id, tail)) = out.split_once(": ")
            && !id.contains(' ')
        {
            out = tail;
        }
    }
    // Drop the "see this wiki page / report this issue" tail: everything from
    // the first link onward is advice for a bug report, not for the user.
    for marker in [" See http", " Please report", "; please report", " http"] {
        if let Some((head, _)) = out.split_once(marker) {
            out = head;
            break;
        }
    }
    let out = out.trim().trim_end_matches(['.', ',']);
    if out.chars().count() > 180 {
        out.chars().take(179).collect::<String>() + "…"
    } else {
        out.to_string()
    }
}
