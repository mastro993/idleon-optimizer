//! The Sushi Station overview: the station's stat cards, the slot grid and the
//! discovered-tier map.

use super::rate_text;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Hsla, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use idleon_sushi::Inputs;
use idleon_sushi::data::game;
use idleon_sushi::math::upg_lv_req;
use idleon_ui::format::hex;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic};

/// How many figures one strip holds before the next strip starts.
const FIGURES_PER_STRIP: usize = 5;

/// One headline figure, formatted.
struct Figure {
    id: &'static str,
    label: &'static str,
    value: SharedString,
    suffix: Option<SharedString>,
    caption: SharedString,
}

/// A figure whose value is scaled with its suffix.
fn figure(
    id: &'static str,
    label: &'static str,
    value: f64,
    decimals: usize,
    suffix: Option<&'static str>,
    caption: impl Into<SharedString>,
) -> Figure {
    let (scaled, scale) = idleon_ui::format::scale(value);
    // Past a thousand Q the scale runs out; an exponent keeps the strip cell
    // from overflowing.
    if scaled.abs() >= 1e3 && scale == "Q" {
        return Figure {
            id,
            label,
            value: format!("{value:.2e}").into(),
            suffix: None,
            caption: caption.into(),
        };
    }
    let suffix = suffix.or((!scale.is_empty()).then_some(scale));
    Figure {
        id,
        label,
        value: format!("{scaled:.decimals$}").into(),
        suffix: suffix.map(SharedString::from),
        caption: caption.into(),
    }
}

/// One square of the station grid.
struct Slot {
    background: Hsla,
    border: Hsla,
    unlocked: bool,
    /// The tier label and the summary of a slot that is cooking.
    cooking: Option<(SharedString, SharedString)>,
}

/// The overview's figures, grid and tier map, derived from the save.
pub(super) struct Overview {
    figures: Vec<Figure>,
    max_cook_tier: f64,
    slots: Vec<Slot>,
    /// Each column's fireplace colour and name, if it has one.
    fires: Vec<Option<(Hsla, &'static str)>>,
    legend: [(Hsla, Hsla); 4],
    /// Each tier's discovered and perfecto flags.
    tiers: Vec<(bool, bool)>,
}

impl Overview {
    pub(super) fn new(inputs: &Inputs) -> Self {
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
        let figures = vec![
            figure(
                "su-unique",
                "Unique Sushi",
                station.unique.into(),
                0,
                None,
                format!("of {}", max_tier + 1),
            ),
            figure(
                "su-active",
                "Active Slots",
                station.active_slots() as f64,
                0,
                None,
                format!("{} owned", station.slots_owned()),
            ),
            figure(
                "su-highest",
                "Highest Tier",
                highest + 1.0,
                0,
                Some("T"),
                "cooking or cooked",
            ),
            figure(
                "su-cook",
                "Max Cook Tier",
                station.max_cook_tier() + 1.0,
                0,
                Some("T"),
                format!("+{:.1}% bonus tier", station.bonus_cook_tier_pct()),
            ),
            figure(
                "su-bucks-hr",
                "Bucks/hr",
                bucks_hr,
                2,
                None,
                rate_text(bucks_hr),
            ),
            figure(
                "su-multi",
                "Currency Multi",
                multi,
                3,
                Some("x"),
                "every account source",
            ),
            figure(
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
            ),
            figure(
                "su-fuel-gen",
                "Fuel Gen/hr",
                fuel_gen,
                2,
                None,
                format!(
                    "{} orange fire",
                    idleon_sushi::fmt_num(station.orange_fire_sum())
                ),
            ),
            figure(
                "su-fuel-cap",
                "Fuel Capacity",
                fuel_cap,
                2,
                None,
                format!("knowledge {:.0}", totals.get(3).copied().unwrap_or(0.0)),
            ),
            figure(
                "su-bucks",
                "Bucks in Bank",
                station.bucks(),
                2,
                None,
                "saved balance",
            ),
            figure(
                "su-sparks",
                "Sparks",
                station.sparks(),
                0,
                None,
                "fireplace fuel",
            ),
            figure(
                "su-rog",
                "RoG Unlocked",
                station.unique.min(50).into(),
                0,
                None,
                "of 50",
            ),
            figure(
                "su-slots",
                "Upgrade Slots",
                upgrade_slots as f64,
                0,
                None,
                format!("of {}", data.slot_to_upg.len()),
            ),
        ];

        // The 15 x 8 slot grid, with the fireplace row under it.
        let slots = (0..15 * 8)
            .map(|slot| {
                let slot_type = station.types.get(slot).copied().unwrap_or(f64::NAN);
                let tier = station.tiers.get(slot).copied().unwrap_or(f64::NAN);
                let (background, border) = slot_colour(slot_type as i64);
                let cooking = (tier >= 0.0).then(|| {
                    let perfecto = station.is_perfecto(tier as usize);
                    let label = if perfecto {
                        format!("{}*", tier + 1.0)
                    } else {
                        format!("{}", tier + 1.0)
                    };
                    let summary = format!(
                        "Slot {slot} | T{} | {} | {}/hr",
                        tier + 1.0,
                        slot_name(slot_type as i64),
                        idleon_sushi::fmt_num(station.currency_per_slot(slot, multi))
                    );
                    (label.into(), summary.into())
                });
                Slot {
                    background: hex(background),
                    border: hex(border),
                    unlocked: slot_type >= 0.0,
                    cooking,
                }
            })
            .collect();
        let fires = (0..15)
            .map(|column| {
                let fire = station.fires.get(column).copied().unwrap_or(f64::NAN);
                (fire >= 0.0).then(|| {
                    let (colour, name) = FIRES[(fire as usize).min(FIRES.len() - 1)];
                    (hex(colour), name)
                })
            })
            .collect();
        let legend = std::array::from_fn(|slot_type| {
            let (background, border) = slot_colour(slot_type as i64);
            (hex(background), hex(border))
        });
        let tiers = (0..=max_tier)
            .map(|tier| (station.discovered(tier), station.is_perfecto(tier)))
            .collect();
        Self {
            figures,
            max_cook_tier: station.max_cook_tier(),
            slots,
            fires,
            legend,
            tiers,
        }
    }
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

pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let sushi = &mut state.research.sushi;
    let Some(inputs) = sushi.inputs.as_ref() else {
        return crate::ui::section::no_save(cx);
    };
    let overview = sushi
        .cache
        .overview
        .get_or_insert_with(|| Overview::new(inputs));
    let theme = cx.theme();
    let muted = theme.muted_foreground;

    let cards =
        div()
            .flex()
            .flex_col()
            .gap_3()
            .children(overview.figures.chunks(FIGURES_PER_STRIP).map(|figures| {
                KpiStrip::new().children(figures.iter().map(|figure| {
                    Statistic::new(figure.id, figure.label, figure.value.clone())
                        .when_some(figure.suffix.clone(), |statistic, suffix| {
                            statistic.suffix(suffix)
                        })
                        .caption(figure.caption.clone())
                }))
            }));

    let mut rows = div().flex().flex_col().gap_1();
    for row in overview.slots.chunks(15).enumerate().map(|(row, slots)| {
        div()
            .flex()
            .items_center()
            .gap_1()
            .children(slots.iter().enumerate().map(move |(column, slot)| {
                div()
                    .id(("su-slot", row * 15 + column))
                    .flex_none()
                    .w(px(26.0))
                    .h(px(26.0))
                    .rounded_sm()
                    .border_1()
                    .bg(slot.background)
                    .border_color(slot.border)
                    .when(!slot.unlocked, |cell| cell.opacity(0.15))
                    .when_some(slot.cooking.clone(), |cell, (label, summary)| {
                        cell.flex()
                            .items_center()
                            .justify_center()
                            .aria_label(summary)
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(label),
                            )
                    })
            }))
    }) {
        rows = rows.child(row);
    }
    let fires =
        div()
            .flex()
            .items_center()
            .gap_1()
            .children(overview.fires.iter().enumerate().map(|(column, fire)| {
                div()
                    .id(("su-fire", column))
                    .flex_none()
                    .w(px(26.0))
                    .h(px(26.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when_some(*fire, |cell, (colour, name)| {
                        cell.aria_label(format!("Column {column}: {name} fireplace"))
                            .child(div().w(px(10.0)).h(px(10.0)).rounded_full().bg(colour))
                    })
            }));
    let legend =
        div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap_3()
            .mb_2()
            .child(Caption::new("Slot types").text_color(muted))
            .children(overview.legend.iter().enumerate().map(
                |(slot_type, (background, border))| {
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
                                .bg(*background)
                                .border_color(*border),
                        )
                        .child(Caption::new(slot_name(slot_type as i64)))
                },
            ));
    let grid = Card::new()
        .header(
            CardHeader::new("Sushi Table")
                .description(format!("Max cook tier {:.0}", overview.max_cook_tier)),
        )
        .child(
            div().overflow_x_scrollbar().id("su-table-scroll").child(
                div()
                    .min_w(px(460.0))
                    .child(legend)
                    .child(rows)
                    .child(fires),
            ),
        );

    // The tier map: discovered tiers in green, perfectos in gold.
    let tier_cells =
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(
                overview
                    .tiers
                    .iter()
                    .enumerate()
                    .map(|(tier, (discovered, perfecto))| {
                        let (background, colour) = if *perfecto {
                            (theme.warning, theme.foreground)
                        } else if *discovered {
                            (theme.success, theme.foreground)
                        } else {
                            (theme.muted, theme.muted_foreground)
                        };
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
                                Caption::new(if *perfecto {
                                    format!("{}*", tier + 1)
                                } else {
                                    format!("{}", tier + 1)
                                })
                                .text_color(colour),
                            )
                    }),
            );
    let tier_map = Card::new()
        .header(
            CardHeader::new("Tier Map").description("Green discovered, gold perfecto, grey locked"),
        )
        .child(tier_cells);

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
