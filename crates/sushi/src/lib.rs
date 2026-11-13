//! The Sushi Station: the formulas, the save-derived inputs and the upgrade
//! ranking the research page's Sushi Station tab renders.
//!
//! Ported from `js/stats/systems/w7/sushi.js` and `js/ui/sushi-ui.js`. The
//! static tables come from `script/extract-sushi-data.mjs`; the tests compare
//! the port with `script/sushi-oracle.mjs`, which runs the upstream calculator
//! itself.

pub mod data;
pub mod inputs;
pub mod math;
pub mod optimize;

pub use data::game;
pub use inputs::{Inputs, inputs};
pub use math::{External, Station, combo_multiplier};
pub use optimize::{Metric, Path, candidates, path};

/// A number as the upstream `fmtNum` prints it: `1.23M`, `12.3K`, `1,235`.
pub fn fmt_num(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if !value.is_finite() {
        return if value.is_nan() {
            "NaN".into()
        } else if value > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    let magnitude = value.abs();
    if magnitude >= 1e24 {
        return exponential(value);
    }
    for (threshold, suffix) in [
        (1e21, "QQQ"),
        (1e18, "QQ"),
        (1e15, "Q"),
        (1e12, "T"),
        (1e9, "B"),
        (1e6, "M"),
    ] {
        if magnitude >= threshold {
            return format!("{:.2}{suffix}", value / threshold);
        }
    }
    if magnitude >= 1e4 {
        return format!("{:.1}K", value / 1e3);
    }
    if magnitude >= 100.0 {
        return grouped(value.round());
    }
    if value.fract() == 0.0 {
        return format!("{value:.0}");
    }
    format!("{value:.2}")
}

/// `toExponential(2)`, with the sign JS prints in the exponent.
fn exponential(value: f64) -> String {
    let text = format!("{value:.2e}");
    match text.split_once('e') {
        Some((mantissa, exponent)) if !exponent.starts_with('-') => {
            format!("{mantissa}e+{exponent}")
        }
        _ => text,
    }
}

/// `Math.round(n).toLocaleString()` in an English locale.
fn grouped(value: f64) -> String {
    let digits = format!("{value:.0}");
    let (sign, digits) = match digits.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", digits.as_str()),
    };
    let mut out = String::new();
    for (position, digit) in digits.chars().enumerate() {
        if position > 0 && (digits.len() - position) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    format!("{sign}{out}")
}

/// The upgrade description with its `{`, `}` and `$` markers filled in, as the
/// Upgrades table prints it (`_fmtDesc` in `js/ui/sushi-ui.js`).
pub fn upgrade_desc(inputs: &Inputs, upg: usize) -> String {
    let data = game();
    let Some(upgrade) = data.upgrade(upg) else {
        return String::new();
    };
    let station = &inputs.station;
    let ext = &inputs.external;
    let levels = &station.levels;
    let totals = station.knowledge_totals();
    let qty = station.qty(upg);
    let mut desc = upgrade.desc.replacen('{', &fmt_num(qty), 1);
    desc = desc.replacen('}', &format!("{:.2}", 1.0 + qty / 100.0), 1);
    if !desc.contains('$') {
        return desc;
    }
    let level = station.level(upg);
    let base = station.fireplace_base();
    let sparks = station.sparks();
    let no_bundle = |levels: &[f64]| {
        math::fuel_capacity(
            data,
            levels,
            &totals,
            false,
            ext.event_shop66 > 0.0,
            &station.doublers,
        )
    };
    let value = match upg {
        2 | 3 | 4 | 5 | 9 | 10 | 11 | 12 => format!("{:.2}", 1.0 + level / 100.0),
        1 => fmt_num(no_bundle(levels)),
        6 => format!("{:.0}", station.max_cook_tier() + 1.0),
        8 => format!(
            "{} Fuel/hr. Max Cap is {}",
            fmt_num(math::fuel_gen_per_hr(
                data,
                levels,
                &station.knowledge,
                &station.found,
                &totals,
                station.orange_fire_sum(),
                false,
                &station.doublers,
            )),
            fmt_num(no_bundle(levels))
        ),
        14 => format!(
            "{:.2}",
            1.0 + 50.0 * math::slot_effect_base(&totals) / 100.0
        ),
        15 => format!("{:.2}", 40.0 * math::slot_effect_base(&totals)),
        16 => format!("{:.2}", 10.0 * math::slot_effect_base(&totals)),
        22 => format!("{:.1}", station.saffron_hrs()),
        25 => format!(
            "{} sparks - boosts fireplaces by {:.2}x",
            fmt_num(sparks),
            base
        ),
        26 => format!(
            "{:.2}",
            1.0 + math::fireplace_effect_by_type(3, base) / 100.0
        ),
        27 => format!(
            "{:.2}",
            1.0 + math::fireplace_effect_by_type(1, base) / 100.0
        ),
        28 => format!(
            "{} overtuned - boosts Bucks by {:.2}x",
            fmt_num(station.spa()),
            1.0 + math::compute_overtuned_multi(station.spa()) / 100.0
        ),
        29 => format!("{:.0}", station.unique as f64 - 6.0),
        36 => format!(
            "{:.2}",
            (1e4 * (1.0 - 1.0 / (1.0 + qty / 100.0))).round() / 100.0
        ),
        37 => format!("{:.1}", math::knowledge_xp_base(qty)),
        _ => fmt_num(qty),
    };
    desc.replacen('$', &value, 1)
}

/// A knowledge category's description with its `{`, `}` and `^` markers
/// replaced by the category total and, when there is one, the per-level gain
/// (`_fmtKnDesc` in `js/ui/sushi-ui.js`).
pub fn knowledge_desc(cat: usize, total: f64, per_level: f64) -> String {
    let Some(text) = game().knowledge_desc_at(cat) else {
        return String::new();
    };
    let mut desc = fill_marker(text, '{', fmt_num(total), note(per_level));
    desc = fill_marker(&desc, '^', fmt_num(total), note(per_level));
    fill_marker(
        &desc,
        '}',
        format!("{:.2}", 1.0 + total / 100.0),
        note(per_level / 100.0),
    )
}

/// The per-level gain's parenthesised suffix, when there is one.
fn note(per_level: f64) -> Option<String> {
    (per_level > 0.0).then(|| fmt_num(per_level))
}

/// Replace the first `marker` and the unit character after it.
fn fill_marker(text: &str, marker: char, value: String, note: Option<String>) -> String {
    let Some(position) = text.find(marker) else {
        return text.to_string();
    };
    let rest = &text[position + marker.len_utf8()..];
    let Some(unit) = rest.chars().next() else {
        return text.to_string();
    };
    let replacement = match note {
        Some(note) => format!("{value}{unit} (+{note}{unit})"),
        None => format!("{value}{unit}"),
    };
    format!(
        "{}{replacement}{}",
        &text[..position],
        &rest[unit.len_utf8()..]
    )
}

/// The Ring of Glory reward text at `tier`, with its `{` and `}` filled in.
pub fn rog_reward(tier: usize) -> String {
    let data = game();
    let value = data.rog_qty_at(tier);
    data.rog_desc_at(tier)
        .unwrap_or_default()
        .replacen('{', &fmt_num(value), 1)
        .replacen('}', &format!("{:.2}", 1.0 + value / 100.0), 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_the_way_fmt_num_prints_them() {
        assert_eq!(fmt_num(0.0), "0");
        assert_eq!(fmt_num(1234.6), "1,235");
        assert_eq!(fmt_num(1_234_567.0), "1.23M");
        assert_eq!(fmt_num(12_345.0), "12.3K");
        assert_eq!(fmt_num(3.5), "3.50");
    }

    #[test]
    fn knowledge_descriptions_fill_their_markers() {
        // Category 0: `+{% more Bucks generated`.
        assert_eq!(
            knowledge_desc(0, 1234.0, 0.0),
            "+1,234% more Bucks generated"
        );
        assert_eq!(
            knowledge_desc(0, 1234.0, 2.0),
            "+1,234% (+2%) more Bucks generated"
        );
        // Category 2 uses the value marker with a caret: `+^% chance ...`.
        assert_eq!(
            knowledge_desc(2, 10.0, 0.0),
            "+10% chance for +1 Tier for cooked sushi"
        );
        // Category 3 uses the multiplier marker: `}x higher Fuel Capacity`.
        assert_eq!(knowledge_desc(3, 50.0, 0.0), "1.50x higher Fuel Capacity");
        assert_eq!(
            knowledge_desc(3, 50.0, 5.0),
            "1.50x (+0.05x) higher Fuel Capacity"
        );
    }
}
