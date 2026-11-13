//! The Sushi Station overview: the station's stat cards, the slot grid and the
//! discovered-tier map, as `_renderOverview` in `js/ui/sushi-ui.js`.

use super::rate_text;
use crate::state::AppState;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use idleon_sushi::data::game;
use idleon_sushi::math::upg_lv_req;

/// A card whose figure is scaled with its suffix.
fn card(
    id: &'static str,
    label: &'static str,
    value: f64,
    decimals: usize,
    suffix: Option<&'static str>,
    caption: impl Into<SharedString>,
) -> KpiCard {
    let (scaled, scale) = idleon_ui::format::scale(value);
    let mut statistic = Statistic::new(id, label, scaled).decimals(decimals);
    if let Some(suffix) = suffix {
        statistic = statistic.suffix(suffix);
    } else if !scale.is_empty() {
        statistic = statistic.suffix(scale);
    }
    KpiCard::new(statistic).caption(caption)
}

/// The slot tile's background and border, as the game's sprites colour them.
fn slot_colour(slot_type: i64) -> (&'static str, &'static str) {
    match slot_type {
        1 => ("#5c2020", "#7a3030"),
        2 => ("#1f3a50", "#2a5570"),
        3 => ("#4a4020", "#6a6030"),
        _ => ("#2b4a2b", "#3a6a3a"),
    }
}

/// A slot's name, as the game's four tile sprites are called.
fn slot_name(slot_type: i64) -> &'static str {
    match slot_type {
        1 => "Hot",
        2 => "Cold",
        3 => "Milktoast",
        _ => "Normal",
    }
}

/// The fireplace colours and names, one per column type.
const FIRES: [(&str, &str); 4] = [
    ("#cc4444", "Red"),
    ("#44cc44", "Green"),
    ("#6666ff", "Blue"),
    ("#ee66ee", "Pink"),
];

pub(super) fn pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(inputs) = state.research.sushi.inputs.clone() else {
        return crate::ui::section::no_save(cx);
    };
    let data = game();
    let station = &inputs.station;
    let ext = &inputs.external;
    let levels = &station.levels;
    let totals = station.knowledge_totals();
    let bucks_hr = station.bucks_per_hour(levels, ext);
    let multi = station.currency_multi(levels, ext);
    let fuel_gen = station.fuel_gen(levels, ext);
    let fuel_cap = station.fuel_capacity(levels, ext);
    let combo = idleon_sushi::combo_multiplier(station.combo_score());
    let combo_open = station.qty(39) > 0.0;
    let highest = station
        .tiers
        .iter()
        .copied()
        .filter(|tier| *tier >= 0.0)
        .fold(0.0_f64, f64::max);
    let upgrade_slots = (0..data.slot_to_upg.len())
        .filter(|slot| {
            inputs.research_level as f64 >= upg_lv_req(*slot)
                || data
                    .slot_upg(*slot)
                    .is_some_and(|upg| station.level(upg) > 0.0)
        })
        .count();
    let max_tier = data.max_tier();

    let cards = div()
        .flex()
        .flex_wrap()
        .gap_3()
        .child(card(
            "su-unique",
            "Unique Sushi",
            station.unique.into(),
            0,
            None,
            format!("of {}", max_tier + 1),
        ))
        .child(card(
            "su-active",
            "Active Slots",
            station.active_slots() as f64,
            0,
            None,
            format!("{} owned", station.slots_owned()),
        ))
        .child(card(
            "su-highest",
            "Highest Tier",
            highest + 1.0,
            0,
            Some("T"),
            "cooking or cooked",
        ))
        .child(card(
            "su-cook",
            "Max Cook Tier",
            station.max_cook_tier() + 1.0,
            0,
            Some("T"),
            format!("+{:.1}% bonus tier", station.bonus_cook_tier_pct()),
        ))
        .child(card(
            "su-bucks-hr",
            "Bucks/hr",
            bucks_hr,
            2,
            None,
            rate_text(bucks_hr),
        ))
        .child(card(
            "su-multi",
            "Currency Multi",
            multi,
            3,
            Some("x"),
            "every account source",
        ))
        .child(card(
            "su-combo",
            "Combo Meter",
            combo,
            3,
            Some("x"),
            format!(
                "score {} · {}",
                idleon_sushi::fmt_num(station.combo_score()),
                if combo_open { "unlocked" } else { "locked" }
            ),
        ))
        .child(card(
            "su-fuel-gen",
            "Fuel Gen/hr",
            fuel_gen,
            2,
            None,
            format!(
                "{} orange fire",
                idleon_sushi::fmt_num(station.orange_fire_sum())
            ),
        ))
        .child(card(
            "su-fuel-cap",
            "Fuel Capacity",
            fuel_cap,
            2,
            None,
            format!("knowledge {:.0}", totals.get(3).copied().unwrap_or(0.0)),
        ))
        .child(card(
            "su-bucks",
            "Bucks in Bank",
            station.bucks(),
            2,
            None,
            "saved balance",
        ))
        .child(card(
            "su-sparks",
            "Sparks",
            station.sparks(),
            0,
            None,
            "fireplace fuel",
        ))
        .child(card(
            "su-rog",
            "RoG Unlocked",
            station.unique.min(50).into(),
            0,
            None,
            "of 50",
        ))
        .child(card(
            "su-slots",
            "Upgrade Slots",
            upgrade_slots as f64,
            0,
            None,
            format!("of {}", data.slot_to_upg.len()),
        ));

    // The 15 x 8 slot grid, with the fireplace row under it.
    let mut rows = div().flex().flex_col().gap_1();
    for row in 0..8 {
        let mut cells = div().flex().items_center().gap_1();
        for column in 0..15 {
            let slot = row * 15 + column;
            let slot_type = station.types.get(slot).copied().unwrap_or(f64::NAN);
            let unlocked = slot_type >= 0.0;
            let tier = station.tiers.get(slot).copied().unwrap_or(f64::NAN);
            let cooking = tier >= 0.0;
            let (background, border) = slot_colour(slot_type as i64);
            let perfecto = cooking && station.is_perfecto(tier as usize);
            let mut cell = div()
                .id(("su-slot", slot))
                .flex_none()
                .w(px(26.0))
                .h(px(26.0))
                .rounded_sm()
                .border_1()
                .bg(idleon_ui::format::hex(background))
                .border_color(idleon_ui::format::hex(border));
            if !unlocked {
                cell = cell.opacity(0.15);
            }
            if cooking {
                cell = cell
                    .flex()
                    .items_center()
                    .justify_center()
                    .aria_label(format!(
                        "Slot {slot} | T{} | {} | {}/hr",
                        tier + 1.0,
                        slot_name(slot_type as i64),
                        idleon_sushi::fmt_num(
                            station.currency_per_slot(slot, station.currency_multi(levels, ext))
                        )
                    ))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(if perfecto {
                                format!("{}*", tier + 1.0)
                            } else {
                                format!("{}", tier + 1.0)
                            }),
                    );
            }
            cells = cells.child(cell);
        }
        rows = rows.child(cells);
    }
    let mut fires = div().flex().items_center().gap_1();
    for column in 0..15 {
        let fire = station.fires.get(column).copied().unwrap_or(f64::NAN);
        let mut cell = div()
            .id(("su-fire", column))
            .flex_none()
            .w(px(26.0))
            .h(px(26.0))
            .flex()
            .items_center()
            .justify_center();
        if fire >= 0.0 {
            let (colour, name) = FIRES[(fire as usize).min(FIRES.len() - 1)];
            cell = cell
                .aria_label(format!("Column {column}: {name} fireplace"))
                .child(
                    div()
                        .w(px(10.0))
                        .h(px(10.0))
                        .rounded_full()
                        .bg(idleon_ui::format::hex(colour)),
                );
        }
        fires = fires.child(cell);
    }
    let legend = div()
        .flex()
        .items_center()
        .flex_wrap()
        .gap_3()
        .mb_2()
        .child(Caption::new("Slot types").text_color(cx.theme().colors.fg_muted));
    let legend = (0..4).fold(legend, |legend, slot_type| {
        let (background, border) = slot_colour(slot_type);
        legend.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .w(px(12.0))
                        .h(px(12.0))
                        .rounded_sm()
                        .border_1()
                        .bg(idleon_ui::format::hex(background))
                        .border_color(idleon_ui::format::hex(border)),
                )
                .child(Caption::new(slot_name(slot_type))),
        )
    });
    let grid = Card::new()
        .header(
            CardHeader::new("Sushi Table")
                .description(format!("Max cook tier {:.0}", station.max_cook_tier())),
        )
        .child(
            ScrollArea::new("su-table-scroll").child(
                div()
                    .p_3()
                    .min_w(px(460.0))
                    .child(legend)
                    .child(rows)
                    .child(fires),
            ),
        );

    // The tier map: discovered tiers in green, perfectos in gold.
    let mut tier_cells = div().flex().flex_wrap().gap_1();
    for tier in 0..=max_tier {
        let discovered = station.discovered(tier);
        let perfecto = station.is_perfecto(tier);
        let (background, colour) = if perfecto {
            (cx.theme().colors.warning, cx.theme().colors.fg)
        } else if discovered {
            (cx.theme().colors.success, cx.theme().colors.fg)
        } else {
            (cx.theme().colors.sunken, cx.theme().colors.fg_subtle)
        };
        tier_cells = tier_cells.child(
            div()
                .id(("su-tier", tier))
                .flex_none()
                .w(px(28.0))
                .h(px(24.0))
                .rounded_sm()
                .flex()
                .items_center()
                .justify_center()
                .bg(background)
                .child(
                    Caption::new(if perfecto {
                        format!("{}*", tier + 1)
                    } else {
                        format!("{}", tier + 1)
                    })
                    .text_color(colour),
                ),
        );
    }
    let tier_map = Card::new()
        .header(
            CardHeader::new("Tier Map").description("Green discovered, gold perfecto, grey locked"),
        )
        .child(div().p_3().child(tier_cells));

    div()
        .id("sushi-overview")
        .flex()
        .flex_col()
        .gap_3()
        .min_h_0()
        .child(cards)
        .child(
            div()
                .flex()
                .gap_3()
                .min_h_0()
                .child(div().flex_1().min_w_0().child(grid))
                .child(div().w(px(400.0)).flex_none().child(tier_map)),
        )
        .into_any_element()
}
