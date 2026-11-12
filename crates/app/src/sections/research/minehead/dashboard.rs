//! The Minehead dashboard: the floor's stat cards, the upgrade table and the
//! floor rewards, as `renderDashboard` in `js/ui/minehead-dashboard.js`.

use super::boss_name;
use crate::state::AppState;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::forms::{Checkbox, Stepper};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use idleon_minehead::{game, upgrade_desc};
use idleon_ui::format;

pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(inputs) = state.research.minehead.inputs.clone() else {
        return crate::ui::section::no_save(cx);
    };
    let data = game();
    let muted = cx.theme().colors.fg_muted;
    let floor = inputs.floor;
    let (cols, rows) = inputs.grid_dims();
    let lives = inputs.lives().round();
    let next_upgrade_req = (0..data.upgrades.len())
        .map(upg_req)
        .filter(|req| *req > inputs.research_level)
        .min()
        .unwrap_or(u32::MAX);
    let show_spoiler = state.research.minehead.show_spoiler;
    let entity = cx.entity();

    let cards = div()
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(Statistic::new("mh-boss", "Floor", floor as f64).decimals(0))
                .caption(boss_name(floor)),
        )
        .child(
            KpiCard::new(
                Statistic::new("mh-currency", "Mine Currency", inputs.currency).decimals(0),
            )
            .caption("saved balance"),
        )
        .child(
            KpiCard::new(Statistic::new("mh-grid", "Grid", (cols * rows) as f64).decimals(0))
                .caption(format!("{cols}×{rows} tiles")),
        )
        .child(
            KpiCard::new(Statistic::new("mh-lives", "Lives", lives).decimals(0))
                .caption("MaxHP_You"),
        )
        .child(
            KpiCard::new(Statistic::new("mh-damage", "Base Damage", inputs.base_dmg()).decimals(0))
                .caption("per tile number"),
        )
        .child(
            KpiCard::new(Statistic::new("mh-hp", "Boss HP", inputs.boss_hp().round()).decimals(0))
                .caption("floor health"),
        )
        .child(
            KpiCard::new(Statistic::new("mh-mines", "Depth Charges", inputs.mines()).decimals(0))
                .caption("per grid"),
        )
        .child(
            KpiCard::new(
                Statistic::new("mh-tries", "Daily Tries", inputs.daily_tries()).decimals(0),
            )
            .caption("from grid 147"),
        )
        .child(
            KpiCard::new(
                Statistic::new("mh-cph", "Currency/hr", inputs.currency_per_hour()).decimals(0),
            )
            .caption("every account source"),
        );

    let mine_cost = entity.clone();
    let server_var = Card::new()
        .header(CardHeader::new("Server var").description(
            "A_MineCost scales every upgrade's price; the save's own value loads first.",
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .pb_3()
                .child(Caption::new("A_MineCost").text_color(muted))
                .child(
                    Stepper::new(SharedString::from("mh-svar-cost"), inputs.server_var_cost)
                        .range(1.0, 20.0)
                        .step(0.01)
                        .precision(2)
                        .on_change(move |value, _window, cx| {
                            mine_cost.update(cx, |state, cx| {
                                if let Some(inputs) = state.research.minehead.inputs.as_mut() {
                                    inputs.server_var_cost = value;
                                }
                                cx.notify();
                            });
                        }),
                ),
        );

    let spoiler = entity.clone();
    let spoiler_toggle = Checkbox::new("mh-spoiler", show_spoiler)
        .label("Show all floors (spoilers)")
        .on_change(move |on, _window, cx| {
            spoiler.update(cx, |state, cx| {
                state.research.minehead.show_spoiler = on;
                cx.notify();
            });
        });

    // The upgrade table.
    let mut table = Table::new(["Name", "Lv", "Description", "Cost", "Req"]);
    for (index, upgrade) in data.upgrades.iter().enumerate() {
        let level = inputs.levels.get(index).copied().unwrap_or(0.0);
        let req = upg_req(index);
        let locked = inputs.research_level < req;
        let maxed = upgrade.max_lv <= 998.0 && level >= upgrade.max_lv;
        let infinite = upgrade.max_lv > 998.0;
        let hidden = locked && req > next_upgrade_req;
        if hidden && !show_spoiler {
            table = table.row([
                div()
                    .id(("mh-upg-name", index))
                    .child("???")
                    .into_any_element(),
                div().child("--").into_any_element(),
                div().child("???").into_any_element(),
                div().child("--").into_any_element(),
                div().child(format!("{req}")).into_any_element(),
            ]);
            continue;
        }
        let level_text = if maxed {
            format!("{level:.0}/{:.0} MAX", upgrade.max_lv)
        } else if infinite {
            format!("{level:.0}")
        } else {
            format!("{level:.0}/{:.0}", upgrade.max_lv)
        };
        let cost = if level < upgrade.max_lv || upgrade.max_lv > 998.0 {
            format::value(inputs.upgrade_cost(index))
        } else {
            "--".into()
        };
        let desc = upgrade_desc(&inputs, index);
        let tone = if maxed {
            cx.theme().colors.success
        } else if locked {
            muted
        } else {
            cx.theme().colors.fg
        };
        table = table.row([
            div()
                .id(("mh-upg-name", index))
                .child(upgrade.name.replace('_', " "))
                .into_any_element(),
            div().text_color(tone).child(level_text).into_any_element(),
            div()
                .id(("mh-upg-desc", index))
                .text_color(muted)
                .child(desc)
                .into_any_element(),
            div().text_color(muted).child(cost).into_any_element(),
            div()
                .text_color(if locked {
                    cx.theme().colors.danger
                } else {
                    muted
                })
                .child(format!("{req}"))
                .into_any_element(),
        ]);
    }
    let table = ScrollArea::new("mh-upgrades-scroll")
        .vertical()
        .h(px(430.0))
        .child(div().min_w_0().child(table));
    let upgrades = Card::new()
        .header(
            CardHeader::new("Upgrades")
                .description(format!("Research level {}", inputs.research_level)),
        )
        .child(div().min_h_0().overflow_hidden().p_3().child(table));

    // The floor rewards.
    let mut rewards = div().flex().flex_wrap().gap_2();
    for floor_index in 0..data.floor_rewards.qty.len().min(23) {
        let cleared = floor > floor_index;
        let hidden = floor_index > floor;
        if hidden && !show_spoiler {
            rewards = rewards.child(
                div()
                    .id(("mh-reward", floor_index))
                    .flex_none()
                    .w(px(170.0))
                    .p_2()
                    .rounded_md()
                    .bg(cx.theme().colors.sunken)
                    .opacity(0.35)
                    .child(Caption::new("???").text_color(muted)),
            );
            continue;
        }
        rewards = rewards.child(
            div()
                .id(("mh-reward", floor_index))
                .flex()
                .flex_col()
                .gap_1()
                .flex_none()
                .w(px(170.0))
                .p_2()
                .rounded_md()
                .bg(cx.theme().colors.sunken)
                .when(!cleared, |card| card.opacity(0.5))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .child(Caption::new(boss_name(floor_index)).text_color(if cleared {
                            cx.theme().colors.success
                        } else {
                            muted
                        }))
                        .child(Caption::new(if cleared { "Yes" } else { "--" }).text_color(muted)),
                )
                .child(
                    Caption::new(data.floor_reward_desc(floor_index))
                        .text_color(muted)
                        .text_size(px(10.0)),
                ),
        );
    }
    let rewards = Card::new()
        .header(CardHeader::new("Boss rewards").description("Each cleared floor's bonus"))
        .child(div().p_3().child(rewards));

    div()
        .id("minehead-dashboard")
        .flex()
        .flex_col()
        .gap_3()
        .min_h_0()
        .child(cards)
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(server_var)
                .child(div().flex_1())
                .child(spoiler_toggle),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .min_h_0()
                .child(div().flex_1().min_w_0().child(upgrades))
                .child(div().w(px(380.0)).flex_none().child(rewards)),
        )
        .into_any_element()
}

/// The research level an upgrade unlocks at.
fn upg_req(index: usize) -> u32 {
    idleon_minehead::math::upg_lv_req(index)
}
