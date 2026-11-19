//! The Cglunko upgrades: every upgrade on the selected page, in the game's
//! order.
//!
//! The Upgrades tab: the game's description line with the saved bonus filled
//! in, the level, the bonus, the per-level value and the cost the next level
//! charges. The stone each upgrade is paid from is added here, because the plan
//! on the Optimizer tab spends exactly those saved balances.

use super::table::{table, text};
use super::{pane, tag};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, div};
use idleon_cglunko::{self as cglunko, Page, stone};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, TableColumn, Tone};

pub(crate) fn upgrades_pane(state: &AppState, cx: &App) -> AnyElement {
    let body = div()
        .id("cglunko-upgrades")
        .flex()
        .flex_col()
        .gap_3()
        .py_3()
        .child(page_card(&state.cglunko, state.cglunko_page, cx))
        .child(Caption::new(
            "Next Cost is the cost of the next level: the base raised to that level plus the level, divided by the Active Drops discount, five times as much on the odd indexes, and paid from the upgrade's own stone. The Optimizer tab spends these saved balances.",
        ));
    pane("cglunko-upgrades-scroll", body)
}

/// Every upgrade on the page with its level, bonus, per-level value and next
/// cost, plus the stone that funds it.
fn page_card(cglunko: &idleon_cglunko::Cglunko, page: Page, cx: &App) -> AnyElement {
    let rows = page.upgrades().map(|index| {
        let cost = cglunko.next_cost(index);
        let balance = cglunko.stone_balance(index);
        let affordable = cost.is_finite() && cost <= balance;
        let stones = div()
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
            )))
            .when(!affordable, |stones| {
                stones.child(tag("Short", Tone::Warning))
            });
        vec![
            div()
                .id(("cglunko-upgrade", index))
                .child(SharedString::from(cglunko::upgrade_name(index)))
                .into_any_element(),
            div()
                .id(("cglunko-effect", index))
                .min_w_0()
                .child(effect(cglunko, index))
                .into_any_element(),
            text(cglunko.level(index).to_string()),
            text(format::value(cglunko.bonus(index))),
            text(format::value(cglunko::upgrade_per_lv(index))),
            text(if cost.is_finite() {
                format::value(cost)
            } else {
                "—".into()
            }),
            stones.into_any_element(),
        ]
    });
    Card::new()
        .header(
            CardHeader::new(format!("Page {} ({})", page.index() + 1, page.label())).description(
                "The twelve upgrades on this page and the stone balance each one is paid from.",
            ),
        )
        .child(table(
            "cglunko-upgrade-table",
            &[
                TableColumn::text("Upgrade"),
                TableColumn::text("Effect"),
                TableColumn::number("Lv"),
                TableColumn::number("Bonus"),
                TableColumn::number("Per Lv"),
                TableColumn::number("Next Cost"),
                TableColumn::text("Stones"),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}

/// The game's description line, with `{` the saved bonus, `}` the multiplier it
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
    fn the_effect_fills_the_description_placeholders() {
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
        // The locked upgrade still carries its own line.
        assert_eq!(effect(&cglunko, 23), "World 7 Skill 3 my man, not yet.");
    }
}
