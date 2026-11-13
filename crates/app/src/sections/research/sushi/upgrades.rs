//! The Sushi Station upgrade table: every upgrade in unlock order with its
//! next cost and the time the bank needs to afford it, as `_renderUpgrades` in
//! `js/ui/sushi-ui.js`.

use super::rate_text;
use crate::state::AppState;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::forms::Checkbox;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_sushi::data::game;
use idleon_sushi::math::upg_lv_req;
use idleon_sushi::{Inputs, upgrade_desc};

/// One row of the upgrade table.
struct Row {
    slot: usize,
    name: String,
    level: f64,
    max_lv: f64,
    req: f64,
    unlocked: bool,
    hidden: bool,
    bonus: f64,
    cost: Option<f64>,
    maxed: bool,
    affordable: bool,
    days: f64,
    hours: f64,
    desc: String,
}

/// The rows and the daily income they are measured against.
fn table_rows(inputs: &Inputs) -> (Vec<Row>, f64, f64, f64, f64, f64) {
    let data = game();
    let station = &inputs.station;
    let ext = &inputs.external;
    let levels = &station.levels;
    let bucks = station.bucks();
    let bucks_hr = station.bucks_per_hour(levels, ext);
    let free_chance = station.free_shaker_chance(ext);
    let expected_uses = if free_chance < 1.0 {
        1.0 / (1.0 - free_chance)
    } else {
        100.0
    };
    let has_saffron = station.qty(19) > 0.0;
    let daily_saffron = if has_saffron {
        bucks_hr * station.saffron_hrs() * expected_uses
    } else {
        0.0
    };
    let daily_passive = bucks_hr * 24.0;
    let daily_total = daily_passive + daily_saffron;
    // The lowest requirement past the current level: the next slot to unlock.
    let mut next_req = f64::INFINITY;
    for slot in 0..data.slot_to_upg.len() {
        let Some(upg) = data.slot_upg(slot) else {
            continue;
        };
        let req = upg_lv_req(slot);
        let level = station.level(upg);
        let previous = if slot == 0 {
            1.0
        } else {
            data.slot_upg(slot - 1)
                .map(|index| station.level(index))
                .unwrap_or(0.0)
        };
        if (req > inputs.research_level as f64 || previous == 0.0) && level == 0.0 && req < next_req
        {
            next_req = req;
        }
    }
    let mut rows = Vec::new();
    for slot in 0..data.slot_to_upg.len() {
        let Some(upg) = data.slot_upg(slot) else {
            continue;
        };
        let Some(upgrade) = data.upgrade(upg) else {
            continue;
        };
        let level = station.level(upg);
        let req = upg_lv_req(slot);
        let previous = if slot == 0 {
            1.0
        } else {
            data.slot_upg(slot - 1)
                .map(|index| station.level(index))
                .unwrap_or(0.0)
        };
        let unlocked = (inputs.research_level as f64 >= req && previous >= 1.0) || level > 0.0;
        let hidden = !unlocked && req > next_req;
        let max_lv = upgrade.max_lv;
        let maxed = level >= max_lv && max_lv < 9999.0;
        let cost = if unlocked && !maxed {
            Some(station.upgrade_cost(slot, levels, ext))
        } else {
            None
        };
        let deficit = cost.map(|cost| cost - bucks);
        let affordable = deficit.is_some_and(|deficit| deficit <= 0.0);
        let hours = match deficit {
            Some(deficit) if deficit > 0.0 && bucks_hr > 0.0 => deficit / bucks_hr,
            _ => 0.0,
        };
        let days = match deficit {
            Some(deficit) if deficit > 0.0 && daily_total > 0.0 => deficit / daily_total,
            _ => 0.0,
        };
        rows.push(Row {
            slot,
            name: upgrade.name.clone(),
            level,
            max_lv,
            req,
            unlocked,
            hidden,
            bonus: station.qty(upg),
            cost,
            maxed,
            affordable,
            days,
            hours,
            desc: upgrade_desc(inputs, upg),
        });
    }
    (
        rows,
        bucks,
        bucks_hr,
        daily_passive,
        daily_saffron,
        daily_total,
    )
}

pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(inputs) = state.research.sushi.inputs.clone() else {
        return crate::ui::section::no_save(cx);
    };
    let (rows, bucks, bucks_hr, daily_passive, daily_saffron, daily_total) = table_rows(&inputs);
    let show_spoiler = state.research.sushi.show_spoiler_upgrades;
    let muted = cx.theme().colors.fg_muted;
    let accent = cx.theme().colors.accent;
    let success = cx.theme().colors.success;

    let card = |id: &'static str, label: &'static str, value: f64, caption: String| {
        let (scaled, scale) = idleon_ui::format::scale(value);
        let mut statistic =
            Statistic::new(id, label, scaled).decimals(if scale.is_empty() { 0 } else { 2 });
        if !scale.is_empty() {
            statistic = statistic.suffix(scale);
        }
        KpiCard::new(statistic).caption(caption)
    };
    let cards = div()
        .flex()
        .flex_wrap()
        .gap_3()
        .child(card(
            "su-bank",
            "Bucks in Bank",
            bucks,
            "saved balance".into(),
        ))
        .child(card(
            "su-bank-rate",
            "Bucks/hr",
            bucks_hr,
            rate_text(bucks_hr),
        ))
        .child(card(
            "su-daily-passive",
            "Daily Passive",
            daily_passive,
            "bucks/hr × 24".into(),
        ))
        .child(card(
            "su-daily-saffron",
            "Daily Saffron",
            daily_saffron,
            format!("{} saffron hours", inputs.station.saffron_hrs()),
        ))
        .child(card(
            "su-daily-total",
            "Effective Daily",
            daily_total,
            "passive plus saffron".into(),
        ));

    let entity = cx.entity();
    let spoiler = Checkbox::new("su-upg-spoiler", show_spoiler)
        .label("Show all (spoilers)")
        .on_change(move |on, _window, cx| {
            entity.update(cx, |state, cx| {
                state.research.sushi.show_spoiler_upgrades = on;
                cx.notify();
            });
        });

    let mut table = Table::new([
        "Res Lv",
        "Upgrade",
        "Level",
        "Bonus",
        "Next Cost",
        "Time",
        "Description",
    ]);
    for row in &rows {
        if row.hidden && !show_spoiler {
            table = table.row([
                div().child(format!("{:.0}", row.req)).into_any_element(),
                div().child("???").into_any_element(),
                div().child("--").into_any_element(),
                div().child("--").into_any_element(),
                div().child("--").into_any_element(),
                div().child("--").into_any_element(),
                div().child("???").into_any_element(),
            ]);
            continue;
        }
        let level_text = if row.maxed {
            format!("{:.0}/{:.0}", row.level, row.max_lv)
        } else if row.max_lv >= 9999.0 {
            format!("{:.0}", row.level)
        } else {
            format!("{:.0}/{:.0}", row.level, row.max_lv)
        };
        let time = if row.maxed {
            "MAX".to_string()
        } else if row.affordable {
            "Now".to_string()
        } else if row.days > 0.0 {
            if row.days < 1.0 {
                format!("{:.1}h", row.hours)
            } else {
                format!("{:.1}d", row.days)
            }
        } else {
            "--".to_string()
        };
        table = table.row([
            div()
                .text_color(accent)
                .child(format!("{:.0}", row.req))
                .into_any_element(),
            div()
                .id(("su-upg-name", row.slot))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(row.name.clone())
                .into_any_element(),
            div()
                .text_color(if row.maxed { success } else { accent })
                .child(level_text)
                .into_any_element(),
            div()
                .text_color(accent)
                .child(idleon_sushi::fmt_num(row.bonus))
                .into_any_element(),
            div()
                .text_color(muted)
                .child(match row.cost {
                    Some(cost) => idleon_sushi::fmt_num(cost),
                    None => "--".into(),
                })
                .into_any_element(),
            div()
                .text_color(if row.maxed || row.affordable {
                    success
                } else {
                    muted
                })
                .child(time)
                .into_any_element(),
            div()
                .id(("su-upg-desc", row.slot))
                .text_color(muted)
                .text_size(px(11.0))
                .child(row.desc.clone())
                .into_any_element(),
        ]);
    }
    let table = ScrollArea::new("su-upgrades-scroll")
        .vertical()
        .h(px(430.0))
        .child(div().min_w(px(720.0)).child(table));

    div()
        .id("sushi-upgrades")
        .flex()
        .flex_col()
        .gap_3()
        .min_h_0()
        .child(cards)
        .child(
            Card::new()
                .header(
                    CardHeader::new("All Upgrades (Unlock Order)").description(format!(
                        "Research level {} · {} slots unlocked",
                        inputs.research_level,
                        rows.iter().filter(|row| row.unlocked).count()
                    )),
                )
                .child(div().flex().justify_end().px_3().pb_2().child(spoiler))
                .child(div().min_h_0().overflow_hidden().p_3().child(table)),
        )
        .into_any_element()
}
