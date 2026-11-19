//! The Trench card: the saved depth and its fish requirement, the fastest lure
//! and line at each projected depth, the depth projection table and the other
//! characters fishing alongside.
//!
//! The projection re-searches all 225 combinations at every depth, so it runs
//! once per [`TrenchKey`](super::controls::TrenchKey) and is kept formatted.
//! Both tables are bounded: one row per character and at most 25 depths.

use super::controls::{cached, helper_choices};
use super::table::{self, Kpi};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Role, SharedString, Styled, Window, div,
    px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use idleon_fishing::{self as fishing, TrenchPlan};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, Choice, Tone};

/// The depth choices the selector offers.
const DEPTH_CHOICES: [usize; 5] = [3, 5, 10, 15, 25];

/// One other character on the Trench, formatted.
struct HelperRow {
    character: usize,
    name: SharedString,
    label: SharedString,
    enabled: bool,
    at_trench: bool,
    target: SharedString,
    level: SharedString,
    toolkit: SharedString,
    rate: SharedString,
    hours: SharedString,
}

/// One projected depth, formatted.
struct DepthRow {
    depth: SharedString,
    label: SharedString,
    current: bool,
    selected: [SharedString; 6],
    team: Option<[SharedString; 3]>,
    fastest: SharedString,
    fastest_selected: bool,
    best: [SharedString; 2],
}

/// The projection, formatted once per Trench key.
pub(crate) struct View {
    title: SharedString,
    description: SharedString,
    depth_label: SharedString,
    depth_line: SharedString,
    kpis: Vec<Kpi>,
    /// The fastest toolkit, when it is not the selected one.
    use_fastest: Option<(usize, usize)>,
    helpers: Vec<HelperRow>,
    with_team: bool,
    depths: Vec<DepthRow>,
    note: SharedString,
    note_line: SharedString,
}

/// The Trench card, or the state that explains why it has nothing to show.
pub(crate) fn card(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let key = state.fishing_trench_key();
    let view = cached(
        state,
        |controls| &mut controls.trench,
        key,
        cx,
        |state, _| build(state),
    );
    let Some(view) = view.as_ref() else {
        return missing(state, cx);
    };
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let use_fastest = view.use_fastest.map(|(lure, line)| {
        div().flex_none().child(
            Button::new("fishing-trench-use-best")
                .label("Use fastest")
                .small()
                .on_click(move |_event, _window, cx| {
                    entity.update(cx, |state, cx| state.use_fishing_toolkit(lure, line, cx));
                }),
        )
    });
    let (first, rest) = view.kpis.split_at(view.kpis.len().min(4));
    Card::new()
        .header(CardHeader::new(view.title.clone()).description(view.description.clone()))
        .child(
            v_flex()
                .id("fishing-trench")
                .gap_4()
                .child(
                    div()
                        .id("fishing-trench-depth")
                        .role(Role::Label)
                        .aria_label(view.depth_label.clone())
                        .child(Caption::new(view.depth_line.clone()).text_color(muted)),
                )
                .child(table::kpis(first))
                .child(
                    h_flex()
                        .gap_3()
                        .child(table::kpis(rest).flex_1())
                        .children(use_fastest),
                )
                .child(helpers(state, view, window, cx))
                .child(projection(state, view, window, cx)),
        )
        .into_any_element()
}

/// Project the depths once.
fn build(state: &AppState) -> Option<View> {
    let config = &state.fishing;
    let plan = fishing::trench_plan(
        &state.fishing_save,
        &config.account,
        &fishing::TrenchRequest {
            character: config.character,
            lure: config.lure,
            line: config.line,
            depths: config.trench_depths,
            mode: config.trench_helper_mode,
            helpers: config.trench_helpers.as_deref().unwrap_or(&[]),
        },
    )?;
    Some(view(state, &plan))
}

fn toolkit(lure: usize, line: usize) -> String {
    let data = fishing::data();
    format!(
        "{} + {}",
        data.toolkit(0, lure)
            .map_or("", |option| option.name.as_str()),
        data.toolkit(1, line)
            .map_or("", |option| option.name.as_str())
    )
}

fn view(state: &AppState, plan: &TrenchPlan) -> View {
    let now = &plan.rows[0];
    let depth = now.depth;
    let saved = &plan.trench;
    let progress = if now.requirement.value > 0.0 {
        (now.requirement.progress / now.requirement.value * 100.0).min(100.0)
    } else {
        0.0
    };
    let credited = now.selected.outcome.fish.iter().find(|fish| fish.credited);
    let fastest = &now.best;
    let selected = &now.selected;
    let enabled = plan.helpers.iter().filter(|helper| helper.enabled).count();
    let kpi = |id, label: String, value: String, caption: String| -> Kpi {
        (id, label.into(), value.into(), caption.into())
    };
    let mut kpis = vec![
        kpi(
            "fishing-trench-progress",
            "Depth progress".into(),
            format!("{progress:.1}%"),
            format!(
                "{} / {} fish",
                format::number(now.requirement.progress),
                format::number(now.requirement.value)
            ),
        ),
        kpi(
            "fishing-trench-remaining",
            "Fish to next depth".into(),
            format::number(now.requirement.remaining),
            format!(
                "After {}% reduction",
                format::number(now.requirement.reduction_pct)
            ),
        ),
        kpi(
            "fishing-trench-rate",
            "Fish / hr".into(),
            format::number(selected.outcome.fish_total),
            match credited {
                Some(fish) => format!(
                    "Fish {} at {:.2}% · {} overwritten",
                    fish.slot + 1,
                    fish.share,
                    format::number(selected.outcome.overwritten_fish)
                ),
                None => "No Trench fish yet".into(),
            },
        ),
        kpi(
            "fishing-trench-eta",
            format!("ETA to Depth {}", depth + 1.0),
            format::hours(selected.hours),
            "This character alone".into(),
        ),
        kpi(
            "fishing-trench-best",
            "Fastest lure + line".into(),
            format::hours(fastest.hours),
            toolkit(fastest.lure, fastest.line),
        ),
        kpi(
            "fishing-trench-requirement",
            "Efficiency requirement".into(),
            format::number(now.eff_reqs[0]),
            format!(
                "Fish 2 · Fish 3 +100 · Fish 4 +200 · yours ×{:.2}",
                state.fishing.account.efficiency / now.eff_reqs[2].max(1.0)
            ),
        ),
    ];
    if enabled > 0 {
        kpis.push(kpi(
            "fishing-trench-team",
            format!("ETA with {enabled} helper(s)"),
            format::hours(plan.team.hours),
            format!(
                "{} fish/hr together ({} from helpers)",
                format::number(plan.team.rate),
                format::number(plan.team.helpers_rate)
            ),
        ));
    }
    let with_team = plan.helpers.iter().any(|helper| helper.enabled);
    View {
        title: format!("Trench · Depth {depth}").into(),
        description: format!(
            "All four Trench fish share one item and the AFK claim keeps only the last \
             positive count, so Fish 4 carries the depth. Saved depth {} cleared · {} \
             fish collected · {}% reduction from the jar collectible and Bolaia.",
            format::number(saved.layers()),
            format::number(saved.progress),
            format::number(now.requirement.reduction_pct)
        )
        .into(),
        depth_label: format!(
            "Trench depth {}: {:.1}% to the next depth, {} fish left",
            depth,
            progress,
            format::number(now.requirement.remaining)
        )
        .into(),
        depth_line: format!(
            "Depth {depth} · {} layers cleared · {} fish left · Fish 4 needs {} efficiency",
            format::number(saved.layers()),
            format::number(now.requirement.remaining),
            format::number(now.eff_reqs[2])
        )
        .into(),
        kpis,
        use_fastest: ((fastest.lure, fastest.line) != (selected.lure, selected.line))
            .then_some((fastest.lure, fastest.line)),
        helpers: plan
            .helpers
            .iter()
            .map(|helper| HelperRow {
                character: helper.character,
                name: helper.name.clone().into(),
                label: format!(
                    "{}: {} fish/hr, {} alone",
                    helper.name,
                    format::number(helper.outcome.fish_total),
                    format::hours(helper.hours)
                )
                .into(),
                enabled: helper.enabled,
                at_trench: helper.at_trench,
                target: if helper.afk_target.is_empty() {
                    "—".into()
                } else {
                    helper.afk_target.clone().into()
                },
                level: helper.level.to_string().into(),
                toolkit: toolkit(helper.lure, helper.line).into(),
                rate: format::number(helper.outcome.fish_total).into(),
                hours: format::hours(helper.hours).into(),
            })
            .collect(),
        with_team,
        depths: plan
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| DepthRow {
                depth: format::number(row.depth).into(),
                label: format!(
                    "Depth {}: {} fish needed, {} fish/hr, {} to clear",
                    row.depth,
                    format::number(row.requirement.remaining),
                    format::number(row.selected.outcome.fish_total),
                    format::hours(row.selected.hours)
                )
                .into(),
                current: row.depth == depth,
                selected: [
                    format::number(row.eff_reqs[0]).into(),
                    format::number(row.requirement.remaining).into(),
                    format::number(row.selected.outcome.ore_value).into(),
                    format::number(row.selected.outcome.fish_total).into(),
                    format::hours(row.selected.hours).into(),
                    format::hours(row.selected.cumulative_hours).into(),
                ],
                team: plan.team.rows.get(index).filter(|_| with_team).map(|team| {
                    [
                        format::number(team.rate).into(),
                        format::hours(team.hours).into(),
                        format::hours(team.cumulative_hours).into(),
                    ]
                }),
                fastest: toolkit(row.best.lure, row.best.line).into(),
                fastest_selected: (row.best.lure, row.best.line)
                    == (row.selected.lure, row.selected.line),
                best: [
                    format::hours(row.best.hours).into(),
                    format::hours(row.best.cumulative_hours).into(),
                ],
            })
            .collect(),
        note: format!("{} depths projected from the saved depth", plan.rows.len()).into(),
        note_line: format!(
            "{} depths projected · the jar collectible and the Bolaia studies reduce every \
             requirement · clearing a depth resets its progress in the game, so the plan starts \
             from the saved amount.",
            plan.rows.len()
        )
        .into(),
    }
}

/// An icon, a title and a line, under `id`.
fn empty(id: &'static str, title: &'static str, description: &'static str, cx: &App) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    div()
        .id(id)
        .child(
            Empty::new().header(
                EmptyHeader::new()
                    .media(EmptyMedia::new().child(Icon::new(IconName::Map).text_color(muted)))
                    .title(EmptyTitle::new().child(title))
                    .description(EmptyDescription::new().child(description)),
            ),
        )
        .into_any_element()
}

/// The Trench card when the save carries no cavern rows.
fn missing(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    Card::new()
        .header(CardHeader::new("Trench").description(
            "The depth projection reads the cavern rows from a save: the Trench depth, the jar \
             collectible and the Bolaia studies.",
        ))
        .child(
            div()
                .id("fishing-trench-missing")
                .role(Role::Label)
                .aria_label("No Trench depth in this save")
                .child(empty(
                    "fishing-trench-empty-state",
                    if state.save_loaded {
                        "This save has no Trench depth"
                    } else {
                        "No Trench depth yet"
                    },
                    "Import a save whose Holes rows carry the skilling caverns to project the \
                     next depths, the fastest lure + line and the characters fishing alongside.",
                    cx,
                )),
        )
        .into_any_element()
}

/// The other characters on the Trench: the mode select, the ticks and their
/// fish per hour.
fn helpers(
    state: &AppState,
    view: &View,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let select = &state.fishing_controls.helper_mode;
    select.show(
        helper_choices(),
        Some(&state.fishing.trench_helper_mode),
        window,
        cx,
    );
    let body = if view.helpers.is_empty() {
        div()
            .id("fishing-trench-helpers-empty")
            .role(Role::Label)
            .aria_label("No other character fishes the Trench")
            .child(empty(
                "fishing-trench-helpers-empty-state",
                "No other character to fish with",
                "A save with a second character lists their Trench fish per hour here; tick \
                 them to add their catch to the team ETA.",
                cx,
            ))
            .into_any_element()
    } else {
        let rows = view.helpers.iter().map(|helper| {
            let character = helper.character;
            let entity = cx.entity();
            table::Row::new(("fishing-trench-helper-row", character))
                .child(table::text(
                    Checkbox::new(SharedString::from(format!(
                        "fishing-trench-helper-{character}"
                    )))
                    .label(helper.name.clone())
                    .checked(helper.enabled)
                    .on_click(move |on, _window, cx| {
                        let on = *on;
                        entity.update(cx, |state, cx| state.set_fishing_helper(character, on, cx));
                    }),
                ))
                .child(table::text(
                    h_flex()
                        .id(SharedString::from(format!(
                            "fishing-trench-helper-name-{character}"
                        )))
                        .role(Role::Label)
                        .aria_label(helper.label.clone())
                        .gap_2()
                        .child(helper.name.clone())
                        .when(helper.at_trench, |cell| {
                            cell.child(table::tag("at the Trench", Tone::Info))
                        }),
                ))
                .child(table::text(
                    div().text_color(muted).child(helper.target.clone()),
                ))
                .child(table::number(helper.level.clone()))
                .child(table::text(helper.toolkit.clone()))
                .child(table::number(
                    div()
                        .id(SharedString::from(format!(
                            "fishing-trench-helper-rate-{character}"
                        )))
                        .child(helper.rate.clone()),
                ))
                .child(table::number(helper.hours.clone()))
        });
        div()
            .id("fishing-trench-helpers")
            .child(table::table(
                "fishing-trench-helpers-scroll",
                &table::columns(&[
                    ("Fish with", false, 110.0),
                    ("Character", false, 220.0),
                    ("Saved target", false, 140.0),
                    ("Fishing lv", true, 90.0),
                    ("Lure + line", false, 250.0),
                    ("Fish / hr", true, 100.0),
                    ("ETA alone", true, 110.0),
                ]),
                rows,
            ))
            .into_any_element()
    };

    v_flex()
        .gap_3()
        .child(
            h_flex()
                .gap_3()
                .child(
                    div().w(px(220.0)).child(
                        Select::new(select.state())
                            .id("fishing-trench-helper-mode")
                            .accessibility_label("Helpers use")
                            .small(),
                    ),
                )
                .child(
                    Caption::new(
                        "Ticked characters are assumed to AFK at the Trench. Characters saved at the \
                         Trench start ticked.",
                    )
                    .text_color(muted),
                ),
        )
        .child(body)
        .into_any_element()
}

/// The projected depths: the depth selector and the per-depth table.
fn projection(
    state: &AppState,
    view: &View,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let depths = state.fishing.trench_depths;
    let mut choices: Vec<Choice<usize>> = DEPTH_CHOICES
        .iter()
        .map(|depths| Choice::new(*depths, depths.to_string()))
        .collect();
    if !DEPTH_CHOICES.contains(&depths) {
        choices.push(Choice::new(depths, depths.to_string()));
    }
    let select = &state.fishing_controls.trench_depths;
    select.show(choices, Some(&depths), window, cx);

    let mut headers = vec![
        ("Depth", false, 120.0),
        ("Fish 2 eff req", true, 110.0),
        ("Fish needed", true, 110.0),
        ("Fish / catch", true, 100.0),
        ("Fish / hr", true, 100.0),
        ("ETA", true, 100.0),
        ("Total", true, 100.0),
    ];
    if view.with_team {
        headers.extend([
            ("Team fish / hr", true, 120.0),
            ("Team ETA", true, 100.0),
            ("Team total", true, 100.0),
        ]);
    }
    headers.extend([
        ("Fastest lure + line", false, 280.0),
        ("ETA", true, 100.0),
        ("Total", true, 100.0),
    ]);
    let rows = view.depths.iter().enumerate().map(|(index, row)| {
        table::Row::new(("fishing-trench-depth-row", index))
            .child(table::text(
                h_flex()
                    .id(SharedString::from(format!(
                        "fishing-trench-depth-{}",
                        row.depth
                    )))
                    .role(Role::Label)
                    .aria_label(row.label.clone())
                    .gap_2()
                    .child(row.depth.clone())
                    .when(row.current, |cell| {
                        cell.child(table::tag("current", Tone::Neutral))
                    }),
            ))
            .children(row.selected.iter().cloned().map(table::number))
            .children(row.team.iter().flatten().cloned().map(table::number))
            .child(table::text(
                h_flex()
                    .gap_2()
                    .child(row.fastest.clone())
                    .when(row.fastest_selected, |cell| {
                        cell.child(table::tag("selected", Tone::Accent))
                    }),
            ))
            .children(row.best.iter().cloned().map(table::number))
    });

    Card::new()
        .header(CardHeader::new("Depth projection").description(
            "Each depth raises the Trench's efficiency requirement and its fish requirement; \
             every row uses the selected lure + line, and the fastest column re-searches all \
             225 combinations at that depth.",
        ))
        .child(
            v_flex()
                .id("fishing-trench-projection")
                .gap_3()
                .child(
                    div().w(px(120.0)).child(
                        Select::new(select.state())
                            .id("fishing-trench-depths")
                            .accessibility_label("Depths to project")
                            .small(),
                    ),
                )
                .child(div().id("fishing-trench-depths-table").child(table::table(
                    "fishing-trench-projection-scroll",
                    &table::columns(&headers),
                    rows,
                )))
                .child(
                    div()
                        .id("fishing-trench-projection-note")
                        .role(Role::Label)
                        .aria_label(view.note.clone())
                        .child(Caption::new(view.note_line.clone()).text_color(muted)),
                ),
        )
        .into_any_element()
}
