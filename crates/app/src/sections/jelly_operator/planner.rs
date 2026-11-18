//! JO-04: the Sections tab, the board section planner.
//!
//! The strategy, objective, EXP target, token, quality and beam controls, the
//! ranked section sequences, the two boards and every individual section. The
//! planner itself lives in `idleon_jelly::planner`; this module owns the
//! controls and renders the plan.

use super::controls::{self, SequenceTable};
use super::grid;
use super::parts::{
    SELECT_WIDTH, boxed, figure, info, number_field, run_button, select, strips, text_kpi,
    text_table,
};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_kit::component::table::DataTable;
use gpui_kit::component::tag::Tag;
use idleon_jelly::board::Layout;
use idleon_jelly::planner::{Objective, Plan, PlanOptions, Quality, SectionRow, Strategy};
use idleon_jelly::{Estimate, Jelly, board};
use idleon_ui::format;
use idleon_ui::run_chrome;
use idleon_ui::widgets::{Caption, Card, CardHeader, PANEL_GAP, Tone};

/// The planner controls. The token count starts
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

    /// Pick a strategy; the objective also moves to what it plays for.
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
    /// The controls as the section planner reads them.
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
    window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
) -> AnyElement {
    let task = state.jelly_plan_task(cx);
    let running = task.running;
    let available = board::slot_purchases_left(jelly).max(0.0) as usize;
    let controls = controls(state, window, cx, jelly, available, running);
    let mut body = div()
        .id("jelly-sections")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(
            Card::new()
                .header(
                    CardHeader::new("Board section planner").description(
                        "Ranks every unpurchased section, then simulates the strongest geometry and multi-section combinations.",
                    ),
                )
                .child(controls),
        )
        .child(div().id("jelly-sections-tokens-note").child(Caption::new(format!(
            "{available} unused section token{} in the current save. Values above that are hypothetical.",
            if available == 1 { "" } else { "s" }
        ))));
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
            body = body.child(info(
                "jelly-sections-empty",
                "Rank the sections to see which token buys the most board, and how its operation compares with the current one.",
            ));
        }
        return body.into_any_element();
    };
    if task.stale {
        body = body.child(run_chrome::stale_banner("jelly-sections-stale"));
    }
    body.child(result(state, cx, &plan)).into_any_element()
}

/// Strategy, objective, EXP target, token count, quality, beam and seed.
fn controls(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
    available: usize,
    running: bool,
) -> AnyElement {
    let planner = &state.jelly_state.planner;
    let controls = &state.jelly_controls;
    controls.plan_strategy.show(
        controls::strategy_choices(),
        Some(&planner.strategy),
        window,
        cx,
    );
    controls.plan_objective.show(
        controls::objective_choices(jelly),
        Some(&planner.objective),
        window,
        cx,
    );
    controls.plan_quality.show(
        controls::quality_choices(),
        Some(&planner.quality),
        window,
        cx,
    );
    let mut row = div()
        .id("jelly-sections-controls")
        .flex()
        .flex_wrap()
        .items_center()
        .gap(PANEL_GAP)
        .child(boxed(
            SELECT_WIDTH,
            select(
                &controls.plan_strategy,
                "jelly-sections-strategy",
                "Strategy",
            ),
        ))
        .child(boxed(
            SELECT_WIDTH,
            select(
                &controls.plan_objective,
                "jelly-sections-objective",
                "Objective",
            ),
        ));
    if planner.objective == Objective::Exp {
        controls.plan_exp.show(
            controls::exp_choices(jelly),
            Some(&planner.exp_cell_type),
            window,
            cx,
        );
        row = row.child(boxed(
            SELECT_WIDTH,
            select(
                &controls.plan_exp,
                "jelly-sections-exp-cell-type",
                "EXP target",
            ),
        ));
    }
    row.child(number_field(
        "jelly-sections-tokens",
        "Sections to unlock",
        &controls.plan_tokens,
        planner.tokens.unwrap_or(available.max(1)) as f64,
        window,
        cx,
    ))
    .child(boxed(
        SELECT_WIDTH,
        select(&controls.plan_quality, "jelly-sections-quality", "Quality"),
    ))
    .child(number_field(
        "jelly-sections-beam",
        "Combination beam",
        &controls.plan_beam,
        planner.beam_width as f64,
        window,
        cx,
    ))
    .child(number_field(
        "jelly-sections-seed",
        "Seed",
        &controls.plan_seed,
        f64::from(planner.seed),
        window,
        cx,
    ))
    .child(run_button(
        "jelly-sections-run",
        "Rank sections",
        running,
        |state, cx| state.run_jelly_plan(cx),
        cx,
    ))
    .into_any_element()
}

/// The plan: the recommendation, the two boards, the operation comparison and
/// both tables.
fn result(state: &AppState, cx: &mut Context<AppState>, plan: &Plan) -> AnyElement {
    let Some(recommended) = plan.recommended.as_ref() else {
        return div()
            .id("jelly-sections-result")
            .flex()
            .flex_col()
            .gap(PANEL_GAP)
            .child(Card::new().child(Caption::new(
                "No section was ranked: the plan asked for no tokens.",
            )))
            .child(individual_sections(&plan.plots))
            .into_any_element();
    };
    let chance = recommended
        .chance_beats_baseline
        .map(format::chance)
        .unwrap_or_else(|| "Adaptive shared-seed comparison".to_string());
    let cards = strips(
        "jelly-sections-summary",
        vec![
            text_kpi(
                "jelly-sections-recommended",
                "Recommended sections",
                sequence_label(&recommended.plot_ids),
                format!(
                    "{} newly usable slots",
                    format::count(recommended.new_usable_slots as u64)
                ),
            ),
            text_kpi(
                "jelly-sections-gain",
                "Objective gain",
                signed(recommended.objective_gain),
                format!("{chance} chance to beat the baseline"),
            ),
            figure(
                "jelly-sections-proximity",
                "Proximity slots",
                recommended.proximity_slots as f64,
                0,
            )
            .caption(footprint_summary(recommended)),
            text_kpi(
                "jelly-sections-quality-card",
                "Quality",
                plan.quality.label().to_string(),
                format!(
                    "{} section sequences evaluated · beam {}",
                    format::count(plan.sequences.len() as u64),
                    plan.beam_width
                ),
            ),
        ],
    );
    let steps = div()
        .id("jelly-sections-steps")
        .flex()
        .flex_wrap()
        .gap(PANEL_GAP)
        .children(
            recommended
                .plot_ids
                .iter()
                .enumerate()
                .map(|(index, plot_id)| {
                    div().id(("jelly-section-step", index)).child(
                        Tag::new()
                            .with_variant(Tone::Neutral.into())
                            .outline()
                            .child(format!("{}. Section {}", index + 1, plot_id + 1)),
                    )
                }),
        );

    let baseline_board = grid::panel(
        state,
        "Current board",
        grid::BoardView {
            prefix: "jelly-sections-baseline",
            layout: &plan.baseline_layout,
            unlocked: &state.jelly_state.save.unlocked,
            highlight: &[],
            obstruction: plan.baseline_obstruction,
            selected: None,
            action: grid::BoardAction::ReadOnly,
        },
        cx,
    );
    let final_board = grid::panel(
        state,
        "Recommended sections",
        grid::BoardView {
            prefix: "jelly-sections-final",
            layout: &recommended.layout,
            unlocked: &state.jelly_state.results.plan_unlocked,
            highlight: &recommended.new_slots,
            obstruction: plan.baseline_obstruction,
            selected: None,
            action: grid::BoardAction::ReadOnly,
        },
        cx,
    );

    div()
        .id("jelly-sections-result")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(cards)
        .child(steps)
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(PANEL_GAP)
                .child(baseline_board)
                .child(final_board),
        )
        .children(comparison(plan))
        .child(sequences_table(&state.jelly_controls.sequences))
        .child(individual_sections(&plan.plots))
        .into_any_element()
}

/// The baseline and final operations side by side.
fn comparison(plan: &Plan) -> Option<AnyElement> {
    let baseline = plan.baseline_operation.as_ref()?;
    let final_operation = plan.final_operation.as_ref()?;
    let delta_pct = |before: f64, after: f64| format!("{:+.3} pp", 100.0 * (after - before));
    let exp_before = exp_value(baseline, plan.exp_cell_type);
    let exp_after = exp_value(final_operation, plan.exp_cell_type);
    Some(strips(
        "jelly-sections-comparison",
        vec![
            text_kpi(
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
            ),
            text_kpi(
                "jelly-sections-damage",
                "Damage",
                format!(
                    "{} → {}",
                    format::value(baseline.mean_damage),
                    format::value(final_operation.mean_damage)
                ),
                signed_value(final_operation.mean_damage - baseline.mean_damage),
            ),
            text_kpi(
                "jelly-sections-bloodcells",
                "Bloodcells",
                format!(
                    "{} → {}",
                    format::value(baseline.mean_bloodcells),
                    format::value(final_operation.mean_bloodcells)
                ),
                signed_value(final_operation.mean_bloodcells - baseline.mean_bloodcells),
            ),
            text_kpi(
                "jelly-sections-exp",
                format!("{} / attempt", exp_label(plan.exp_cell_type)),
                format!(
                    "{} → {}",
                    format::value(exp_before),
                    format::value(exp_after)
                ),
                signed_value(exp_after - exp_before),
            ),
        ],
    ))
}

/// The ranked sequences: as many as the plan screened, so a virtualized table
/// whose rows were built when the plan was stored.
fn sequences_table(table: &SequenceTable) -> AnyElement {
    Card::new()
        .header(CardHeader::new("Ranked section sequences").description(
            "Every finalist with its shared-seed comparison against the current board.",
        ))
        .child(
            div()
                .id("jelly-sections-candidates")
                .h(px(360.0))
                .child(DataTable::new(table)),
        )
        .into_any_element()
}

/// One row per unpurchased section: at most the board's 40.
fn individual_sections(rows: &[SectionRow]) -> AnyElement {
    let dash = || "—".to_string();
    let table = text_table(
        &[
            ("Section", true),
            ("Slots", true),
            ("Proximity", true),
            ("Proxy gain", true),
            ("Simulated gain", true),
            ("Beats baseline", true),
            ("New footprint options", false),
        ],
        rows.iter().map(|row| {
            vec![
                row.plot_id.map_or_else(dash, |plot| (plot + 1).to_string()),
                row.new_usable_slots.to_string(),
                row.proximity_slots.to_string(),
                signed(row.proxy_gain),
                if row.operation.is_some() {
                    signed(row.objective_gain)
                } else {
                    dash()
                },
                row.chance_beats_baseline.map_or_else(dash, format::chance),
                footprint_summary(row),
            ]
        }),
    );
    Card::new()
        .header(CardHeader::new("Every individual section").description(
            "Each unpurchased section's completed board, ranked by the plan's objective.",
        ))
        .child(div().id("jelly-sections-individual").child(table))
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

pub(crate) fn sequence_label(plot_ids: &[usize]) -> String {
    if plot_ids.is_empty() {
        return "None".to_string();
    }
    plot_ids
        .iter()
        .map(|plot| format!("Section {}", plot + 1))
        .collect::<Vec<_>>()
        .join(" → ")
}

pub(crate) fn footprint_summary(row: &SectionRow) -> String {
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

pub(crate) fn signed(value: f64) -> String {
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

pub(crate) fn signed_value(value: f64) -> String {
    if !value.is_finite() {
        return "—".to_string();
    }
    if value >= 0.0 {
        format!("+{}", format::value(value))
    } else {
        format!("-{}", format::value(-value))
    }
}
