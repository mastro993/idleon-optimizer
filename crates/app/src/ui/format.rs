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

/// The upstream `fmtVal` tiers: `1.23K`, `4.56M`, `7.89QQ` or an exponent.
pub fn fmt_val(value: f64) -> String {
    let magnitude = value.abs();
    let (divisor, suffix) = if magnitude >= 1e24 {
        // `toExponential(2)` prints a signed exponent.
        let exponent = magnitude.log10().floor() as i32;
        let mantissa = value / 10_f64.powi(exponent);
        return format!(
            "{mantissa:.2}e{}{exponent}",
            if exponent < 0 { "-" } else { "+" }
        );
    } else if magnitude >= 1e21 {
        (1e21, "QQQ")
    } else if magnitude >= 1e18 {
        (1e18, "QQ")
    } else if magnitude >= 1e15 {
        (1e15, "Q")
    } else if magnitude >= 1e12 {
        (1e12, "T")
    } else if magnitude >= 1e9 {
        (1e9, "B")
    } else if magnitude >= 1e6 {
        (1e6, "M")
    } else {
        (1e3, "K")
    };
    if magnitude >= 1e6 {
        format!("{:.2}{suffix}", value / divisor)
    } else {
        format!("{:.1}{suffix}", value / divisor)
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

/// A large number as `1.23M`, `4.56T` or `7.89QQ`, matching the upstream
/// `fmtNum` tiers the farming page uses.
pub fn number(value: f64) -> String {
    let magnitude = value.abs();
    if !value.is_finite() {
        return "—".into();
    }
    let (divisor, suffix) = if magnitude >= 1e24 {
        return format!("{value:.2e}");
    } else if magnitude >= 1e21 {
        (1e21, "QQQ")
    } else if magnitude >= 1e18 {
        (1e18, "QQ")
    } else if magnitude >= 1e15 {
        (1e15, "Q")
    } else if magnitude >= 1e12 {
        (1e12, "T")
    } else if magnitude >= 1e9 {
        (1e9, "B")
    } else if magnitude >= 1e6 {
        (1e6, "M")
    } else if magnitude >= 1e3 {
        (1e3, "K")
    } else {
        (1.0, "")
    };
    if suffix.is_empty() {
        format!("{value:.0}")
    } else if divisor >= 1e6 {
        format!("{:.2}{suffix}", value / divisor)
    } else {
        format!("{:.1}{suffix}", value / divisor)
    }
}

/// A probability as a percentage, `1 in N`, `100%` or `Never`.
pub fn chance(value: f64) -> String {
    if !value.is_finite() || value <= 0.0 {
        return "Never".into();
    }
    if value >= 1.0 {
        return "100%".into();
    }
    if value >= 0.0001 {
        return if value >= 0.01 {
            format!("{:.2}%", value * 100.0)
        } else {
            format!("{:.4}%", value * 100.0)
        };
    }
    format!("1 in {}", number(1.0 / value))
}

/// A duration in seconds as `0.50s`, `13m` or `2.5d`; `Never` when infinite.
pub fn duration(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "Never".into();
    }
    if seconds < 1.0 {
        format!("{:.2}s", seconds.max(0.0))
    } else if seconds < 60.0 {
        format!("{seconds:.0}s")
    } else if seconds < 3_600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else if seconds < 86_400.0 {
        format!("{:.2}h", seconds / 3_600.0)
    } else if seconds < 31_557_600.0 {
        let days = seconds / 86_400.0;
        if seconds < 864_000.0 {
            format!("{days:.2}d")
        } else {
            format!("{days:.1}d")
        }
    } else {
        format!("{}y", number(seconds / 31_557_600.0))
    }
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
    fn farming_numbers_chances_and_durations_read_like_upstream() {
        assert_eq!(number(845.0), "845");
        assert_eq!(number(12_345.0), "12.3K");
        assert_eq!(number(4.5e9), "4.50B");
        assert_eq!(number(1.2e15), "1.20Q");
        assert_eq!(chance(0.0), "Never");
        assert_eq!(chance(0.5), "50.00%");
        assert_eq!(chance(0.0005), "0.0500%");
        assert_eq!(chance(1e-6), "1 in 1.00M");
        assert_eq!(chance(2.0), "100%");
        assert_eq!(duration(f64::INFINITY), "Never");
        assert_eq!(duration(0.5), "0.50s");
        assert_eq!(duration(780.0), "13.0m");
        assert_eq!(duration(86_400.0), "1.00d");
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
