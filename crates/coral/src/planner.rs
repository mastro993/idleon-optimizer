//! The bounded deterministic beam search behind the planner and its projection.
//!
//! Ports `optimizeCoralGodRank` from
//! `js/stats/systems/w5/coral-god-rank.js`. The search starts from the save's
//! Coral Kid levels, expands every unlocked upgrade and one God Rank offering
//! per state, keeps only candidates that strictly improve the goal, dedupes the
//! states that share a signature, sorts by the goal and keeps the best beam.
//! Every choice is a total order, so the same inputs always produce the plan.

use super::{CORAL_KID_UNLOCK_LEVELS, Goal, Inputs, Metrics, offer, upgrade_cost};
use idleon_common::Progress;

/// The reasons an upgrade carries when it is not simply unlocked.
const UNLOCK_ASSUMED: &str = "Assumed unlocked";
const UNLOCK_UNKNOWN: &str = "Upgrade unlock is unknown";
const UNLOCK_REQUIREMENT: &str = "Divinity level requirement not met";

/// One projection row: a metric before and after the plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projection {
    pub key: ProjectionKey,
    pub current: f64,
    pub projected: f64,
}

impl Projection {
    /// The absolute change, as Blessing Cap reads it.
    pub fn delta(self) -> f64 {
        self.projected - self.current
    }

    /// The relative change, as the multipliers read it.
    pub fn ratio(self) -> f64 {
        if self.current > 0.0 {
            self.projected / self.current - 1.0
        } else {
            0.0
        }
    }
}

/// The six metrics the projection table shows, in upstream order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionKey {
    DivinityPoints,
    DivinityExp,
    ClassExp,
    MinorLinks,
    BlessingCap,
    DailyReefCoral,
}

impl ProjectionKey {
    pub const ALL: [Self; 6] = [
        Self::DivinityPoints,
        Self::DivinityExp,
        Self::ClassExp,
        Self::MinorLinks,
        Self::BlessingCap,
        Self::DailyReefCoral,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::DivinityPoints => "Divinity Points",
            Self::DivinityExp => "Divinity EXP",
            Self::ClassExp => "Class EXP",
            Self::MinorLinks => "Minor Links",
            Self::BlessingCap => "Blessing Cap",
            Self::DailyReefCoral => "Daily Reef Coral",
        }
    }

    /// Whether the metric is a multiplier rather than a count.
    pub fn is_multiplier(self) -> bool {
        !matches!(self, Self::BlessingCap)
    }

    fn of(self, metrics: &Metrics) -> f64 {
        match self {
            Self::DivinityPoints => metrics.divinity_points,
            Self::DivinityExp => metrics.divinity_exp,
            Self::ClassExp => metrics.class_exp,
            Self::MinorLinks => metrics.minor_links,
            Self::BlessingCap => metrics.blessing_cap,
            Self::DailyReefCoral => metrics.daily_reef_coral,
        }
    }
}

/// Every metric before and after a plan.
pub fn project(current: &Metrics, projected: &Metrics) -> [Projection; 6] {
    std::array::from_fn(|index| {
        let key = ProjectionKey::ALL[index];
        Projection {
            key,
            current: key.of(current),
            projected: key.of(projected),
        }
    })
}

/// One Coral Kid level state and its God Rank, with the plan that reached it.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub levels: [u32; 6],
    pub god_rank: u32,
    pub metrics: Metrics,
}

/// One action the best plan takes.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Buy one level of Coral Kid upgrade `index`.
    CoralKid { index: usize, level: u32, cost: f64 },
    /// Offer for one God Rank, budgeting the attempts a confidence needs.
    GodOffering {
        from_rank: u32,
        to_rank: u32,
        attempts: u32,
        expected_spend: f64,
        max_paid_spend: f64,
        confidence: f64,
        whole_plan_confidence: f64,
    },
}

/// Whether one upgrade is unlocked, and whether the inputs know that or assume it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unlock {
    pub index: usize,
    pub unlocked: bool,
    pub known: bool,
    pub reason: Option<&'static str>,
}

/// The best plan the search found, with its projection.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub goal: Goal,
    pub current: Snapshot,
    pub best: Snapshot,
    pub actions: Vec<Action>,
    pub budget: f64,
    /// The worst case the plan commits: paid offerings and every level bought.
    pub spent: f64,
    pub planning_confidence: f64,
    pub whole_plan_confidence: f64,
    pub dominance_pruned: usize,
    /// Inputs the plan had to assume, named for the UI.
    pub partial: Vec<String>,
    pub unlocks: [Unlock; 6],
}

impl Plan {
    /// What the plan leaves unspent.
    pub fn remaining(&self) -> f64 {
        self.budget - self.spent
    }

    /// Every metric before and after the plan.
    pub fn projection(&self) -> [Projection; 6] {
        project(&self.current.metrics, &self.best.metrics)
    }
}

/// The upgrade unlock rows, matching `_upgradeStatus`.
pub fn unlocks(inputs: &Inputs) -> [Unlock; 6] {
    std::array::from_fn(|index| {
        let required = CORAL_KID_UNLOCK_LEVELS[index];
        let total = inputs.total_divinity_levels;
        let (unlocked, known, reason) = if inputs.assume_unlocked {
            (true, total.is_some(), None)
        } else {
            match total {
                Some(total) => {
                    let unlocked = f64::from(total) >= required;
                    (unlocked, true, (!unlocked).then_some(UNLOCK_REQUIREMENT))
                }
                None => (false, false, Some(UNLOCK_UNKNOWN)),
            }
        };
        Unlock {
            index,
            unlocked,
            known,
            reason: reason.or_else(|| (!known).then_some(UNLOCK_ASSUMED)),
        }
    })
}

/// One search state: the levels, the rank, the worst-case spend so far, the
/// plan's confidence and the actions that reached it.
#[derive(Clone, Debug)]
struct State {
    levels: [u32; 6],
    god_rank: u32,
    spent: f64,
    confidence: f64,
    actions: Vec<Action>,
    signature: String,
}

impl State {
    fn new(
        levels: [u32; 6],
        god_rank: u32,
        spent: f64,
        confidence: f64,
        actions: Vec<Action>,
    ) -> Self {
        Self {
            levels,
            god_rank,
            spent,
            confidence,
            actions,
            signature: signature(&levels, god_rank),
        }
    }
}

/// `_stateSignature`: the levels and the rank, as the dedupe key.
fn signature(levels: &[u32; 6], god_rank: u32) -> String {
    let levels: Vec<String> = levels.iter().map(u32::to_string).collect();
    format!("{}|{god_rank}", levels.join(","))
}

/// `_strictlyImproves`: an improvement beyond rounding noise counts.
fn strictly_improves(next: f64, prior: f64) -> bool {
    let epsilon = 1e-12 * 1.0f64.max(next.abs()).max(prior.abs());
    next > prior + epsilon
}

/// `_objective`: the goal's score for a state, relative to the current metrics
/// when the goal is balanced.
fn objective(goal: Goal, metrics: &Metrics, baseline: &Metrics) -> f64 {
    if goal != Goal::Balanced {
        return match goal {
            Goal::DivinityPoints => metrics.divinity_points,
            Goal::DivinityExp => metrics.divinity_exp,
            Goal::ClassExp => metrics.class_exp,
            Goal::MinorLinks => metrics.minor_links,
            Goal::BlessingCap => metrics.blessing_cap,
            Goal::DailyReefCoral => metrics.daily_reef_coral,
            Goal::Balanced => unreachable!("handled above"),
        };
    }
    let total: f64 = ProjectionKey::ALL
        .into_iter()
        .map(|key| {
            let value = key.of(metrics);
            let prior = key.of(baseline);
            if prior > 0.0 {
                value / prior
            } else if value > 0.0 {
                1.0
            } else {
                0.0
            }
        })
        .sum();
    total / ProjectionKey::ALL.len() as f64
}

/// `coralStateDominates`: one state is as good and no more expensive.
fn dominates(a: &State, b: &State, score_a: f64, score_b: f64) -> bool {
    if a.signature != b.signature {
        return false;
    }
    let no_worse = score_a >= score_b && a.spent <= b.spent && a.confidence >= b.confidence;
    no_worse && (score_a > score_b || a.spent < b.spent || a.confidence > b.confidence)
}

/// Run the beam search without progress reports.
pub fn plan(inputs: &Inputs) -> Plan {
    plan_with(inputs, |_| true).expect("a reporter that always continues never stops the plan")
}

/// Run the beam search, reporting each step; `report` returns `false` to stop.
///
/// The search is deterministic: no randomness, no map iteration order, and
/// every tie is broken by a total order, so the same inputs always give the
/// same plan.
pub fn plan_with(inputs: &Inputs, mut report: impl FnMut(Progress) -> bool) -> Option<Plan> {
    let budget = inputs.budget.max(0.0);
    let max_levels = inputs.max_levels as usize;
    let beam_size = inputs.beam_size.max(1) as usize;
    let baseline = inputs.metrics();
    let initial_rank = inputs.god_rank;
    let current = Snapshot {
        levels: inputs.levels,
        god_rank: initial_rank,
        metrics: baseline.clone(),
    };
    let mut beam = vec![State::new(
        inputs.levels,
        initial_rank,
        0.0,
        1.0,
        Vec::new(),
    )];
    let mut dominance_pruned = 0;

    for step in 0..max_levels {
        let mut next: Vec<State> = beam.clone();
        for state in &beam {
            if state.actions.len() >= max_levels {
                continue;
            }
            for index in 0..6 {
                if !inputs.upgrade_unlocked(index) {
                    continue;
                }
                let cost = upgrade_cost(index, f64::from(state.levels[index]));
                if state.spent + cost > budget {
                    continue;
                }
                let mut levels = state.levels;
                levels[index] += 1;
                let candidate = inputs.metrics_at(&levels, state.god_rank);
                let prior = inputs.metrics_at(&state.levels, state.god_rank);
                if !strictly_improves(
                    objective(inputs.goal, &candidate, &baseline),
                    objective(inputs.goal, &prior, &baseline),
                ) {
                    continue;
                }
                let mut actions = state.actions.clone();
                actions.push(Action::CoralKid {
                    index,
                    level: levels[index],
                    cost,
                });
                next.push(State::new(
                    levels,
                    state.god_rank,
                    state.spent + cost,
                    state.confidence,
                    actions,
                ));
            }
            if let Some(state_next) = offering_step(inputs, state, &baseline, budget) {
                next.push(state_next);
            }
        }

        next = dedupe(next, inputs, &baseline, &mut dominance_pruned);
        next.sort_by(|a, b| {
            let score_a = objective(
                inputs.goal,
                &inputs.metrics_at(&a.levels, a.god_rank),
                &baseline,
            );
            let score_b = objective(
                inputs.goal,
                &inputs.metrics_at(&b.levels, b.god_rank),
                &baseline,
            );
            score_b
                .total_cmp(&score_a)
                .then_with(|| a.spent.total_cmp(&b.spent))
                .then_with(|| a.signature.cmp(&b.signature))
        });
        next.truncate(beam_size);
        beam = next;
        let progress = Progress {
            fraction: (step + 1) as f32 / max_levels.max(1) as f32,
            message: format!("Step {} of {max_levels}", step + 1),
        };
        if !report(progress) {
            return None;
        }
    }

    let best = beam
        .into_iter()
        .next()
        .expect("the beam always holds at least the starting state");
    let best_metrics = inputs.metrics_at(&best.levels, best.god_rank);
    let mut partial = Vec::new();
    if inputs.total_divinity_levels.is_none() {
        partial.push("Coral Kid unlock state".into());
    }
    Some(Plan {
        goal: inputs.goal,
        current,
        best: Snapshot {
            levels: best.levels,
            god_rank: best.god_rank,
            metrics: best_metrics,
        },
        actions: best.actions,
        budget,
        spent: best.spent,
        planning_confidence: inputs.planning_confidence,
        whole_plan_confidence: best.confidence,
        dominance_pruned,
        partial,
        unlocks: unlocks(inputs),
    })
}

/// The offering branch: one rank up, when it improves the goal and fits.
fn offering_step(inputs: &Inputs, state: &State, baseline: &Metrics, budget: f64) -> Option<State> {
    let row = offer(inputs, state.god_rank);
    if row.attempts == 0 || state.god_rank >= 100 {
        return None;
    }
    if state.spent + row.max_paid_spend > budget {
        return None;
    }
    let rank_metrics = inputs.metrics_at(&state.levels, state.god_rank + 1);
    let state_metrics = inputs.metrics_at(&state.levels, state.god_rank);
    if !strictly_improves(
        objective(inputs.goal, &rank_metrics, baseline),
        objective(inputs.goal, &state_metrics, baseline),
    ) {
        return None;
    }
    let confidence = row.confidence;
    let whole_plan_confidence = state.confidence * confidence;
    let mut actions = state.actions.clone();
    actions.push(Action::GodOffering {
        from_rank: state.god_rank,
        to_rank: state.god_rank + 1,
        attempts: row.attempts,
        expected_spend: row.expected_spend,
        max_paid_spend: row.max_paid_spend,
        confidence,
        whole_plan_confidence,
    });
    Some(State::new(
        state.levels,
        state.god_rank + 1,
        state.spent + row.max_paid_spend,
        whole_plan_confidence,
        actions,
    ))
}

/// Collapse the states that share a signature, keeping the dominant one.
fn dedupe(
    states: Vec<State>,
    inputs: &Inputs,
    baseline: &Metrics,
    dominance_pruned: &mut usize,
) -> Vec<State> {
    let mut unique: Vec<State> = Vec::with_capacity(states.len());
    for state in states {
        let Some(prior) = unique
            .iter()
            .position(|prior| prior.signature == state.signature)
        else {
            unique.push(state);
            continue;
        };
        let score = |state: &State| {
            objective(
                inputs.goal,
                &inputs.metrics_at(&state.levels, state.god_rank),
                baseline,
            )
        };
        let state_score = score(&state);
        let prior_score = score(&unique[prior]);
        if dominates(&state, &unique[prior], state_score, prior_score) {
            unique[prior] = state;
            *dominance_pruned += 1;
        } else if dominates(&unique[prior], &state, prior_score, state_score) {
            *dominance_pruned += 1;
        } else if state.spent < unique[prior].spent
            || (state.spent == unique[prior].spent && state.confidence > unique[prior].confidence)
        {
            unique[prior] = state;
        }
    }
    unique
}

/// A report that only counts the steps, for tests.
#[cfg(test)]
pub(crate) fn steps(inputs: &Inputs) -> usize {
    let mut steps = 0;
    let _ = plan_with(inputs, |_| {
        steps += 1;
        true
    });
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> Inputs {
        Inputs {
            budget: 1e9,
            max_levels: 8,
            beam_size: 100,
            god_rank: 31,
            reef_upgrade_count: 10.0,
            planning_confidence: 0.9,
            levels: [10, 10, 10, 10, 10, 10],
            total_divinity_levels: Some(10_000),
            ..Inputs::default()
        }
    }

    #[test]
    fn the_plan_respects_the_budget_and_the_level_cap() {
        let plan = plan(&inputs());
        assert!(plan.spent <= plan.budget, "{plan:?}");
        assert!(plan.actions.len() <= 8);
        let mut committed = 0.0;
        for action in &plan.actions {
            committed += match action {
                Action::CoralKid { cost, .. } => *cost,
                Action::GodOffering { max_paid_spend, .. } => *max_paid_spend,
            };
        }
        assert!((committed - plan.spent).abs() <= plan.spent.max(1.0) * 1e-9);
        assert!(plan.remaining() >= 0.0);
    }

    #[test]
    fn a_zero_budget_or_level_cap_holds_the_current_state() {
        for inputs in [
            Inputs {
                budget: 0.0,
                ..inputs()
            },
            Inputs {
                max_levels: 0,
                ..inputs()
            },
        ] {
            let plan = plan(&inputs);
            assert!(plan.actions.is_empty(), "{:?}", plan.actions);
            assert_eq!(plan.best.levels, inputs.levels);
            assert_eq!(plan.best.god_rank, inputs.god_rank);
        }
    }

    #[test]
    fn a_larger_beam_finds_a_better_plan() {
        let narrow = plan(&Inputs {
            beam_size: 1,
            ..inputs()
        });
        let wide = plan(&inputs());
        let baseline = wide.current.metrics.clone();
        let score = |plan: &Plan| objective(plan.goal, &plan.best.metrics, &baseline);
        assert!(score(&wide) > score(&narrow), "{wide:?} vs {narrow:?}");
        assert_ne!(wide.actions, narrow.actions);
    }

    #[test]
    fn the_same_inputs_give_the_same_plan() {
        let first = plan(&inputs());
        let second = plan(&inputs());
        assert_eq!(first, second);
        assert_eq!(steps(&inputs()), 8);
    }

    #[test]
    fn the_plan_stops_when_the_reporter_asks() {
        let mut reported = 0;
        let result = plan_with(&inputs(), |_| {
            reported += 1;
            reported < 3
        });
        assert_eq!(result, None);
        assert_eq!(reported, 3);
    }

    #[test]
    fn the_projection_reads_current_against_projected() {
        let plan = plan(&Inputs {
            goal: Goal::ClassExp,
            ..inputs()
        });
        let rows = plan.projection();
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0].key, ProjectionKey::DivinityPoints);
        assert_eq!(rows[3].key, ProjectionKey::MinorLinks);
        let class_exp = rows[2];
        assert_eq!(class_exp.current, plan.current.metrics.class_exp);
        assert_eq!(class_exp.projected, plan.best.metrics.class_exp);
        assert!(class_exp.ratio() > 0.0);
        assert!(class_exp.delta() > 0.0);
        assert!(!ProjectionKey::BlessingCap.is_multiplier());
    }

    #[test]
    fn the_unlock_rows_say_what_the_inputs_know() {
        let assumed = unlocks(&Inputs {
            assume_unlocked: true,
            total_divinity_levels: None,
            ..inputs()
        });
        assert!(assumed.iter().all(|unlock| unlock.unlocked));
        assert!(assumed.iter().all(|unlock| !unlock.known));
        assert_eq!(assumed[0].reason, Some(UNLOCK_ASSUMED));

        let known = unlocks(&Inputs {
            assume_unlocked: false,
            total_divinity_levels: Some(1600),
            ..inputs()
        });
        assert!(known[4].unlocked);
        assert!(!known[5].unlocked);
        assert_eq!(known[5].reason, Some(UNLOCK_REQUIREMENT));

        let plan = plan(&Inputs {
            assume_unlocked: true,
            total_divinity_levels: None,
            ..inputs()
        });
        assert_eq!(plan.partial, ["Coral Kid unlock state"]);
    }
}
