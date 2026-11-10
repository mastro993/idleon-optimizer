//! The Clamworks upgrades: every pearl upgrade and the upstream priority table.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div,
};
use idleon_core::Clamworks;
use idleon_core::clamworks::{Category, UpgradeRank};

pub fn upgrades_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let clamworks = state.clamworks.clone();
    let multikill = state.clamworks_multikill;
    let muted = cx.theme().colors.fg_muted;
    let body = div()
        .id("clamworks-upgrades")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(all_upgrades_card(&clamworks, multikill))
        .child(priority_card(&clamworks, multikill))
        .child(
            Caption::new(
                "Next Cost is Clam_Cost at the next level, with FRUGALITY, ANTI INFLATION and the Scurge cost reductions applied.",
            )
            .text_color(muted),
        );
    ScrollArea::new("clamworks-upgrades-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// Every upgrade with its level, saved bonus, per-level value and next cost.
fn all_upgrades_card(clamworks: &Clamworks, multikill: f64) -> AnyElement {
    let data = idleon_core::data::clamworks();
    let mut table = Table::new(["Upgrade", "Effect", "Lv", "Bonus", "Per Lv", "Next Cost"]);
    for (index, upgrade) in data.upgrades.iter().enumerate() {
        table = table.row([
            div()
                .id(("clamworks-upgrade", index))
                .child(SharedString::from(upgrade.name.clone()))
                .into_any_element(),
            div()
                .id(("clamworks-effect", index))
                .min_w_0()
                .child(effect(clamworks, index, multikill))
                .into_any_element(),
            div()
                .child(clamworks.upgrades[index].to_string())
                .into_any_element(),
            div()
                .child(money(clamworks.upgrade_bonus(index, multikill).round()))
                .into_any_element(),
            div().child(money(upgrade.per_lv)).into_any_element(),
            div()
                .child(money(clamworks.next_upgrade_cost(index)))
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("All upgrades")
                .description("The nine pearl upgrades, in the game's order, at the saved levels."),
        )
        .child(div().p_3().child(div().min_w_0().child(table)))
        .into_any_element()
}

/// The upstream rank table: pearl upgrades first, then the utility upgrades.
fn priority_card(clamworks: &Clamworks, multikill: f64) -> AnyElement {
    let ranks = clamworks.priority(multikill);
    let mut table = Table::new([
        "#",
        "Upgrade",
        "Lv",
        "Next Cost",
        "Pearls/Kill Gain",
        "Per Cost",
    ]);
    let mut position = 0;
    for rank in ranks.iter().filter(|rank| rank.is_pearl_upgrade) {
        position += 1;
        table = table.row(priority_row(
            clamworks,
            multikill,
            rank,
            Some(position),
            position == 1,
            false,
        ));
    }
    let utility: Vec<&UpgradeRank> = ranks.iter().filter(|rank| !rank.is_pearl_upgrade).collect();
    if !utility.is_empty() {
        table = table.row([
            div().into_any_element(),
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Utility upgrades")
                .into_any_element(),
            div().into_any_element(),
            div().into_any_element(),
            div().into_any_element(),
            div().into_any_element(),
        ]);
        for rank in utility {
            table = table.row(priority_row(clamworks, multikill, rank, None, false, true));
        }
    }
    Card::new()
        .header(
            CardHeader::new("Upgrade priority").description(
                "Ranked by marginal pearls-per-kill improvement per unit cost. Utility upgrades are listed separately.",
            ),
        )
        .child(div().p_3().child(div().min_w_0().child(table)))
        .into_any_element()
}

fn priority_row(
    clamworks: &Clamworks,
    multikill: f64,
    rank: &UpgradeRank,
    position: Option<usize>,
    best: bool,
    utility: bool,
) -> [AnyElement; 6] {
    let mut name = div()
        .id(("clamworks-priority", rank.index))
        .flex()
        .items_center()
        .gap_2()
        .child(effect(clamworks, rank.index, multikill));
    if best {
        name = name.child(Badge::new("Best").tone(Tone::Success));
    }
    if utility {
        let category = match rank.category {
            Category::Cost => Some(("Cost", Tone::Info)),
            Category::Clams => Some(("Clams", Tone::Accent)),
            Category::Black => Some(("Pure Pearl", Tone::Neutral)),
            Category::Pearl => None,
        };
        if let Some((label, tone)) = category {
            name = name.child(Badge::new(label).tone(tone));
        }
    }
    let gain = if rank.cost_save > 0.0 {
        div()
            .child(format!("-{:.2}% cost", rank.cost_save * 100.0))
            .into_any_element()
    } else if rank.ppk_gain > 0.0 {
        div()
            .child(format!("+{:.4}%", rank.ppk_gain * 100.0))
            .into_any_element()
    } else {
        div().child("—").into_any_element()
    };
    [
        div()
            .child(position.map_or_else(String::new, |position| position.to_string()))
            .into_any_element(),
        name.into_any_element(),
        div().child(rank.level.to_string()).into_any_element(),
        div().child(money(rank.next_cost)).into_any_element(),
        gain,
        div()
            .child(if utility {
                "—".to_string()
            } else {
                format!("{:.2e}", rank.efficiency)
            })
            .into_any_element(),
    ]
}

/// The upstream `_fmtDesc` line for an upgrade, with the saved values filled in.
fn effect(clamworks: &Clamworks, index: usize, multikill: f64) -> String {
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
fn money(value: f64) -> String {
    if value.is_finite() {
        format::rate(value)
    } else {
        "—".into()
    }
}
