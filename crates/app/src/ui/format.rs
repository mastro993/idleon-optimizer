//! Formatting and palette helpers shared by the panes.

use gpui::{Hsla, rgb};
use idleon_core::game;

/// A rate as `1.23M` or `845K`, in EXP per hour.
pub fn rate(value: f64) -> String {
    let (scaled, suffix) = scale(value);
    match suffix {
        "" => format!("{scaled:.0}"),
        "K" => format!("{scaled:.1}K"),
        _ => format!("{scaled:.2}{suffix}"),
    }
}

/// A large count split into its scaled value and suffix, for `Statistic`.
pub fn scale(value: f64) -> (f64, &'static str) {
    if !value.is_finite() {
        return (value, "");
    }
    let magnitude = value.abs();
    if magnitude >= 1e9 {
        (value / 1e9, "B")
    } else if magnitude >= 1e6 {
        (value / 1e6, "M")
    } else if magnitude >= 1e3 {
        (value / 1e3, "K")
    } else {
        (value, "")
    }
}

/// A game value as `1.23M`, `845.0` or `0.0`, matching the upstream `fmtVal`.
pub fn value(value: f64) -> String {
    let magnitude = value.abs();
    if magnitude >= 1e24 {
        return format!("{value:.2e}");
    }
    let (scaled, suffix) = scaled(value);
    if magnitude >= 1e6 {
        format!("{scaled:.2}{suffix}")
    } else if suffix.is_empty() {
        format!("{value:.1}")
    } else {
        format!("{scaled:.1}{suffix}")
    }
}

/// A probability as a percentage with three decimals.
pub fn percent(value: f64) -> String {
    if value > 0.0 && value < 0.000001 {
        "<0.0001%".into()
    } else {
        format!("{:.3}%", 100.0 * value)
    }
}

/// A count with thousands separators.
pub fn count(value: u64) -> String {
    group(&value.to_string())
}

/// A leaderboard value, as the upstream endgame page prints it: `Unavailable`
/// without a value, a grouped number below a million, `fmt_val` above.
pub fn stat_value(value: Option<f64>) -> String {
    let Some(value) = value else {
        return "Unavailable".into();
    };
    if value < 1e6 {
        let rounded = format!("{value:.2}");
        let (whole, fraction) = match rounded.split_once('.') {
            Some((whole, fraction)) => (whole, fraction.trim_end_matches('0')),
            None => (rounded.as_str(), ""),
        };
        return match fraction {
            "" => group(whole),
            fraction => format!("{}.{fraction}", group(whole)),
        };
    }
    fmt_val(value)
}

/// The upstream `fmtVal` tiers: `1.23K`, `4.56M`, `7.89QQ` or a signed exponent.
pub fn fmt_val(value: f64) -> String {
    let magnitude = value.abs();
    let (scaled, suffix) = scaled(value);
    if magnitude >= 1e6 {
        format!("{scaled:.2}{suffix}")
    } else {
        format!("{scaled:.1}{suffix}")
    }
}

/// Thousands separators around the whole part of a number.
fn group(whole: &str) -> String {
    let (sign, digits) = match whole.strip_prefix('-') {
        Some(digits) => ("-", digits),
        None => ("", whole),
    };
    let mut out = String::with_capacity(whole.len() + whole.len() / 3);
    out.push_str(sign);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A value and its suffix, in the upstream `fmtVal` shorthand.
pub fn scaled(value: f64) -> (f64, String) {
    let magnitude = value.abs();
    if magnitude >= 1e24 {
        // `toExponential(2)` prints a signed exponent.
        let exponent = magnitude.log10().floor() as i32;
        let mantissa = value / 10_f64.powi(exponent);
        (
            mantissa,
            format!("e{}{exponent}", if exponent < 0 { "-" } else { "+" }),
        )
    } else if magnitude >= 1e21 {
        (value / 1e21, "QQQ".into())
    } else if magnitude >= 1e18 {
        (value / 1e18, "QQ".into())
    } else if magnitude >= 1e15 {
        (value / 1e15, "Q".into())
    } else if magnitude >= 1e12 {
        (value / 1e12, "T".into())
    } else if magnitude >= 1e9 {
        (value / 1e9, "B".into())
    } else if magnitude >= 1e6 {
        (value / 1e6, "M".into())
    } else if magnitude >= 1e3 {
        (value / 1e3, "K".into())
    } else {
        (value, String::new())
    }
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
    let hex = def.color.trim_start_matches('#');
    let value = u32::from_str_radix(hex, 16).unwrap_or(0x88_88_88);
    rgb(value).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_and_percentages_follow_the_upstream_display() {
        assert_eq!(value(0.0), "0.0");
        assert_eq!(value(845.0), "845.0");
        assert_eq!(value(121_945.0), "121.9K");
        assert_eq!(value(73_598_959.0), "73.60M");
        assert_eq!(value(2_909_587_276_919_980_000.0), "2.91QQ");
        assert_eq!(value(-1_500.0), "-1.5K");
        assert_eq!(percent(0.018_089_4), "1.809%");
        assert_eq!(percent(0.0000001), "<0.0001%");
    }

    #[test]
    fn absurd_hours_read_as_a_dash() {
        assert_eq!(hours(f64::INFINITY), "—");
        assert_eq!(hours(1.1e19), "—");
        assert_eq!(hours(26.0), "1d 2h");
    }

    #[test]
    fn large_counts_scale_to_a_suffix() {
        assert_eq!(scale(845.0), (845.0, ""));
        assert_eq!(scale(1_500.0), (1.5, "K"));
        assert_eq!(scale(2_960_058_875.172_09), (2.960_058_875_172_09, "B"));
        assert_eq!(rate(1_500.0), "1.5K");
        assert_eq!(rate(2_960_058_875.172_09), "2.96B");
    }

    #[test]
    fn leaderboard_values_match_the_upstream_formatter() {
        assert_eq!(stat_value(None), "Unavailable");
        assert_eq!(stat_value(Some(3_748.500_000_000_336)), "3,748.5");
        assert_eq!(stat_value(Some(746.926)), "746.93");
        assert_eq!(stat_value(Some(0.0)), "0");
        assert_eq!(stat_value(Some(1_000_000.0)), "1.00M");
        assert_eq!(stat_value(Some(6.100_056_391_290_721e44)), "6.10e+44");
        assert_eq!(stat_value(Some(14_754_186_378_368_600_000.0)), "14.75QQ");
        assert_eq!(count(1_234_567), "1,234,567");
        assert_eq!(group("-1234"), "-1,234");
    }
}
