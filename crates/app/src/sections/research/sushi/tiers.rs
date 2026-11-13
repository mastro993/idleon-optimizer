//! The Sushi tab: the knowledge totals and the tier table with each tier's
//! perfecto odds, knowledge bonus and Ring of Glory reward, as `_renderSushi`
//! in `js/ui/sushi-ui.js`.

use crate::state::AppState;
use ely_gpui_component::forms::Checkbox;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_sushi::data::game;
use idleon_sushi::{fmt_num, knowledge_desc, rog_reward};

pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(inputs) = state.research.sushi.inputs.clone() else {
        return crate::ui::section::no_save(cx);
    };
    let data = game();
    let station = &inputs.station;
    let totals = station.knowledge_totals();
    let show_spoiler = state.research.sushi.show_spoiler_rog;
    let muted = cx.theme().colors.fg_muted;
    let warning = cx.theme().colors.warning;
    let subtle = cx.theme().colors.fg_subtle;

    // The knowledge categories, with their per-level gain.
    let mut knowledge = Table::new(["Category"]);
    for (cat, total) in totals.iter().enumerate() {
        knowledge = knowledge.row([div()
            .id(("su-knowledge", cat))
            .child(knowledge_desc(cat, *total, 0.0))
            .into_any_element()]);
    }

    let mut tiers = Table::new(["Tier", "Perfecto", "KnLv", "Kn Bonus", "RoG Reward"]);
    let rog_len = data.rog_qty.len();
    let max_tier = data.max_tier().max(rog_len.saturating_sub(1));
    for tier in 0..=max_tier {
        let discovered = station.discovered(tier);
        let perfecto = station.is_perfecto(tier);
        let rog_unlocked = station.unique as usize > tier;
        let rog_next = station.unique as usize == tier;
        let hidden = !rog_unlocked && !rog_next && !discovered;
        if hidden && !show_spoiler {
            tiers = tiers.row([
                div()
                    .text_color(muted)
                    .child(format!("T{}", tier + 1))
                    .into_any_element(),
                div().child("--").into_any_element(),
                div().child("--").into_any_element(),
                div().child("--").into_any_element(),
                div().child("???").into_any_element(),
            ]);
            continue;
        }
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
        let knowledge_cell = if discovered {
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
        tiers = tiers.row([
            div()
                .id(("su-tier-row", tier))
                .text_color(muted)
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(format!("T{}", tier + 1))
                .into_any_element(),
            div().text_color(warning).child(odds).into_any_element(),
            div()
                .text_color(subtle)
                .child(if discovered {
                    format!("{:.0}", station.knowledge.get(tier).copied().unwrap_or(0.0))
                } else {
                    "--".into()
                })
                .into_any_element(),
            div()
                .text_color(muted)
                .child(knowledge_cell)
                .into_any_element(),
            div()
                .text_color(if rog_unlocked { warning } else { muted })
                .child(if tier < rog_len {
                    rog_reward(tier)
                } else {
                    "--".into()
                })
                .into_any_element(),
        ]);
    }
    let tiers = ScrollArea::new("su-tiers-scroll")
        .vertical()
        .h(px(460.0))
        .child(div().min_w(px(560.0)).child(tiers));
    let knowledge = ScrollArea::new("su-knowledge-scroll")
        .vertical()
        .h(px(460.0))
        .child(knowledge);

    let entity = cx.entity();
    let spoiler = Checkbox::new("su-rog-spoiler", show_spoiler)
        .label("Show all (spoilers)")
        .on_change(move |on, _window, cx| {
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
            Card::new()
                .header(CardHeader::new("Sushi Tiers").description(format!(
                    "* = Perfecto (2x knowledge). {} unique sushi.",
                    station.unique
                )))
                .child(div().flex().justify_end().px_3().child(spoiler))
                .child(div().min_h_0().overflow_hidden().p_3().child(tiers)),
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
                        .child(div().min_h_0().overflow_hidden().p_3().child(knowledge)),
                )
                .child(
                    Caption::new("A tier's bonus counts its discovery multiplier, up to 2x.")
                        .text_color(muted),
                ),
        )
        .into_any_element()
}
