//! The Trench card: the saved depth and its fish requirement, the fastest lure
//! and line at each projected depth, the depth projection table and the other
//! characters fishing alongside.
//!
//! Ported from `renderTrench` in the upstream `fishing-calc.html`.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::forms::{Checkbox, Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Hsla, IntoElement, ParentElement, Pixels, Role, SharedString, Styled, div,
    px,
};
use idleon_core::fishing::{self, HelperMode, TrenchPlan};

/// The depth choices the upstream selector offers.
const DEPTH_CHOICES: [usize; 5] = [3, 5, 10, 15, 25];

/// The Trench card, or the state that explains why it has nothing to show.
pub fn card(state: &mut AppState, table_height: Pixels, cx: &mut Context<AppState>) -> AnyElement {
    let Some(plan) = plan(state) else {
        return missing(state, cx);
    };
    let muted = cx.theme().colors.fg_muted;
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
    let is_best_selected = (fastest.lure, fastest.line) == (selected.lure, selected.line);
    let toolkit = |lure: usize, line: usize| {
        let data = fishing::data();
        format!(
            "{} + {}",
            data.toolkit(0, lure)
                .map_or("", |option| option.name.as_str()),
            data.toolkit(1, line)
                .map_or("", |option| option.name.as_str())
        )
    };
    let enabled_helpers = plan.helpers.iter().filter(|helper| helper.enabled).count();
    let entity = cx.entity();

    let mut kpis = div()
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            "fishing-trench-progress",
            "Depth progress",
            format!("{progress:.1}%"),
            format!(
                "{} / {} fish",
                format::number(now.requirement.progress),
                format::number(now.requirement.value)
            ),
            muted,
        ))
        .child(kpi(
            "fishing-trench-remaining",
            "Fish to next depth",
            format::number(now.requirement.remaining),
            format!(
                "After {}% reduction",
                format::number(now.requirement.reduction_pct)
            ),
            muted,
        ))
        .child(kpi(
            "fishing-trench-rate",
            "Fish / hr",
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
            muted,
        ))
        .child(kpi(
            "fishing-trench-eta",
            &format!("ETA to Depth {}", depth + 1.0),
            format::hours(selected.hours),
            "This character alone".to_string(),
            muted,
        ))
        .child(kpi(
            "fishing-trench-best",
            "Fastest lure + line",
            format::hours(fastest.hours),
            toolkit(fastest.lure, fastest.line),
            muted,
        ))
        .child(kpi(
            "fishing-trench-requirement",
            "Efficiency requirement",
            format::number(now.eff_reqs[0]),
            format!(
                "Fish 2 · Fish 3 +100 · Fish 4 +200 · yours ×{:.2}",
                state.fishing.account.efficiency / now.eff_reqs[2].max(1.0)
            ),
            muted,
        ));
    if enabled_helpers > 0 {
        kpis = kpis.child(kpi(
            "fishing-trench-team",
            &format!("ETA with {enabled_helpers} helper(s)"),
            format::hours(plan.team.hours),
            format!(
                "{} fish/hr together ({} from helpers)",
                format::number(plan.team.rate),
                format::number(plan.team.helpers_rate)
            ),
            muted,
        ));
    }
    if !is_best_selected {
        let (lure, line) = (fastest.lure, fastest.line);
        let entity = entity.clone();
        kpis = kpis.child(
            div().flex_none().child(
                Button::new("fishing-trench-use-best", "Use fastest")
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        entity.update(cx, |state, cx| state.use_fishing_toolkit(lure, line, cx));
                    }),
            ),
        );
    }

    Card::new()
        .header(
            CardHeader::new(format!("Trench · Depth {depth}")).description(format!(
                "All four Trench fish share one item and the AFK claim keeps only the last \
                     positive count, so Fish 4 carries the depth. Saved depth {} cleared · {} \
                     fish collected · {}% reduction from the jar collectible and Bolaia.",
                format::number(saved.layers()),
                format::number(saved.progress),
                format::number(now.requirement.reduction_pct)
            )),
        )
        .child(
            div()
                .id("fishing-trench")
                .flex()
                .flex_col()
                .gap_4()
                .p_3()
                .child(
                    div()
                        .id("fishing-trench-depth")
                        .role(Role::Label)
                        .aria_label(format!(
                            "Trench depth {}: {:.1}% to the next depth, {} fish left",
                            depth,
                            progress,
                            format::number(now.requirement.remaining)
                        ))
                        .child(
                            Caption::new(format!(
                                "Depth {depth} · {} layers cleared · {} fish left · Fish 4 needs \
                                 {} efficiency",
                                format::number(saved.layers()),
                                format::number(now.requirement.remaining),
                                format::number(now.eff_reqs[2])
                            ))
                            .text_color(muted),
                        ),
                )
                .child(kpis)
                .child(helpers(state, &plan, table_height, cx))
                .child(projection(state, &plan, table_height, cx)),
        )
        .into_any_element()
}

/// The Trench card when the save carries no cavern rows.
fn missing(state: &mut AppState, _cx: &mut Context<AppState>) -> AnyElement {
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
                .p_3()
                .child(
                    EmptyState::new(
                        "fishing-trench-empty-state",
                        IconName::Map,
                        if state.save_loaded {
                            "This save has no Trench depth"
                        } else {
                            "No Trench depth yet"
                        },
                    )
                    .body(
                        "Import a save whose Holes rows carry the skilling caverns to project the \
                         next depths, the fastest lure + line and the characters fishing alongside.",
                    ),
                ),
        )
        .into_any_element()
}

/// The other characters on the Trench: the mode select, the ticks and their
/// fish per hour.
fn helpers(
    state: &mut AppState,
    plan: &TrenchPlan,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let mode = state.fishing.trench_helper_mode;
    let select = Select::new(
        "fishing-trench-helper-mode",
        [
            Choice::new("saved", HelperMode::Saved.label()),
            Choice::new("best", HelperMode::Best.label()),
        ],
    )
    .label("Helpers use")
    .size(ControlSize::Sm)
    .selected(match mode {
        HelperMode::Saved => "saved",
        HelperMode::Best => "best",
    })
    .on_change(move |value, _window, cx| {
        let mode = if value == "best" {
            HelperMode::Best
        } else {
            HelperMode::Saved
        };
        entity.update(cx, |state, cx| state.set_fishing_helper_mode(mode, cx));
    });

    let body = if plan.helpers.is_empty() {
        div()
            .id("fishing-trench-helpers-empty")
            .role(Role::Label)
            .aria_label("No other character fishes the Trench")
            .child(
                EmptyState::new(
                    "fishing-trench-helpers-empty-state",
                    IconName::Map,
                    "No other character to fish with",
                )
                .body(
                    "A save with a second character lists their Trench fish per hour here; tick \
                     them to add their catch to the team ETA.",
                ),
            )
            .into_any_element()
    } else {
        let entity = cx.entity();
        let mut table = Table::new([
            "Fish with",
            "Character",
            "Saved target",
            "Fishing lv",
            "Lure + line",
            "Fish / hr",
            "ETA alone",
        ]);
        let data = fishing::data();
        for helper in &plan.helpers {
            let character = helper.character;
            let checked = helper.enabled;
            let entity = entity.clone();
            let check = Checkbox::new(format!("fishing-trench-helper-{character}"), checked)
                .label(SharedString::from(helper.name.clone()))
                .on_change(move |on, _window, cx| {
                    entity.update(cx, |state, cx| state.set_fishing_helper(character, on, cx));
                });
            let mut name = div()
                .id(format!("fishing-trench-helper-name-{character}"))
                .role(Role::Label)
                .aria_label(format!(
                    "{}: {} fish/hr, {} alone",
                    helper.name,
                    format::number(helper.outcome.fish_total),
                    format::hours(helper.hours)
                ))
                .flex()
                .items_center()
                .gap_2()
                .child(helper.name.clone());
            if helper.at_trench {
                name = name.child(Badge::new("at the Trench"));
            }
            let target = if helper.afk_target.is_empty() {
                "—".to_string()
            } else {
                helper.afk_target.clone()
            };
            table = table.row([
                check.into_any_element(),
                name.into_any_element(),
                div().text_color(muted).child(target).into_any_element(),
                div().child(helper.level.to_string()).into_any_element(),
                div()
                    .child(format!(
                        "{} + {}",
                        data.toolkit(0, helper.lure)
                            .map_or("", |option| option.name.as_str()),
                        data.toolkit(1, helper.line)
                            .map_or("", |option| option.name.as_str())
                    ))
                    .into_any_element(),
                div()
                    .id(format!("fishing-trench-helper-rate-{character}"))
                    .child(format::number(helper.outcome.fish_total))
                    .into_any_element(),
                div().child(format::hours(helper.hours)).into_any_element(),
            ]);
        }
        div()
            .id("fishing-trench-helpers")
            .min_h_0()
            .child(
                ScrollArea::new("fishing-trench-helpers-scroll")
                    .vertical()
                    .h(table_height)
                    .child(table),
            )
            .into_any_element()
    };

    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div().flex().items_center().gap_3().child(select).child(
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
    state: &mut AppState,
    plan: &TrenchPlan,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let selected_depths = state.fishing.trench_depths;
    let mut choices: Vec<Choice> = DEPTH_CHOICES
        .iter()
        .map(|depths| Choice::new(depths.to_string(), depths.to_string()))
        .collect();
    if !DEPTH_CHOICES.contains(&selected_depths) {
        choices.push(Choice::new(
            selected_depths.to_string(),
            selected_depths.to_string(),
        ));
    }
    let select = Select::new("fishing-trench-depths", choices)
        .label("Depths to project")
        .size(ControlSize::Sm)
        .selected(selected_depths.to_string())
        .on_change(move |value, _window, cx| {
            if let Ok(depths) = value.parse::<usize>() {
                entity.update(cx, |state, cx| state.set_fishing_trench_depths(depths, cx));
            }
        });

    let data = fishing::data();
    let with_team = plan.helpers.iter().any(|helper| helper.enabled);
    let mut headers = vec![
        "Depth",
        "Fish 2 eff req",
        "Fish needed",
        "Fish / catch",
        "Fish / hr",
        "ETA",
        "Total",
    ];
    if with_team {
        headers.extend(["Team fish / hr", "Team ETA", "Team total"]);
    }
    headers.extend(["Fastest lure + line", "ETA", "Total"]);
    let mut table = Table::new(headers);
    let current = plan.rows[0].depth;
    for (index, row) in plan.rows.iter().enumerate() {
        let team = plan.team.rows.get(index).copied();
        let mut cells: Vec<AnyElement> = vec![
            div()
                .id(format!("fishing-trench-depth-{}", row.depth))
                .role(Role::Label)
                .aria_label(format!(
                    "Depth {}: {} fish needed, {} fish/hr, {} to clear",
                    row.depth,
                    format::number(row.requirement.remaining),
                    format::number(row.selected.outcome.fish_total),
                    format::hours(row.selected.hours)
                ))
                .flex()
                .items_center()
                .gap_2()
                .child(format::number(row.depth))
                .when(row.depth == current, |cell| {
                    cell.child(Badge::new("current"))
                })
                .into_any_element(),
            div()
                .child(format::number(row.eff_reqs[0]))
                .into_any_element(),
            div()
                .child(format::number(row.requirement.remaining))
                .into_any_element(),
            div()
                .child(format::number(row.selected.outcome.ore_value))
                .into_any_element(),
            div()
                .child(format::number(row.selected.outcome.fish_total))
                .into_any_element(),
            div()
                .child(format::hours(row.selected.hours))
                .into_any_element(),
            div()
                .child(format::hours(row.selected.cumulative_hours))
                .into_any_element(),
        ];
        if let (true, Some(team)) = (with_team, team) {
            cells.extend([
                div().child(format::number(team.rate)).into_any_element(),
                div().child(format::hours(team.hours)).into_any_element(),
                div()
                    .child(format::hours(team.cumulative_hours))
                    .into_any_element(),
            ]);
        }
        let mut fastest = div().flex().items_center().gap_2().child(format!(
            "{} + {}",
            data.toolkit(0, row.best.lure)
                .map_or("", |option| option.name.as_str()),
            data.toolkit(1, row.best.line)
                .map_or("", |option| option.name.as_str())
        ));
        if (row.best.lure, row.best.line) == (row.selected.lure, row.selected.line) {
            fastest = fastest.child(Badge::new("selected"));
        }
        cells.extend([
            fastest.into_any_element(),
            div()
                .child(format::hours(row.best.hours))
                .into_any_element(),
            div()
                .child(format::hours(row.best.cumulative_hours))
                .into_any_element(),
        ]);
        table = table.row(cells);
    }

    Card::new()
        .header(
            CardHeader::new("Depth projection").description(
                "Each depth raises the Trench's efficiency requirement and its fish requirement; \
                 every row uses the selected lure + line, and the fastest column re-searches all \
                 225 combinations at that depth.",
            ),
        )
        .child(
            div()
                .id("fishing-trench-projection")
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(select)
                .child(
                    ScrollArea::new("fishing-trench-projection-scroll")
                        .vertical()
                        .h(table_height)
                        .child(table),
                )
                .child(
                    div()
                        .id("fishing-trench-projection-note")
                        .role(Role::Label)
                        .aria_label(format!(
                            "{} depths projected from the saved depth",
                            plan.rows.len()
                        ))
                        .child(
                            Caption::new(format!(
                                "{} depths projected · the jar collectible and the Bolaia studies \
                                 reduce every requirement · clearing a depth resets its progress in \
                                 the game, so the plan starts from the saved amount.",
                                plan.rows.len()
                            ))
                            .text_color(muted),
                        ),
                ),
        )
        .into_any_element()
}

/// One Trench KPI card.
fn kpi(id: &'static str, label: &str, value: String, caption: String, muted: Hsla) -> AnyElement {
    Card::new()
        .child(
            div()
                .id(id)
                .role(Role::Label)
                .aria_label(format!("{label}: {value}"))
                .flex()
                .flex_col()
                .gap_1()
                .p_3()
                .min_w(px(150.0))
                .child(Caption::new(label.to_string()).text_color(muted))
                .child(div().text_size(px(18.0)).child(value))
                .child(Caption::new(caption).text_color(muted)),
        )
        .into_any_element()
}

/// The projection the section's current state asks for.
fn plan(state: &AppState) -> Option<TrenchPlan> {
    let config = &state.fishing;
    let helpers = config.trench_helpers.as_deref().unwrap_or(&[]);
    fishing::trench_plan(
        &state.fishing_save,
        &config.account,
        &fishing::TrenchRequest {
            character: config.character,
            lure: config.lure,
            line: config.line,
            depths: config.trench_depths,
            mode: config.trench_helper_mode,
            helpers,
        },
    )
}
