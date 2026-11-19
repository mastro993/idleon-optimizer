//! The Clamworks upgrades: every pearl upgrade in the game's order.

use super::pane;
use super::table::{Col, note, table, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, SharedString, Styled, div};
use gpui_kit::component::v_flex;
use idleon_clamworks::Clamworks;
use idleon_ui::format;
use idleon_ui::widgets::{Card, CardHeader, PANEL_GAP};

pub(crate) fn upgrades_pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let clamworks = state.clamworks_shown();
    let multikill = state.clamworks_multikill;
    let body = v_flex()
        .id("clamworks-upgrades")
        .gap(PANEL_GAP)
        .child(all_upgrades_card(&clamworks, multikill, cx))
        .child(note(
            "Next Cost is Clam_Cost at the next level, with FRUGALITY, ANTI INFLATION and the Scurge cost reductions applied. The Optimizer tab spends a budget with these costs.",
            cx,
        ));
    pane("clamworks-upgrades-scroll", body)
}

/// Every upgrade with its level, saved bonus, per-level value and next cost.
fn all_upgrades_card(clamworks: &Clamworks, multikill: f64, cx: &App) -> AnyElement {
    let data = idleon_clamworks::data::clamworks();
    let rows = data.upgrades.iter().enumerate().map(|(index, upgrade)| {
        [
            div()
                .id(("clamworks-upgrade", index))
                .child(SharedString::from(upgrade.name.clone()))
                .into_any_element(),
            div()
                .id(("clamworks-effect", index))
                .min_w_0()
                .child(effect(clamworks, index, multikill))
                .into_any_element(),
            text(clamworks.upgrades[index].to_string()),
            text(money(clamworks.upgrade_bonus(index, multikill).round())),
            text(money(upgrade.per_lv)),
            text(money(clamworks.next_upgrade_cost(index))),
        ]
    });
    Card::new()
        .header(
            CardHeader::new("All upgrades")
                .description("The nine pearl upgrades, in the game's order, at the saved levels."),
        )
        .child(table(
            "clamworks-upgrades",
            &[
                Col::Text("Upgrade"),
                Col::Text("Effect"),
                Col::Number("Lv"),
                Col::Number("Bonus"),
                Col::Number("Per Lv"),
                Col::Number("Next Cost"),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}

/// The game's description line for an upgrade, with the saved values filled in.
pub(super) fn effect(clamworks: &Clamworks, index: usize, multikill: f64) -> String {
    let bonus = clamworks.upgrade_bonus(index, multikill);
    match index {
        0 => format!(
            "Pearls are worth {} each instead of 1.",
            money(clamworks.pearl_value(multikill))
        ),
        1 => format!(
            "Increases the total number of clams on the map to {}.",
            clamworks.clam_count()
        ),
        2 => format!(
            "Pearls have a {:.2}% chance to be worth 10x when picked up.",
            clamworks.pearl_10x_chance() * 100.0
        ),
        3 => format!(
            "Multikill boosts pearl value by {:.2}x.",
            1.0 + bonus / 100.0
        ),
        4 => format!("Divides all upgrade costs by {:.2}x.", 1.0 + bonus / 100.0),
        5 => format!(
            "Pure pearls are worth {}x normal pearls.",
            money(clamworks.black_pearl_value())
        ),
        6 => format!(
            "Boosts the drop rate of pearls by {:.2}x.",
            1.0 + bonus / 100.0
        ),
        7 => format!(
            "Increases the value of all pearls found by {:.2}x.",
            1.0 + bonus / 100.0
        ),
        8 => format!("Divides all upgrade costs by {:.2}x.", 1.0 + bonus / 100.0),
        _ => String::new(),
    }
}

/// A finite value scaled for a table cell; an unrepresentable cost reads as a dash.
pub(super) fn money(value: f64) -> String {
    if value.is_finite() {
        format::rate(value)
    } else {
        "—".into()
    }
}
