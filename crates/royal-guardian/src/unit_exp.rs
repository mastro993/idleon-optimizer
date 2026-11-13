//! The unit EXP planner: which traders and surveyors the outpost should run, and
//! when to switch, to reach the rank targets a player sets.
//!
//! Ported from `planRoyalUnitExp`, `_model`, `_rates`, `_run`, `_optimize`,
//! `_summary`, `_retarget`, `_transientPools` and `projectRoyalUnitExp` in
//! `js/stats/systems/w7/royal-unit-exp.js`.
//!
//! Trading EXP comes from the traders and Intel EXP from the surveyors; Command,
//! Military and Purity EXP come from the Commander, Knight and Priest
//! assignments. Only traders and surveyors earn EXP themselves, so the plan is a
//! split: how many units trade until some Intel rank, and how many trade after
//! it. The search tries every legal split and every switch point, keeps the
//! fastest, and replays it into steps a player takes. Each step's profession
//! changes come from the saved packed unit row, so a step only ever retargets a
//! slot the outpost already fields.
//!
//! Upstream also moves the same-world Commander, Knight and Militia assignments
//! onto the target outpost when a target needs them; the port keeps that pass.
//! Priests stay put: the port sets no Purity target.

use super::*;
use crate::sandbox::{rank_name, transient_name};
use idleon_common::{Control, Progress};
use std::collections::BTreeMap;

/// The longest projection the planner runs before it gives up.
pub const MAX_HOURS: f64 = 87_600.0;
/// The most rank events one run simulates.
pub const MAX_EVENTS: usize = 50_000;
/// The assignments the planner may move: Militia, Commander and Knight.
const TRANSIENT_TYPES: [i64; 3] = [4, 5, 6];
/// The largest combination count the planner enumerates instead of walking.
const JOINT_LIMIT: usize = 400;
/// How many switch ranks the search tries past the saved Intel rank.
const SWITCH_SCAN: usize = 1_000;
const EPSILON: f64 = 1e-9;

/// The three rank bars a player can set a target on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Trading,
    Command,
    Military,
}

impl Target {
    /// The three targets, in the order the controls list them.
    pub const ALL: [Target; 3] = [Target::Trading, Target::Command, Target::Military];

    /// The bar the target sits on.
    pub fn bar(self) -> usize {
        match self {
            Target::Trading => 0,
            Target::Command => 2,
            Target::Military => 3,
        }
    }

    /// The word the controls print.
    pub fn label(self) -> &'static str {
        rank_name(self.bar())
    }

    /// The stable id the controls key by.
    pub fn id(self) -> &'static str {
        match self {
            Target::Trading => "trading",
            Target::Command => "command",
            Target::Military => "military",
        }
    }

    /// The target an id names.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|target| target.id() == id)
    }
}

/// The rank each bar should reach, when one is set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UnitTargets {
    pub trading: Option<u32>,
    pub command: Option<u32>,
    pub military: Option<u32>,
}

impl UnitTargets {
    /// The target a bar reads.
    pub fn get(&self, target: Target) -> Option<u32> {
        match target {
            Target::Trading => self.trading,
            Target::Command => self.command,
            Target::Military => self.military,
        }
    }

    /// Whether any target is set.
    pub fn any(&self) -> bool {
        self.trading.is_some() || self.command.is_some() || self.military.is_some()
    }
}

/// What one run plans and the save it reads.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitExpRequest {
    pub royal: RoyalGuardian,
    pub map: usize,
    pub targets: UnitTargets,
    pub move_transients: bool,
}

/// One profession change a step makes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfessionChange {
    pub slot: usize,
    /// The profession the slot holds now.
    pub from: usize,
    pub to: usize,
}

/// Why a step is taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepReason {
    Start,
    TradingTarget,
    IntelRank,
}

impl StepReason {
    /// The id a control keys by.
    pub fn id(self) -> &'static str {
        match self {
            StepReason::Start => "start",
            StepReason::TradingTarget => "trading-target",
            StepReason::IntelRank => "intel-rank",
        }
    }
}

/// One point in the plan where the trader and surveyor split changes.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// Hours into the projection.
    pub hours: f64,
    pub traders: f64,
    pub surveyors: f64,
    pub reason: StepReason,
    pub ranks: [f64; 5],
    pub changes: Vec<ProfessionChange>,
    /// The packed unit row after the step's changes.
    pub packed: f64,
}

/// The split the plan runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    pub first: f64,
    pub second: f64,
    pub after: f64,
    pub switch_rank: f64,
}

/// How a projection ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Already,
    Reached,
    NoProgress,
    Cutoff,
    Limit,
}

impl Status {
    /// The id a control keys by.
    pub fn id(self) -> &'static str {
        match self {
            Status::Already => "already",
            Status::Reached => "reached",
            Status::NoProgress => "no-progress",
            Status::Cutoff => "cutoff",
            Status::Limit => "limit",
        }
    }

    /// Whether the targets are met.
    pub fn reached(self) -> bool {
        matches!(self, Status::Already | Status::Reached)
    }
}

/// What one projection found.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub status: Status,
    /// The target the projection cannot reach, when it stopped on one.
    pub blocked: Option<Target>,
    pub eta_hours: Option<f64>,
    pub elapsed_hours: f64,
    pub changes: usize,
    pub switched_at: Option<f64>,
    /// When each target was reached, in target order.
    pub reached_at: [Option<f64>; 3],
    pub final_ranks: [f64; 5],
}

/// The projected plan.
#[derive(Clone, Debug, PartialEq)]
pub struct ExpPlan {
    pub status: Status,
    pub blocked: Option<Target>,
    pub eta_hours: Option<f64>,
    pub elapsed_hours: f64,
    pub reached_at: [Option<f64>; 3],
    pub final_ranks: [f64; 5],
    pub spec: Spec,
    pub steps: Vec<Step>,
    /// How many times the split changed after the start.
    pub switches: usize,
}

/// One transient assignment the plan moves onto the target outpost.
#[derive(Clone, Debug, PartialEq)]
pub struct Donor {
    pub kind: i64,
    pub name: &'static str,
    pub map: usize,
    pub assignment: usize,
    pub bar: usize,
    /// The EXP per hour the donor's outpost loses when the unit leaves.
    pub lost_rate: f64,
}

/// One row of the transient table: how a rank target responds to the available
/// same-world assignments.
#[derive(Clone, Debug, PartialEq)]
pub struct TransientRow {
    pub kind: i64,
    pub name: &'static str,
    pub current: f64,
    pub available: f64,
    pub best: f64,
    pub options: Vec<TransientOption>,
}

/// One option in a transient row.
#[derive(Clone, Debug, PartialEq)]
pub struct TransientOption {
    pub count: f64,
    pub status: Status,
    pub eta_hours: Option<f64>,
    pub blocked: Option<Target>,
}

/// The transient assignment pass's result.
#[derive(Clone, Debug, PartialEq)]
pub struct Transients {
    pub rows: Vec<TransientRow>,
    /// How many of each assignment the best plan uses.
    pub counts: BTreeMap<i64, f64>,
    pub moves: Vec<Donor>,
    /// The saved counts' own best projection, for comparison.
    pub stay_eta_hours: Option<f64>,
    pub stay_status: Status,
}

/// The whole planner result.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitExpPlan {
    pub map: usize,
    pub targets: UnitTargets,
    /// The movable units the outpost can trade with.
    pub pool: f64,
    pub current_traders: f64,
    pub surveyors_unlocked: bool,
    pub banked_hours: f64,
    pub saved_ranks: [f64; 5],
    /// The ranks the plan starts from, after the banked EXP is collected.
    pub start_ranks: [f64; 5],
    /// The assignments on the outpost when the plan starts.
    pub counts: [f64; 8],
    pub plan: ExpPlan,
    /// What the saved split reaches, for comparison.
    pub current: Run,
    /// How many hours the plan saves over the saved split.
    pub saved_hours: Option<f64>,
    pub transients: Option<Transients>,
    pub limits: (f64, usize),
}

/// What the planner refuses, with the sentence a section shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Refusal {
    message: String,
}

impl Refusal {
    /// The sentence a section prints.
    pub fn message(&self) -> &str {
        &self.message
    }

    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// The state one projection walks.
#[derive(Clone, Debug)]
struct State {
    exp: [f64; 5],
    ranks: [f64; 5],
}

/// One outpost's planner model: upstream `_model`.
#[derive(Clone, Debug)]
struct Model {
    map: usize,
    a72: f64,
    unlocked: bool,
    pool: f64,
    current: f64,
    order: Vec<i64>,
    counts: [f64; 8],
    feed: bool,
    purified: bool,
    k: [f64; 5],
    legal: Vec<f64>,
}

/// One recorded step inside a projection.
#[derive(Clone, Debug)]
struct RunStep {
    hours: f64,
    traders: f64,
    ranks: [f64; 5],
    reason: StepReason,
}

/// One projection's result and record.
#[derive(Clone, Debug)]
struct Recorded {
    spec: Spec,
    run: Run,
    steps: Vec<RunStep>,
}

impl RoyalGuardian {
    /// One outpost's planner model: upstream `_model`.
    fn unit_exp_model(&self, map: usize) -> Model {
        let a72 = self.armory_bonus(72) / 100.0;
        let mult = 1.0 + self.rank(map, 1) * a72;
        let units = self.units(map);
        let order: Vec<i64> = self
            .militia_assignments()
            .into_iter()
            .filter(|assignment| assignment.map == map as i64)
            .map(|assignment| assignment.kind)
            .filter(|kind| (4..=7).contains(kind))
            .collect();
        let mut counts = [0.0; 8];
        for kind in &order {
            counts[*kind as usize] += 1.0;
        }
        let unlocked = self.unit_types().len() == 4;
        let pool = units
            .iter()
            .filter(|(_, profession)| *profession == 1 || *profession == 3)
            .count() as f64;
        let current = units
            .iter()
            .filter(|(_, profession)| *profession == 1)
            .count() as f64;
        let mut legal: Vec<f64> = if unlocked {
            (0..=pool as usize).map(|traders| traders as f64).collect()
        } else {
            vec![current, pool]
        };
        legal.sort_by(|left, right| left.total_cmp(right));
        legal.dedup();
        Model {
            map,
            a72,
            unlocked,
            pool,
            current,
            order,
            counts,
            feed: self.armory_level(17) >= 1.0,
            purified: self.purified(map),
            k: [0, 1, 2, 3, 4].map(|bar| self.bar_exp_rate(bar, map) / mult),
            legal,
        }
    }
}

impl Model {
    /// The EXP per hour each bar earns at one split: upstream `_rates`.
    fn rates(&self, ranks: &[f64; 5], traders: f64, counts: &[f64; 8], out: &mut [f64; 5]) {
        let mult = 1.0 + ranks[1] * self.a72;
        let purity = if !self.purified && ranks[4] >= 1.0 {
            3.0
        } else {
            1.0
        };
        out[0] = self.k[0] * purity * mult * (traders + ((ranks[2] - 1.0).max(0.0) / 4.0).ceil());
        out[1] = self.k[1]
            * purity
            * mult
            * (self.pool - traders + ((ranks[2] - 3.0).max(0.0) / 4.0).ceil());
        out[2] = self.k[2] * purity * mult * counts[5];
        out[3] = self.k[3] * purity * mult * counts[6];
        out[4] = self.k[4] * purity * mult * counts[7];
        if self.feed && counts[4] > 0.0 {
            out[highest(ranks)] += self.k[3] * purity * mult / 2.0 * counts[4];
        }
    }

    /// The traders the split runs: upstream `_policy`.
    fn traders(&self, spec: &Spec, ranks: &[f64; 5], targets: &UnitTargets) -> f64 {
        let trading_done = targets
            .trading
            .is_some_and(|target| ranks[0] >= target as f64);
        if self.unlocked && (targets.trading.is_none() || trading_done) {
            return spec.after;
        }
        if ranks[1] >= spec.switch_rank {
            spec.second
        } else {
            spec.first
        }
    }

    /// One native `CollectAll`: Trading and Intel at the saved split, then the
    /// saved transient order: upstream `_collectOnce`.
    fn collect_once(&self, state: &mut State, hours: f64) {
        let mut rates = [0.0; 5];
        self.rates(&state.ranks, self.current, &[0.0; 8], &mut rates);
        add(&mut state.exp, &mut state.ranks, 0, rates[0] * hours);
        add(&mut state.exp, &mut state.ranks, 1, rates[1] * hours);
        for kind in &self.order {
            let mult = 1.0 + state.ranks[1] * self.a72;
            let purity = if !self.purified && state.ranks[4] >= 1.0 {
                3.0
            } else {
                1.0
            };
            if *kind >= 5 {
                let bar = (*kind - 3) as usize;
                add(
                    &mut state.exp,
                    &mut state.ranks,
                    bar,
                    self.k[bar] * purity * mult * hours,
                );
            } else if self.feed {
                let bar = highest(&state.ranks);
                add(
                    &mut state.exp,
                    &mut state.ranks,
                    bar,
                    self.k[3] * purity * mult / 2.0 * hours,
                );
            }
        }
    }

    /// One projection: upstream `_run`.
    fn run(
        &self,
        start: &State,
        targets: &UnitTargets,
        counts: &[f64; 8],
        spec: &Spec,
        cutoff: f64,
        record: bool,
    ) -> Recorded {
        let mut state = start.clone();
        let mut rates = [0.0; 5];
        let mut reached_at: [Option<f64>; 3] = [None; 3];
        let mut steps: Vec<RunStep> = Vec::new();
        let mut hours = 0.0f64;
        let mut events = 0usize;
        let mut traders = self.current;
        let mut changes = 0usize;
        let mut switched_at = (state.ranks[1] >= spec.switch_rank).then_some(0.0);
        let mark = |state: &State,
                    hours: f64,
                    reached_at: &mut [Option<f64>; 3],
                    switched_at: &mut Option<f64>| {
            for target in Target::ALL {
                if let Some(value) = targets.get(target)
                    && reached_at[target_order(target)].is_none()
                    && state.ranks[target.bar()] >= value as f64
                {
                    reached_at[target_order(target)] = Some(hours);
                }
            }
            if switched_at.is_none() && state.ranks[1] >= spec.switch_rank {
                *switched_at = Some(hours);
            }
        };
        let done = |status: Status,
                    blocked: Option<Target>,
                    hours: f64,
                    changes: usize,
                    switched_at: Option<f64>,
                    state: &State,
                    reached_at: [Option<f64>; 3],
                    steps: &[RunStep]|
         -> Recorded {
            Recorded {
                spec: *spec,
                run: Run {
                    status,
                    blocked,
                    eta_hours: status.reached().then_some(hours),
                    elapsed_hours: hours,
                    changes,
                    switched_at,
                    reached_at,
                    final_ranks: state.ranks,
                },
                steps: steps.to_vec(),
            }
        };
        mark(&state, hours, &mut reached_at, &mut switched_at);
        if let Some(blocked) = unreachable(self, targets, &state.ranks, counts)
            && !met(targets, &state.ranks)
        {
            return done(
                Status::NoProgress,
                Some(blocked),
                hours,
                changes,
                switched_at,
                &state,
                reached_at,
                &steps,
            );
        }
        loop {
            if met(targets, &state.ranks) {
                if record && steps.is_empty() {
                    steps.push(RunStep {
                        hours,
                        traders,
                        ranks: state.ranks,
                        reason: StepReason::Start,
                    });
                }
                return done(
                    if hours == 0.0 {
                        Status::Already
                    } else {
                        Status::Reached
                    },
                    None,
                    hours,
                    changes,
                    switched_at,
                    &state,
                    reached_at,
                    &steps,
                );
            }
            let next = self.traders(spec, &state.ranks, targets);
            if next != traders || (record && steps.is_empty()) {
                if next != traders {
                    changes += 1;
                }
                if record {
                    steps.push(RunStep {
                        hours,
                        traders: next,
                        ranks: state.ranks,
                        reason: if steps.is_empty() {
                            StepReason::Start
                        } else if targets
                            .trading
                            .is_some_and(|target| state.ranks[0] >= target as f64)
                        {
                            StepReason::TradingTarget
                        } else {
                            StepReason::IntelRank
                        },
                    });
                }
                traders = next;
            }
            self.rates(&state.ranks, traders, counts, &mut rates);
            // A zero-rate bar keeps its rank, so it can never become the
            // militia-fed highest bar.
            for target in [Target::Command, Target::Military] {
                if let Some(value) = targets.get(target)
                    && state.ranks[target.bar()] < value as f64
                    && rates[target.bar()] <= 0.0
                {
                    return done(
                        Status::NoProgress,
                        Some(target),
                        hours,
                        changes,
                        switched_at,
                        &state,
                        reached_at,
                        &steps,
                    );
                }
            }
            let mut dt = f64::INFINITY;
            let mut bar = None;
            for (index, rate) in rates.iter().enumerate() {
                if *rate <= 0.0 {
                    continue;
                }
                let need = ((RoyalGuardian::exp_formula(state.ranks[index], index)
                    - state.exp[index])
                    / *rate)
                    .max(0.0);
                if need < dt {
                    dt = need;
                    bar = Some(index);
                }
            }
            let Some(bar) = bar else {
                let blocked = Target::ALL.into_iter().find(|target| {
                    targets
                        .get(*target)
                        .is_some_and(|value| state.ranks[target.bar()] < value as f64)
                });
                return done(
                    Status::NoProgress,
                    blocked,
                    hours,
                    changes,
                    switched_at,
                    &state,
                    reached_at,
                    &steps,
                );
            };
            if hours + dt > cutoff && cutoff < MAX_HOURS {
                return done(
                    Status::Cutoff,
                    None,
                    hours,
                    changes,
                    switched_at,
                    &state,
                    reached_at,
                    &steps,
                );
            }
            if hours + dt > MAX_HOURS {
                return done(
                    Status::Limit,
                    None,
                    MAX_HOURS,
                    changes,
                    switched_at,
                    &state,
                    reached_at,
                    &steps,
                );
            }
            events += 1;
            if events > MAX_EVENTS {
                return done(
                    Status::Limit,
                    None,
                    hours,
                    changes,
                    switched_at,
                    &state,
                    reached_at,
                    &steps,
                );
            }
            for (index, rate) in rates.iter().enumerate() {
                state.exp[index] += *rate * dt;
            }
            state.exp[bar] = state.exp[bar].max(RoyalGuardian::exp_formula(state.ranks[bar], bar));
            hours += dt;
            for index in 0..5 {
                let rank = state.ranks[index];
                let mut rank_from = rank;
                while state.exp[index] >= RoyalGuardian::exp_formula(rank_from, index) {
                    rank_from += 1.0;
                }
                state.ranks[index] = rank_from;
            }
            mark(&state, hours, &mut reached_at, &mut switched_at);
        }
    }
}

/// The index a target uses in `reached_at`.
fn target_order(target: Target) -> usize {
    match target {
        Target::Trading => 0,
        Target::Command => 1,
        Target::Military => 2,
    }
}

/// Adds EXP to a bar and moves its rank up: upstream `_add`.
fn add(exp: &mut [f64; 5], ranks: &mut [f64; 5], bar: usize, amount: f64) {
    exp[bar] += amount;
    while exp[bar] >= RoyalGuardian::exp_formula(ranks[bar], bar) {
        ranks[bar] += 1.0;
    }
}

/// The bar with the highest rank: upstream `_highest`.
fn highest(ranks: &[f64; 5]) -> usize {
    let mut bar = 0;
    for index in 1..5 {
        if ranks[index] > ranks[bar] {
            bar = index;
        }
    }
    bar
}

/// Whether every target is met: upstream `_met`.
fn met(targets: &UnitTargets, ranks: &[f64; 5]) -> bool {
    Target::ALL.iter().all(|target| {
        targets
            .get(*target)
            .is_none_or(|value| ranks[target.bar()] >= value as f64)
    })
}

/// Why no plan can reach the targets, when none can: upstream `_reachable`.
fn unreachable(
    model: &Model,
    targets: &UnitTargets,
    ranks: &[f64; 5],
    counts: &[f64; 8],
) -> Option<Target> {
    let feed = model.feed && counts[4] > 0.0;
    if let Some(target) = targets.command
        && ranks[2] < target as f64
        && counts[5] == 0.0
        && !(feed && highest(ranks) == 2)
    {
        return Some(Target::Command);
    }
    if let Some(target) = targets.military
        && ranks[3] < target as f64
        && counts[6] == 0.0
        && !(feed && highest(ranks) == 3)
    {
        return Some(Target::Military);
    }
    if let Some(target) = targets.trading
        && ranks[0] < target as f64
        && model.pool == 0.0
        && !feed
        && ranks[2] < 2.0
        && counts[5] == 0.0
    {
        return Some(Target::Trading);
    }
    None
}

/// Whether a projection is faster, or as fast with fewer switches: upstream
/// `_better`.
fn better(result: &Run, best: Option<&Run>) -> bool {
    let Some(eta) = result.eta_hours else {
        return false;
    };
    let Some(best) = best else {
        return true;
    };
    let Some(best_eta) = best.eta_hours else {
        return true;
    };
    if (eta - best_eta).abs() > EPSILON * eta.abs().max(best_eta.abs()).max(1.0) {
        eta < best_eta
    } else {
        result.changes < best.changes
    }
}

/// The split a fixed trader count runs: upstream `fixed`.
fn fixed(traders: f64) -> Spec {
    Spec {
        first: traders,
        second: traders,
        after: traders,
        switch_rank: f64::INFINITY,
    }
}

/// The split search: every saved split, then every two-step split and switch
/// point: upstream `_optimize`.
fn optimize(
    model: &Model,
    start: &State,
    targets: &UnitTargets,
    counts: &[f64; 8],
    report: &mut dyn FnMut(f32) -> Control,
) -> Option<(Spec, Run)> {
    let mut best: Option<(Spec, Run)> = None;
    let mut done = 0usize;
    let estimate = 1
        + model.legal.len()
        + if targets.trading.is_some() {
            model.legal.len() * model.legal.len() * 8
        } else {
            0
        };
    let mut consider = |spec: Spec| -> Option<Run> {
        done += 1;
        if report((done as f32 / estimate.max(1) as f32).min(1.0)) == Control::Stop {
            return None;
        }
        let cutoff = best
            .as_ref()
            .and_then(|(_, run)| run.eta_hours)
            .map(|eta| eta * (1.0 + 1e-9) + 1e-9)
            .unwrap_or(f64::INFINITY);
        let run = model.run(start, targets, counts, &spec, cutoff, false).run;
        if better(&run, best.as_ref().map(|(_, run)| run)) {
            best = Some((spec, run.clone()));
        }
        Some(run)
    };
    let base = consider(fixed(model.current))?;
    if base.status == Status::Already || base.status == Status::NoProgress {
        return Some((fixed(model.current), base));
    }
    let after = if model.unlocked { 0.0 } else { model.current };
    for traders in model.legal.clone() {
        consider(Spec {
            first: traders,
            second: traders,
            after: if model.unlocked { after } else { traders },
            switch_rank: f64::INFINITY,
        })?;
    }
    if targets.trading.is_some() {
        let firsts = if model.unlocked {
            model.legal.clone()
        } else {
            vec![model.current]
        };
        for first in firsts {
            for second in model.legal.clone() {
                if second <= first {
                    continue;
                }
                let from = start.ranks[1].floor() as usize + 1;
                for switch in from..from + SWITCH_SCAN {
                    let result = consider(Spec {
                        first,
                        second,
                        after: if model.unlocked { after } else { second },
                        switch_rank: switch as f64,
                    })?;
                    if result.switched_at.is_none() {
                        break;
                    }
                }
            }
        }
    }
    Some(best.unwrap_or_else(|| (fixed(model.current), base)))
}

/// The packed digits a unit row holds: upstream `_digits`.
fn digits(packed: f64) -> [char; 9] {
    let text = format!("{:09}", packed.max(0.0) as u64);
    let text = &text[text.len().saturating_sub(9)..];
    std::array::from_fn(|index| text.as_bytes()[index] as char)
}

/// Retargets the outpost's traders: upstream `_retarget`.
fn retarget(mut packed: [char; 9], traders: f64) -> ([char; 9], Vec<ProfessionChange>) {
    let slots: Vec<(usize, usize)> = packed
        .iter()
        .enumerate()
        .filter_map(|(slot, digit)| match digit {
            '3' => Some((slot, 1usize)),
            '5' => Some((slot, 3usize)),
            _ => None,
        })
        .collect();
    let current = slots.iter().filter(|(_, kind)| *kind == 1).count() as f64;
    let (from, to) = if traders > current { (3, 1) } else { (1, 3) };
    let mut remaining = (traders - current).abs() as usize;
    let mut changes = Vec::new();
    for (slot, kind) in slots {
        if remaining == 0 {
            break;
        }
        if kind != from {
            continue;
        }
        packed[slot] = char::from_digit((to + 2) as u32, 10).unwrap_or('0');
        changes.push(ProfessionChange { slot, from, to });
        remaining -= 1;
    }
    (packed, changes)
}

/// The same-world assignments a target outpost may borrow: upstream
/// `_transientPools`.
fn transient_pools(royal: &RoyalGuardian, map: usize) -> Vec<Donor> {
    let world = RoyalGuardian::world_of(map);
    let mut donors: Vec<Donor> = royal
        .militia_assignments()
        .into_iter()
        .filter(|assignment| {
            assignment.world == world
                && TRANSIENT_TYPES.contains(&assignment.kind)
                && assignment.map != map as i64
                && (assignment.kind != 4 || royal.outpost(assignment.map.max(0) as usize).built)
        })
        .map(|assignment| {
            let donor = assignment.map.max(0) as usize;
            let built = royal.outpost(donor).built;
            let bar = if assignment.kind == 4 {
                let mut highest = 0;
                for index in 1..5 {
                    if royal.rank(donor, index) > royal.rank(donor, highest) {
                        highest = index;
                    }
                }
                highest
            } else {
                (assignment.kind - 3) as usize
            };
            let lost_rate = if !built {
                0.0
            } else if assignment.kind == 4 {
                if royal.armory_level(17) >= 1.0 {
                    royal.bar_exp_rate(3, donor) / 2.0
                } else {
                    0.0
                }
            } else {
                royal.bar_exp_rate(bar, donor)
            };
            Donor {
                kind: assignment.kind,
                name: transient_name(assignment.kind),
                map: donor,
                assignment: assignment.assignment,
                bar,
                lost_rate,
            }
        })
        .collect();
    donors.sort_by(|left, right| {
        left.lost_rate
            .total_cmp(&right.lost_rate)
            .then_with(|| left.map.cmp(&right.map))
            .then_with(|| left.assignment.cmp(&right.assignment))
    });
    donors
}

/// The ranks the plan starts from, after the banked EXP is collected.
fn start_state(royal: &RoyalGuardian, model: &Model) -> State {
    let mut state = State {
        exp: [0, 1, 2, 3, 4].map(|bar| royal.map_cell(model.map, cell::RANK_EXP + bar)),
        ranks: [0, 1, 2, 3, 4].map(|bar| royal.rank(model.map, bar)),
    };
    let banked = royal.banked_hours();
    if banked > 0.0 {
        model.collect_once(&mut state, banked);
    }
    state
}

/// The plan's steps and targets: upstream `_summary`.
fn summary(
    royal: &RoyalGuardian,
    model: &Model,
    start: &State,
    targets: &UnitTargets,
    counts: &[f64; 8],
    best: &Recorded,
) -> ExpPlan {
    let replay = model.run(start, targets, counts, &best.spec, f64::INFINITY, true);
    let mut packed = digits(royal.map_cell(model.map, cell::UNITS));
    let mut steps = Vec::new();
    for step in replay.steps {
        let (next, changes) = retarget(packed, step.traders);
        packed = next;
        let packed_value = next.iter().fold(0.0, |value, digit| {
            value * 10.0 + digit.to_digit(10).unwrap_or(0) as f64
        });
        steps.push(Step {
            hours: step.hours,
            traders: step.traders,
            surveyors: model.pool - step.traders,
            reason: step.reason,
            ranks: step.ranks,
            changes,
            packed: packed_value,
        });
    }
    let switches = steps.len().saturating_sub(1);
    ExpPlan {
        status: replay.run.status,
        blocked: replay.run.blocked,
        eta_hours: replay.run.eta_hours,
        elapsed_hours: replay.run.elapsed_hours,
        reached_at: replay.run.reached_at,
        final_ranks: replay.run.final_ranks,
        spec: best.spec,
        steps,
        switches,
    }
}

/// The plan's steps as the line a section card prints.
pub fn step_text(step: &Step) -> String {
    format!(
        "{:.1}h: {} Traders / {} Surveyors",
        step.hours, step.traders, step.surveyors
    )
}

/// The targets as the planner reads them: upstream `_targets`.
fn validate_targets(request: &UnitExpRequest, unlocked: &[bool; 5]) -> Result<(), Refusal> {
    for target in Target::ALL {
        let Some(value) = request.targets.get(target) else {
            continue;
        };
        if !(1..=500).contains(&value) {
            return Err(Refusal::new(format!(
                "{} target must be an integer between 1 and 500.",
                target.label()
            )));
        }
        if !unlocked[target.bar()] {
            return Err(Refusal::new(format!("{} is locked.", target.label())));
        }
    }
    if !request.targets.any() {
        return Err(Refusal::new("Set at least one target rank."));
    }
    Ok(())
}

/// The cache key of an assignment count vector.
fn key(counts: &[f64; 8]) -> String {
    counts
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<String>>()
        .join(",")
}

/// How many donors a save has for one assignment kind.
fn donors_of(donors: &[Donor], kind: i64) -> usize {
    donors.iter().filter(|donor| donor.kind == kind).count()
}

/// A count vector for one joint combination: upstream's mixed-radix walk.
fn extra_counts(model: &Model, donors: &[Donor], index: usize) -> [f64; 8] {
    let mut counts = model.counts;
    let mut rest = index;
    for kind in TRANSIENT_TYPES {
        let choices = donors_of(donors, kind) + 1;
        counts[kind as usize] = model.counts[kind as usize] + (rest % choices) as f64;
        rest /= choices;
    }
    counts
}

/// The count vector that takes every available donor.
fn filled_counts(model: &Model, donors: &[Donor]) -> [f64; 8] {
    let mut counts = model.counts;
    for kind in TRANSIENT_TYPES {
        counts[kind as usize] = model.counts[kind as usize] + donors_of(donors, kind) as f64;
    }
    counts
}

/// The best split for each assignment count a save can reach.
struct Solver<'a> {
    model: &'a Model,
    start: &'a State,
    targets: &'a UnitTargets,
    cache: BTreeMap<String, Recorded>,
}

impl Solver<'_> {
    /// The plan for one count vector, from the cache when it was already run.
    fn solve(
        &mut self,
        counts: &[f64; 8],
        report: &mut dyn FnMut(f32) -> Control,
    ) -> Option<Recorded> {
        let key = key(counts);
        if let Some(recorded) = self.cache.get(&key) {
            return Some(recorded.clone());
        }
        let (spec, _) = optimize(self.model, self.start, self.targets, counts, report)?;
        let recorded = self.model.run(
            self.start,
            self.targets,
            counts,
            &spec,
            f64::INFINITY,
            false,
        );
        self.cache.insert(key, recorded.clone());
        Some(recorded)
    }
}

/// Solves one count vector and reports its own fraction, mapping a cancelled
/// run to `None`.
fn solve_reported(
    solver: &mut Solver,
    counts: &[f64; 8],
    done: &mut usize,
    total: usize,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Option<Recorded> {
    let base = *done;
    let mut adapter = |fraction: f32| {
        report(Progress {
            fraction: ((base as f32 + fraction) / total.max(1) as f32).min(1.0),
            message: format!("Planning the split ({} of {total})", base + 1),
        })
    };
    let recorded = solver.solve(counts, &mut adapter);
    *done = base + 1;
    recorded
}

/// How many assignments a plan used beyond the saved counts.
fn moved(model: &Model, counts: &[f64; 8]) -> f64 {
    TRANSIENT_TYPES
        .iter()
        .map(|kind| counts[*kind as usize] - model.counts[*kind as usize])
        .sum()
}

/// Whether `left` reaches the targets earlier than `right`, or as early with
/// fewer moved assignments: upstream's `better` inside the transient pass.
fn better_counts(
    solver: &mut Solver,
    model: &Model,
    left: &[f64; 8],
    right: &[f64; 8],
    done: &mut usize,
    total: usize,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Option<bool> {
    let left_eta = solve_reported(solver, left, done, total, report)?
        .run
        .eta_hours;
    let right_eta = solve_reported(solver, right, done, total, report)?
        .run
        .eta_hours;
    Some(match (left_eta, right_eta) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(left_eta), Some(right_eta)) => {
            if (left_eta - right_eta).abs() > EPSILON * left_eta.abs().max(right_eta.abs()).max(1.0)
            {
                left_eta < right_eta
            } else {
                moved(model, left) < moved(model, right)
            }
        }
    })
}

impl RoyalGuardian {
    /// Plan the split that reaches `targets` fastest: upstream
    /// `planRoyalUnitExp`, with the search reported through `report`.
    pub fn plan_unit_exp(
        &self,
        request: &UnitExpRequest,
        mut report: impl FnMut(Progress) -> Control,
    ) -> Result<UnitExpPlan, Refusal> {
        let map = request.map;
        if !self.outpost(map).built {
            return Err(Refusal::new("Select a built outpost to plan its units."));
        }
        let unlocked = self.unlocked_bars();
        if !Target::ALL.iter().any(|target| unlocked[target.bar()]) {
            return Err(Refusal::new(
                "Unlock Trading, Command, or Military Rank to plan.",
            ));
        }
        validate_targets(request, &unlocked)?;
        let model = self.unit_exp_model(map);
        let saved = State {
            exp: [0, 1, 2, 3, 4].map(|bar| self.map_cell(map, cell::RANK_EXP + bar)),
            ranks: [0, 1, 2, 3, 4].map(|bar| self.rank(map, bar)),
        };
        let start = start_state(self, &model);
        let donors = if request.move_transients {
            transient_pools(self, map)
        } else {
            Vec::new()
        };
        let mut solver = Solver {
            model: &model,
            start: &start,
            targets: &request.targets,
            cache: BTreeMap::new(),
        };
        let total = 3 + donors.len() * 2;
        let mut done = 0usize;
        let cancelled = || Refusal::new("The plan was cancelled.");
        let mut best_counts = model.counts;
        if !donors.is_empty() {
            let combos = TRANSIENT_TYPES.iter().fold(1usize, |product, kind| {
                product.saturating_mul(donors_of(&donors, *kind) + 1)
            });
            if combos <= JOINT_LIMIT {
                // Targets often need several unit types at once, so single-type
                // changes from the saved counts are not enough.
                for index in 0..combos {
                    let counts = extra_counts(&model, &donors, index);
                    let better = better_counts(
                        &mut solver,
                        &model,
                        &counts,
                        &best_counts,
                        &mut done,
                        total,
                        &mut report,
                    )
                    .ok_or_else(cancelled)?;
                    if better {
                        best_counts = counts;
                    }
                }
            } else {
                for seed in [model.counts, filled_counts(&model, &donors)] {
                    let mut local = seed;
                    for _ in 0..8 {
                        let mut changed = false;
                        for kind in TRANSIENT_TYPES {
                            for extra in 0..=donors_of(&donors, kind) {
                                let mut counts = local;
                                counts[kind as usize] = model.counts[kind as usize] + extra as f64;
                                let better = better_counts(
                                    &mut solver,
                                    &model,
                                    &counts,
                                    &local,
                                    &mut done,
                                    total,
                                    &mut report,
                                )
                                .ok_or_else(cancelled)?;
                                if better {
                                    local = counts;
                                    changed = true;
                                }
                            }
                        }
                        if !changed {
                            break;
                        }
                    }
                    let better = better_counts(
                        &mut solver,
                        &model,
                        &local,
                        &best_counts,
                        &mut done,
                        total,
                        &mut report,
                    )
                    .ok_or_else(cancelled)?;
                    if better {
                        best_counts = local;
                    }
                }
            }
        }
        let stay = solve_reported(&mut solver, &model.counts, &mut done, total, &mut report)
            .ok_or_else(cancelled)?;
        let mut rows = Vec::new();
        let mut moves = Vec::new();
        let mut transients = None;
        if request.move_transients {
            for kind in TRANSIENT_TYPES {
                let count = donors_of(&donors, kind);
                let mut options = Vec::new();
                for extra in 0..=count {
                    let mut counts = best_counts;
                    counts[kind as usize] = model.counts[kind as usize] + extra as f64;
                    let recorded =
                        solve_reported(&mut solver, &counts, &mut done, total, &mut report)
                            .ok_or_else(cancelled)?;
                    options.push(TransientOption {
                        count: counts[kind as usize],
                        status: recorded.run.status,
                        eta_hours: recorded.run.eta_hours,
                        blocked: recorded.run.blocked,
                    });
                }
                rows.push(TransientRow {
                    kind,
                    name: transient_name(kind),
                    current: model.counts[kind as usize],
                    available: model.counts[kind as usize] + count as f64,
                    best: best_counts[kind as usize],
                    options,
                });
                let used = best_counts[kind as usize] - model.counts[kind as usize];
                moves.extend(
                    donors
                        .iter()
                        .filter(|donor| donor.kind == kind)
                        .take(used.max(0.0) as usize)
                        .cloned(),
                );
            }
            transients = Some(Transients {
                rows,
                counts: TRANSIENT_TYPES
                    .iter()
                    .map(|kind| (*kind, best_counts[*kind as usize]))
                    .collect(),
                moves,
                stay_eta_hours: stay.run.eta_hours,
                stay_status: stay.run.status,
            });
        }
        let best = solve_reported(&mut solver, &best_counts, &mut done, total, &mut report)
            .ok_or_else(cancelled)?;
        let plan = summary(self, &model, &start, &request.targets, &best_counts, &best);
        let current = model
            .run(
                &start,
                &request.targets,
                &model.counts,
                &fixed(model.current),
                f64::INFINITY,
                false,
            )
            .run;
        report(Progress {
            fraction: 1.0,
            message: "Plan complete".to_owned(),
        });
        let saved_hours = match (plan.eta_hours, current.eta_hours) {
            (Some(plan), Some(current)) => Some(current - plan),
            _ => None,
        };
        let banked_hours = self.banked_hours();
        Ok(UnitExpPlan {
            map,
            targets: request.targets,
            pool: model.pool,
            current_traders: model.current,
            surveyors_unlocked: model.unlocked,
            banked_hours,
            saved_ranks: saved.ranks,
            start_ranks: start.ranks,
            counts: model.counts,
            plan,
            current,
            saved_hours,
            transients,
            limits: (MAX_HOURS, MAX_EVENTS),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_round_trips_through_its_id() {
        for target in Target::ALL {
            assert_eq!(Target::from_id(target.id()), Some(target));
        }
        assert_eq!(Target::from_id("purity"), None);
    }

    #[test]
    fn a_step_only_retargets_a_slot_the_row_fields() {
        let (packed, changes) = retarget(digits(222_111_555.0), 2.0);
        assert_eq!(changes.len(), 2, "two surveyors become traders");
        assert_eq!(changes[0].from, 3);
        assert_eq!(changes[0].to, 1);
        let text: String = packed.iter().collect();
        assert_eq!(text, "222111335", "the last surveyor slot keeps its unit");
    }
}
