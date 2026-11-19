//! The Clamworks comparison: every unlocked `+1` beside the current claim.
//!
//! The comparison table's first-claim columns, one row per purchase, with the
//! payback the extra pearls of one claim imply.

use super::claim::quantity;
use super::table::{Col, note, table, tag, text};
use gpui::prelude::*;
use gpui::{AnyElement, App, IntoElement, ParentElement, Role, SharedString, Styled, div};
use gpui_kit::component::{h_flex, v_flex};
use idleon_clamworks::afk::{ClaimError, Compare, CompareRow, Phase};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_CAPTION, Tone};

/// The comparison: every unlocked `+1` beside the current claim.
pub(crate) fn compare_card(
    outcome: Result<&Compare, &ClaimError>,
    phase: Phase,
    cx: &App,
) -> AnyElement {
    let card = Card::new().header(CardHeader::new("Compare +1 upgrades").description(
        "Each unlocked pearl upgrade's claim beside the current one, at the same hours and kills.",
    ));
    let Ok(compare) = outcome else {
        return card
            .child(Caption::new(
                "The comparison needs a claim the game would credit.",
            ))
            .into_any_element();
    };
    let best = compare.best().and_then(|row| row.index);
    let rows = compare.rows.iter().map(|row| compare_row(row, best));
    card.child(div().id("clamworks-compare").child(table(
        "clamworks-compare",
        &[
            Col::Text("Upgrade"),
            Col::Number("Pearls / claim"),
            Col::Number("Pure chance"),
            Col::Number("Gain / claim"),
            Col::Number("Cost"),
            Col::Number("Payback"),
        ],
        rows,
        cx,
    )))
    .child(
        v_flex()
            .id("clamworks-compare-notes")
            .gap_1()
            .child(note(
                format!(
                    "The {} pearl table, at the selected character's kills and drop rate. Cost is the saved \
                     level's purchase price; payback divides it by the extra pearls one claim earns.",
                    phase.label().to_lowercase()
                ),
                cx,
            ))
            .child(note(
                "Extra clams do not change the claim, the cost reducers stay analytical and no purchase is \
                 applied.",
                cx,
            ))
            .children(compare.warnings.iter().enumerate().map(|(index, warning)| {
                div()
                    .id(("clamworks-compare-warning", index))
                    .role(Role::Label)
                    .aria_label(warning.clone())
                    .child(note(warning.clone(), cx))
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
        _ => "—".to_string(),
    };
    let payback = match (row.payback_claims, row.payback_hours) {
        (Some(claims), Some(hours)) => {
            format!("{} claims · {}", quantity(claims), format::hours(hours))
        }
        _ => "—".to_string(),
    };
    let level = if row.index.is_some() {
        format!("Lv {} → {}", row.level, row.next_level)
    } else {
        "Baseline".to_string()
    };
    let name = v_flex()
        .id(id)
        .role(Role::Label)
        .aria_label(format!(
            "{}: {} pearls per claim",
            row.name,
            quantity(row.claim.expected_pearls)
        ))
        .gap_0p5()
        .child(SharedString::from(row.name.clone()))
        .child(
            h_flex()
                .gap_2()
                .child(div().text_size(TEXT_CAPTION).child(level))
                .when(best == row.index && row.index.is_some(), |line| {
                    line.child(
                        div()
                            .id(("clamworks-compare-best", key))
                            .child(tag("Best payback", Tone::Success)),
                    )
                })
                .when(!row.affordable, |line| {
                    line.child(tag("Not yet affordable", Tone::Warning))
                }),
        );
    [
        name.into_any_element(),
        text(quantity(row.claim.expected_pearls)),
        text(format::chance(row.claim.pure.chance)),
        text(gain),
        text(row.cost.map_or_else(|| "—".to_string(), quantity)),
        text(payback),
    ]
}
