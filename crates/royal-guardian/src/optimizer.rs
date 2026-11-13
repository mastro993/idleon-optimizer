//! The outpost optimizer: a bounded beam search over the moves a kingdom can
//! make, scored by what the account produces next.
//!
//! Ported from `optimizeRoyalGuardian`, `_candidateMoves`, `kingdomMetrics`,
//! `rankObjective`, `_compareObjectives`, `_archiveAdd` and the caps helpers of
//! `js/stats/systems/w7/royal-guardian-optimizer.js`.
//!
//! Upstream offers fourteen objectives, several of which need inputs this build
//! does not model (an Armory target purchase, a currency portfolio, the reset
//! projection's collection commitments). The port offers the three the kingdom
//! metrics carry on their own: the currency the linked nodes pay per hour, the
//! rank EXP the stationed units and militia earn per hour, and the balanced score
//! that sums both with the outposts' reach.
//!
//! The search is deterministic and bounded: each iteration applies every legal
//! move to every state on the beam, keeps the best `beam` states by the
//! objective, and stops after `iterations` rounds or `candidates` evaluations.
//! It reports progress after every evaluation and stops when the caller asks, so
//! a section can cancel it and keep the result it already showed. A returned
//! recommendation is replayed through [`sandbox::apply`], so its moves are the
//! ones a player can make and its state is one the game accepts.

use super::*;
use idleon_common::{Control, Progress};
use std::cmp::Ordering;

/// What the optimizer maximizes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Objective {
    /// Currency the Normal outposts' linked nodes pay per hour.
    #[default]
    Resources,
    /// Command, Military and Purity EXP the militia and units earn per hour.
    Rank,
    /// Both metrics plus every outpost's reach.
    Balanced,
}

impl Objective {
    /// Every objective, in the order the picker lists them.
    pub const ALL: [Objective; 3] = [Objective::Resources, Objective::Rank, Objective::Balanced];

    /// The word the picker prints.
    pub fn label(self) -> &'static str {
        match self {
            Objective::Resources => "Resources",
            Objective::Rank => "Rank EXP",
            Objective::Balanced => "Balanced",
        }
    }

    /// The stable id the picker selects by.
    pub fn id(self) -> &'static str {
        match self {
            Objective::Resources => "resources",
            Objective::Rank => "rank",
            Objective::Balanced => "balanced",
        }
    }

    /// The objective an id names.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|entry| entry.id() == id)
    }

    /// The score a layout earns.
    fn score(self, metrics: &Metrics) -> f64 {
        match self {
            Objective::Resources => metrics.resources_per_hour,
            Objective::Rank => metrics.rank_exp_per_hour,
            Objective::Balanced => {
                metrics.resources_per_hour + metrics.rank_exp_per_hour + metrics.range
            }
        }
    }
}

/// How far a run may search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Caps {
    /// Beam rounds.
    pub iterations: usize,
    /// Move applications one run may evaluate.
    pub candidates: usize,
    /// States kept after each round.
    pub beam: usize,
}

impl Default for Caps {
    fn default() -> Self {
        Self {
            iterations: 5,
            candidates: 150,
            beam: 3,
        }
    }
}

impl Caps {
    /// The caps with every field inside the range a run supports.
    pub fn bounded(self) -> Self {
        Self {
            iterations: self.iterations.clamp(1, 50),
            candidates: self.candidates.clamp(1, 5_000),
            beam: self.beam.clamp(1, 50),
        }
    }
}

/// What a run maximizes and the save it reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub royal: RoyalGuardian,
    pub objective: Objective,
    pub caps: Caps,
}

impl Request {
    /// A run over a saved state with the default caps.
    pub fn new(royal: &RoyalGuardian, objective: Objective) -> Self {
        Self {
            royal: royal.clone(),
            objective,
            caps: Caps::default(),
        }
    }
}

/// What a kingdom produces per hour: upstream `kingdomMetrics`, narrowed to the
/// three figures the port's objectives read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Metrics {
    pub resources_per_hour: f64,
    pub rank_exp_per_hour: f64,
    /// Every built outpost's reach, summed.
    pub range: f64,
}

/// One recommended layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// The moves the search applied, in order.
    pub moves: Vec<Move>,
    /// The same layout as the readable edits that reach it.
    pub layout: Vec<Move>,
    pub metrics: Metrics,
    /// The layout itself, which a section may compare with the save.
    pub state: RoyalGuardian,
}

/// What one run found.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub objective: Objective,
    /// The scored layouts, best first.
    pub candidates: Vec<Candidate>,
    /// The starting state's own figures, so a recommendation can be compared.
    pub start: Metrics,
    pub evaluated: usize,
    /// Whether the caps cut the search short.
    pub truncated: bool,
}

impl Outcome {
    /// The best layout, when any move improved the starting state.
    pub fn best(&self) -> Option<&Candidate> {
        self.candidates.first()
    }
}

/// One state on the beam.
#[derive(Clone, Debug)]
struct Item {
    sandbox: Sandbox,
    moves: Vec<Move>,
    metrics: Metrics,
}

impl RoyalGuardian {
    /// Every figure the optimizer scores with: upstream `kingdomMetrics`.
    pub fn metrics(&self) -> Metrics {
        let inputs = self.collection_inputs();
        let global = self.global_collection_rate_with(&inputs);
        let support = self.support_counts();
        let assignments = self.assignment_counts();
        let mut resources = 0.0;
        let mut rank = 0.0;
        let mut range = 0.0;
        for map in 0..self.maps.len() {
            if !self.outpost(map).built {
                continue;
            }
            range += self.range(map);
            let kind = self.outpost_type(map);
            let support = support.get(map).copied().unwrap_or(0);
            let highest = self.highest_bar(map);
            for bar in 0..5 {
                let contributors = match bar {
                    0 | 1 => self.units_by_type(map)[if bar == 0 { 1 } else { 3 }],
                    _ => assignments
                        .get(map)
                        .map(|kinds| kinds[bar + 3] as f64)
                        .unwrap_or(0.0),
                };
                let mut value = self.bar_exp_rate_given(bar, map, support) * contributors;
                if bar == highest && self.armory_level(17) >= 1.0 {
                    value += self.bar_exp_rate_given(3, map, support)
                        * (assignments.get(map).map(|kinds| kinds[4]).unwrap_or(0) as f64)
                        / 2.0;
                }
                rank += value;
            }
            if kind == 1.0 {
                continue;
            }
            let rate = self.outpost_collection_rate_given(map, &global).value;
            for connection in self.outpost_connections(map) {
                if let Endpoint::Resource(resource) = connection.endpoint
                    && kind != 2.0
                {
                    resources += rate * (1.0 + self.resource_grade(resource) * 25.0 / 100.0);
                }
            }
        }
        Metrics {
            resources_per_hour: resources,
            rank_exp_per_hour: rank,
            range,
        }
    }

    /// How many Support outposts link each map, in one pass over every row.
    fn support_counts(&self) -> Vec<usize> {
        let mut counts = vec![0usize; self.maps.len()];
        for map in 0..self.maps.len() {
            if !self.outpost(map).built || self.outpost_type(map) != 1.0 {
                continue;
            }
            for connection in self.outpost_connections(map) {
                if let Endpoint::Map(target) = connection.endpoint
                    && let Some(slot) = counts.get_mut(target)
                {
                    *slot += 1;
                }
            }
        }
        counts
    }

    /// How many assignments of each kind sit on each outpost, in one pass.
    fn assignment_counts(&self) -> Vec<[usize; 8]> {
        let mut counts = vec![[0usize; 8]; self.maps.len()];
        for assignment in self.militia_assignments() {
            if !(4..=7).contains(&assignment.kind) {
                continue;
            }
            if let Some(slot) = counts
                .get_mut(assignment.map.max(0) as usize)
                .and_then(|kinds| kinds.get_mut(assignment.kind as usize))
            {
                *slot += 1;
            }
        }
        counts
    }

    /// The bar with the highest rank: upstream `_highestOutpostRankBar`.
    fn highest_bar(&self, map: usize) -> usize {
        let mut highest = 0;
        for bar in 1..5 {
            if self.rank(map, bar) > self.rank(map, highest) {
                highest = bar;
            }
        }
        highest
    }

    /// The search: every legal move from every state on the beam, best first.
    /// Returns `None` when the caller stops it.
    pub fn optimize(
        &self,
        request: &Request,
        mut report: impl FnMut(Progress) -> Control,
    ) -> Option<Outcome> {
        let caps = request.caps.bounded();
        let objective = request.objective;
        let start = self.metrics();
        let root = Sandbox::new(self);
        let mut beam = vec![Item {
            sandbox: root.clone(),
            moves: Vec::new(),
            metrics: start,
        }];
        let mut archive: Vec<Item> = vec![beam[0].clone()];
        let mut evaluated = 0usize;
        let mut truncated = false;
        'search: for _ in 0..caps.iterations {
            let mut pool: Vec<Item> = Vec::new();
            let mut capped = false;
            for item in &beam {
                for change in sandbox::candidates(&item.sandbox) {
                    if evaluated >= caps.candidates {
                        truncated = true;
                        capped = true;
                        break;
                    }
                    let Ok(next) = sandbox::apply(&item.sandbox, &change) else {
                        continue;
                    };
                    evaluated += 1;
                    let mut moves = item.moves.clone();
                    moves.push(change);
                    let metrics = next.state().metrics();
                    pool.push(Item {
                        sandbox: next,
                        moves,
                        metrics,
                    });
                    let fraction = evaluated as f32 / caps.candidates.max(1) as f32;
                    if report(Progress {
                        fraction: fraction.min(1.0),
                        message: format!("Evaluating layout {evaluated} of {}", caps.candidates),
                    }) == Control::Stop
                    {
                        return None;
                    }
                }
            }
            if pool.is_empty() {
                break;
            }
            pool.sort_by(|left, right| compare(objective, left, right));
            for item in &pool {
                keep(&mut archive, objective, item.clone());
            }
            if pool.len() > caps.beam {
                truncated = true;
            }
            pool.truncate(caps.beam);
            beam = pool;
            if capped {
                break 'search;
            }
        }
        archive.sort_by(|left, right| compare(objective, left, right));
        archive.dedup_by(|left, right| left.sandbox.state() == right.sandbox.state());
        archive.truncate(5);
        let candidates = archive
            .iter()
            .map(|item| Candidate {
                moves: item.moves.clone(),
                layout: sandbox::changes(self, item.sandbox.state()),
                metrics: item.metrics,
                state: item.sandbox.state().clone(),
            })
            .filter(|candidate| !candidate.moves.is_empty())
            .collect();
        report(Progress {
            fraction: 1.0,
            message: format!("Scored {} layouts", evaluated),
        });
        Some(Outcome {
            objective,
            candidates,
            start,
            evaluated,
            truncated,
        })
    }
}

/// Keeps the best few states the search has seen, so a round that improves one
/// objective does not discard the state that led to the final result.
fn keep(archive: &mut Vec<Item>, objective: Objective, item: Item) {
    const ARCHIVE: usize = 16;
    if archive
        .iter()
        .any(|kept| kept.sandbox.state() == item.sandbox.state())
    {
        return;
    }
    archive.push(item);
    archive.sort_by(|left, right| compare(objective, left, right));
    archive.truncate(ARCHIVE);
}

/// Best objective first, then the shorter move list, then the move names.
fn compare(objective: Objective, left: &Item, right: &Item) -> Ordering {
    objective
        .score(&right.metrics)
        .total_cmp(&objective.score(&left.metrics))
        .then_with(|| left.moves.len().cmp(&right.moves.len()))
        .then_with(|| labels(&left.moves).cmp(&labels(&right.moves)))
}

/// A move list's stable text, for the final tie-break.
fn labels(moves: &[Move]) -> String {
    moves
        .iter()
        .map(Move::label)
        .collect::<Vec<String>>()
        .join("|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_caps_stay_inside_the_search_budget() {
        let caps = Caps {
            iterations: 0,
            candidates: 0,
            beam: 0,
        }
        .bounded();
        assert_eq!(caps.iterations, 1);
        assert_eq!(caps.candidates, 1);
        assert_eq!(caps.beam, 1);
    }

    #[test]
    fn an_objective_round_trips_through_its_id() {
        for objective in Objective::ALL {
            assert_eq!(Objective::from_id(objective.id()), Some(objective));
        }
        assert_eq!(Objective::from_id("nope"), None);
    }
}
