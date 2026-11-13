//! JO-04: the Sections tab, the board section planner.
//!
//! Ports the upstream `#tab-sections` panel of `jelly-operator-calc.html`: the
//! strategy, objective, EXP target, token, quality and beam controls, the ranked
//! section sequences, the two boards and every individual section. The planner
//! itself lives in `idleon_jelly::planner`; this module owns the controls and
//! renders the plan.

use super::grid;
use crate::sections::jelly_operator::estimate::text_kpi;
use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::{Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use idleon_jelly::board::Layout;
use idleon_jelly::planner::{self, Objective, Plan, PlanOptions, Quality, SectionRow, Strategy};
use idleon_jelly::{Estimate, Jelly, board};
use idleon_ui::format;
use idleon_ui::run_chrome;

/// The page's planner controls (`sectionControls`). The token count starts
/// unset so the first imported save fills in its unused sections.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Planner {
    pub(crate) strategy: Strategy,
    pub(crate) objective: Objective,
    pub(crate) exp_cell_type: Option<usize>,
    pub(crate) tokens: Option<usize>,
    pub(crate) quality: Quality,
    pub(crate) beam_width: usize,
    pub(crate) seed: u32,
}

impl Default for Planner {
    fn default() -> Self {
        Self {
            strategy: Strategy::Progression,
            objective: Objective::Clear,
            exp_cell_type: None,
            tokens: None,
            quality: Quality::Thorough,
            beam_width: 8,
            seed: 1,
        }
    }
}

impl Planner {
    /// Take the save's choices: its unused sections, a visible objective and an
    /// EXP target its board can own.
    pub(crate) fn adopt(&mut self, jelly: &Jelly) {
        let available = board::slot_purchases_left(jelly).max(0.0) as usize;
        if self.tokens.is_none() {
            self.tokens = Some(available.max(1));
        }
        if !self.objective.visible(jelly) {
            self.objective = Objective::Clear;
        }
        self.exp_cell_type = self
            .exp_cell_type
            .map(|cell_type| cell_type.min(jelly.units_owned().saturating_sub(1)));
    }

    /// Pick a strategy; the page also moves the objective to what it plays for.
    pub(crate) fn set_strategy(&mut self, strategy: Strategy) {
        self.strategy = strategy;
        if let Some(objective) = strategy.objective() {
            self.objective = objective;
        }
    }

    /// The request a run started now would use.
    pub(crate) fn inputs(&self, jelly: Option<&Jelly>, layout: &Layout) -> RunInputs {
        RunInputs {
            jelly: jelly.cloned(),
            layout: layout.clone(),
            planner: self.clone(),
        }
    }
}

/// The inputs one plan run reads, so an edit marks its result stale.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunInputs {
    pub(crate) jelly: Option<Jelly>,
    pub(crate) layout: Layout,
    pub(crate) planner: Planner,
}

impl RunInputs {
    /// `sectionControls` as `planJellySections` reads it.
    pub(crate) fn options(&self) -> PlanOptions {
        let available = self
            .jelly
            .as_ref()
            .map(|jelly| board::slot_purchases_left(jelly).max(0.0) as usize)
            .unwrap_or(0);
        let requested = self.planner.tokens.unwrap_or(available.min(1));
        PlanOptions {
            strategy: self.planner.strategy,
            objective: self.planner.objective,
            exp_cell_type: self.planner.exp_cell_type,
            tokens: self.planner.tokens,
            allow_hypothetical_tokens: requested > available,
            quality: self.planner.quality,
            beam_width: self.planner.beam_width,
            seed: self.planner.seed,
            layout: Some(self.layout.clone()),
        }
    }
}

/// The Sections tab: the controls, the run and the ranked plan.
pub(crate) fn pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
) -> AnyElement {
    let task = state.jelly_plan_task(cx);
    let running = task.running;
    let available = board::slot_purchases_left(jelly).max(0.0) as usize;
    let controls = controls(state, cx, jelly, available);
    let mut body = div()
        .id("jelly-sections")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            Card::new()
                .header(
                    CardHeader::new("Board section planner").description(
                        "Ranks every unpurchased section, then simulates the strongest geometry and multi-section combinations.",
                    ),
                )
                .child(div().p_3().child(controls)),
        )
        .child(
            div()
                .id("jelly-sections-tokens-note")
                .child(Caption::new(format!(
                    "{available} unused section token{} in the current save. Values above that are hypothetical.",
                    if available == 1 { "" } else { "s" }
                )))
                .text_color(cx.theme().colors.fg_muted),
        );
    if running && let Some(progress) = task.progress.clone() {
        let entity = cx.entity();
        body = body.child(run_chrome::progress_card(
            "jelly-sections",
            "Ranking board sections",
            "Screening every unpurchased section, then simulating the finalists.",
            progress,
            move |_window, cx| {
                entity.update(cx, |state, cx| state.cancel_jelly_plan(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(run_chrome::cancelled_banner(
            "jelly-sections-cancelled",
            task.stale,
        ));
    }
    let Some(plan) = task.result else {
        if !running && !task.cancelled {
            body = body.child(Banner::new(
                "jelly-sections-empty",
                Severity::Info,
                "Rank the sections to see which token buys the most board, and how its operation compares with the current one.",
            ));
        }
        return body.into_any_element();
    };
    if task.stale {
        body = body.child(run_chrome::stale_banner("jelly-sections-stale"));
    }
    body.child(result(state, cx, jelly, &plan))
        .into_any_element()
}

/// Strategy, objective, EXP target, token count, quality, beam and seed.
fn controls(
    state: &AppState,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
    available: usize,
) -> AnyElement {
    let task = state.jelly_plan_task(cx);
    let running = task.running;
    let planner = &state.jelly_state.planner;
    let entity = cx.entity();

    let strategy_entity = entity.clone();
    let strategy = Select::new(
        "jelly-sections-strategy",
        Strategy::PAGE.map(|strategy| Choice::new(strategy.id(), strategy.label())),
    )
    .label("Strategy")
    .selected(planner.strategy.id())
    .size(ControlSize::Sm)
    .on_change(move |value, _window, cx| {
        if let Some(strategy) = Strategy::from_id(value) {
            let entity = strategy_entity.clone();
            entity.update(cx, |state, cx| state.set_jelly_plan_strategy(strategy, cx));
        }
    });

    let objective_entity = entity.clone();
    let objectives: Vec<Choice> = [
        Objective::Clear,
        Objective::Bloodcells,
        Objective::Exp,
        Objective::Dps,
        Objective::Survival,
    ]
    .into_iter()
    .filter(|objective| objective.visible(jelly))
    .map(|objective| Choice::new(objective.id(), objective.label()))
    .collect();
    let objective = Select::new("jelly-sections-objective", objectives)
        .label("Objective")
        .selected(planner.objective.id())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Some(objective) = Objective::from_id(value) {
                let entity = objective_entity.clone();
                entity.update(cx, |state, cx| {
                    state.set_jelly_plan_objective(objective, cx)
                });
            }
        });

    let exp_entity = entity.clone();
    let mut exp_choices = vec![Choice::new("total", "Total EXP")];
    exp_choices.extend((0..jelly.units_owned()).map(|cell_type| {
        Choice::new(
            cell_type.to_string(),
            idleon_jelly::data().cell_name(cell_type),
        )
    }));
    let exp_target = Select::new("jelly-sections-exp-cell-type", exp_choices)
        .label("EXP target")
        .selected(
            planner
                .exp_cell_type
                .map(|cell_type| cell_type.to_string())
                .unwrap_or_else(|| "total".to_string()),
        )
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let cell_type = value.parse::<usize>().ok();
            let entity = exp_entity.clone();
            entity.update(cx, |state, cx| {
                state.set_jelly_plan_exp_cell_type(cell_type, cx)
            });
        });

    let tokens_entity = entity.clone();
    let tokens_input = NumberInput::new(
        "jelly-sections-tokens",
        planner.tokens.unwrap_or(available.max(1)) as f64,
    )
    .label("Sections to unlock")
    .range(1.0, 40.0)
    .step(1.0)
    .size(ControlSize::Sm)
    .on_change(move |value, _window, cx| {
        let tokens = value.clamp(1.0, 40.0).round() as usize;
        let entity = tokens_entity.clone();
        entity.update(cx, |state, cx| state.set_jelly_plan_tokens(tokens, cx));
    });

    let quality_entity = entity.clone();
    let quality = Select::new(
        "jelly-sections-quality",
        [Quality::Fast, Quality::Balanced, Quality::Thorough]
            .map(|quality| Choice::new(quality.id(), quality.label())),
    )
    .label("Quality")
    .selected(planner.quality.id())
    .size(ControlSize::Sm)
    .on_change(move |value, _window, cx| {
        if let Some(quality) = Quality::from_id(value) {
            let entity = quality_entity.clone();
            entity.update(cx, |state, cx| state.set_jelly_plan_quality(quality, cx));
        }
    });

    let beam_entity = entity.clone();
    let beam = NumberInput::new("jelly-sections-beam", planner.beam_width as f64)
        .label("Combination beam")
        .range(2.0, 24.0)
        .step(1.0)
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let beam_width = value.clamp(2.0, 24.0).round() as usize;
            let entity = beam_entity.clone();
            entity.update(cx, |state, cx| state.set_jelly_plan_beam(beam_width, cx));
        });

    let seed_entity = entity.clone();
    let seed = NumberInput::new("jelly-sections-seed", f64::from(planner.seed))
        .label("Seed")
        .range(0.0, f64::from(u32::MAX))
        .step(1.0)
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let seed = if value.is_finite() {
                value.clamp(0.0, f64::from(u32::MAX)) as u32
            } else {
                1
            };
            let entity = seed_entity.clone();
            entity.update(cx, |state, cx| state.set_jelly_plan_seed(seed, cx));
        });

    let run_entity = entity;
    let run = Button::new("jelly-sections-run", "Rank sections")
        .variant(ButtonVariant::Primary)
        .size(ControlSize::Sm)
        .icon(IconName::Play)
        .disabled(running)
        .on_click(move |_event, _window, cx| {
            let run_entity = run_entity.clone();
            run_entity.update(cx, |state, cx| state.run_jelly_plan(cx));
        });

    let mut row = div()
        .id("jelly-sections-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(strategy)
        .child(objective);
    if planner.objective == Objective::Exp {
        row = row.child(exp_target);
    }
    row.child(tokens_input)
        .child(quality)
        .child(beam)
        .child(seed)
        .child(run)
        .into_any_element()
}

/// The plan: the recommendation, the two boards, the operation comparison and
/// both tables.
fn result(state: &AppState, cx: &mut Context<AppState>, jelly: &Jelly, plan: &Plan) -> AnyElement {
    let Some(recommended) = plan.recommended.as_ref() else {
        return div()
            .id("jelly-sections-result")
            .flex()
            .flex_col()
            .gap_3()
            .child(Card::new().child(div().p_3().child(Caption::new(
                "No section was ranked: the plan asked for no tokens.",
            ))))
            .child(individual_sections(&plan.plots))
            .into_any_element();
    };
    let sections = sequence_label(&recommended.plot_ids);
    let chance = recommended
        .chance_beats_baseline
        .map(format::chance)
        .unwrap_or_else(|| "Adaptive shared-seed comparison".to_string());
    let cards = div()
        .id("jelly-sections-summary")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(text_kpi(
            "jelly-sections-recommended",
            "Recommended sections",
            sections.clone(),
            format!(
                "{} newly usable slots",
                format::count(recommended.new_usable_slots as u64)
            ),
            cx,
        ))
        .child(text_kpi(
            "jelly-sections-gain",
            "Objective gain",
            signed(recommended.objective_gain),
            format!("{chance} chance to beat the baseline"),
            cx,
        ))
        .child(
            KpiCard::new(
                Statistic::new(
                    "jelly-sections-proximity",
                    "Proximity slots",
                    recommended.proximity_slots as f64,
                )
                .decimals(0),
            )
            .caption(footprint_summary(recommended)),
        )
        .child(text_kpi(
            "jelly-sections-quality-card",
            "Quality",
            plan.quality.label().to_string(),
            format!(
                "{} section sequences evaluated · beam {}",
                format::count(plan.sequences.len() as u64),
                plan.beam_width
            ),
            cx,
        ));
    let steps = div()
        .id("jelly-sections-steps")
        .flex()
        .flex_wrap()
        .gap_2()
        .children(
            recommended
                .plot_ids
                .iter()
                .enumerate()
                .map(|(index, plot_id)| {
                    let step = format!("{}. Section {}", index + 1, plot_id + 1);
                    div()
                        .id(("jelly-section-step", index))
                        .px_2()
                        .py_1()
                        .rounded_2xl()
                        .border_1()
                        .border_color(cx.theme().colors.border)
                        .child(step)
                        .into_any_element()
                }),
        );

    let baseline_unlocked = board::unlocked_slots(jelly);
    let expanded = planner::save_with_plots(jelly, &recommended.plot_ids);
    let final_unlocked = board::unlocked_slots(&expanded);
    let baseline_board = board_panel(
        state,
        cx,
        "Current board",
        grid::BoardView {
            prefix: "jelly-sections-baseline",
            layout: &plan.baseline_layout,
            unlocked: &baseline_unlocked,
            highlight: &[],
            obstruction: plan.baseline_obstruction,
            selected: None,
            action: grid::BoardAction::ReadOnly,
        },
    );
    let final_board = board_panel(
        state,
        cx,
        "Recommended sections",
        grid::BoardView {
            prefix: "jelly-sections-final",
            layout: &recommended.layout,
            unlocked: &final_unlocked,
            highlight: &recommended.new_slots,
            obstruction: plan.baseline_obstruction,
            selected: None,
            action: grid::BoardAction::ReadOnly,
        },
    );

    let mut body = div()
        .id("jelly-sections-result")
        .flex()
        .flex_col()
        .gap_3()
        .child(cards)
        .child(steps)
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .child(baseline_board)
                .child(final_board),
        );
    if let Some(comparison) = comparison(plan, cx) {
        body = body.child(comparison);
    }
    body.child(sequences_table(&plan.sequences))
        .child(individual_sections(&plan.plots))
        .into_any_element()
}

fn board_panel(
    state: &AppState,
    cx: &mut Context<AppState>,
    title: &'static str,
    view: grid::BoardView<'_>,
) -> AnyElement {
    Card::new()
        .header(CardHeader::new(title))
        .child(div().p_3().child(grid::board(state, view, cx)))
        .into_any_element()
}

/// The baseline and final operations side by side.
fn comparison(plan: &Plan, cx: &mut Context<AppState>) -> Option<AnyElement> {
    let baseline = plan.baseline_operation.as_ref()?;
    let final_operation = plan.final_operation.as_ref()?;
    let delta_pct = |before: f64, after: f64| format!("{:+.3} pp", 100.0 * (after - before));
    Some(
        div()
            .id("jelly-sections-comparison")
            .flex()
            .flex_wrap()
            .gap_3()
            .child(text_kpi(
                "jelly-sections-clear",
                "Clear probability",
                format!(
                    "{} → {}",
                    format::percent(baseline.clear_probability),
                    format::percent(final_operation.clear_probability)
                ),
                delta_pct(
                    baseline.clear_probability,
                    final_operation.clear_probability,
                ),
                cx,
            ))
            .child(text_kpi(
                "jelly-sections-damage",
                "Damage",
                format!(
                    "{} → {}",
                    format::value(baseline.mean_damage),
                    format::value(final_operation.mean_damage)
                ),
                signed_value(final_operation.mean_damage - baseline.mean_damage),
                cx,
            ))
            .child(text_kpi(
                "jelly-sections-bloodcells",
                "Bloodcells",
                format!(
                    "{} → {}",
                    format::value(baseline.mean_bloodcells),
                    format::value(final_operation.mean_bloodcells)
                ),
                signed_value(final_operation.mean_bloodcells - baseline.mean_bloodcells),
                cx,
            ))
            .child(text_kpi(
                "jelly-sections-exp",
                format!("{} / attempt", exp_label(plan.exp_cell_type)),
                format!(
                    "{} → {}",
                    format::value(exp_value(baseline, plan.exp_cell_type)),
                    format::value(exp_value(final_operation, plan.exp_cell_type))
                ),
                signed_value(
                    exp_value(final_operation, plan.exp_cell_type)
                        - exp_value(baseline, plan.exp_cell_type),
                ),
                cx,
            ))
            .into_any_element(),
    )
}

fn sequences_table(rows: &[SectionRow]) -> AnyElement {
    let mut table = Table::new([
        "#",
        "Section sequence",
        "Slots",
        "Proximity",
        "Gain",
        "Gain / slot",
        "Beats baseline",
        "New footprint options",
    ]);
    for (rank, row) in rows.iter().enumerate() {
        table = table.row([
            (rank + 1).to_string(),
            sequence_label(&row.plot_ids),
            row.new_usable_slots.to_string(),
            row.proximity_slots.to_string(),
            signed(row.objective_gain),
            signed(row.gain_per_slot),
            row.chance_beats_baseline
                .map(format::chance)
                .unwrap_or_else(|| "—".to_string()),
            footprint_summary(row),
        ]);
    }
    Card::new()
        .header(CardHeader::new("Ranked section sequences").description(
            "Every finalist with its shared-seed comparison against the current board.",
        ))
        .child(div().id("jelly-sections-candidates").p_3().child(table))
        .into_any_element()
}

fn individual_sections(rows: &[SectionRow]) -> AnyElement {
    let mut table = Table::new([
        "Section",
        "Slots",
        "Proximity",
        "Proxy gain",
        "Simulated gain",
        "Beats baseline",
        "New footprint options",
    ]);
    for row in rows {
        table = table.row([
            row.plot_id
                .map(|plot| (plot + 1).to_string())
                .unwrap_or_else(|| "—".to_string()),
            row.new_usable_slots.to_string(),
            row.proximity_slots.to_string(),
            signed(row.proxy_gain),
            if row.operation.is_some() {
                signed(row.objective_gain)
            } else {
                "—".to_string()
            },
            row.chance_beats_baseline
                .map(format::chance)
                .unwrap_or_else(|| "—".to_string()),
            footprint_summary(row),
        ]);
    }
    Card::new()
        .header(CardHeader::new("Every individual section").description(
            "Each unpurchased section's completed board, ranked by the plan's objective.",
        ))
        .child(div().id("jelly-sections-individual").p_3().child(table))
        .into_any_element()
}

fn exp_label(cell_type: Option<usize>) -> String {
    cell_type
        .map(|cell_type| idleon_jelly::data().cell_name(cell_type).to_string())
        .unwrap_or_else(|| "Total EXP".to_string())
}

fn exp_value(estimate: &Estimate, cell_type: Option<usize>) -> f64 {
    match cell_type {
        Some(cell_type) => estimate
            .mean_cell_exp_by_type
            .get(cell_type)
            .copied()
            .unwrap_or(0.0),
        None => estimate.mean_cell_exp,
    }
}

fn sequence_label(plot_ids: &[usize]) -> String {
    if plot_ids.is_empty() {
        return "None".to_string();
    }
    plot_ids
        .iter()
        .map(|plot| format!("Section {}", plot + 1))
        .collect::<Vec<_>>()
        .join(" → ")
}

fn footprint_summary(row: &SectionRow) -> String {
    let places: Vec<String> = row
        .newly_legal_footprint_placements
        .iter()
        .filter(|place| place.count > 0)
        .map(|place| format!("{} +{}", place.name, place.count))
        .collect();
    if places.is_empty() {
        "No new large-cell placements".to_string()
    } else {
        places.join(", ")
    }
}

fn signed(value: f64) -> String {
    if !value.is_finite() {
        return "—".to_string();
    }
    if value.abs() >= 1e3 {
        signed_value(value)
    } else if value != 0.0 && value.abs() < 1e-4 {
        // A clear-probability gain can be smaller than four decimals.
        format!("{value:+.2e}")
    } else if value != 0.0 && value.abs() < 0.01 {
        format!("{value:+.6}")
    } else {
        format!("{value:+.4}")
    }
}

fn signed_value(value: f64) -> String {
    if !value.is_finite() {
        return "—".to_string();
    }
    if value >= 0.0 {
        format!("+{}", format::value(value))
    } else {
        format!("-{}", format::value(-value))
    }
}
