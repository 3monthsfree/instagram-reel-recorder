//! Palette. Muted panel frames, bright content, gradient meters.

use ratatui::style::Color;

pub const TITLE: Color = Color::Rgb(238, 238, 244);
pub const FG: Color = Color::Rgb(214, 216, 226);
pub const DIM: Color = Color::Rgb(122, 124, 142);
pub const FAINT: Color = Color::Rgb(74, 76, 92);
pub const METER_BG: Color = Color::Rgb(54, 54, 64);
pub const HOTKEY: Color = Color::Rgb(226, 88, 88);
pub const SEL_BG: Color = Color::Rgb(58, 34, 50);

pub const BOX_ADD: Color = Color::Rgb(86, 106, 138);
pub const BOX_QUEUE: Color = Color::Rgb(128, 82, 104);
pub const BOX_SPEED: Color = Color::Rgb(92, 88, 141);
pub const BOX_STATS: Color = Color::Rgb(108, 108, 75);
pub const BOX_INFO: Color = Color::Rgb(85, 109, 89);

pub const OK: Color = Color::Rgb(122, 202, 142);
pub const WARN: Color = Color::Rgb(226, 182, 92);
pub const ERR: Color = Color::Rgb(226, 88, 88);
pub const ACCENT: Color = Color::Rgb(214, 41, 118);
pub const CYAN: Color = Color::Rgb(98, 190, 206);
pub const VIOLET: Color = Color::Rgb(158, 118, 226);

/// Warm-to-cool ramp used for progress meters.
const METER: [(u8, u8, u8); 5] = [
    (254, 214, 116),
    (250, 126, 30),
    (214, 41, 118),
    (150, 47, 191),
    (86, 98, 214),
];

/// Dark-to-light ramp used for the speed graph, bottom row darkest.
const GRAPH: [(u8, u8, u8); 4] = [
    (58, 52, 120),
    (104, 84, 186),
    (168, 124, 226),
    (214, 178, 244),
];

/// Second ramp, for the load waveform underneath it.
const GRAPH_LOAD: [(u8, u8, u8); 4] = [
    (92, 30, 62),
    (186, 52, 110),
    (232, 104, 156),
    (250, 176, 206),
];

fn ramp(stops: &[(u8, u8, u8)], t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    let last = stops.len() - 1;
    let pos = t * last as f64;
    let i = (pos.floor() as usize).min(last.saturating_sub(1));
    let f = pos - i as f64;
    let (r1, g1, b1) = stops[i];
    let (r2, g2, b2) = stops[(i + 1).min(last)];
    let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * f).round() as u8;
    Color::Rgb(mix(r1, r2), mix(g1, g2), mix(b1, b2))
}

pub fn meter(t: f64) -> Color {
    ramp(&METER, t)
}

pub fn graph(t: f64) -> Color {
    ramp(&GRAPH, t)
}

pub fn graph_load(t: f64) -> Color {
    ramp(&GRAPH_LOAD, t)
}
