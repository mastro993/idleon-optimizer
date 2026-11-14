//! JO-05: the coordinator, the multi-start full-board layout search.
//!
//! Ports the coordinator from `js/optimizers/jelly-coordinator.js`
//! (`jellySelectSharedElites`, `jellySearchAssignment`) and the bounded
//! construction and local search of `createJellyLayoutSearch` from
//! `js/stats/systems/w7/jelly-operator.js`:
//!
//! - a *start* builds a full board from composition seeds (the greedy
//!   `greedyCompositionSeed` over anchor and cell-type orders) and its shared
//!   elites, then keeps a diverse beam of placements, replacements, relocations
//!   and re-fills (`placeWithReplacement`, `fillOpenSlots`,
//!   `selectDiverseBy`) across the build, saved-board and refinement rounds;
//! - the coordinator rotates one focus per start (`explore`, `refine`,
//!   `repair`, `measure`) and hands the previous starts' strongest, most
//!   distinct layouts to the next one as shared elites;
//! - the strongest finalists are compared with the saved board on the same
//!   seeds (`simulateJellyTrials`, `simulatedObjectiveScore`,
//!   `pairedOperationConfidence`) and the recommendation is never worse than
//!   the board that was handed in.
//!
//! The upstream search is a multi-worker stochastic engine with policy
//! screening, exact repair, certification and wall-clock scheduling. This port
//! keeps the coordinator and the bounded search it schedules, and drops the
//! stages that only exist to speed that engine up: the coordinator's payoff
//! scheduling (it reads `performance.now()`, so it would make the result depend
//! on the machine), policy screening (a finalist runs the save's Fever and
//! revive policy), exact enumeration and certificates. Every search round is a
//! total order on a seeded generator, so the same inputs always produce the
//! same board.

use super::board::{self, Layout};
use super::data;
use super::operation::rng::{Rng, random_int};
use super::planner::{self, Confidence, Objective};
use super::surrogate;
use super::{Jelly, PROXIMITY_ANCHORS};
use idleon_common::Progress;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// What one start concentrates on (`FOCUSES`), in the page's own order.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Focus {
    #[default]
    Explore,
    Refine,
    Repair,
    Measure,
}

impl Focus {
    pub const ALL: [Focus; 4] = [Focus::Explore, Focus::Refine, Focus::Repair, Focus::Measure];

    pub fn id(self) -> &'static str {
        match self {
            Focus::Explore => "explore",
            Focus::Refine => "refine",
            Focus::Repair => "repair",
            Focus::Measure => "measure",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Focus::Explore => "Explore",
            Focus::Refine => "Refine",
            Focus::Repair => "Repair",
            Focus::Measure => "Measure",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|focus| focus.id() == id)
    }
}

/// How many elites a start hands to the next (`jellySelectSharedElites` limit).
pub const SHARED_ELITES: usize = 6;

/// The knobs one optimizer run reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub objective: Objective,
    /// The cell type an EXP objective maximizes; `None` is total EXP.
    pub exp_cell_type: Option<usize>,
    /// The obstruction to optimize for; `None` is the save's own stage.
    pub obstruction: Option<usize>,
    /// How many independent starts the coordinator runs.
    pub starts: usize,
    /// How many layouts one start keeps between rounds.
    pub beam_width: usize,
    /// The construction rounds before the saved-board and refinement rounds.
    pub iterations: usize,
    /// The rounds that re-fill the strongest leaders around the saved board.
    pub saved_refinement_rounds: usize,
    /// The local placement and relocation rounds after the saved board.
    pub refinement_rounds: usize,
    /// How many distinct finalists are simulated.
    pub finalists: usize,
    /// How many shared-seed trials each finalist runs.
    pub trials: u32,
    pub seed: u32,
    /// The board to optimize from; `None` is the save's own board.
    pub layout: Option<Layout>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            objective: Objective::Clear,
            exp_cell_type: None,
            obstruction: None,
            starts: 4,
            beam_width: 8,
            iterations: 12,
            saved_refinement_rounds: 2,
            refinement_rounds: 3,
            finalists: 4,
            trials: 64,
            seed: 1,
            layout: None,
        }
    }
}

impl Options {
    fn normalized(self) -> Self {
        Self {
            starts: self.starts.clamp(1, 32),
            beam_width: self.beam_width.clamp(4, 128),
            iterations: self.iterations.clamp(1, 120),
            saved_refinement_rounds: self.saved_refinement_rounds.min(30),
            refinement_rounds: self.refinement_rounds.min(30),
            finalists: self.finalists.clamp(1, 64),
            trials: self.trials.clamp(4, 8192),
            ..self
        }
    }
}

/// One start's strongest board.
#[derive(Clone, Debug, PartialEq)]
pub struct Start {
    pub start: usize,
    pub focus: Focus,
    pub layout: Layout,
    pub score: f64,
    /// What the start cost, in the coordinator's own units. The page reads the
    /// elapsed milliseconds; this port counts the layouts the start scored, so
    /// the scheduling stays a deterministic function of its inputs.
    pub elapsed_ms: f64,
    /// The strongest score when the start began, which the payoff reads.
    pub baseline_score: Option<f64>,
}

/// One layout the coordinator shares with the later starts.
#[derive(Clone, Debug, PartialEq)]
pub struct Elite {
    pub source_start: usize,
    pub layout: Layout,
    pub score: f64,
}

/// What the coordinator assigns one start (`jellySearchAssignment`).
#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    pub focus: Focus,
    pub iterations: usize,
    pub saved_refinement_rounds: usize,
    /// The exact-repair budget the `repair` focus sets (`activeExactNodeBudget`);
    /// this port reads its presence as the extra repair round's trigger.
    pub active_exact_node_budget: Option<usize>,
    /// The strongest score the earlier starts measured (`baselineScore`).
    pub baseline_score: Option<f64>,
    /// The starts whose rows this assignment read, in ascending order.
    pub source_starts: Vec<usize>,
    pub shared_elites: Vec<Elite>,
}

/// One simulated finalist.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub layout: Layout,
    pub proxy_score: f64,
    /// The finalist's board summary.
    pub operation: planner::TrialSet,
    pub objective_value: f64,
    pub confidence: Option<Confidence>,
    pub chance_beats_baseline: Option<f64>,
    /// The start that found the board; `None` is the saved board.
    pub start: Option<usize>,
    pub focus: Option<Focus>,
    pub simulated_score: f64,
}

/// One anchor that changed between the saved and the recommended board.
#[derive(Clone, Debug, PartialEq)]
pub struct Move {
    pub anchor: usize,
    pub before: Option<usize>,
    pub after: Option<usize>,
}

/// One optimizer run.
#[derive(Clone, Debug, PartialEq)]
pub struct Optimization {
    pub objective: Objective,
    pub exp_cell_type: Option<usize>,
    pub obstruction: usize,
    pub beam_width: usize,
    pub starts: Vec<Start>,
    pub baseline_layout: Layout,
    pub baseline_score: f64,
    pub baseline: planner::TrialSet,
    /// The baseline's own objective value, which each finalist is measured on.
    pub baseline_objective_value: f64,
    /// The finalists, ranked by their simulated objective.
    pub finalists: Vec<Row>,
    /// The recommendation; the saved board when no finalist measured better.
    pub recommended: Row,
    pub moves: Vec<Move>,
    pub note: String,
}

const NOTE: &str = "Every start builds full boards from composition seeds and the coordinator's shared elites, then keeps a diverse beam of placements, replacements and relocations across the build, saved-board and refinement rounds. The strongest finalists are simulated on the saved board's own seeds, and the recommendation is never worse than the board that was handed in.";

/// `jellyLayoutKey`: the anchor-to-type map as one comparable string.
pub fn layout_key(layout: &Layout) -> String {
    layout
        .iter()
        .map(|(anchor, cell_type)| format!("{anchor}:{cell_type}"))
        .collect::<Vec<_>>()
        .join("|")
}

/// `jellySelectSharedElites`: the strongest, sufficiently distinct leaders.
pub fn shared_elites(rows: &[Start], limit: usize) -> Vec<Elite> {
    let mut ranked: Vec<&Start> = rows.iter().filter(|row| row.score.is_finite()).collect();
    ranked.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.start.cmp(&b.start))
    });
    let mut selected: Vec<(&Start, BTreeSet<String>)> = Vec::new();
    let mut seen = BTreeSet::new();
    for row in ranked {
        let key = layout_key(&row.layout);
        if !seen.insert(key) {
            continue;
        }
        let cells: BTreeSet<String> = row
            .layout
            .iter()
            .map(|(anchor, cell_type)| format!("{anchor}:{cell_type}"))
            .collect();
        let distinct = selected.iter().all(|(_, other)| {
            let intersection = cells.intersection(other).count();
            let union = cells.len() + other.len() - intersection;
            1.0 - intersection as f64 / union.max(1) as f64 >= 0.1
        });
        if !distinct {
            continue;
        }
        selected.push((row, cells));
        if selected.len() >= limit {
            break;
        }
    }
    selected
        .into_iter()
        .map(|(row, _)| Elite {
            source_start: row.start,
            layout: row.layout.clone(),
            score: row.score,
        })
        .collect()
}

/// `jellySearchAssignment`: the focus and the elites one start receives.
///
/// Every start keeps the focus the page's rotation assigns, and a later start
/// may switch focus by the payoff each earlier focus bought per second: an
/// untried focus first, then the upper confidence bound over the tested ones.
/// The page reads the elapsed milliseconds; this port reads the start's own
/// score count, so the recommendation stays independent of the machine.
pub fn assignment(rows: &[Start], start: usize, options: &Options) -> Assignment {
    let mut focus = Focus::ALL[start % Focus::ALL.len()];
    if !start.is_multiple_of(4) && !rows.is_empty() {
        let mut stats: Vec<(Focus, usize, f64)> = Focus::ALL
            .iter()
            .map(|name| {
                let samples: Vec<&Start> = rows.iter().filter(|row| row.focus == *name).collect();
                let seconds: f64 = samples
                    .iter()
                    .map(|row| (row.elapsed_ms / 1000.0).max(0.001))
                    .sum();
                let gain: f64 = samples
                    .iter()
                    .map(|row| {
                        let baseline = row.baseline_score.unwrap_or(row.score);
                        if baseline.is_finite() && row.score.is_finite() {
                            (row.score - baseline).max(0.0)
                                / 1.0_f64.max(row.score.abs()).max(baseline.abs())
                        } else {
                            0.0
                        }
                    })
                    .sum();
                (*name, samples.len(), gain / seconds.max(0.001))
            })
            .collect();
        if let Some(untried) = stats.iter().find(|(_, count, _)| *count == 0) {
            focus = untried.0;
        } else {
            let scale = stats
                .iter()
                .map(|(_, _, payoff)| *payoff)
                .fold(1e-6_f64, f64::max);
            let exploration = (2.0 * ((rows.len() + 1) as f64).ln()).sqrt();
            stats.sort_by(|a, b| {
                let left = b.2 / scale + (exploration / b.1 as f64).sqrt();
                let right = a.2 / scale + (exploration / a.1 as f64).sqrt();
                left.total_cmp(&right).then_with(|| a.0.cmp(&b.0))
            });
            focus = stats[0].0;
        }
    }
    let mut iterations = options.iterations;
    let mut saved_refinement_rounds = options.saved_refinement_rounds;
    let mut active_exact_node_budget = None;
    if focus == Focus::Refine {
        let rounds = iterations.max(1);
        let moved = (rounds - 1)
            .min(rounds.div_ceil(4))
            .min(30 - saved_refinement_rounds);
        iterations = rounds - moved;
        saved_refinement_rounds += moved;
    } else if focus == Focus::Repair {
        active_exact_node_budget = Some(1024);
    }
    let mut source_starts: Vec<usize> = rows.iter().map(|row| row.start).collect();
    source_starts.sort_unstable();
    Assignment {
        focus,
        iterations,
        saved_refinement_rounds,
        active_exact_node_budget,
        baseline_score: rows
            .iter()
            .filter(|row| row.score.is_finite())
            .map(|row| row.score)
            .reduce(f64::max),
        source_starts,
        shared_elites: shared_elites(rows, SHARED_ELITES),
    }
}

/// `jellyLayoutMoves`: every anchor the recommendation changed.
pub fn moves(before: &Layout, after: &Layout) -> Vec<Move> {
    let mut anchors: BTreeSet<usize> = before.keys().copied().collect();
    anchors.extend(after.keys().copied());
    anchors
        .into_iter()
        .filter_map(|anchor| {
            let old = before.get(&anchor).copied();
            let new = after.get(&anchor).copied();
            (old != new).then_some(Move {
                anchor,
                before: old,
                after: new,
            })
        })
        .collect()
}

impl Optimization {
    /// The run a set of options describes, without progress reports.
    pub fn run(jelly: &Jelly, options: Options) -> Optimization {
        Self::run_with(jelly, options, |_| true).expect("a run that never cancels finishes")
    }

    /// The same run, reporting after every round and finalist. The callback
    /// returns whether to keep going; a stopped run reports `None`.
    pub fn run_with(
        jelly: &Jelly,
        options: Options,
        mut report: impl FnMut(Progress) -> bool,
    ) -> Option<Optimization> {
        let options = options.normalized();
        let obstruction = options
            .obstruction
            .unwrap_or(jelly.progress.obstruction)
            .min(71);
        let saved = options
            .layout
            .clone()
            .unwrap_or_else(|| jelly.layout.clone());
        let baseline_layout = if board::validate(jelly, &saved).is_ok() {
            saved
        } else {
            board::normalize(jelly, &saved).0
        };
        let mut search = Search::new(jelly, &options, obstruction);
        let baseline_score = search.score(&baseline_layout);
        let rounds =
            options.iterations + options.saved_refinement_rounds + options.refinement_rounds;
        let total = (options.starts * rounds + options.finalists + 2).max(1) as f64;
        let mut reporter = Reporter {
            report: &mut report,
            completed: 0.0,
            total,
        };

        let mut starts: Vec<Start> = Vec::new();
        for start in 0..options.starts {
            let assignment = assignment(&starts, start, &options);
            let baseline_score = starts
                .iter()
                .map(|row| row.score)
                .fold(baseline_score, f64::max);
            let leader = search.search_start(
                start,
                &assignment,
                &baseline_layout,
                baseline_score,
                options.seed,
                &mut reporter,
            )?;
            starts.push(leader);
        }
        let mut start_keys: BTreeSet<String> =
            starts.iter().map(|row| layout_key(&row.layout)).collect();
        start_keys.insert(layout_key(&baseline_layout));
        let mut leaders: Vec<(f64, Layout, Option<usize>, Option<Focus>)> = starts
            .iter()
            .map(|row| {
                (
                    row.score,
                    row.layout.clone(),
                    Some(row.start),
                    Some(row.focus),
                )
            })
            .collect();
        leaders.push((baseline_score, baseline_layout.clone(), None, None));
        leaders.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| layout_key(&a.1).cmp(&layout_key(&b.1)))
        });
        let mut seen = BTreeSet::new();
        let mut chosen: Vec<(f64, Layout, Option<usize>, Option<Focus>)> = leaders
            .into_iter()
            .filter(|(_, layout, _, _)| seen.insert(layout_key(layout)))
            .take(options.finalists.max(1))
            .collect();
        if !chosen
            .iter()
            .any(|(_, layout, _, _)| layout == &baseline_layout)
        {
            let baseline_entry = (baseline_score, baseline_layout.clone(), None, None);
            chosen.pop();
            chosen.push(baseline_entry);
        }

        if !reporter.step(format!("Simulating {} finalists", chosen.len())) {
            return None;
        }
        let baseline = planner::trial_set(
            jelly,
            &baseline_layout,
            options.trials,
            options.seed,
            options.objective,
            options.exp_cell_type,
        );
        let mut finalists: Vec<Row> = Vec::with_capacity(chosen.len());
        for (proxy_score, layout, start, focus) in chosen {
            let trial_set = planner::trial_set(
                jelly,
                &layout,
                options.trials,
                options.seed,
                options.objective,
                options.exp_cell_type,
            );
            let confidence = planner::paired_confidence(&trial_set.values, &baseline.values);
            finalists.push(Row {
                proxy_score,
                objective_value: planner::operation_value(
                    &trial_set.estimate,
                    options.objective,
                    options.exp_cell_type,
                ),
                simulated_score: planner::simulated_score(
                    &trial_set.estimate,
                    options.objective,
                    options.exp_cell_type,
                ),
                chance_beats_baseline: confidence
                    .as_ref()
                    .map(|bounds| bounds.chance_beats_baseline),
                confidence,
                operation: trial_set,
                start,
                focus,
                layout,
            });
            if !reporter.step("Simulating the finalists".to_string()) {
                return None;
            }
        }
        finalists.sort_by(|a, b| {
            b.simulated_score
                .total_cmp(&a.simulated_score)
                .then_with(|| b.proxy_score.total_cmp(&a.proxy_score))
                .then_with(|| layout_key(&a.layout).cmp(&layout_key(&b.layout)))
        });
        let baseline_objective_value =
            planner::operation_value(&baseline.estimate, options.objective, options.exp_cell_type);
        let baseline_value =
            planner::simulated_score(&baseline.estimate, options.objective, options.exp_cell_type);
        let recommended = finalists
            .iter()
            .find(|row| row.layout != baseline_layout && row.simulated_score >= baseline_value)
            .or_else(|| finalists.iter().find(|row| row.layout == baseline_layout))
            .or_else(|| finalists.first())
            .cloned()
            .expect("the baseline is always a finalist");
        let moves = moves(&baseline_layout, &recommended.layout);
        reporter.step("Layout optimization complete".to_string());

        Some(Optimization {
            objective: options.objective,
            exp_cell_type: options.exp_cell_type,
            obstruction,
            beam_width: options.beam_width,
            starts,
            baseline_layout,
            baseline_score,
            baseline_objective_value,
            baseline,
            finalists,
            recommended,
            moves,
            note: NOTE.to_string(),
        })
    }
}

/// The run's progress and its cancel point.
struct Reporter<'a> {
    report: &'a mut dyn FnMut(Progress) -> bool,
    completed: f64,
    total: f64,
}

impl Reporter<'_> {
    fn step(&mut self, message: String) -> bool {
        self.completed = (self.completed + 1.0 / self.total.max(1.0)).min(1.0);
        (self.report)(Progress {
            fraction: self.completed as f32,
            message,
        })
    }
}

/// One scored layout inside a start's beam.
#[derive(Clone, Debug)]
struct Entry {
    layout: Layout,
    score: f64,
}

/// The search's immutable board facts and its score cache.
struct Search<'a> {
    jelly: &'a Jelly,
    options: &'a Options,
    obstruction: usize,
    unlocked: Vec<bool>,
    open_slots: Vec<usize>,
    anchors_by_type: Vec<Vec<usize>>,
    one_slot_types: Vec<usize>,
    virus_limit: f64,
    scores: HashMap<String, f64>,
    /// How many layouts the whole run scored, the coordinator's elapsed measure.
    scored: usize,
}

impl<'a> Search<'a> {
    fn new(jelly: &'a Jelly, options: &'a Options, obstruction: usize) -> Self {
        let unlocked = board::unlocked_slots(jelly);
        let open_slots: Vec<usize> = (0..unlocked.len()).filter(|slot| unlocked[*slot]).collect();
        let types = jelly.units_owned();
        let anchors_by_type: Vec<Vec<usize>> = (0..types)
            .map(|cell_type| {
                (0..data().size())
                    .filter(|anchor| {
                        board::footprint(*anchor, cell_type).is_some_and(|slots| {
                            slots
                                .iter()
                                .all(|slot| unlocked.get(*slot).copied() == Some(true))
                        })
                    })
                    .collect()
            })
            .collect();
        let one_slot_types = (0..types)
            .filter(|cell_type| {
                board::footprint(0, *cell_type).is_some_and(|slots| slots.len() == 1)
            })
            .collect();
        Self {
            jelly,
            options,
            obstruction,
            unlocked,
            open_slots,
            anchors_by_type,
            one_slot_types,
            virus_limit: 1.0 + jelly.upgrade_quantity(15),
            scores: HashMap::new(),
            scored: 0,
        }
    }

    fn score(&mut self, layout: &Layout) -> f64 {
        let key = layout_key(layout);
        if let Some(score) = self.scores.get(&key) {
            return *score;
        }
        let score = surrogate::proxy_score(
            self.jelly,
            layout,
            self.options.objective.surrogate(),
            self.options.exp_cell_type,
            Some(self.obstruction),
        );
        self.scores.insert(key, score);
        self.scored += 1;
        score
    }

    fn viruses(&self, layout: &Layout) -> usize {
        layout.values().filter(|cell_type| **cell_type == 5).count()
    }

    fn slots(&self, anchor: usize, cell_type: usize) -> Option<Vec<usize>> {
        board::footprint(anchor, cell_type)
    }

    /// Add a cell without removing anything (`addWithoutReplacement`).
    fn add(
        &self,
        layout: &Layout,
        occupied: &BTreeMap<usize, (usize, usize)>,
        anchor: usize,
        cell_type: usize,
    ) -> Option<Layout> {
        if cell_type == 5 && self.viruses(layout) as f64 >= self.virus_limit {
            return None;
        }
        let slots = self.slots(anchor, cell_type)?;
        if slots
            .iter()
            .any(|slot| occupied.contains_key(slot) || self.unlocked.get(*slot) != Some(&true))
        {
            return None;
        }
        let mut next = layout.clone();
        next.insert(anchor, cell_type);
        Some(next)
    }

    /// Place a cell, removing whatever it overlaps (`placeWithReplacement`).
    fn replace(&self, layout: &Layout, anchor: usize, cell_type: usize) -> Option<Layout> {
        if cell_type >= self.jelly.units_owned() {
            return None;
        }
        let slots = self.slots(anchor, cell_type)?;
        if slots
            .iter()
            .any(|slot| self.unlocked.get(*slot) != Some(&true))
        {
            return None;
        }
        let mut next = layout.clone();
        next.retain(|other_anchor, _| {
            board::footprint(*other_anchor, layout[other_anchor])
                .is_none_or(|other| !other.iter().any(|slot| slots.contains(slot)))
        });
        if cell_type == 5 && self.viruses(&next) as f64 >= self.virus_limit {
            return None;
        }
        next.insert(anchor, cell_type);
        (next != *layout).then_some(next)
    }

    /// Fill every open slot with the best one-slot cell (`fillOpenSlots`).
    fn fill(&mut self, layout: &Layout) -> Layout {
        let mut filled = layout.clone();
        let mut occupied = board::occupancy(&filled);
        for index in 0..self.open_slots.len() {
            let slot = self.open_slots[index];
            if occupied.contains_key(&slot) {
                continue;
            }
            let mut best: Option<(f64, Layout)> = None;
            for index in 0..self.one_slot_types.len() {
                let cell_type = self.one_slot_types[index];
                let Some(next) = self.add(&filled, &occupied, slot, cell_type) else {
                    continue;
                };
                let score = self.score(&next);
                if best
                    .as_ref()
                    .is_none_or(|(best_score, _)| score > *best_score)
                {
                    best = Some((score, next));
                }
            }
            if let Some((_, next)) = best {
                filled = next;
                occupied.insert(slot, (slot, filled[&slot]));
            }
        }
        filled
    }

    /// `greedyCompositionSeed`: one full board from a type order and an anchor order.
    fn greedy(&mut self, type_order: &[usize], anchors: &[usize]) -> Layout {
        let mut layout = Layout::new();
        let mut changed = true;
        while changed && layout.len() < self.open_slots.len() {
            changed = false;
            let occupied = board::occupancy(&layout);
            for cell_type in type_order {
                if layout.len() >= self.open_slots.len() {
                    break;
                }
                for anchor in anchors {
                    if let Some(next) = self.add(&layout, &occupied, *anchor, *cell_type) {
                        layout = next;
                        changed = true;
                        break;
                    }
                }
            }
        }
        layout
    }

    /// The composition seeds one start builds from its own orders.
    fn composition_seeds(&mut self, start: usize) -> Vec<Layout> {
        let types: Vec<usize> = (0..self.jelly.units_owned()).collect();
        let damage_order = {
            let mut ranked = types.clone();
            ranked.sort_by(|a, b| {
                let rate = |cell_type: usize| {
                    let cells = &data().cells;
                    cells.base_damage[cell_type] / cells.base_cooldowns[cell_type].max(1.0)
                };
                rate(*b).total_cmp(&rate(*a)).then_with(|| a.cmp(b))
            });
            ranked
        };
        let footprint_order = {
            let mut ranked = types.clone();
            ranked.sort_by(|a, b| {
                data()
                    .footprint(*b)
                    .len()
                    .cmp(&data().footprint(*a).len())
                    .then_with(|| a.cmp(b))
            });
            ranked
        };
        let orders: Vec<Vec<usize>> = vec![
            damage_order,
            footprint_order,
            types.clone(),
            types.iter().rev().copied().collect(),
        ];
        let mut shuffled: Vec<usize> = self.open_slots.clone();
        let mut rng =
            Rng::new(self.options.seed ^ (start as u32).wrapping_mul(0x9e37_79b1) ^ 0x27d4_eb2d);
        for index in (1..shuffled.len()).rev() {
            let swap = random_int(&mut rng, 0.0, index as f64);
            shuffled.swap(index, swap);
        }
        let mut proximity_first: Vec<usize> = self.open_slots.clone();
        proximity_first.sort_by(|a, b| {
            PROXIMITY_ANCHORS
                .contains(b)
                .cmp(&PROXIMITY_ANCHORS.contains(a))
                .then_with(|| b.cmp(a))
        });
        let anchor_orders: Vec<Vec<usize>> =
            vec![self.open_slots.clone(), proximity_first, shuffled];
        let mut seeds = Vec::new();
        for order in &orders {
            for anchors in &anchor_orders {
                seeds.push(self.greedy(order, anchors));
            }
        }
        seeds
    }

    /// One start: its seeds, then its rounds of mutations and re-fills.
    fn search_start(
        &mut self,
        start: usize,
        assignment: &Assignment,
        baseline: &Layout,
        baseline_score: f64,
        seed: u32,
        reporter: &mut Reporter<'_>,
    ) -> Option<Start> {
        let mut candidates: Vec<Entry> = Vec::new();
        let mut seen = BTreeSet::new();
        let mut push = |search: &mut Search, layout: Layout, candidates: &mut Vec<Entry>| {
            let key = layout_key(&layout);
            if !seen.insert(key) {
                return;
            }
            let score = search.score(&layout);
            candidates.push(Entry { layout, score });
        };
        push(self, baseline.clone(), &mut candidates);
        for elite in &assignment.shared_elites {
            push(self, elite.layout.clone(), &mut candidates);
        }
        for layout in self.composition_seeds(start) {
            push(self, layout, &mut candidates);
        }
        let mut beam = select_diverse(candidates, self.options.beam_width);
        let rounds = assignment.iterations
            + assignment.saved_refinement_rounds
            + self.options.refinement_rounds;
        for round in 0..rounds {
            let refilling = round >= assignment.iterations
                && round < assignment.iterations + assignment.saved_refinement_rounds;
            let mut rng = Rng::new(
                seed ^ (start as u32).wrapping_mul(0x85eb_ca6b)
                    ^ (round as u32).wrapping_mul(0x9e37_79b1),
            );
            let mut pool = beam.clone();
            for entry in &beam {
                if entry.layout.is_empty() {
                    continue;
                }
                let anchors: Vec<usize> = entry.layout.keys().copied().collect();
                for _ in 0..3 {
                    let anchor = anchors[random_int(&mut rng, 0.0, anchors.len() as f64 - 1.0)];
                    let cell_type =
                        random_int(&mut rng, 0.0, self.jelly.units_owned() as f64 - 1.0);
                    if let Some(next) = self.replace(&entry.layout, anchor, cell_type) {
                        let score = self.score(&next);
                        pool.push(Entry {
                            layout: next,
                            score,
                        });
                    }
                }
                for _ in 0..2 {
                    let anchor = anchors[random_int(&mut rng, 0.0, anchors.len() as f64 - 1.0)];
                    let cell_type = entry.layout[&anchor];
                    let legal = &self.anchors_by_type[cell_type];
                    if legal.is_empty() {
                        continue;
                    }
                    let target = legal[random_int(&mut rng, 0.0, legal.len() as f64 - 1.0)];
                    if target == anchor {
                        continue;
                    }
                    let removed = board::remove(&entry.layout, anchor);
                    if let Some(next) =
                        self.add(&removed, &board::occupancy(&removed), target, cell_type)
                    {
                        let score = self.score(&next);
                        pool.push(Entry {
                            layout: next,
                            score,
                        });
                    }
                }
                if refilling || assignment.active_exact_node_budget.is_some() {
                    let mut refilled = entry.layout.clone();
                    let remove_count = 2
                        + random_int(&mut rng, 0.0, 1.0)
                        + usize::from(assignment.active_exact_node_budget.is_some());
                    let mut shuffled = anchors.clone();
                    for index in (1..shuffled.len()).rev() {
                        let swap = random_int(&mut rng, 0.0, index as f64);
                        shuffled.swap(index, swap);
                    }
                    for anchor in shuffled.into_iter().take(remove_count) {
                        refilled = board::remove(&refilled, anchor);
                    }
                    let filled = self.fill(&refilled);
                    if filled != entry.layout {
                        let score = self.score(&filled);
                        pool.push(Entry {
                            layout: filled,
                            score,
                        });
                    }
                }
            }
            beam = select_diverse(pool, self.options.beam_width);
            if !reporter.step(format!(
                "Start {} of {} · {} · round {} of {}",
                start + 1,
                self.options.starts,
                assignment.focus.label(),
                round + 1,
                rounds
            )) {
                return None;
            }
        }
        beam.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| layout_key(&a.layout).cmp(&layout_key(&b.layout)))
        });
        let leader = beam.into_iter().next().expect("a start always has a beam");
        Some(Start {
            start,
            focus: assignment.focus,
            layout: leader.layout,
            score: leader.score,
            elapsed_ms: self.scored as f64,
            baseline_score: Some(baseline_score),
        })
    }
}

/// `selectDiverseBy`: strongest one per composition, then the rest by score.
fn select_diverse(mut candidates: Vec<Entry>, limit: usize) -> Vec<Entry> {
    candidates.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| layout_key(&a.layout).cmp(&layout_key(&b.layout)))
    });
    let mut selected = Vec::with_capacity(limit.min(candidates.len()));
    let mut selected_keys = BTreeSet::new();
    let mut compositions = BTreeSet::new();
    let target = limit.min(4.max(limit.div_ceil(2)));
    for entry in &candidates {
        if compositions.insert(composition_key(&entry.layout)) {
            selected.push(entry.clone());
            selected_keys.insert(layout_key(&entry.layout));
            if selected.len() >= target {
                break;
            }
        }
    }
    for entry in candidates {
        if selected.len() >= limit {
            break;
        }
        if selected_keys.insert(layout_key(&entry.layout)) {
            selected.push(entry);
        }
    }
    selected
}

/// The cell counts, joined as the page's `compositionKey` does.
fn composition_key(layout: &Layout) -> String {
    let mut counts = [0_usize; 8];
    for cell_type in layout.values() {
        if *cell_type < counts.len() {
            counts[*cell_type] += 1;
        }
    }
    counts
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board(entries: &[(usize, usize)]) -> Layout {
        entries.iter().copied().collect()
    }

    #[test]
    fn shared_elites_skip_duplicates_and_near_copies() {
        // Nineteen of twenty cells shared is inside the 10% distance floor.
        let first: Layout = (0..20).map(|anchor| (anchor, 0)).collect();
        let same = first.clone();
        let near: Layout = (0..19).map(|anchor| (anchor, 0)).chain([(99, 0)]).collect();
        let far: Layout = (100..120).map(|anchor| (anchor, 1)).collect();
        let rows = vec![
            Start {
                start: 1,
                focus: Focus::Explore,
                layout: far,
                score: 5.0,
                elapsed_ms: 1000.0,
                baseline_score: None,
            },
            Start {
                start: 0,
                focus: Focus::Explore,
                layout: first,
                score: 9.0,
                elapsed_ms: 1000.0,
                baseline_score: None,
            },
            Start {
                start: 2,
                focus: Focus::Refine,
                layout: same,
                score: 8.0,
                elapsed_ms: 1000.0,
                baseline_score: None,
            },
            Start {
                start: 3,
                focus: Focus::Repair,
                layout: near,
                score: 7.0,
                elapsed_ms: 1000.0,
                baseline_score: None,
            },
        ];
        let elites = shared_elites(&rows, SHARED_ELITES);
        assert_eq!(elites.len(), 2, "the near copy overlaps too much");
        assert_eq!(elites[0].source_start, 0);
        assert_eq!(elites[1].source_start, 1);
    }

    #[test]
    fn assignments_rotate_and_refine_trades_rounds() {
        let explore = assignment(&[], 0, &Options::default());
        assert_eq!(explore.focus, Focus::Explore);
        assert_eq!(explore.iterations, Options::default().iterations);
        let refine = assignment(&[], 1, &Options::default());
        assert_eq!(refine.focus, Focus::Refine);
        assert!(refine.iterations < explore.iterations);
        assert_eq!(
            refine.iterations + refine.saved_refinement_rounds,
            explore.iterations + explore.saved_refinement_rounds
        );
        assert_eq!(
            assignment(&[], 4, &Options::default()).focus,
            Focus::Explore
        );
    }

    #[test]
    fn moves_report_every_changed_anchor_in_slot_order() {
        let before = board(&[(1, 0), (2, 1), (3, 2)]);
        let after = board(&[(2, 1), (3, 5), (4, 0)]);
        let moves = moves(&before, &after);
        assert_eq!(
            moves
                .iter()
                .map(|row| (row.anchor, row.before, row.after))
                .collect::<Vec<_>>(),
            vec![
                (1, Some(0), None),
                (3, Some(2), Some(5)),
                (4, None, Some(0))
            ]
        );
    }
}
