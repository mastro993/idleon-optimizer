//! The Sushi tab: the knowledge totals and the tier table with each tier's
//! perfecto odds, knowledge bonus and Ring of Glory reward.

use super::columns::{self, Column};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableRow};
use idleon_sushi::Inputs;
use idleon_sushi::data::game;
use idleon_sushi::{fmt_num, knowledge_desc, rog_reward};
use idleon_ui::widgets::{Caption, Card, CardHeader};

/// One tier's row, formatted.
struct TierRow {
    /// Neither discovered nor next for a Ring of Glory: drawn as `???`
    /// unless the spoilers show.
    hidden: bool,
    odds: SharedString,
    knowledge_level: SharedString,
    knowledge: SharedString,
    rog: SharedString,
    rog_unlocked: bool,
}

/// The knowledge totals and the tier rows, derived from the save.
pub(super) struct Tiers {
    knowledge: Vec<SharedString>,
    rows: Vec<TierRow>,
    unique: u32,
}

impl Tiers {
    pub(super) fn new(inputs: &Inputs) -> Self {
        let data = game();
        let station = &inputs.station;
        let knowledge = station
            .knowledge_totals()
            .iter()
            .enumerate()
            .map(|(cat, total)| knowledge_desc(cat, *total, 0.0).into())
            .collect();
        let rog_len = data.rog_qty.len();
        let max_tier = data.max_tier().max(rog_len.saturating_sub(1));
        let rows = (0..=max_tier)
            .map(|tier| {
                let discovered = station.discovered(tier);
                let perfecto = station.is_perfecto(tier);
                let rog_unlocked = station.unique as usize > tier;
                let rog_next = station.unique as usize == tier;
                let odds = if perfecto {
                    "*".to_string()
                } else if discovered {
                    let chance = station.perfecto_odds(tier);
                    if chance > 0.0 {
                        format!("1 in {}", fmt_num((1.0 / chance).round()))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                let knowledge = if discovered {
                    match data.knowledge_cat(tier) {
                        Some(cat) => knowledge_desc(
                            cat,
                            station.knowledge_bonus(tier),
                            station.knowledge_bonus_next(tier),
                        ),
                        None => String::new(),
                    }
                } else {
                    "--".to_string()
                };
                TierRow {
                    hidden: !rog_unlocked && !rog_next && !discovered,
                    odds: odds.into(),
                    knowledge_level: if discovered {
                        format!("{:.0}", station.knowledge.get(tier).copied().unwrap_or(0.0))
                            .into()
                    } else {
                        "--".into()
                    },
                    knowledge: knowledge.into(),
                    rog: if tier < rog_len {
                        rog_reward(tier).into()
                    } else {
                        "--".into()
                    },
                    rog_unlocked,
                }
            })
            .collect();
        Self {
            knowledge,
            rows,
            unique: station.unique,
        }
    }
}

/// The tier table's columns.
const COLUMNS: [Column; 5] = [
    ("Tier", Some(56.0), false),
    ("Perfecto", Some(96.0), false),
    ("KnLv", Some(56.0), true),
    ("Kn Bonus", None, false),
    ("RoG Reward", None, false),
];

/// A cell of the tier table's column `index`.
fn cell(index: usize) -> TableCell {
    columns::cell(&COLUMNS, index)
}

pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let sushi = &mut state.research.sushi;
    let Some(inputs) = sushi.inputs.as_ref() else {
        return crate::ui::section::no_save(cx);
    };
    let tiers = sushi.cache.tiers.get_or_insert_with(|| Tiers::new(inputs));
    let show_spoiler = sushi.show_spoiler_rog;
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let warning = theme.warning;

    // The knowledge categories, with their per-level gain.
    let knowledge = Table::new()
        .small()
        .child(columns::header(&[("Category", None, false)]))
        .child(
            TableBody::new().children(tiers.knowledge.iter().enumerate().map(|(cat, line)| {
                TableRow::new().child(
                    columns::cell(&[("Category", None, false)], 0)
                        .child(div().id(("su-knowledge", cat)).child(line.clone())),
                )
            })),
        );

    let rows = TableBody::new().children(tiers.rows.iter().enumerate().map(|(tier, row)| {
        let name = format!("T{}", tier + 1);
        if row.hidden && !show_spoiler {
            return TableRow::new()
                .child(cell(0).text_color(muted).child(name))
                .child(cell(1).child("--"))
                .child(cell(2).child("--"))
                .child(cell(3).child("--"))
                .child(cell(4).child("???"));
        }
        TableRow::new()
            .child(
                cell(0).child(
                    div()
                        .id(("su-tier-row", tier))
                        .text_color(muted)
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(name),
                ),
            )
            .child(cell(1).text_color(warning).child(row.odds.clone()))
            .child(cell(2).text_color(muted).child(row.knowledge_level.clone()))
            .child(cell(3).text_color(muted).child(row.knowledge.clone()))
            .child(
                cell(4)
                    .text_color(if row.rog_unlocked { warning } else { muted })
                    .child(row.rog.clone()),
            )
    }));
    let table = div().h(px(460.0)).child(
        div()
            .overflow_y_scrollbar()
            .id("su-tiers-scroll")
            .size_full()
            .child(
                div().min_w(px(560.0)).child(
                    Table::new()
                        .small()
                        .child(columns::header(&COLUMNS))
                        .child(rows),
                ),
            ),
    );
    let knowledge = div().h(px(460.0)).child(
        div()
            .overflow_y_scrollbar()
            .id("su-knowledge-scroll")
            .size_full()
            .child(knowledge),
    );

    let entity = cx.entity();
    let spoiler = Checkbox::new("su-rog-spoiler")
        .label("Show all (spoilers)")
        .checked(show_spoiler)
        .on_click(move |on, _window, cx| {
            let on = *on;
            entity.update(cx, |state, cx| {
                state.research.sushi.show_spoiler_rog = on;
                cx.notify();
            });
        });

    div()
        .id("sushi-tiers")
        .flex()
        .gap_3()
        .min_h_0()
        .child(
            div().flex_1().min_w_0().child(
                Card::new()
                    .header(
                        CardHeader::new("Sushi Tiers")
                            .description(format!(
                                "* = Perfecto (2x knowledge). {} unique sushi.",
                                tiers.unique
                            ))
                            .action(spoiler),
                    )
                    .child(table),
            ),
        )
        .child(
            div()
                .w(px(360.0))
                .flex_none()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    Card::new()
                        .header(CardHeader::new("Knowledge Totals"))
                        .child(knowledge),
                )
                .child(
                    Caption::new("A tier's bonus counts its discovery multiplier, up to 2x.")
                        .text_color(muted),
                ),
        )
        .into_any_element()
}
