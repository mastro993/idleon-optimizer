//! The enchant simulator: a seeded Monte Carlo over the forty collectibles.
//!
//! Ported from `js/workers/enchant-sim-worker.js` and the `_renderEnchantSim`
//! controls in `jar-calc.html`: every simulation starts from the found
//! collectibles at one level, distributes `enchants` rolls, optionally rolling
//! for a focus first, and reports the spread each simulation reached. The
//! upstream worker draws from `Math.random()`, so it cannot repeat a run; here
//! the generator is seeded, which is what makes the spread reproducible.
//!
//! Progress and stop follow the protocol every long run in the app speaks
//! ([`idleon_common::Progress`]/[`idleon_common::Control`]), so the section runs
//! it through the shared task runner.

use crate::{COLLECTIBLES, collectible_name};
use idleon_common::{Control, Progress};
use serde::{Deserialize, Serialize};

/// How many simulations run between two progress reports, the worker's `BATCH`.
const BATCH: u32 = 500;

/// The simulator's focus policy, the upstream `esim-strategy` select.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Strategy {
    /// Every enchant picks a uniformly random found collectible.
    #[default]
    None,
    /// Every enchant rolls for the selected focus first.
    Fixed,
    /// Every enchant focuses the target furthest behind its goal, falling back
    /// to the selected focus once every target is met.
    Smart,
}

impl Strategy {
    /// The three strategies, in the select's order.
    pub const ALL: [Strategy; 3] = [Strategy::None, Strategy::Fixed, Strategy::Smart];

    /// The id the page and a saved project use.
    pub fn id(self) -> &'static str {
        match self {
            Strategy::None => "none",
            Strategy::Fixed => "fixed",
            Strategy::Smart => "smart",
        }
    }

    /// The select's label.
    pub fn label(self) -> &'static str {
        match self {
            Strategy::None => "No Focus",
            Strategy::Fixed => "Fixed Focus",
            Strategy::Smart => "Smart Focus",
        }
    }

    /// The strategy an id names, as the select reports it.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|strategy| strategy.id() == id)
    }
}

/// Everything a simulation reads: the same inputs and seed reproduce the same
/// spread exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct SimInputs {
    /// `collectibleLevels`: the forty saved levels, one per collectible.
    pub levels: [u32; COLLECTIBLES],
    /// `enchCount`: how many enchants each simulation distributes.
    pub enchants: u32,
    /// `simCount`: how many independent simulations to run.
    pub sims: u32,
    pub strategy: Strategy,
    /// `focusIdx`: the fixed focus, or Smart's fallback; `None` is upstream `-1`.
    pub focus: Option<usize>,
    /// The target level per collectible index, in ascending index order.
    pub targets: Vec<(usize, u32)>,
    /// The generator's seed.
    pub seed: u64,
}

impl SimInputs {
    /// `foundList`: the collectibles the save has found, in save order.
    pub fn found(&self) -> Vec<usize> {
        (0..COLLECTIBLES)
            .filter(|index| self.levels[*index] >= 1)
            .collect()
    }

    /// `focusChance`: `5 / (found + 3)`, the roll that lands an enchant on the
    /// focus. Zero when nothing has been found.
    pub fn focus_chance(&self) -> f64 {
        match self.found().len() {
            0 => 0.0,
            found => 5.0 / (found as f64 + 3.0),
        }
    }

    /// The targets a run can hit: found collectibles with a positive goal.
    fn target_list(&self) -> Vec<(usize, u32)> {
        self.targets
            .iter()
            .copied()
            .filter(|(index, target)| {
                *index < COLLECTIBLES && *target > 0 && self.levels[*index] >= 1
            })
            .collect()
    }
}

/// One found collectible's simulated spread.
#[derive(Clone, Debug, PartialEq)]
pub struct Spread {
    pub index: usize,
    pub name: &'static str,
    /// `spreadSums[index] / simCount`: the average enchant levels gained.
    pub average: f64,
    pub min: u32,
    pub max: u32,
    /// The level the collectible was aimed at, or zero with no target.
    pub target: u32,
    /// The share of simulations that reached `target`, with a target set.
    pub hit: Option<f64>,
}

/// One finished run.
#[derive(Clone, Debug, PartialEq)]
pub struct SimResult {
    pub strategy: Strategy,
    pub enchants: u32,
    pub sims: u32,
    /// The selected focus, which Fixed always uses and Smart falls back to.
    pub focus: Option<usize>,
    /// The found collectibles, in save order.
    pub spread: Vec<Spread>,
    /// `allHit / simCount`, when at least one target was set.
    pub all_hit: Option<f64>,
    /// The seed the spread came from.
    pub seed: u64,
}

impl SimResult {
    /// The average total enchant levels across the spread, which the odds table
    /// reads (the page's `avgTotalEnchLvs`).
    pub fn average_total(&self) -> f64 {
        self.spread.iter().map(|entry| entry.average).sum()
    }

    /// The strategy's label, with a fixed focus named.
    pub fn strategy_label(&self) -> String {
        match self.strategy {
            Strategy::None => Strategy::None.label().to_owned(),
            Strategy::Fixed => match self.focus {
                Some(index) => format!("{}: {}", Strategy::Fixed.label(), collectible_name(index)),
                None => Strategy::Fixed.label().to_owned(),
            },
            Strategy::Smart => Strategy::Smart.label().to_owned(),
        }
    }
}

/// Run `inputs`, reporting progress every [`BATCH`] simulations.
///
/// Returns `None` when the reporter asks the run to stop; the caller keeps the
/// result it already showed.
pub fn simulate(
    inputs: &SimInputs,
    mut report: impl FnMut(Progress) -> Control,
) -> Option<SimResult> {
    let found = inputs.found();
    let targets = inputs.target_list();
    // The page only ever offers a found collectible as the focus; a config that
    // names one the save has not found simulates as if no focus were picked.
    let fallback = inputs
        .focus
        .filter(|index| *index < COLLECTIBLES && found.contains(index));
    let chance = inputs.focus_chance();
    let sims = inputs.sims.max(1);
    // Nothing found means nothing to enchant: the page refuses to run rather
    // than distributing rolls over an empty list.
    if found.is_empty() {
        return Some(SimResult {
            strategy: inputs.strategy,
            enchants: inputs.enchants,
            sims,
            focus: fallback,
            spread: Vec::new(),
            all_hit: None,
            seed: inputs.seed,
        });
    }
    let mut sums = [0u64; COLLECTIBLES];
    let mut mins = [u32::MAX; COLLECTIBLES];
    let mut maxs = [0u32; COLLECTIBLES];
    let mut hits = [0u32; COLLECTIBLES];
    let mut all_hit = 0u32;
    let mut rng = Rng(inputs.seed);
    let mut done = 0;
    while done < sims {
        let end = (done + BATCH).min(sims);
        for _ in done..end {
            let mut levels = [0u32; COLLECTIBLES];
            for index in &found {
                levels[*index] = 1;
            }
            for _ in 0..inputs.enchants {
                let mut focus = None;
                match inputs.strategy {
                    Strategy::Smart => {
                        let mut worst = f64::INFINITY;
                        for (index, target) in &targets {
                            let level = levels[*index].saturating_sub(1);
                            if level < *target {
                                let ratio = f64::from(level) / f64::from(*target);
                                if ratio < worst {
                                    worst = ratio;
                                    focus = Some(*index);
                                }
                            }
                        }
                        // Every target met, or none set: the dropdown stands in.
                        if focus.is_none() {
                            focus = fallback;
                        }
                    }
                    Strategy::Fixed => focus = fallback,
                    Strategy::None => {}
                }
                // The roll happens only when there is a focus, exactly as the
                // worker's `curFocus >= 0 && Math.random() < focusChance` does.
                let target = match focus {
                    Some(index) if rng.unit() < chance => index,
                    _ => pick(&mut rng, &found),
                };
                levels[target] = levels[target].saturating_add(1);
            }
            for index in &found {
                let enchant = levels[*index].saturating_sub(1);
                sums[*index] += u64::from(enchant);
                mins[*index] = mins[*index].min(enchant);
                maxs[*index] = maxs[*index].max(enchant);
            }
            if !targets.is_empty() {
                let mut every = true;
                for (index, target) in &targets {
                    if levels[*index].saturating_sub(1) >= *target {
                        hits[*index] += 1;
                    } else {
                        every = false;
                    }
                }
                if every {
                    all_hit += 1;
                }
            }
        }
        done = end;
        let progress = Progress {
            fraction: done as f32 / sims as f32,
            message: format!("{done} / {sims} simulations"),
        };
        if report(progress) == Control::Stop {
            return None;
        }
    }
    let share = |count: u32| f64::from(count) / f64::from(sims);
    Some(SimResult {
        strategy: inputs.strategy,
        enchants: inputs.enchants,
        sims,
        focus: fallback,
        spread: found
            .iter()
            .map(|index| {
                let target = targets
                    .iter()
                    .find(|(aimed, _)| aimed == index)
                    .map_or(0, |(_, target)| *target);
                Spread {
                    index: *index,
                    name: collectible_name(*index),
                    average: sums[*index] as f64 / f64::from(sims),
                    min: mins[*index],
                    max: maxs[*index],
                    target,
                    hit: (target > 0).then(|| share(hits[*index])),
                }
            })
            .collect(),
        all_hit: (!targets.is_empty()).then(|| share(all_hit)),
        seed: inputs.seed,
    })
}

/// A uniformly random found collectible, as the worker's
/// `foundList[Math.floor(Math.random() * foundList.length)]` picks one.
fn pick(rng: &mut Rng, found: &[usize]) -> usize {
    let index = (rng.unit() * found.len() as f64) as usize;
    found[index.min(found.len() - 1)]
}

/// `splitmix64`: a seeded generator, so the same inputs reproduce the same
/// spread on every platform and every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A draw in `[0, 1)`, the range `Math.random()` covers.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cavern with six found collectibles, the harness save's own spread.
    fn inputs(seed: u64) -> SimInputs {
        let mut levels = [0u32; COLLECTIBLES];
        for (index, level) in [(0, 3), (1, 2), (3, 4), (17, 2), (28, 1), (36, 5)] {
            levels[index] = level;
        }
        SimInputs {
            levels,
            enchants: 50,
            sims: 200,
            strategy: Strategy::None,
            focus: None,
            targets: Vec::new(),
            seed,
        }
    }

    fn run(inputs: &SimInputs) -> SimResult {
        simulate(inputs, |_| Control::Continue).expect("a run without reports never stops")
    }

    #[test]
    fn a_seeded_run_reproduces_its_spread_exactly() {
        let first = run(&inputs(7));
        let second = run(&inputs(7));
        assert_eq!(first, second);
        let other = run(&inputs(8));
        assert_ne!(
            first.spread, other.spread,
            "a different seed drew the same spread"
        );
    }

    #[test]
    fn every_enchant_lands_on_a_found_collectible() {
        let result = run(&inputs(1));
        let total: f64 = result.average_total();
        assert!(
            (total - 50.0).abs() < 1e-9,
            "the spread spends {total} enchants, not 50"
        );
        assert_eq!(result.spread.len(), 6);
        for entry in &result.spread {
            assert!(inputs(1).levels[entry.index] >= 1);
            assert!(
                entry.max <= 50,
                "one collectible took {} enchants",
                entry.max
            );
            assert!(entry.min <= entry.max);
        }
        // Every enchant starts from one level, so no simulation reports fewer
        // than the enchants it was given across the spread.
        assert_eq!(result.sims, 200);
    }

    #[test]
    fn a_fixed_focus_takes_most_of_the_enchants() {
        let mut with_focus = inputs(3);
        with_focus.strategy = Strategy::Fixed;
        with_focus.focus = Some(36);
        let result = run(&with_focus);
        let focused = result
            .spread
            .iter()
            .find(|entry| entry.index == 36)
            .expect("the focus is found");
        assert!(
            focused.average > 20.0,
            "a 5/9 focus chance left only {} enchants",
            focused.average
        );
        for entry in result.spread.iter().filter(|entry| entry.index != 36) {
            assert!(
                entry.average < focused.average,
                "{} beat the focus",
                entry.name
            );
        }
    }

    #[test]
    fn smart_focus_feeds_the_collectible_furthest_behind_its_target() {
        let mut smart = inputs(5);
        smart.strategy = Strategy::Smart;
        smart.enchants = 120;
        smart.targets = vec![(0, 8), (1, 8)];
        let result = run(&smart);
        assert!(
            result.all_hit.is_some_and(|share| share > 0.9),
            "only {:?} of the runs hit both targets",
            result.all_hit
        );
        for entry in result.spread.iter().filter(|entry| entry.target > 0) {
            assert!(
                entry.hit.is_some_and(|share| share > 0.9),
                "{} hit its target in only {:?} of the runs",
                entry.name,
                entry.hit
            );
        }
    }

    #[test]
    fn a_target_reports_its_hit_share_and_the_untargeted_share_is_none() {
        let mut targeted = inputs(11);
        targeted.targets = vec![(0, 20), (3, 20)];
        let result = run(&targeted);
        assert!(
            result
                .all_hit
                .is_some_and(|share| (0.0..=1.0).contains(&share))
        );
        for entry in &result.spread {
            match entry.target {
                0 => assert_eq!(entry.hit, None),
                _ => assert!(
                    entry.hit.is_some_and(|share| (0.0..=1.0).contains(&share)),
                    "{} has no hit share",
                    entry.name
                ),
            }
        }
    }

    #[test]
    fn a_focus_on_a_collectible_the_save_has_not_found_is_ignored() {
        let mut unfound = inputs(9);
        unfound.strategy = Strategy::Fixed;
        unfound.focus = Some(2);
        let result = run(&unfound);
        assert_eq!(result.focus, None);
        let total: f64 = result.average_total();
        assert!((total - 50.0).abs() < 1e-9, "the spread spends {total}");
        assert_eq!(result.spread.len(), 6);
    }

    #[test]
    fn a_cavern_with_no_collectible_returns_an_empty_spread() {
        let empty = SimInputs {
            levels: [0; COLLECTIBLES],
            ..inputs(1)
        };
        assert_eq!(empty.focus_chance(), 0.0);
        let result = run(&empty);
        assert!(result.spread.is_empty());
        assert_eq!(result.all_hit, None);
    }

    #[test]
    fn the_report_stops_the_run_and_keeps_nothing() {
        let mut reports = 0;
        let outcome = simulate(&inputs(2), |_| {
            reports += 1;
            Control::Stop
        });
        assert_eq!(outcome, None);
        assert_eq!(reports, 1, "the run reported again after a stop");
    }

    #[test]
    fn the_strategy_ids_round_trip() {
        for strategy in Strategy::ALL {
            assert_eq!(Strategy::from_id(strategy.id()), Some(strategy));
            let saved = serde_json::to_string(&strategy).expect("a strategy saves");
            assert_eq!(saved, format!("\"{}\"", strategy.id()));
        }
        assert_eq!(Strategy::from_id("unknown"), None);
        assert_eq!(Strategy::default(), Strategy::None);
    }
}
