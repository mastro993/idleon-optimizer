//! The Sushi Station upgrade table: every upgrade in unlock order with its
//! next cost and the time the bank needs to afford it.

use super::columns::{self, Column};
use super::rate_text;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableRow};
use idleon_sushi::data::game;
use idleon_sushi::math::upg_lv_req;
use idleon_sushi::{Inputs, upgrade_desc};
use idleon_ui::widgets::{Card, CardHeader, KpiStrip, Statistic, TEXT_CAPTION};

/// One row of the upgrade table, formatted.
struct Row {
    slot: usize,
    req: SharedString,
    name: SharedString,
    level: SharedString,
    bonus: SharedString,
    cost: SharedString,
    time: SharedString,
    desc: SharedString,
    unlocked: bool,
    /// Past the next unlock: drawn as `???` unless the spoilers show.
    hidden: bool,
    maxed: bool,
    affordable: bool,
}

/// The upgrade table and the income figures above it, derived from the save.
pub(super) struct Upgrades {
    rows: Vec<Row>,
    bucks: f64,
    bucks_hr: f64,
    daily_passive: f64,
    daily_saffron: f64,
    daily_total: f64,
    saffron_hrs: f64,
    research_level: u32,
}

impl Upgrades {
    pub(super) fn new(inputs: &Inputs) -> Self {
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
            if (req > inputs.research_level as f64 || previous == 0.0)
                && level == 0.0
                && req < next_req
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
            let level_text = if max_lv >= 9999.0 && !maxed {
                format!("{level:.0}")
            } else {
                format!("{level:.0}/{max_lv:.0}")
            };
            let time = if maxed {
                "MAX".to_string()
            } else if affordable {
                "Now".to_string()
            } else if days > 0.0 {
                if days < 1.0 {
                    format!("{hours:.1}h")
                } else {
                    format!("{days:.1}d")
                }
            } else {
                "--".to_string()
            };
            rows.push(Row {
                slot,
                req: format!("{req:.0}").into(),
                name: upgrade.name.clone().into(),
                level: level_text.into(),
                bonus: idleon_sushi::fmt_num(station.qty(upg)).into(),
                cost: cost.map_or("--".into(), idleon_sushi::fmt_num).into(),
                time: time.into(),
                desc: upgrade_desc(inputs, upg).into(),
                unlocked,
                hidden,
                maxed,
                affordable,
            });
        }
        Self {
            rows,
            bucks,
            bucks_hr,
            daily_passive,
            daily_saffron,
            daily_total,
            saffron_hrs: station.saffron_hrs(),
            research_level: inputs.research_level,
        }
    }
}

/// A figure scaled with its suffix, two decimals once scaled.
fn figure(id: &'static str, label: &'static str, value: f64, caption: String) -> Statistic {
    let (scaled, scale) = idleon_ui::format::scale(value);
    let decimals = if scale.is_empty() { 0 } else { 2 };
    Statistic::new(id, label, format!("{scaled:.decimals$}"))
        .when(!scale.is_empty(), |statistic| statistic.suffix(scale))
        .caption(caption)
}

/// The table's columns.
const COLUMNS: [Column; 7] = [
    ("Res Lv", Some(64.0), true),
    ("Upgrade", None, false),
    ("Level", Some(80.0), true),
    ("Bonus", Some(90.0), true),
    ("Next Cost", Some(90.0), true),
    ("Time", Some(64.0), true),
    ("Description", None, false),
];

/// A cell of the table's column `index`.
fn cell(index: usize) -> TableCell {
    columns::cell(&COLUMNS, index)
}

pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let sushi = &mut state.research.sushi;
    let Some(inputs) = sushi.inputs.as_ref() else {
        return crate::ui::section::no_save(cx);
    };
    let upgrades = sushi
        .cache
        .upgrades
        .get_or_insert_with(|| Upgrades::new(inputs));
    let show_spoiler = sushi.show_spoiler_upgrades;
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let accent = theme.primary;
    let success = theme.success;

    let cards = KpiStrip::new()
        .child(figure(
            "su-bank",
            "Bucks in Bank",
            upgrades.bucks,
            "saved balance".into(),
        ))
        .child(figure(
            "su-bank-rate",
            "Bucks/hr",
            upgrades.bucks_hr,
            rate_text(upgrades.bucks_hr),
        ))
        .child(figure(
            "su-daily-passive",
            "Daily Passive",
            upgrades.daily_passive,
            "bucks/hr × 24".into(),
        ))
        .child(figure(
            "su-daily-saffron",
            "Daily Saffron",
            upgrades.daily_saffron,
            format!("{} saffron hours", upgrades.saffron_hrs),
        ))
        .child(figure(
            "su-daily-total",
            "Effective Daily",
            upgrades.daily_total,
            "passive plus saffron".into(),
        ));

    let entity = cx.entity();
    let spoiler = Checkbox::new("su-upg-spoiler")
        .label("Show all (spoilers)")
        .checked(show_spoiler)
        .on_click(move |on, _window, cx| {
            let on = *on;
            entity.update(cx, |state, cx| {
                state.research.sushi.show_spoiler_upgrades = on;
                cx.notify();
            });
        });

    let body = TableBody::new().children(upgrades.rows.iter().map(|row| {
        if row.hidden && !show_spoiler {
            return TableRow::new()
                .child(cell(0).child(row.req.clone()))
                .child(cell(1).child("???"))
                .child(cell(2).child("--"))
                .child(cell(3).child("--"))
                .child(cell(4).child("--"))
                .child(cell(5).child("--"))
                .child(cell(6).child("???"));
        }
        TableRow::new()
            .child(cell(0).text_color(accent).child(row.req.clone()))
            .child(
                cell(1).child(
                    div()
                        .id(("su-upg-name", row.slot))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(row.name.clone()),
                ),
            )
            .child(
                cell(2)
                    .text_color(if row.maxed { success } else { accent })
                    .child(row.level.clone()),
            )
            .child(cell(3).text_color(accent).child(row.bonus.clone()))
            .child(cell(4).text_color(muted).child(row.cost.clone()))
            .child(
                cell(5)
                    .text_color(if row.maxed || row.affordable {
                        success
                    } else {
                        muted
                    })
                    .child(row.time.clone()),
            )
            .child(
                cell(6).child(
                    div()
                        .id(("su-upg-desc", row.slot))
                        .text_color(muted)
                        .text_size(TEXT_CAPTION)
                        .child(row.desc.clone()),
                ),
            )
    }));
    let table = div().h(px(430.0)).child(
        div()
            .overflow_y_scrollbar()
            .id("su-upgrades-scroll")
            .size_full()
            .child(
                div().min_w(px(720.0)).child(
                    Table::new()
                        .small()
                        .child(columns::header(&COLUMNS))
                        .child(body),
                ),
            ),
    );

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
                    CardHeader::new("All Upgrades (Unlock Order)")
                        .description(format!(
                            "Research level {} · {} slots unlocked",
                            upgrades.research_level,
                            upgrades.rows.iter().filter(|row| row.unlocked).count()
                        ))
                        .action(spoiler),
                )
                .child(table),
        )
        .into_any_element()
}
