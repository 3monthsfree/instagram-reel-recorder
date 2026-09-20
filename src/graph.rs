//! Braille plotting. Every cell holds 2 columns x 4 rows of dots, so a graph
//! packs twice the samples and four times the vertical detail of block chars.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::theme;

const LEFT: [u8; 4] = [0x40, 0x04, 0x02, 0x01]; // bottom dot upward
const RIGHT: [u8; 4] = [0x80, 0x20, 0x10, 0x08];
const BLANK: char = '\u{2800}';
const FLOOR: char = '⣀'; // the two lowest dots: a zero line
const GRID: char = '⠒'; // a mid-height dotted rule

fn braille(left: u8, right: u8) -> char {
    let mut mask = 0u8;
    for bit in LEFT.iter().take(left.min(4) as usize) {
        mask |= bit;
    }
    for bit in RIGHT.iter().take(right.min(4) as usize) {
        mask |= bit;
    }
    char::from_u32(0x2800 + mask as u32).unwrap_or(BLANK)
}

/// Dot height of a sample within a graph `rows` cells tall.
fn level(sample: f64, max: f64, rows: u16) -> u8 {
    if max <= 0.0 || sample <= 0.0 {
        return 0;
    }
    let dots = rows as f64 * 4.0;
    // Anything non-zero earns at least one dot, so slow transfers still show.
    ((sample / max) * dots).round().clamp(1.0, dots) as u8
}

/// Squash a long sample window down to `buckets` columns, keeping peaks.
pub fn bucket(samples: &[f64], buckets: usize) -> Vec<f64> {
    if buckets == 0 {
        return Vec::new();
    }
    if samples.len() <= buckets {
        let mut out = vec![0.0; buckets - samples.len()];
        out.extend_from_slice(samples);
        return out;
    }
    let per = samples.len() as f64 / buckets as f64;
    (0..buckets)
        .map(|i| {
            let start = (i as f64 * per).floor() as usize;
            let end = (((i + 1) as f64 * per).ceil() as usize).min(samples.len());
            samples[start..end.max(start + 1)]
                .iter()
                .copied()
                .fold(0.0_f64, f64::max)
        })
        .collect()
}

fn dim(color: Color, factor: f64) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f64 * factor) as u8,
            (g as f64 * factor) as u8,
            (b as f64 * factor) as u8,
        ),
        other => other,
    }
}

/// Two waveforms sharing the full height of one chart: `back` fills behind as
/// context, `front` rides over it. The silhouette is the union of the two, and
/// a cell is painted in the front colour wherever the front series reaches it.
#[allow(clippy::too_many_arguments)]
pub fn area_dual(
    front: &[f64],
    front_max: f64,
    back: &[f64],
    back_max: f64,
    width: u16,
    rows: u16,
    front_ramp: fn(f64) -> Color,
    back_ramp: fn(f64) -> Color,
) -> Vec<Line<'static>> {
    let want = width as usize * 2;
    let fit = |values: &[f64], max: f64| -> Vec<u8> {
        let mut pts: Vec<f64> = vec![0.0; want.saturating_sub(values.len())];
        pts.extend_from_slice(&values[values.len().saturating_sub(want)..]);
        pts.iter().map(|s| level(*s, max, rows)).collect()
    };
    let front_levels = fit(front, front_max);
    let back_levels = fit(back, back_max);

    let midline = rows.saturating_sub(1) - ((rows - 1) as f64 * 0.5).round() as u16;

    (0..rows)
        .map(|row| {
            let floor = (rows - 1 - row) as u8 * 4;
            let t = if rows > 1 {
                (rows - 1 - row) as f64 / (rows - 1) as f64
            } else {
                1.0
            };
            let front_style = Style::default().fg(front_ramp(t));
            let back_style = Style::default().fg(dim(back_ramp(t), 0.75));
            let faint = Style::default().fg(theme::FAINT);
            let bottom = row == rows - 1;
            let ruled = row == midline;

            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut run = String::new();
            let mut run_style: Option<Style> = None;
            for col in 0..width as usize {
                let at = |levels: &Vec<u8>, i: usize| {
                    levels.get(i).copied().unwrap_or(0).saturating_sub(floor)
                };
                let (fl, fr) = (at(&front_levels, col * 2), at(&front_levels, col * 2 + 1));
                let (bl, br) = (at(&back_levels, col * 2), at(&back_levels, col * 2 + 1));
                let ch = braille(fl.max(bl), fr.max(br));

                let (ch, style) = if ch != BLANK {
                    (
                        ch,
                        if fl > 0 || fr > 0 {
                            front_style
                        } else {
                            back_style
                        },
                    )
                } else if bottom {
                    (FLOOR, faint)
                } else if ruled {
                    (GRID, faint)
                } else {
                    (' ', faint)
                };
                if run_style.is_some_and(|s| s != style) {
                    spans.push(Span::styled(
                        std::mem::take(&mut run),
                        run_style.unwrap_or(faint),
                    ));
                }
                run_style = Some(style);
                run.push(ch);
            }
            if !run.is_empty() {
                spans.push(Span::styled(run, run_style.unwrap_or(faint)));
            }
            Line::from(spans)
        })
        .collect()
}

/// One-line history, for a table row.
pub fn spark(samples: &[f64], max: f64, width: usize) -> String {
    let want = width * 2;
    let tail = &samples[samples.len().saturating_sub(want)..];
    let mut pts: Vec<f64> = vec![0.0; want.saturating_sub(tail.len())];
    pts.extend_from_slice(tail);

    (0..width)
        .map(|col| {
            let l = level(pts.get(col * 2).copied().unwrap_or(0.0), max, 1);
            let r = level(pts.get(col * 2 + 1).copied().unwrap_or(0.0), max, 1);
            braille(l, r)
        })
        .collect()
}
