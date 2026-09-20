//! Rendering. Five framed panels, numbered like a system monitor.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Paragraph, Row, Table, Wrap};

use crate::{
    App, Focus, GRAPH_WINDOW, Job, Status, graph, home_relative, human_bytes, human_rate,
    human_secs, short_url, theme,
};

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn shade(color: Color, factor: f64) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f64 * factor).min(255.0) as u8,
            (g as f64 * factor).min(255.0) as u8,
            (b as f64 * factor).min(255.0) as u8,
        ),
        other => other,
    }
}

/// A panel frame whose title sits in a notch in the border, monitor style.
fn panel(num: &str, name: &str, color: Color, focused: bool) -> Block<'static> {
    let border = if focused {
        shade(color, 1.5)
    } else {
        shade(color, 0.85)
    };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
        .title_top(Line::from(vec![
            Span::styled("┐", Style::default().fg(border)),
            Span::styled(
                num.to_string(),
                Style::default()
                    .fg(theme::HOTKEY)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                name.to_string(),
                Style::default()
                    .fg(theme::TITLE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("┌", Style::default().fg(border)),
        ]))
}

fn hint(key: &str, label: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            key.to_string(),
            Style::default()
                .fg(theme::HOTKEY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {label}  "), Style::default().fg(theme::DIM)),
    ]
}

fn wrap_title(spans: Vec<Span<'static>>) -> Line<'static> {
    let mut out = vec![Span::styled("┘", Style::default().fg(theme::FAINT))];
    out.extend(spans);
    out.push(Span::styled("└", Style::default().fg(theme::FAINT)));
    Line::from(out)
}

/// Keep the end of a string, which is the useful half of a long path.
fn tail(text: &str, width: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width || width < 4 {
        return text.to_string();
    }
    "…".to_string()
        + &chars[chars.len() - (width - 1)..]
            .iter()
            .collect::<String>()
}

fn truncate(text: &str, width: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        return text.to_string();
    }
    chars
        .into_iter()
        .take(width.saturating_sub(1))
        .collect::<String>()
        + "…"
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let wide = area.width >= 108;

    if wide {
        let [left, right] =
            Layout::horizontal([Constraint::Min(56), Constraint::Length(42)]).areas(area);
        // Staging gets a generous share of the column: this is where links land.
        let add_h = ((left.height as u32 * 45 / 100) as u16).clamp(9, 18);
        let [add, queue] =
            Layout::vertical([Constraint::Length(add_h), Constraint::Min(6)]).areas(left);
        draw_add(frame, app, add);
        draw_queue(frame, app, queue);

        let graph_h = if right.height >= 34 {
            17
        } else if right.height >= 26 {
            13
        } else {
            9
        };
        let [speed, stats, info] = Layout::vertical([
            Constraint::Length(graph_h),
            Constraint::Length(9),
            Constraint::Min(5),
        ])
        .areas(right);
        draw_speed(frame, app, speed);
        draw_stats(frame, app, stats);
        draw_info(frame, app, info);
    } else {
        let [add, queue, info] = Layout::vertical([
            Constraint::Length(8),
            Constraint::Min(6),
            Constraint::Length(7),
        ])
        .areas(area);
        draw_add(frame, app, add);
        draw_queue(frame, app, queue);
        draw_info(frame, app, info);
    }
}

// ---- 1: add / staging ----------------------------------------------------

fn draw_add(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = matches!(app.focus, Focus::Add | Focus::Input);
    let typing = app.focus == Focus::Input;
    let ready = app.ready_count();

    let mut bottom_left = vec![Span::styled("┘", Style::default().fg(theme::FAINT))];
    bottom_left.extend(hint("enter", "start"));
    bottom_left.extend(hint("v", "clipboard"));
    bottom_left.extend(hint("a", "type"));
    bottom_left.extend(hint("d", "drop"));
    bottom_left.extend(hint("tab", "queue"));
    bottom_left.push(Span::styled("└", Style::default().fg(theme::FAINT)));

    let mut bottom_right = vec![Span::styled("┘", Style::default().fg(theme::FAINT))];
    bottom_right.extend(hint("c", &format!("cookies: {}", app.cookie_name())));
    bottom_right.extend(hint("-", &format!("{} workers", app.workers)));
    bottom_right.push(Span::styled(
        "+",
        Style::default()
            .fg(theme::HOTKEY)
            .add_modifier(Modifier::BOLD),
    ));
    bottom_right.push(Span::styled("└", Style::default().fg(theme::FAINT)));

    let block = panel("¹", "add", theme::BOX_ADD, focused)
        .title_top(
            wrap_title(vec![
                Span::styled("→ ", Style::default().fg(theme::FAINT)),
                Span::styled(
                    tail(&home_relative(&app.out_dir), (area.width / 3) as usize),
                    Style::default().fg(theme::DIM),
                ),
                Span::styled("  ·  ", Style::default().fg(theme::FAINT)),
                Span::styled(crate::clock(), Style::default().fg(theme::DIM)),
            ])
            .right_aligned(),
        )
        .title_bottom(Line::from(bottom_left).left_aligned())
        .title_bottom(Line::from(bottom_right).right_aligned());

    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 3 {
        return;
    }

    let [prompt_row, rest] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(2)]).areas(inner);

    // Prompt line, with any flash message parked on the right.
    let flash = app.flash.as_ref();
    let flash_w = flash
        .map(|f| f.text.chars().count() as u16 + 2)
        .unwrap_or(0);
    let [prompt_area, flash_area] =
        Layout::horizontal([Constraint::Min(10), Constraint::Length(flash_w)]).areas(prompt_row);
    let cursor = if app.tick % 10 < 6 { "▌" } else { " " };
    let prompt = if typing {
        Line::from(vec![
            Span::styled(
                "› ",
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(app.input.clone(), Style::default().fg(theme::FG)),
            Span::styled(cursor, Style::default().fg(theme::ACCENT)),
        ])
    } else {
        Line::from(vec![
            Span::styled("› ", Style::default().fg(theme::FAINT)),
            Span::styled(
                "paste links here — as many as you like",
                Style::default().fg(theme::DIM),
            ),
            Span::styled("   ctrl+shift+v", Style::default().fg(theme::FAINT)),
        ])
    };
    frame.render_widget(Paragraph::new(prompt), prompt_area);
    if let Some(f) = flash {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                f.text.clone(),
                Style::default().fg(f.color).add_modifier(Modifier::BOLD),
            )))
            .alignment(Alignment::Right),
            flash_area,
        );
    }

    let [list_area, side] =
        Layout::horizontal([Constraint::Min(20), Constraint::Length(24)]).areas(rest);
    app.hit_staged = list_area;

    if app.staged.is_empty() {
        frame.render_widget(Paragraph::new(drop_zone(list_area)), list_area);
    } else {
        let rows = list_area.height as usize;
        let first = app.staged_sel.saturating_sub(rows.saturating_sub(1));
        let mut lines: Vec<Line> = Vec::new();
        for (i, item) in app.staged.iter().enumerate().skip(first).take(rows) {
            let picked = i == app.staged_sel && app.focus == Focus::Add;
            let mark = if item.dup { "⊘" } else { "▸" };
            let color = if item.dup { theme::WARN } else { theme::ACCENT };
            let text = if item.dup {
                format!("{}  already queued", short_url(&item.url))
            } else {
                short_url(&item.url).to_string()
            };
            let mut style = Style::default().fg(if item.dup { theme::DIM } else { theme::FG });
            if picked {
                style = style.bg(theme::SEL_BG).add_modifier(Modifier::BOLD);
            }
            lines.push(Line::from(vec![
                Span::styled(format!(" {:>2} ", i + 1), Style::default().fg(theme::FAINT)),
                Span::styled(format!("{mark} "), Style::default().fg(color)),
                Span::styled(
                    truncate(&text, list_area.width.saturating_sub(8) as usize),
                    style,
                ),
            ]));
        }
        if app.staged.len() > first + rows {
            lines.pop();
            lines.push(Line::from(Span::styled(
                format!("    +{} more", app.staged.len() - first - rows + 1),
                Style::default().fg(theme::DIM),
            )));
        }
        frame.render_widget(Paragraph::new(lines), list_area);
    }

    draw_start_button(frame, app, side, ready);
}

/// Dashed placeholder that says "links go here" without saying only two fit.
fn drop_zone(area: Rect) -> Vec<Line<'static>> {
    let w = area.width.saturating_sub(4) as usize;
    let h = area.height as usize;
    let dash = Style::default().fg(theme::FAINT);
    let mut lines = vec![Line::from(Span::styled(
        format!("  ╭{}╮", "╌".repeat(w)),
        dash,
    ))];
    let body = [
        ("drop your links here", theme::DIM),
        ("one per line, or a whole block at once", theme::FAINT),
    ];
    for row in 1..h.saturating_sub(1) {
        let middle = h / 2;
        let text = match row.cmp(&middle) {
            std::cmp::Ordering::Equal => body[0],
            std::cmp::Ordering::Greater if row == middle + 1 => body[1],
            _ => ("", theme::FAINT),
        };
        let pad = w.saturating_sub(text.0.chars().count());
        let (l, r) = (pad / 2, pad - pad / 2);
        lines.push(Line::from(vec![
            Span::styled("  ┊", dash),
            Span::styled(" ".repeat(l), dash),
            Span::styled(text.0.to_string(), Style::default().fg(text.1)),
            Span::styled(" ".repeat(r), dash),
            Span::styled("┊", dash),
        ]));
    }
    lines.push(Line::from(Span::styled(
        format!("  ╰{}╯", "╌".repeat(w)),
        dash,
    )));
    lines
}

fn draw_start_button(frame: &mut Frame, app: &mut App, side: Rect, ready: usize) {
    let width = 20.min(side.width);
    let height = 3.min(side.height);
    let x = side.x + (side.width.saturating_sub(width)) / 2;
    let y = side.y + side.height.saturating_sub(height + 1);
    let rect = Rect::new(x, y, width, height);
    app.hit_start = rect;

    let live = ready > 0;
    let border = if live {
        theme::ACCENT
    } else {
        shade(theme::FAINT, 1.0)
    };
    let label = if live {
        format!("▶  start  {ready}")
    } else {
        "▶  start".to_string()
    };
    let text_style = if live {
        Style::default()
            .fg(theme::TITLE)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::FAINT)
    };

    let button = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border));
    let inner = button.inner(rect);
    frame.render_widget(button, rect);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(label, text_style))).alignment(Alignment::Center),
        inner,
    );

    if side.height > height + 1 {
        let caption = Rect::new(side.x, y + height, side.width, 1);
        let text = if live {
            "enter · or click"
        } else {
            "nothing staged yet"
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                text,
                Style::default().fg(theme::FAINT),
            )))
            .alignment(Alignment::Center),
            caption,
        );
    }
}

// ---- 2: queue ------------------------------------------------------------

fn status_cell(job: &Job, tick: u64) -> Line<'static> {
    let spin = SPINNER[(tick / 2) as usize % SPINNER.len()];
    let (icon, label, color) = match job.status {
        Status::Queued => ("·".into(), "queue", theme::DIM),
        Status::Fetching => (spin.to_string(), "fetch", theme::CYAN),
        Status::Downloading => (spin.to_string(), "down", theme::ACCENT),
        Status::Converting => (spin.to_string(), "mp3", theme::VIOLET),
        // Nothing crossed the wire: yt-dlp found the file already in the folder.
        Status::Done if job.cached => ("✓".into(), "have", theme::CYAN),
        Status::Done => ("✓".into(), "done", theme::OK),
        Status::Failed => ("✗".into(), "fail", theme::ERR),
        Status::Canceled => ("⊘".into(), "stop", theme::DIM),
    };
    Line::from(vec![
        Span::styled(
            icon,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {label}"), Style::default().fg(color)),
    ])
}

/// Gradient meter, a marching pulse when the total size is unknown.
fn progress_cell(job: &Job, tick: u64, width: usize) -> Line<'static> {
    let mut spans = Vec::new();
    match job.status {
        Status::Failed | Status::Canceled => {
            let msg = job.error.clone().unwrap_or_else(|| "failed".into());
            spans.push(Span::styled(
                truncate(&msg, width + 5),
                Style::default().fg(theme::ERR),
            ));
            return Line::from(spans);
        }
        Status::Queued => {
            spans.push(Span::styled(
                "·".repeat(width),
                Style::default().fg(theme::FAINT),
            ));
            spans.push(Span::styled(" idle", Style::default().fg(theme::DIM)));
            return Line::from(spans);
        }
        _ => {}
    }

    match job.frac() {
        Some(frac) => {
            let filled = (frac * width as f64).round() as usize;
            for i in 0..width {
                let color = if i < filled {
                    theme::meter(i as f64 / width.max(1) as f64)
                } else {
                    theme::METER_BG
                };
                spans.push(Span::styled("■", Style::default().fg(color)));
            }
            spans.push(Span::styled(
                format!(" {:>3.0}%", frac * 100.0),
                Style::default().fg(theme::FG),
            ));
        }
        None => {
            let pos = (tick / 2) as usize % width.max(1);
            for i in 0..width {
                let near = (i as isize - pos as isize).unsigned_abs();
                let color = match near {
                    0 => theme::meter(0.5),
                    1 => theme::meter(0.8),
                    _ => theme::METER_BG,
                };
                spans.push(Span::styled("■", Style::default().fg(color)));
            }
            let label = if job.status == Status::Converting {
                " mp3 "
            } else {
                "  ·  "
            };
            spans.push(Span::styled(label, Style::default().fg(theme::DIM)));
        }
    }
    Line::from(spans)
}

fn draw_queue(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Queue;
    let total = app.jobs.len();
    let position = if total == 0 {
        "0/0".to_string()
    } else {
        format!("{}/{}", app.table.selected().unwrap_or(0) + 1, total)
    };

    let mut hints = vec![Span::styled("┘", Style::default().fg(theme::FAINT))];
    hints.extend(hint("↑↓", "select"));
    hints.extend(hint("r", "retry"));
    hints.extend(hint("d", "remove"));
    hints.extend(hint("p", "play"));
    hints.extend(hint("o", "folder"));
    hints.extend(hint("x", "clear"));
    hints.extend(hint("q", "quit"));
    hints.push(Span::styled("└", Style::default().fg(theme::FAINT)));

    let block = panel("²", "queue", theme::BOX_QUEUE, focused)
        .title_bottom(Line::from(hints).left_aligned())
        .title_bottom(
            wrap_title(vec![Span::styled(
                position,
                Style::default().fg(theme::DIM),
            )])
            .right_aligned(),
        );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.jobs.is_empty() {
        app.hit_rows = Rect::ZERO;
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "the queue is empty",
                Style::default().fg(theme::DIM).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "stage links above, then press ",
                    Style::default().fg(theme::FAINT),
                ),
                Span::styled("enter", Style::default().fg(theme::HOTKEY)),
                Span::styled(" to start them", Style::default().fg(theme::FAINT)),
            ]),
        ];
        frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
        return;
    }
    // Rows begin below the header, which matters for click targeting.
    app.hit_rows = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(1),
    );

    let show_trend = inner.width >= 84;
    let show_size = inner.width >= 74;
    let bar_w = if inner.width >= 96 { 16 } else { 10 };

    let peak = app.net_peak.max(1.0);
    let rows: Vec<Row> = app
        .jobs
        .iter()
        .enumerate()
        .map(|(i, job)| {
            let name = match job.status {
                Status::Done => Style::default().fg(theme::FG),
                Status::Failed | Status::Canceled => Style::default().fg(theme::DIM),
                _ => Style::default().fg(theme::TITLE),
            };
            let mut cells = vec![
                Cell::from(Line::from(Span::styled(
                    format!("{:>2}", i + 1),
                    Style::default().fg(theme::FAINT),
                ))),
                Cell::from(status_cell(job, app.tick)),
                Cell::from(Line::from(Span::styled(job.label().to_string(), name))),
                Cell::from(progress_cell(job, app.tick, bar_w)),
                Cell::from(Line::from(Span::styled(
                    match job.status {
                        Status::Downloading => human_rate(job.speed.max(job.rate)),
                        Status::Done => job
                            .elapsed
                            .map(|d| format!("in {}", human_secs(d.as_secs())))
                            .unwrap_or_default(),
                        _ => String::new(),
                    },
                    Style::default().fg(theme::DIM),
                ))),
            ];
            if show_trend {
                let hist: Vec<f64> = job.hist.iter().copied().collect();
                let t = (job.rate / peak).clamp(0.25, 1.0);
                cells.push(Cell::from(Line::from(Span::styled(
                    graph::spark(&hist, peak, 6),
                    Style::default().fg(theme::graph(t)),
                ))));
            }
            if show_size {
                let (text, color) = match (job.status, job.size, job.total) {
                    (Status::Done, Some(size), _) => (human_bytes(size), theme::OK),
                    (Status::Downloading, _, Some(total)) => (human_bytes(total), theme::DIM),
                    _ => (String::new(), theme::DIM),
                };
                cells.push(Cell::from(Line::from(Span::styled(
                    text,
                    Style::default().fg(color),
                ))));
            }
            Row::new(cells)
        })
        .collect();

    let mut widths = vec![
        Constraint::Length(3),
        Constraint::Length(7),
        Constraint::Min(16),
        Constraint::Length(bar_w as u16 + 5),
        Constraint::Length(10),
    ];
    let mut header = vec!["", "status", "title", "progress", "speed"];
    if show_trend {
        widths.push(Constraint::Length(6));
        header.push("trend");
    }
    if show_size {
        widths.push(Constraint::Length(9));
        header.push("size");
    }

    let table = Table::new(rows, widths)
        .header(
            Row::new(header).style(Style::default().fg(theme::DIM).add_modifier(Modifier::BOLD)),
        )
        .column_spacing(1)
        .row_highlight_style(
            Style::default()
                .bg(theme::SEL_BG)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(table, inner, &mut app.table);
}

// ---- 3: speed ------------------------------------------------------------

fn draw_speed(frame: &mut Frame, app: &App, area: Rect) {
    let busy = app.load().ceil() as usize;
    let block = panel("³", "speed 15s", theme::BOX_SPEED, false)
        .title_top(
            wrap_title(vec![
                Span::styled(
                    format!("load {}/{}", busy, app.workers),
                    Style::default().fg(if busy > 0 {
                        theme::ACCENT
                    } else {
                        theme::FAINT
                    }),
                ),
                Span::styled("  ▼ ", Style::default().fg(theme::FAINT)),
                Span::styled(
                    human_rate(app.net_speed()),
                    Style::default()
                        .fg(theme::VIOLET)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
            .right_aligned(),
        )
        .title_bottom(
            wrap_title(vec![
                Span::styled("peak ", Style::default().fg(theme::FAINT)),
                Span::styled(human_rate(app.net_peak), Style::default().fg(theme::DIM)),
            ])
            .left_aligned(),
        )
        .title_bottom(
            wrap_title(vec![
                Span::styled("avg ", Style::default().fg(theme::FAINT)),
                Span::styled(
                    human_rate(app.session_bytes as f64 / app.started.elapsed().as_secs_f64()),
                    Style::default().fg(theme::DIM),
                ),
            ])
            .right_aligned(),
        );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 3 || inner.width < 10 {
        return;
    }

    // One chart over the whole panel: busy workers fill behind in rose, the
    // transfer rate rides over them in violet.
    let columns = inner.width as usize * 2;
    let flow = graph::bucket(&recent(&app.flow_hist), columns);
    let load = graph::bucket(&recent(&app.load_hist), columns);
    let scale = app.scale.max(app.net_speed() * 1.15).max(256.0 * 1024.0);
    frame.render_widget(
        Paragraph::new(graph::area_dual(
            &flow,
            scale,
            &load,
            (app.workers as f64).max(1.0),
            inner.width,
            inner.height,
            theme::graph,
            theme::graph_load,
        )),
        inner,
    );

    // Rate scale markers along the left edge, over the plot.
    let mark = |text: String, y: u16| {
        (
            Paragraph::new(Line::from(Span::styled(
                text,
                Style::default().fg(theme::FAINT),
            ))),
            Rect::new(inner.x, y, 12.min(inner.width), 1),
        )
    };
    let mut marks = vec![mark(human_rate(scale), inner.y)];
    if inner.height >= 5 {
        marks.push(mark(human_rate(scale / 2.0), inner.y + inner.height / 2));
    }
    for (widget, rect) in marks {
        frame.render_widget(widget, rect);
    }
}

/// The tail of a history buffer that the graphs cover.
fn recent(hist: &std::collections::VecDeque<f64>) -> Vec<f64> {
    hist.iter()
        .skip(hist.len().saturating_sub(GRAPH_WINDOW))
        .copied()
        .collect()
}

// ---- 4: stats ------------------------------------------------------------

fn meter_line(
    label: &str,
    value: usize,
    total: usize,
    color: Color,
    width: usize,
) -> Line<'static> {
    let frac = if total == 0 {
        0.0
    } else {
        value as f64 / total as f64
    };
    let filled = (frac * width as f64).round() as usize;
    let mut spans = vec![Span::styled(
        format!(" {label:<8}"),
        Style::default().fg(theme::DIM),
    )];
    for i in 0..width {
        spans.push(Span::styled(
            "■",
            Style::default().fg(if i < filled { color } else { theme::METER_BG }),
        ));
    }
    spans.push(Span::styled(
        format!(" {value:>3}"),
        Style::default().fg(theme::FG),
    ));
    Line::from(spans)
}

fn draw_stats(frame: &mut Frame, app: &App, area: Rect) {
    let block = panel("⁴", "stats", theme::BOX_STATS, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let total = app.jobs.len();
    let count = |f: fn(&Job) -> bool| app.jobs.iter().filter(|j| f(j)).count();
    let done = count(|j| j.status == Status::Done);
    let active = count(|j| j.status.active());
    let queued = count(|j| j.status == Status::Queued);
    let failed = count(|j| matches!(j.status, Status::Failed | Status::Canceled));

    let width = (inner.width as usize).saturating_sub(14).clamp(6, 16);
    let (lib_count, lib_bytes) = app.library;
    let lines = vec![
        meter_line("done", done, total, theme::OK, width),
        meter_line("running", active, total, theme::ACCENT, width),
        meter_line("queued", queued, total, theme::DIM, width),
        meter_line("failed", failed, total, theme::ERR, width),
        Line::from(""),
        Line::from(vec![
            Span::styled(" staged   ", Style::default().fg(theme::DIM)),
            Span::styled(
                format!("{}", app.staged.len()),
                Style::default().fg(theme::ACCENT),
            ),
            Span::styled(
                format!("  ·  saved {}", human_bytes(app.session_bytes)),
                Style::default().fg(theme::FAINT),
            ),
        ]),
        Line::from(vec![
            Span::styled(" folder   ", Style::default().fg(theme::DIM)),
            Span::styled(format!("{lib_count} mp3"), Style::default().fg(theme::FG)),
            Span::styled(
                format!("  ·  {}", human_bytes(lib_bytes)),
                Style::default().fg(theme::FAINT),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

// ---- 5: info -------------------------------------------------------------

fn field(label: &str, value: String, color: Color, width: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {label:<7}"), Style::default().fg(theme::FAINT)),
        Span::styled(truncate(&value, width), Style::default().fg(color)),
    ])
}

fn draw_info(frame: &mut Frame, app: &App, area: Rect) {
    let block = panel("⁵", "info", theme::BOX_INFO, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let w = inner.width.saturating_sub(9) as usize;

    let Some(job) = app.selected() else {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " nothing in the queue yet",
                Style::default().fg(theme::FAINT),
            ))),
            inner,
        );
        return;
    };

    let mut lines = vec![field(
        "title",
        job.title.clone().unwrap_or_else(|| "—".into()),
        theme::TITLE,
        w,
    )];
    let mut by = job.uploader.clone().unwrap_or_else(|| "—".into());
    if let Some(d) = job.duration {
        by.push_str(&format!("  ·  {}", human_secs(d as u64)));
    }
    lines.push(field("by", by, theme::FG, w));
    lines.push(field(
        "link",
        short_url(&job.url).to_string(),
        theme::CYAN,
        w,
    ));

    match (&job.file, job.size) {
        (Some(path), size) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            lines.push(field("file", name, theme::OK, w));
            if let Some(size) = size {
                lines.push(field("size", human_bytes(size), theme::FG, w));
            }
            if job.cached {
                lines.push(field(
                    "note",
                    "was already in the folder — nothing downloaded".into(),
                    theme::CYAN,
                    w,
                ));
            }
        }
        (None, _) => {
            let pending = match job.status {
                Status::Done => "saved earlier".to_string(),
                Status::Downloading => format!(
                    "{} downloaded{}",
                    human_bytes(job.done),
                    job.eta
                        .map(|e| format!("  ·  {} left", human_secs(e)))
                        .unwrap_or_default()
                ),
                Status::Converting => "converting to mp3".into(),
                _ => "—".into(),
            };
            lines.push(field("file", pending, theme::DIM, w));
        }
    }

    if let Some(err) = &job.error {
        lines.push(Line::from(vec![
            Span::styled(" error  ", Style::default().fg(theme::FAINT)),
            Span::styled(err.clone(), Style::default().fg(theme::ERR)),
        ]));
        if matches!(job.status, Status::Failed) && app.cookies == 0 {
            lines.push(field(
                "try",
                "press c for cookies, then r to retry".into(),
                theme::WARN,
                w,
            ));
        }
    } else if let Some(last) = job.log.back() {
        lines.push(field("log", last.clone(), theme::FAINT, w));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}
