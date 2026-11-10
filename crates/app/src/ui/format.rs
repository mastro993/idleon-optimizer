//! Formatting and palette helpers shared by the panes.

use ely_gpui_component::typography::format::{DurationStyle, duration};
use gpui::{Hsla, rgb};
use idleon_core::game;

/// A figure in the game's own short form: `544.16B`, `8.71T`, `845.2`.
///
/// The upstream calculators format every large number this way
/// (`fmtVal` in `js/renderers/format.js`), which is the only readable form for a
/// rupie balance past a billion.
pub fn value(figure: f64) -> String {
    let magnitude = figure.abs();
    if magnitude >= 1e24 {
        return format!("{figure:.2e}");
    }
    for (divisor, suffix, decimals) in [
        (1e21, "QQQ", 2),
        (1e18, "QQ", 2),
        (1e15, "Q", 2),
        (1e12, "T", 2),
        (1e9, "B", 2),
        (1e6, "M", 2),
        (1e3, "K", 1),
    ] {
        if magnitude >= divisor {
            return format!("{:.decimals$}{suffix}", figure / divisor);
        }
    }
    format!("{figure:.1}")
}

/// A count in the game's short form, kept whole below a thousand: `21`, `40`,
/// `503.48K` (the upstream `fmtNum`).
pub fn whole(figure: f64) -> String {
    if figure.fract() == 0.0 && figure.abs() < 1e3 {
        return format!("{figure:.0}");
    }
    value(figure)
}

/// A projected span of hours as `3d 4h`, `4m 5s`, or a dash when it is endless.
pub fn projection(hours: f64) -> String {
    if !hours.is_finite() || hours <= 0.0 {
        return "—".into();
    }
    duration((hours * 3600.0).round() as u64, DurationStyle::Compact)
}

/// A `#rgb` or `#rrggbb` colour from the upstream data as a theme colour.
pub fn hex(color: &str) -> Hsla {
    let digits = color.trim_start_matches('#');
    // The upstream data writes both shorthands; `#f44` is `#ff4444`.
    let expanded: String = match digits.len() {
        3 => digits.chars().flat_map(|digit| [digit, digit]).collect(),
        _ => digits.to_string(),
    };
    let value = u32::from_str_radix(&expanded, 16).unwrap_or(0x88_88_88);
    rgb(value).into()
}

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

/// Hours as `3d 4h`, or minutes when short; absurd projections read as a dash.
pub fn hours(value: f64) -> String {
    if !value.is_finite() || value > 1_000_000.0 {
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
    hex(&def.color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absurd_hours_read_as_a_dash() {
        assert_eq!(hours(f64::INFINITY), "—");
        assert_eq!(hours(1.1e19), "—");
        assert_eq!(hours(26.0), "1d 2h");
    }

    #[test]
    fn large_figures_keep_the_games_short_suffixes() {
        assert_eq!(value(544_159_529_207.887), "544.16B");
        assert_eq!(value(8_715_286_155_646.38), "8.72T");
        assert_eq!(value(2.572_064_456_133_22e20), "257.21QQ");
        assert_eq!(value(2.5e21), "2.50QQQ");
        assert_eq!(value(1e24), "1.00e24");
        assert_eq!(value(-1_500_000.0), "-1.50M");
        assert_eq!(value(845.24), "845.2");
    }

    #[test]
    fn whole_counts_stay_whole_below_a_thousand() {
        assert_eq!(whole(21.0), "21");
        assert_eq!(whole(503_476.0), "503.5K");
        assert_eq!(whole(36_000.0), "36.0K");
        assert_eq!(whole(0.0), "0");
    }

    #[test]
    fn both_hex_shorthands_expand() {
        assert_eq!(hex("#f44"), rgb(0xff_44_44).into());
        assert_eq!(hex("#4488ff"), rgb(0x44_88_ff).into());
        assert_eq!(hex("nonsense"), rgb(0x88_88_88).into());
    }

    #[test]
    fn a_projection_reads_as_days_hours_minutes_and_seconds() {
        assert_eq!(projection(0.000_85), "3s");
        assert_eq!(projection(6.285_905_338_846_417), "6h 17m");
        assert_eq!(projection(f64::INFINITY), "—");
        assert_eq!(projection(0.0), "—");
    }
}
