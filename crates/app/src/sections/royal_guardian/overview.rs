//! The Overview tab: counters, world progress, raw clearing values and range
//! targets, in the upstream page's order.

use super::world_rail;
use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, KpiCard, Statistic, Tone};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use idleon_ui::format;

use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_royal_guardian::RoyalGuardian;
use idleon_royal_guardian::{self as royal_guardian, Clear, ClearState, RangeTarget};

/// The overview pane, in the upstream order: counters, world progress, the
/// clearing values and the range targets.
pub(crate) fn overview(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let royal = state
        .royal
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let world = state.royal_state.world.min(royal_guardian::WORLDS - 1);
    let mut pane = div().id("royal-overview").flex().flex_col().gap_3();
    if !royal.missing_sources().is_empty() {
        pane = pane.child(missing(royal, cx));
    }
    pane.child(counters(royal))
        .child(world_progress(royal, cx))
        .child(world_rail(world, cx))
        .child(clearing_values(royal, world, cx))
        .child(range_targets(royal, world, state.royal_state.outpost, cx))
        .child(rate_note(royal, cx))
        .into_any_element()
}

/// The bonus sources this save does not carry.
fn missing(royal: &RoyalGuardian, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut lines = div().flex().flex_col().gap_1();
    for source in royal.missing_sources() {
        lines = lines.child(Caption::new(source.clone()).text_color(muted));
    }
    Card::new()
        .header(
            CardHeader::new("Inputs this save does not carry")
                .description("These factors read as zero or one instead of being guessed"),
        )
        .child(
            div()
                .id("royal-missing")
                .flex()
                .flex_col()
                .gap_1()
                .child(lines),
        )
        .into_any_element()
}

/// The first card row: what the save holds.
fn counters(royal: &RoyalGuardian) -> AnyElement {
    let counters = royal.counters();
    let outposts = format!(
        "{} of {} maps carry an outpost",
        counters.built_outposts, counters.outposts
    );
    let kinds = format!(
        "{} purified · {} support · {} savage",
        counters.purified, counters.support, counters.savage
    );
    div()
        .id("royal-counters")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            "royal-kpi-built",
            "Built outposts",
            counters.built_outposts as f64,
            "0",
            outposts,
        ))
        .child(kpi(
            "royal-kpi-worlds",
            "Worlds unlocked",
            counters.worlds_unlocked as f64,
            "0",
            format!("of {}", royal_guardian::WORLDS),
        ))
        .child(kpi(
            "royal-kpi-armory",
            "Armory levels",
            counters.armory_levels,
            "",
            "Every Armory upgrade the save bought",
        ))
        .child(kpi(
            "royal-kpi-units",
            "Units stationed",
            counters.units,
            "",
            "Permanent units included",
        ))
        .child(kpi(
            "royal-kpi-clearing",
            "Militia clearing",
            counters.militia_clearing as f64,
            "0",
            kinds,
        ))
        .child(kpi(
            "royal-kpi-clears",
            "Militia clears /hr",
            counters.militia_clears_per_hour,
            "",
            "Across every clearing outpost",
        ))
        .into_any_element()
}

/// The world progress cards: built outposts and militia clearing per world.
fn world_progress(royal: &RoyalGuardian, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut grid = div().id("royal-worlds").flex().flex_wrap().gap_2();
    for world in 0..royal_guardian::WORLDS {
        let outposts = royal.world_outposts(world);
        let built = outposts.iter().filter(|outpost| outpost.built).count();
        let clearing = outposts
            .iter()
            .filter(|outpost| {
                !outpost.built && royal.clear(outpost.map).state == ClearState::Clearing
            })
            .count();
        let unlocked = royal.world_unlocked(world);
        let label = format!(
            "W{}: {built} / {} built, {clearing} militia clearing{}",
            world + 1,
            outposts.len(),
            if unlocked { "" } else { ", locked" }
        );
        grid = grid.child(
            div()
                .id(format!("royal-world-card-{world}"))
                .role(gpui::Role::Label)
                .aria_label(label)
                .flex()
                .flex_col()
                .gap_1()
                .min_w(px(150.0))
                .flex_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Caption::new(format!("W{}", world + 1)))
                        .when(!unlocked, |row| {
                            row.child(Badge::new("Locked").tone(Tone::Danger))
                        }),
                )
                .child(
                    div()
                        .text_size(px(18.0))
                        .child(format!("{built} / {}", outposts.len())),
                )
                .child(Caption::new(format!("{clearing} militia clearing")).text_color(muted)),
        );
    }
    Card::new()
        .header(CardHeader::new("World progress").description(
            "Built outposts against the worlds the Armory has opened; a world page lists the \
                 outposts the game data carries",
        ))
        .child(div().id("royal-world-progress").child(grid))
        .into_any_element()
}

/// The raw clearing values every unbuilt outpost in the world reports.
fn clearing_values(royal: &RoyalGuardian, world: usize, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let outposts: Vec<_> = royal
        .world_outposts(world)
        .into_iter()
        .filter(|outpost| !outpost.built)
        .collect();
    let card = Card::new().header(CardHeader::new("Raw clearing values").description(
        "Progress, requirement, remaining kills, the militia rate and the ETA the save implies",
    ));
    if outposts.is_empty() {
        return card
            .child(
                div()
                    .id("royal-clear-empty")
                    .child(Caption::new("Every outpost in this world is built.").text_color(muted)),
            )
            .into_any_element();
    }
    let mut table = Table::new([
        "Outpost",
        "State",
        "Progress",
        "Requirement",
        "Remaining",
        "Militia",
        "Clears /hr",
        "ETA",
    ]);
    for outpost in outposts {
        let clear = royal.clear(outpost.map);
        table = table.row([
            div()
                .id(format!("royal-clear-{}", outpost.map))
                .flex()
                .flex_col()
                .gap_1()
                .child(outpost.name.replace('_', " "))
                .child(
                    Caption::new(match &clear.reason {
                        Some(reason) => reason.clone(),
                        None => outpost.reason.unwrap_or("").to_owned(),
                    })
                    .text_color(muted),
                )
                .into_any_element(),
            state_cell(&clear),
            number_cell(clear.progress),
            number_cell(clear.requirement),
            number_cell(clear.remaining),
            number_cell(clear.assignments),
            rate_cell(clear.rate_per_hour),
            eta_cell(clear.eta_hours),
        ]);
    }
    card.child(div().id("royal-clear-values").child(table))
        .into_any_element()
}

/// The range targets: the selected outpost's next node and the world's table.
fn range_targets(
    royal: &RoyalGuardian,
    world: usize,
    saved: Option<usize>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let built: Vec<_> = royal
        .world_outposts(world)
        .into_iter()
        .filter(|outpost| outpost.built)
        .collect();
    let card = Card::new().header(CardHeader::new("Range targets").description(
        "The nearest resource node an outpost cannot link yet, and the Advanced Logistics \
             levels that reach it",
    ));
    if built.is_empty() {
        return card
            .child(
                div()
                    .id("royal-range-empty")
                    .child(Caption::new("No built outpost in this world.").text_color(muted)),
            )
            .into_any_element();
    }
    let selected = selected_outpost(royal, world, saved);
    let entity = cx.entity();
    let choices: Vec<Choice> = built
        .iter()
        .map(|outpost| Choice::new(outpost.map.to_string(), outpost.name.replace('_', " ")))
        .collect();
    let select = Select::new("royal-outpost", choices)
        .label("Outpost")
        .selected(selected.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(map) = value.parse::<usize>() {
                entity.update(cx, |state, cx| state.set_royal_outpost(map, cx));
            }
        });
    let mut table = Table::new([
        "Outpost",
        "Type",
        "Guards",
        "Range",
        "Next node",
        "Distance",
        "Plan",
    ]);
    for outpost in &built {
        let target = royal.next_range_target(outpost.map);
        table = table.row([
            div()
                .child(outpost.name.replace('_', " "))
                .into_any_element(),
            div()
                .child(type_label(royal.outpost_type(outpost.map)))
                .into_any_element(),
            number_cell(royal.units_by_type(outpost.map)[2]),
            number_cell(royal.range(outpost.map)),
            div()
                .child(match &target {
                    Some(target) => format!("Resource {}", target.resource + 1),
                    None => "None".to_owned(),
                })
                .into_any_element(),
            div()
                .child(match &target {
                    Some(target) => format::value(target.distance),
                    None => "—".to_owned(),
                })
                .into_any_element(),
            div().child(plan_text(target.as_ref())).into_any_element(),
        ]);
    }
    card.child(
        div()
            .id("royal-range")
            .flex()
            .flex_col()
            .gap_3()
            .child(select)
            .child(panel(royal, selected))
            .child(div().id("royal-range-table").child(table)),
    )
    .into_any_element()
}

/// The selected outpost's next range target, as the upstream panel draws it.
fn panel(royal: &RoyalGuardian, map: usize) -> AnyElement {
    let outpost = royal.outpost(map);
    let mut body = div()
        .id("royal-range-target")
        .role(gpui::Role::Label)
        .flex()
        .flex_col()
        .gap_1();
    let Some(target) = royal.next_range_target(map) else {
        let label = format!("{}: every resource node is within range", outpost.name);
        return body
            .aria_label(label.clone())
            .child(Caption::new("Next range target"))
            .child(label)
            .into_any_element();
    };
    let label = format!(
        "{}: Resource {} at {}, reach {}",
        outpost.name,
        target.resource + 1,
        format::value(target.distance),
        format::value(target.reach)
    );
    body = body
        .aria_label(label)
        .child(Caption::new("Next range target"))
        .child(
            div()
                .text_size(px(15.0))
                .child(format!("Resource {}", target.resource + 1)),
        )
        .child(format!(
            "Distance {} · current reach {} · Advanced Logistics level {}",
            format::value(target.distance),
            format::value(target.reach),
            format::value(target.plan.current_level)
        ))
        .child(
            div()
                .id("royal-range-plan")
                .flex()
                .items_center()
                .gap_2()
                .child(plan_text(Some(&target)))
                .when(
                    target.plan.possible && target.plan.levels_needed.unwrap_or(0.0) > 0.0,
                    |row| {
                        row.child(
                            Badge::new(format!("{} points", format::value(target.plan.points())))
                                .tone(Tone::Accent),
                        )
                    },
                ),
        );
    body.into_any_element()
}

/// The plan as the upstream panel words it.
pub(super) fn plan_text(target: Option<&RangeTarget>) -> String {
    let Some(target) = target else {
        return "—".to_owned();
    };
    let plan = &target.plan;
    if !plan.possible {
        return plan.reason.unwrap_or("Not reachable").to_owned();
    }
    let levels = plan.levels_needed.unwrap_or(0.0);
    if levels <= 0.0 {
        return "Already in range".to_owned();
    }
    format!(
        "+{} upgrade(s): level {} → {}",
        format::value(levels),
        format::value(plan.current_level),
        format::value(plan.target_level.unwrap_or(plan.current_level)),
    )
}

/// The clearing rate's factors, and the one factor the port does not read yet.
fn rate_note(royal: &RoyalGuardian, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    Card::new()
        .header(
            CardHeader::new("Clearing rate")
                .description("Militia clearing is assignments × 4,000 × the Armory and talent factors"),
        )
        .child(
            div()
                .id("royal-rate-note")
                .flex()
                .flex_col()
                .gap_1()
                .child(format!(
                    "Xtra Clear Killz: talent 231 at level {} (×{:.4}), ×{:.4} from the clearing rate",
                    format::value(royal.xtra_clear_level()),
                    royal.talent_multi(),
                    royal.xtra_clear_killz()
                ))
                .child(
                    Caption::new(
                        "The all-talent bonus levels that raise talent 231 are not decoded yet, so a \
                         Royal Guardian carrying them reads a lower rate than the game shows.",
                    )
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

// ----- shared formatting and cells -----

/// A headline figure as a KPI card, labelled so the harness reads the number.
pub(super) fn kpi(
    id: &'static str,
    label: &'static str,
    value: f64,
    suffix: &'static str,
    caption: impl Into<gpui::SharedString>,
) -> AnyElement {
    let mut statistic = Statistic::new(id, label, value).size(TextSize::Xl);
    if !suffix.is_empty() {
        statistic = statistic.suffix(suffix);
    }
    div()
        .id(format!("{id}-figure"))
        .role(gpui::Role::Label)
        .aria_label(format!("{label}: {}", figure_text(value)))
        .flex()
        .flex_1()
        .min_w(px(170.0))
        .child(KpiCard::new(statistic).caption(caption.into()))
        .into_any_element()
}

/// A figure as the card prints it: whole numbers grouped, rates as `fmtVal`.
pub(super) fn figure_text(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format::count(value.max(0.0) as u64)
    } else {
        format::value(value)
    }
}

/// An outpost's clearing state, badged.
fn state_cell(clear: &Clear) -> AnyElement {
    let tone = match clear.state {
        ClearState::Clearing => Tone::Success,
        ClearState::WaitingMilitia | ClearState::Secured => Tone::Warning,
        ClearState::Built => Tone::Accent,
        ClearState::NonAccruing | ClearState::Unavailable => Tone::Danger,
    };
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(Badge::new(clear.state.label()).tone(tone))
        .when(clear.active, |cell| cell.child(Caption::new("active")))
        .into_any_element()
}

/// A right-aligned number.
pub(super) fn number_cell(value: f64) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_end()
        .w_full()
        .child(format::value(value))
        .into_any_element()
}

/// A rate, dimmed when nothing is clearing.
pub(super) fn rate_cell(value: f64) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_end()
        .w_full()
        .child(if value > 0.0 {
            format::value(value)
        } else {
            "—".to_owned()
        })
        .into_any_element()
}

/// An ETA in hours, or a dash when the outpost is not clearing.
pub(super) fn eta_cell(hours: Option<f64>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_end()
        .w_full()
        .child(match hours {
            Some(hours) if hours.is_finite() => format!("{}h", format::value(hours)),
            _ => "—".to_owned(),
        })
        .into_any_element()
}

/// An outpost's type, by the upstream names.
pub(super) fn type_label(kind: f64) -> &'static str {
    match kind as i64 {
        1 => "Support",
        2 => "Savage",
        _ => "Normal",
    }
}

/// The outpost the range panel explains: the saved one while it is still built
/// in this world, else the world's first built outpost.
pub(super) fn selected_outpost(royal: &RoyalGuardian, world: usize, saved: Option<usize>) -> usize {
    let built: Vec<usize> = royal
        .world_outposts(world)
        .into_iter()
        .filter(|outpost| outpost.built)
        .map(|outpost| outpost.map)
        .collect();
    saved
        .filter(|map| built.contains(map))
        .or_else(|| built.first().copied())
        .unwrap_or(0)
}
