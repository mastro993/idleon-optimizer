//! The Clamworks comparison: every unlocked `+1` beside the current claim.
//!
//! Ports `summarizeClamworksAfk` from
//! `js/stats/systems/w7/clamworks-combat.js`: the upstream comparison table's
//! first-claim columns, one row per purchase, with the payback the extra pearls
//! of one claim imply.

use super::claim::quantity;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Hsla, IntoElement, ParentElement, Role, SharedString, Styled, div, px};
use idleon_clamworks::afk::{ClaimError, Compare, CompareRow, Phase};
use idleon_ui::format;

/// The comparison: every unlocked `+1` beside the current claim.
pub(crate) fn compare_card(
    outcome: Result<&Compare, &ClaimError>,
    phase: Phase,
    muted: Hsla,
) -> AnyElement {
    let card = Card::new().header(CardHeader::new("Compare +1 upgrades").description(
        "Each unlocked pearl upgrade's claim beside the current one, at the same hours and kills.",
    ));
    let Ok(compare) = outcome else {
        return card
            .child(div().p_3().child(Caption::new(
                "The comparison needs a claim the game would credit.",
            )))
            .into_any_element();
    };
    let best = compare.best().and_then(|row| row.index);
    let mut table = Table::new([
        "Upgrade",
        "Pearls / claim",
        "Pure chance",
        "Gain / claim",
        "Cost",
        "Payback",
    ]);
    for row in &compare.rows {
        table = table.row(compare_row(row, best));
    }
    card.child(div().p_3().child(div().id("clamworks-compare").child(table)))
        .child(
            div()
                .id("clamworks-compare-notes")
                .flex()
                .flex_col()
                .gap_1()
                .px_3()
                .pb_3()
                .text_size(px(11.0))
                .text_color(muted)
                .child(format!(
                    "The {} pearl table, at the selected character's kills and drop rate. Cost is the saved \
                     level's purchase price; payback divides it by the extra pearls one claim earns.",
                    phase.label().to_lowercase()
                ))
                .child(
                    "Extra clams do not change the claim, the cost reducers stay analytical and no purchase is \
                     applied.",
                )
                .children(compare.warnings.iter().enumerate().map(|(index, warning)| {
                    div()
                        .id(("clamworks-compare-warning", index))
                        .role(Role::Label)
                        .aria_label(warning.clone())
                        .child(warning.clone())
                })),
        )
        .into_any_element()
}

fn compare_row(row: &CompareRow, best: Option<usize>) -> [AnyElement; 6] {
    let key = row.index.unwrap_or(usize::MAX);
    let id = match row.index {
        Some(index) => gpui::ElementId::from(("clamworks-compare-row", index)),
        None => gpui::ElementId::from("clamworks-compare-baseline"),
    };
    let gain = match row.gain {
        Some(gain) if gain > 0.0 => format!("+{}", quantity(gain)),
        Some(_) => "—".to_string(),
        None => "—".to_string(),
    };
    let payback = match (row.payback_claims, row.payback_hours) {
        (Some(claims), Some(hours)) => {
            format!("{} claims · {}", quantity(claims), format::hours(hours))
        }
        _ => "—".to_string(),
    };
    let mut name = div()
        .id(id)
        .role(Role::Label)
        .aria_label(format!(
            "{}: {} pearls per claim",
            row.name,
            quantity(row.claim.expected_pearls)
        ))
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(SharedString::from(row.name.clone()));
    if best == row.index && row.index.is_some() {
        name = name.child(
            div()
                .id(("clamworks-compare-best", key))
                .child(Badge::new("Best payback").tone(Tone::Success)),
        );
    }
    let level = if row.index.is_some() {
        format!("Lv {} → {}", row.level, row.next_level)
    } else {
        "Baseline".to_string()
    };
    let mut name = name.child(div().text_size(px(11.0)).child(level));
    if !row.affordable {
        name = name.child(Badge::new("Not yet affordable").tone(Tone::Warning));
    }
    [
        name.into_any_element(),
        div()
            .child(quantity(row.claim.expected_pearls))
            .into_any_element(),
        div()
            .child(format::chance(row.claim.pure.chance))
            .into_any_element(),
        div().child(gain).into_any_element(),
        div()
            .child(row.cost.map_or_else(|| "—".to_string(), quantity))
            .into_any_element(),
        div().child(payback).into_any_element(),
    ]
}
