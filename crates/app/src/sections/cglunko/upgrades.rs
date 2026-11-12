//! The Cglunko upgrades: every upgrade on the selected page, in the game's
//! order.
//!
//! Ports the Upgrades tab of `cglunko-calc.html`: the upstream `_fmtDesc` line
//! with the saved bonus filled in, the level, the bonus, the per-level value and
//! the `upgNextCost` the next level charges. The stone each upgrade is paid from
//! is added here, because the plan on the Optimizer tab spends exactly those
//! saved balances.

use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div};
use idleon_cglunko::{self as cglunko, Page, stone};
use idleon_ui::format;

pub(crate) fn upgrades_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let cglunko = state.cglunko.clone();
    let page = state.cglunko_page;
    let muted = cx.theme().colors.fg_muted;
    let body = div()
        .id("cglunko-upgrades")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(page_card(&cglunko, page))
        .child(
            Caption::new(
                "Next Cost is the upstream upgNextCost at the next level: the base raised to that level plus the level, divided by the Active Drops discount, five times as much on the odd indexes, and paid from the upgrade's own stone. The Optimizer tab spends these saved balances.",
            )
            .text_color(muted),
        );
    ScrollArea::new("cglunko-upgrades-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// Every upgrade on the page with its level, bonus, per-level value and next
/// cost, plus the stone that funds it.
fn page_card(cglunko: &idleon_cglunko::Cglunko, page: Page) -> AnyElement {
    let mut table = Table::new([
        "Upgrade",
        "Effect",
        "Lv",
        "Bonus",
        "Per Lv",
        "Next Cost",
        "Stones",
    ]);
    for index in page.upgrades() {
        let cost = cglunko.next_cost(index);
        let balance = cglunko.stone_balance(index);
        let affordable = cost.is_finite() && cost <= balance;
        let mut stones = div()
            .id(("cglunko-stones", index))
            .flex()
            .flex_col()
            .items_start()
            .gap_1()
            .child(Caption::new(format!(
                "{} #{} · {} held",
                page.label(),
                stone(index) % 6 + 1,
                format::rate(balance)
            )));
        if !affordable {
            stones = stones.child(Badge::new("Short").tone(Tone::Warning));
        }
        table = table.row([
            div()
                .id(("cglunko-upgrade", index))
                .child(SharedString::from(cglunko::upgrade_name(index)))
                .into_any_element(),
            div()
                .id(("cglunko-effect", index))
                .min_w_0()
                .child(effect(cglunko, index))
                .into_any_element(),
            div()
                .child(cglunko.level(index).to_string())
                .into_any_element(),
            div()
                .child(format::value(cglunko.bonus(index)))
                .into_any_element(),
            div()
                .child(format::value(cglunko::upgrade_per_lv(index)))
                .into_any_element(),
            div()
                .child(if cost.is_finite() {
                    format::value(cost)
                } else {
                    "—".into()
                })
                .into_any_element(),
            stones.into_any_element(),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new(format!("Page {} ({})", page.index() + 1, page.label())).description(
                "The twelve upgrades on this page and the stone balance each one is paid from.",
            ),
        )
        .child(div().p_3().child(div().min_w_0().child(table)))
        .into_any_element()
}

/// The upstream `_fmtDesc` line, with `{` the saved bonus, `}` the multiplier it
/// buys and `$` the cost reduction it applies.
pub(super) fn effect(cglunko: &idleon_cglunko::Cglunko, index: usize) -> String {
    let bonus = cglunko.bonus(index);
    let multiplier = 1.0 + bonus / 100.0;
    let value = if bonus > 0.0 {
        bonus.round()
    } else {
        cglunko::upgrade_per_lv(index)
    };
    cglunko::upgrade_description(index)
        .replace('{', &format::value(bonus))
        .replace('}', &format!("{multiplier:.2}"))
        .replace('$', &format::value(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_cglunko::Cglunko;

    fn sample() -> Cglunko {
        let mut upgrades = [0_u32; cglunko::UPGRADE_COUNT];
        upgrades[1] = 2; // Slop Drop: 15 per level.
        upgrades[7] = 5; // Active Drops: 3 per level.
        Cglunko {
            upgrades,
            saved: true,
            ..Cglunko::default()
        }
    }

    #[test]
    fn the_effect_fills_the_upstream_placeholders() {
        let cglunko = sample();
        assert_eq!(
            effect(&cglunko, 1),
            "Boost Drop Rate here in The Cove by 30.0%"
        );
        assert_eq!(
            effect(&cglunko, 3),
            "Mulitply Drop Rate here in this Cove by 1.00x"
        );
        // A zero level reports the per-level value for the cost reduction.
        assert_eq!(
            effect(&cglunko, 7),
            "Decrease the cost of all upgrades here by 15.0%"
        );
        // The locked upgrade still carries its upstream line.
        assert_eq!(effect(&cglunko, 23), "World 7 Skill 3 my man, not yet.");
    }
}
