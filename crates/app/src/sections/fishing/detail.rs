//! The selected pool's fish split: requirements, shares, counts and EXP, plus
//! the real-versus-panel comparison the footer explains.

use super::pools::ratio;
use super::spot_text;
use super::table;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use idleon_fishing::Evaluation;
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_CAPTION, TableColumn, Tone};

/// One fish of the pool, formatted.
struct FishRow {
    name: SharedString,
    status: Option<SharedString>,
    cells: [SharedString; 7],
}

/// The selected pool's split, formatted once per evaluation.
pub(crate) struct View {
    title: SharedString,
    description: SharedString,
    claim: SharedString,
    summary: SharedString,
    rows: Vec<FishRow>,
    totals: [SharedString; 3],
    ratio: f64,
    ratio_line: SharedString,
}

pub(crate) fn view(evaluation: &Evaluation, hours: f64) -> View {
    let outcome = &evaluation.outcome;
    let ratio = ratio(outcome);
    View {
        title: format!("{} · {}", evaluation.location.label, outcome.pool.name).into(),
        description: format!(
            "Depth bonus {} · fish per catch {} · ×{:.2} Multi-Fish · panel {} EXP/catch",
            spot_text(&evaluation.spot_bonus),
            format::number(outcome.ore_value),
            1.0 + evaluation.account.multi_fish / 100.0,
            format::number(outcome.pool.panel_exp_per_catch)
        )
        .into(),
        claim: if hours == 1.0 {
            "/hr".into()
        } else {
            format!("{hours}h").into()
        },
        summary: format!(
            "{}: {} fish and {} EXP",
            outcome.pool.name,
            format::number(outcome.fish_total),
            format::number(outcome.exp)
        )
        .into(),
        rows: outcome
            .fish
            .iter()
            .map(|fish| {
                let catches = if fish.active {
                    outcome.catches * fish.share / 100.0
                } else {
                    0.0
                };
                let status = if !fish.unlocked {
                    Some(format!("needs {} eff", format::number(fish.eff_req)).into())
                } else if !fish.active {
                    Some("skipped".into())
                } else if fish.overwritten {
                    Some("overwritten".into())
                } else {
                    None
                };
                FishRow {
                    name: format!("{}: {}", fish.slot + 1, fish.name).into(),
                    status,
                    cells: [
                        if fish.eff_req > 0.0 {
                            format::number(fish.eff_req)
                        } else {
                            "—".into()
                        }
                        .into(),
                        format!("{:.2}%", fish.share).into(),
                        format::number(fish.exp_per_catch).into(),
                        format::number(catches).into(),
                        format::number(fish.credited_count).into(),
                        format::number(fish.exp).into(),
                        if outcome.exp > 0.0 {
                            format!("{:.1}%", fish.exp / outcome.exp * 100.0)
                        } else {
                            "—".into()
                        }
                        .into(),
                    ],
                }
            })
            .collect(),
        totals: [
            format!("Share total {}%", format::number(outcome.share_total)).into(),
            format!("{} avg EXP/catch", format::number(outcome.exp_per_catch)).into(),
            format!("{} catches", format::number(outcome.catches)).into(),
        ],
        ratio,
        ratio_line: format!(
            "Real EXP/hr {} vs panel EXP/hr {} · ×{ratio:.3}",
            format::number(outcome.exp / outcome.hours),
            format::number(outcome.panel_exp_per_hour)
        )
        .into(),
    }
}

/// The selected pool's split card.
pub(crate) fn card(view: &View, cx: &mut Context<AppState>) -> AnyElement {
    let theme = cx.theme();
    let (muted, success, warning, border) = (
        theme.muted_foreground,
        theme.success,
        theme.warning,
        theme.border,
    );
    let ratio = view.ratio;
    let rows = view.rows.iter().enumerate().map(|(index, fish)| {
        table::Row::new(("fishing-detail-row", index))
            .child(table::cell(
                h_flex().gap_2().child(fish.name.clone()).children(
                    fish.status
                        .clone()
                        .map(|status| table::tag(status, Tone::Neutral)),
                ),
            ))
            .children(fish.cells.iter().cloned().map(table::cell))
    });
    Card::new()
        .header(
            CardHeader::new(view.title.clone())
                .description(view.description.clone())
                .action(table::tag(view.claim.clone(), Tone::Neutral)),
        )
        .child(
            v_flex()
                .id("fishing-detail")
                .role(Role::Label)
                .aria_label(view.summary.clone())
                .gap_3()
                .child(table::table("fishing-detail-scroll", 
                    &[
                        TableColumn::text("Fish"),
                        TableColumn::number("Efficiency req").width(96.),
                        TableColumn::number("Share").width(72.),
                        TableColumn::number("EXP / catch").width(88.),
                        TableColumn::number("Catches").width(80.),
                        TableColumn::number("Fish").width(64.),
                        TableColumn::number("EXP").width(72.),
                        TableColumn::number("EXP %").width(64.),
                    ],
                    rows,
                ))
                .child(
                    h_flex()
                        .id("fishing-detail-totals")
                        .gap_4()
                        .text_size(TEXT_CAPTION)
                        .text_color(muted)
                        .children(view.totals.iter().cloned()),
                )
                .child(
                    v_flex()
                        .id("fishing-panel-ratio")
                        .role(Role::Label)
                        .aria_label(format!("Real EXP is {ratio:.3} times the panel"))
                        .gap_1()
                        .border_t_1()
                        .border_color(border)
                        .pt_3()
                        .child(
                            Caption::new(view.ratio_line.clone()).text_color(if ratio >= 1.0 {
                                success
                            } else {
                                warning
                            }),
                        )
                        .child(
                            Caption::new(
                                "The AFK panel uses MonsterDefinitions[pool].ExpGiven for every fish and omits ore value and Multi-Fish.",
                            )
                            .text_color(muted),
                        ),
                ),
        )
        .into_any_element()
}
