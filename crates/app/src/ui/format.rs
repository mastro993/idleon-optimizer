//! Formatting and palette helpers shared by the panes.

use gpui::{Hsla, rgb};
use idleon_core::game;

/// A rate as `1.23M` or `845K`, in EXP per hour.
pub fn rate(value: f64) -> String {
    if value >= 1e9 {
        format!("{:.2}B", value / 1e9)
    } else if value >= 1e6 {
        format!("{:.2}M", value / 1e6)
    } else if value >= 1e3 {
        format!("{:.1}K", value / 1e3)
    } else {
        format!("{value:.0}")
    }
}

/// A count with thousands separators.
pub fn count(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Hours as `3d 4h`, or minutes when short.
pub fn hours(value: f64) -> String {
    if !value.is_finite() {
        return "—".into();
    }
    if value >= 24.0 {
        format!("{}d {:.0}h", (value / 24.0).floor(), value % 24.0)
    } else if value >= 1.0 {
        format!("{value:.1}h")
    } else {
        format!("{:.0}m", (value * 60.0).max(1.0))
    }
}

/// A node's goal tint, so the grid reads at a glance.
pub fn goal_color(goal: Option<&str>) -> Hsla {
    let (hue, saturation) = match goal.unwrap_or("") {
        "Res EXP" => (0.34, 0.55),
        "Res True×" => (0.0, 0.65),
        "Insight" => (0.78, 0.5),
        "Obs×Insight" => (0.12, 0.7),
        "Kaleido" => (0.51, 0.55),
        "Crowns" => (0.13, 0.6),
        "Menagerie" => (0.9, 0.45),
        "Sushi" => (0.06, 0.6),
        _ => (0.0, 0.0),
    };
    Hsla {
        h: hue,
        s: saturation,
        l: 0.55,
        a: 1.0,
    }
}

/// A shape's own colour, brightened for the dark canvas.
pub fn shape_color(shape: usize) -> Hsla {
    let fallback = rgb(0x88_88_88);
    let Some(def) = game().shapes.get(shape) else {
        return fallback.into();
    };
    let hex = def.color.trim_start_matches('#');
    let value = u32::from_str_radix(hex, 16).unwrap_or(0x88_88_88);
    rgb(value).into()
}

