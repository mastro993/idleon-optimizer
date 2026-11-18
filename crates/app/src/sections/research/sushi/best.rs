//! The Best Upgrade pane: the efficiency ranking and the affordable path for
//! bucks, fuel generation and fuel capacity.

use super::columns::{self, Column};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{Table, TableBody, TableRow};
use idleon_sushi::{Inputs, Metric, Path, candidates, fmt_num, path};
use idleon_ui::widgets::{Caption, Card, CardHeader};

/// One ranking's key, title and the metric it ranks by.
struct Ranking {
    key: &'static str,
    title: &'static str,
    metric: Metric,
}

static RANKINGS: [Ranking; 3] = [
    Ranking {
        key: "bucks",
        title: "Bucks/hr",
        metric: Metric::Bucks,
    },
    Ranking {
        key: "fuel",
        title: "Fuel/hr",
        metric: Metric::FuelGen,
    },
    Ranking {
        key: "cap",
        title: "Fuel Cap",
        metric: Metric::FuelCap,
    },
];

/// How many candidates each ranking lists.
const SHOWN: usize = 25;

/// One ranked candidate, formatted.
struct Candidate {
    slot: usize,
    name: SharedString,
    level: SharedString,
    cost: SharedString,
    gain: SharedString,
    efficiency: SharedString,
}

/// One ranking's candidates and its affordable path.
struct Board {
    ranking: &'static Ranking,
    candidates: Vec<Candidate>,
    path: Path,
}

/// The three rankings and the current rates, derived from the save.
pub(super) struct Best {
    boards: Vec<Board>,
    summary: SharedString,
}

impl Best {
    pub(super) fn new(inputs: &Inputs) -> Self {
        let station = &inputs.station;
        let ext = &inputs.external;
        let levels = &station.levels;
        let boards = RANKINGS
            .iter()
            .map(|ranking| Board {
                ranking,
                candidates: candidates(inputs, ranking.metric)
                    .iter()
                    .take(SHOWN)
                    .map(|candidate| Candidate {
                        slot: candidate.slot,
                        name: candidate.name.clone().into(),
                        level: format!("{:.0}", candidate.level).into(),
                        cost: fmt_num(candidate.cost).into(),
                        gain: format!("+{}", fmt_num(candidate.gain)).into(),
                        efficiency: if candidate.efficiency > 0.0 {
                            format!("{:.2e}", candidate.efficiency).into()
                        } else {
                            "--".into()
                        },
                    })
                    .collect(),
                path: path(inputs, ranking.metric, station.bucks()),
            })
            .collect();
        let summary = format!(
            "{} bucks/hr · {} fuel/hr · {} fuel cap · {} bucks in bank",
            fmt_num(station.bucks_per_hour(levels, ext)),
            fmt_num(station.fuel_gen(levels, ext)),
            fmt_num(station.fuel_capacity(levels, ext)),
            fmt_num(station.bucks()),
        );
        Self {
            boards,
            summary: summary.into(),
        }
    }
}

/// The path table's columns.
const PATH_COLUMNS: [Column; 3] = [
    ("Upgrade", None, false),
    ("Level", Some(96.0), true),
    ("Cost", Some(80.0), true),
];

/// The ranking table's columns.
const RANK_COLUMNS: [Column; 5] = [
    ("Upgrade", None, false),
    ("Lvl", Some(48.0), true),
    ("Cost", Some(80.0), true),
    ("Gain", Some(80.0), true),
    ("Eff.", Some(72.0), true),
];

/// The path summary and its grouped purchases.
fn path_card(ranking: &Ranking, outcome: &Path, cx: &Context<AppState>) -> AnyElement {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let accent = theme.primary;
    let cell = |index| columns::cell(&PATH_COLUMNS, index);
    let table = Table::new()
        .small()
        .child(columns::header(&PATH_COLUMNS))
        .child(
            TableBody::new().children(outcome.purchases.iter().map(|purchase| {
                TableRow::new()
                    .child(
                        cell(0).child(
                            div()
                                .id(format!("su-path-{}-row-{}", ranking.key, purchase.upg))
                                .child(purchase.name.clone()),
                        ),
                    )
                    .child(
                        cell(1)
                            .text_color(accent)
                            .child(format!("{:.0} -> {:.0}", purchase.from, purchase.to)),
                    )
                    .child(cell(2).text_color(muted).child(fmt_num(purchase.cost)))
            })),
        );
    let headline = if outcome.steps == 0 {
        "No affordable upgrades.".to_string()
    } else {
        format!(
            "{} buys · {} spent · {} left",
            outcome.steps,
            fmt_num(outcome.spent),
            fmt_num(outcome.remaining)
        )
    };
    div()
        .id(format!("su-path-{}", ranking.key))
        .flex()
        .flex_col()
        .gap_1()
        .rounded(theme.radius)
        .bg(theme.muted)
        .p_2()
        .child(Caption::new(headline).text_color(muted))
        .child(
            Caption::new(format!("Total gain +{}", fmt_num(outcome.gain)))
                .text_color(theme.success),
        )
        .children((outcome.steps > 0).then_some(table))
        .into_any_element()
}

pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let sushi = &mut state.research.sushi;
    let Some(inputs) = sushi.inputs.as_ref() else {
        return crate::ui::section::no_save(cx);
    };
    let best = sushi.cache.best.get_or_insert_with(|| Best::new(inputs));
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let accent = theme.primary;
    let success = theme.success;

    let mut body = div().flex().flex_col().gap_3();
    for board in &best.boards {
        let ranking = board.ranking;
        let cell = |index| columns::cell(&RANK_COLUMNS, index);
        let table = Table::new()
            .small()
            .child(columns::header(&RANK_COLUMNS))
            .child(
                TableBody::new().children(board.candidates.iter().map(|candidate| {
                    TableRow::new()
                        .child(
                            cell(0).child(
                                div()
                                    .id(format!("su-best-{}-{}", ranking.key, candidate.slot))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(candidate.name.clone()),
                            ),
                        )
                        .child(cell(1).text_color(accent).child(candidate.level.clone()))
                        .child(cell(2).text_color(muted).child(candidate.cost.clone()))
                        .child(cell(3).text_color(success).child(candidate.gain.clone()))
                        .child(
                            cell(4)
                                .text_color(accent)
                                .child(candidate.efficiency.clone()),
                        )
                })),
            );
        let table = div().h(px(330.0)).child(
            div()
                .overflow_y_scrollbar()
                .id(format!("su-best-{}-scroll", ranking.key))
                .size_full()
                .child(div().min_w(px(520.0)).child(table)),
        );
        body = body.child(
            Card::new()
                .header(
                    CardHeader::new(ranking.title)
                        .description("Gain per buck, at the saved levels"),
                )
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap_3()
                        .child(div().w(px(380.0)).flex_none().child(path_card(
                            ranking,
                            &board.path,
                            cx,
                        )))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .overflow_hidden()
                                .child(table),
                        ),
                ),
        );
    }

    div()
        .id("sushi-best")
        .flex()
        .flex_col()
        .gap_3()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .child(Caption::new(best.summary.clone()))
                .child(
                    Caption::new("Ranked by gain per buck, at the saved levels.").text_color(muted),
                ),
        )
        .child(
            div().flex_1().min_h_0().child(
                div()
                    .overflow_y_scrollbar()
                    .id("su-best-scroll")
                    .size_full()
                    .child(body),
            ),
        )
        .into_any_element()
}
