//! Observation Unlocks: which undiscovered observation is worth rolling for.
//!
//! Ported from `computeObsUnlockPriority` in the upstream `js/analysis.js`. Each
//! day gives a fixed number of observation rolls; a roll lands on an integer
//! between a minimum (raised by the failed-roll pity) and a maximum (raised by
//! Sharp Eye, Ring of Glory, Jelly and mastery). An observation unlocks when a
//! roll reaches its roll requirement, so the chance of unlocking it today — and
//! the expected days to the first success — follow from those two bounds.
//!
//! The reward side is the research EXP/hour the observation adds once it is
//! found, measured against the save's own layout with the glasses and
//! kaleidoscopes re-placed, as upstream `optimizeMagsFor` leaves them.

use crate::data::game;
use crate::model::{Effort, SimContext, Simulation};
use crate::optimize::mags;
use crate::sim;
use idleon_common::{Control, Progress};

/// Grid cells the estimate reads: Smart Eye's pity, Sharp Eye's ceiling and
/// Observationalistic's rolls per day.
const SMART_EYE: usize = 31;
const SHARP_EYE: usize = 51;
const OBSERVATIONALISTIC: usize = 90;

/// Days the expected-time loop walks before giving up, the upstream bound.
const MAX_DAYS: u32 = 365;

/// The roll state one day of hunting is priced with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rolls {
    /// The lowest roll possible right now, after the failed-roll pity.
    pub min_roll: f64,
    /// The highest roll possible.
    pub max_roll: f64,
    /// Rolls the account gets each day.
    pub per_day: u32,
    /// Rolls left today; `None` when the save does not carry the row.
    pub remaining: Option<u32>,
    /// Consecutive failed rolls; `None` when the save does not carry it.
    pub failed: Option<f64>,
    /// Smart Eye levels, each worth +1 minimum roll per failure.
    pub smart_eye: u32,
    /// Smart Eye's cap, 25 per level.
    pub smart_eye_cap: u32,
    /// Mastery's permanent +1 minimum roll.
    pub mastery_min: f64,
}

impl Rolls {
    /// The lowest roll after `failures` consecutive misses.
    fn low(&self, failures: f64) -> f64 {
        (1.0 + (failures * f64::from(self.smart_eye)).min(f64::from(self.smart_eye_cap))
            + self.mastery_min)
            .floor()
    }

    /// `1 - P(every roll misses)`, and the streak the misses leave behind.
    fn chance(&self, threshold: f64, failures: f64, rolls: u32) -> (f64, f64) {
        let mut miss = 1.0;
        let mut streak = failures;
        for _ in 0..rolls {
            let low = self.low(streak);
            let span = self.max_roll - low + 1.0;
            if span <= 0.0 {
                return (1.0, streak);
            }
            let reach = (self.max_roll - threshold + 1.0).max(0.0);
            miss *= 1.0 - (reach / span).clamp(0.0, 1.0);
            if miss <= 0.0 {
                return (1.0, streak);
            }
            streak += 1.0;
        }
        (1.0 - miss, streak)
    }

    /// Expected days to the first unlock, endless when it never lands.
    fn expected_days(&self, threshold: f64, today: f64) -> f64 {
        if today >= 1.0 {
            return 1.0;
        }
        if today <= 0.0 {
            return f64::INFINITY;
        }
        let mut cumulative = 0.0;
        let mut days = 0.0;
        let mut streak = self.failed.unwrap_or(0.0);
        for day in 1..=MAX_DAYS {
            let (chance, next) = self.chance(threshold, streak, self.per_day);
            streak = next;
            let first = (1.0 - cumulative) * chance;
            days += f64::from(day) * first;
            cumulative += first;
            if cumulative >= 0.9999 {
                break;
            }
        }
        if cumulative > 0.0 {
            days / cumulative
        } else {
            f64::INFINITY
        }
    }
}

/// One undiscovered observation and what hunting for it is worth.
#[derive(Clone, Debug)]
pub struct Row {
    pub obs: usize,
    pub name: String,
    /// The roll that unlocks it.
    pub threshold: f64,
    /// The research level that unlocks it.
    pub required_level: f64,
    /// Whether the research level already meets that requirement.
    pub usable: bool,
    /// Research EXP/hour with the observation found and the layout re-placed.
    pub new_rate: f64,
    /// `new_rate` against the save's layout; zero until the observation is usable.
    pub exp_gain: f64,
    /// Chance the day's rolls unlock it; `None` without the save's roll data.
    pub chance_today: Option<f64>,
    /// Expected days to the first unlock, endless when it never lands.
    pub expected_days: Option<f64>,
    /// `exp_gain ÷ expected days`, the ranking score.
    pub score: Option<f64>,
}

/// The priority label the tab shows, following the upstream cutoffs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    /// Not usable at this research level yet.
    Wait,
    /// Usable, but no gain or no roll data to rank it.
    Low,
    /// The top 70% of the best score.
    Best,
    /// The top 30% to 70%.
    Good,
    /// Anywhere below.
    Minor,
}

impl Priority {
    pub fn label(self) -> &'static str {
        match self {
            Priority::Wait => "Wait",
            Priority::Best => "Best",
            Priority::Good => "Good",
            Priority::Low | Priority::Minor => "Low",
        }
    }
}

impl Row {
    /// The row's priority against the best score the table shows.
    pub fn priority(&self, best: f64) -> Priority {
        if !self.usable {
            return Priority::Wait;
        }
        let Some(score) = self.score else {
            return Priority::Low;
        };
        if score <= 0.0 || best <= 0.0 {
            Priority::Low
        } else if score >= best * 0.7 {
            Priority::Best
        } else if score >= best * 0.3 {
            Priority::Good
        } else {
            Priority::Minor
        }
    }
}

/// The tab's stored result.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// Research EXP/hour of the save's own layout.
    pub current_rate: f64,
    /// The roll state the probabilities read.
    pub rolls: Rolls,
    /// The save fields the estimate needs and the save does not carry.
    pub missing: Vec<String>,
    /// One row per undiscovered observation, the upstream order.
    pub rows: Vec<Row>,
}

impl Outcome {
    /// The best score the table shows, which the priority labels are cut from.
    pub fn best_score(&self) -> f64 {
        self.rows
            .iter()
            .filter_map(|row| row.score)
            .fold(0.0, f64::max)
    }

    /// The rows the availability filter keeps.
    pub fn filtered(&self, filter: Filter) -> Vec<&Row> {
        self.rows.iter().filter(|row| filter.keeps(row)).collect()
    }
}

/// The table's availability filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Filter {
    /// Every undiscovered observation.
    #[default]
    All,
    /// Only the ones the research level can already use.
    Usable,
    /// Only the ones waiting on a research level.
    Waiting,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::All, Filter::Usable, Filter::Waiting];

    pub fn id(self) -> &'static str {
        match self {
            Filter::All => "all",
            Filter::Usable => "usable",
            Filter::Waiting => "waiting",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Usable => "Usable now",
            Filter::Waiting => "Waiting",
        }
    }

    fn keeps(self, row: &Row) -> bool {
        match self {
            Filter::All => true,
            Filter::Usable => row.usable,
            Filter::Waiting => !row.usable,
        }
    }
}

/// Rank every undiscovered observation, reporting progress per observation.
///
/// Returns `None` when the run was asked to stop; nothing is stored then.
pub fn analyze(
    sim: &Simulation,
    ctx: &SimContext,
    effort: Effort,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Option<Outcome> {
    let current_rate = sim::exp_rate(sim, ctx).total;
    let sharp_eye = sim.grid.level(SHARP_EYE);
    let max_roll = (100.0
        + f64::from(sharp_eye)
        + ctx.rog(30)
        + ctx.jelly_observation_max_roll
        + ctx.mastery_max_roll)
        .floor();
    let per_day = (3.0
        + f64::from(sim.grid.level(OBSERVATIONALISTIC))
        + 3.0 * ctx.event_shop(35)
        + ctx.rog(2))
    .round()
    .max(0.0) as u32;

    let mut missing = Vec::new();
    if ctx.observation_rolls_remaining.is_none() {
        missing.push(
            "Research[7][2]: the rolls left today are not in the save, so the chance of unlocking \
             anything today is unknown rather than a full day of rolls."
                .to_string(),
        );
    }
    if ctx.observation_failed_rolls.is_none() {
        missing.push(
            "OptLacc[514]: the failed-roll streak is not in the save, so the pity bonus on the \
             minimum roll is unknown."
                .to_string(),
        );
    }

    let mut rolls = Rolls {
        min_roll: 0.0,
        max_roll,
        per_day,
        remaining: ctx
            .observation_rolls_remaining
            .map(|left| left.max(0.0).floor() as u32),
        failed: ctx.observation_failed_rolls.map(|failed| failed.max(0.0)),
        smart_eye: sim.grid.level(SMART_EYE),
        smart_eye_cap: 25 * sim.grid.level(SMART_EYE),
        mastery_min: ctx.mastery_min_roll,
    };
    rolls.min_roll = rolls.low(rolls.failed.unwrap_or(0.0));

    let to_find = sim::observations_to_find(sim.research_level, &sim.observations.found);
    let undiscovered: Vec<usize> = (0..to_find)
        .filter(|obs| !sim.observations.found.get(*obs).copied().unwrap_or(false))
        .collect();
    let total = undiscovered.len().max(1);
    let mut rows = Vec::with_capacity(undiscovered.len());
    for (done, obs) in undiscovered.iter().copied().enumerate() {
        let data = &game().observations[obs];
        let name = data.name.replace('_', " ");
        if report(Progress {
            fraction: done as f32 / total as f32,
            message: format!("Observation unlocks: {name} ({} of {}).", done + 1, total),
        }) == Control::Stop
        {
            return None;
        }
        let usable = sim.research_level as f64 >= data.roll;
        let new_rate = found_rate(sim, ctx, effort, obs);
        let exp_gain = if usable { new_rate - current_rate } else { 0.0 };
        let (chance_today, expected_days, score) = match (rolls.remaining, rolls.failed) {
            (Some(remaining), Some(failed)) => {
                let (chance, _) = rolls.chance(data.roll_req, failed, remaining);
                let days = rolls.expected_days(data.roll_req, chance);
                (
                    Some(chance),
                    Some(days),
                    Some(if days > 0.0 { exp_gain / days } else { 0.0 }),
                )
            }
            _ => (None, None, None),
        };
        rows.push(Row {
            obs,
            name,
            threshold: data.roll_req,
            required_level: data.roll,
            usable,
            new_rate,
            exp_gain,
            chance_today,
            expected_days,
            score,
        });
    }
    rows.sort_by(|left, right| {
        right.usable.cmp(&left.usable).then_with(|| {
            right
                .score
                .unwrap_or(0.0)
                .total_cmp(&left.score.unwrap_or(0.0))
        })
    });
    Some(Outcome {
        current_rate,
        rolls,
        missing,
        rows,
    })
}

/// EXP/hour with one more observation found and the magnifiers re-placed.
fn found_rate(sim: &Simulation, ctx: &SimContext, effort: Effort, obs: usize) -> f64 {
    let mut trial = sim.clone();
    trial.observations.found[obs] = true;
    trial.mags = mags::optimize_exp(&trial, ctx, effort).mags;
    sim::exp_rate(&trial, ctx).total
}
