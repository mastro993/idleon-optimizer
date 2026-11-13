//! The distribution optimizer: the best way to split a total across the villagers.
//!
//! `opal-calc.html` ships two shapes. `optimizeTotal` compares an even split with
//! a star-weighted split, and `optimizeForTarget` moves a floor under the other
//! villagers and puts the rest on the target. Both miss the plans that finish a
//! villager's next ten opals, and neither guarantees the plan beats the save.
//!
//! This module searches the whole space. A villager's rate is
//! `holeoz × base × count × star × (1 + spread/100) × (1 + (additive +
//! enthusiasm)/100)`: for a fixed lowest count the Equal Spread factor is
//! constant and every villager's share is linear between multiples of ten, so
//! an optimum puts each villager either on that lowest count or on the next
//! multiple of ten, with the rest of the total on one villager. Checking every
//! lowest count and both placements therefore finds a plan at least as good as
//! any other split, and the saved allocation is always a candidate, so a plan
//! is never worse than the save.

use super::{Opal, VILLAGERS, name, relative, whole};
use idleon_common::Control;
use idleon_common::Progress;

/// What a plan maximizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    /// Every unlocked villager's rate added up, the page's `optimizeTotal`.
    Total,
    /// One villager's own rate, the page's `optimizeForTarget`.
    Villager(usize),
}

impl Target {
    /// The objective's name, for the plan text and the section's table.
    pub fn label(self) -> String {
        match self {
            Target::Total => "All villagers".to_owned(),
            Target::Villager(villager) => name(villager).to_owned(),
        }
    }
}

/// A request: which rate to maximize, the opals to split and the saved split.
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    /// The rate the plan maximizes.
    pub target: Target,
    /// The opals to distribute across the unlocked villagers.
    pub total: f64,
    /// The saved opal counts, kept so the result can show and beat them.
    pub current: [f64; VILLAGERS],
}

impl Input {
    /// The save's own total, split for one target.
    pub fn saved(opal: &Opal, target: Target) -> Self {
        Self {
            target,
            total: opal.total_opals(),
            current: opal.opals(),
        }
    }

    /// Whole opals only, and never a negative total.
    fn normalized(self) -> Self {
        Self {
            target: self.target,
            total: whole(self.total),
            current: self.current.map(whole),
        }
    }
}

/// One distribution and the saved state it replaces.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The rate the plan maximizes.
    pub target: Target,
    /// The opals the plan distributes.
    pub total: f64,
    /// The opals each villager receives, in save order; locked villagers get none.
    pub opals: [f64; VILLAGERS],
    /// The saved opal counts, for the comparison the table shows.
    pub current: [f64; VILLAGERS],
}

impl Plan {
    /// Every opal the plan hands out.
    pub fn spent(&self) -> f64 {
        self.opals.iter().sum()
    }

    /// How many opals the villager gains (or loses) against the save.
    pub fn change(&self, villager: usize) -> f64 {
        self.opals.get(villager).copied().unwrap_or(0.0)
            - self.current.get(villager).copied().unwrap_or(0.0)
    }

    /// The rate this distribution yields for the plan's target.
    pub fn rate(&self, opal: &Opal) -> f64 {
        opal.objective(self.target, &self.opals)
    }

    /// The rate the saved distribution yields for the plan's target.
    pub fn saved_rate(&self, opal: &Opal) -> f64 {
        opal.objective(self.target, &self.current)
    }

    /// The relative gain over the save, or `None` when the save earns nothing.
    pub fn improvement(&self, opal: &Opal) -> Option<f64> {
        relative(self.saved_rate(opal), self.rate(opal))
    }

    /// The plan as tab-separated text, for the section's copy action.
    pub fn text(&self, opal: &Opal) -> String {
        let mut out = format!(
            "Opal plan\t{}\t{} opals\nVillager\tOpals\tChange\tEXP/hr\n",
            self.target.label(),
            number(self.total)
        );
        for villager in (0..VILLAGERS).filter(|villager| opal.unlocked(*villager)) {
            let change = self.change(villager);
            out.push_str(&format!(
                "{}\t{}\t{}\t{:.0}\n",
                name(villager),
                number(self.opals[villager]),
                signed(change),
                opal.exp_rate(villager, &self.opals)
            ));
        }
        out.push_str(&format!(
            "Total\t{}\t\t{:.0}\n",
            number(self.spent()),
            self.rate(opal)
        ));
        if let Some(improvement) = self.improvement(opal) {
            out.push_str(&format!("Improvement\t{:+.2}%\n", improvement * 100.0));
        }
        out
    }
}

impl Opal {
    /// The rate `target` reads from `opals`: the total or one villager.
    pub fn objective(&self, target: Target, opals: &[f64; VILLAGERS]) -> f64 {
        match target {
            Target::Total => self.total_exp_rate(opals),
            Target::Villager(villager) if self.unlocked(villager) => self.exp_rate(villager, opals),
            Target::Villager(_) => 0.0,
        }
    }

    /// The best split of `input.total` for `input.target`.
    pub fn plan(&self, input: Input) -> Plan {
        self.plan_with(input, |_| Control::Continue)
            .expect("a reporter that always continues never stops the plan")
    }

    /// Like [`Self::plan`], reporting each lowest count and stopping on cancel.
    ///
    /// Returns `None` when the reporter asked to stop; the caller keeps whatever
    /// plan it showed before.
    pub fn plan_with(
        &self,
        input: Input,
        mut report: impl FnMut(Progress) -> Control,
    ) -> Option<Plan> {
        let Input {
            target,
            total,
            current,
        } = input.normalized();
        let unlocked: Vec<usize> = (0..VILLAGERS)
            .filter(|villager| self.unlocked(*villager))
            .collect();
        let mut best: Option<([f64; VILLAGERS], f64)> = None;
        let valid_target = match target {
            Target::Total => !unlocked.is_empty(),
            Target::Villager(villager) => self.unlocked(villager),
        };
        if valid_target && total > 0.0 {
            // The save's own split is the baseline the result must never lose to.
            if current.iter().sum::<f64>() == total
                && (0..VILLAGERS)
                    .all(|villager| self.unlocked(villager) || current[villager] == 0.0)
            {
                let rate = self.objective(target, &current);
                keep(&mut best, current, rate, &current);
            }

            let count = unlocked.len() as f64;
            let max_floor = (total / count).floor();
            let stride = (max_floor as u64 / 64).max(1);
            // Only the total can prefer a villager above the floor: a villager
            // target always reads best with every spare opal on it.
            let others = |special: usize| -> Vec<usize> {
                unlocked
                    .iter()
                    .copied()
                    .filter(|candidate| *candidate != special)
                    .collect()
            };
            let specials: Vec<(usize, Vec<usize>)> = match target {
                Target::Villager(villager) => vec![(villager, others(villager))],
                Target::Total => unlocked
                    .iter()
                    .map(|special| (*special, others(*special)))
                    .collect(),
            };
            for floor in 0..=max_floor as u64 {
                if floor % stride == 0 || floor == max_floor as u64 {
                    let progress = Progress {
                        fraction: ((floor + 1) as f32 / (max_floor as f32 + 1.0)).min(1.0),
                        message: format!("Checking {floor} opals under each villager…"),
                    };
                    if report(progress) == Control::Stop {
                        return None;
                    }
                }
                let floor = floor as f64;
                // The only other placement a villager can profitably take: the
                // next multiple of ten, which is where the enthusiasm step lands.
                let next_ten = if floor > 0.0 && floor % 10.0 != 0.0 {
                    (floor / 10.0).floor() * 10.0 + 10.0
                } else {
                    floor
                };
                let placements = if next_ten > floor { 2 } else { 1 };
                for (special, others) in &specials {
                    for mask in 0..(1usize << others.len()) {
                        let mut opals = [0.0; VILLAGERS];
                        let mut spent = 0.0;
                        let mut at_floor = false;
                        for (index, villager) in others.iter().enumerate() {
                            let high = placements == 2 && mask & (1 << index) != 0;
                            let count = if high { next_ten } else { floor };
                            opals[*villager] = count;
                            spent += count;
                            at_floor |= count == floor;
                        }
                        let rest = total - spent;
                        if rest < floor {
                            continue;
                        }
                        opals[*special] = rest;
                        // `floor` has to stay the lowest count, or this split is
                        // the same one a larger floor already covers.
                        if !at_floor && rest != floor {
                            continue;
                        }
                        let rate = self.objective(target, &opals);
                        keep(&mut best, opals, rate, &current);
                    }
                }
            }
        }
        let (opals, _) = best.unwrap_or(([0.0; VILLAGERS], 0.0));
        Some(Plan {
            target,
            total,
            opals,
            current,
        })
    }
}

/// Keep `opals` when it beats the best rate; on a tie prefer the plan that moves
/// the fewest opals, so an unchanged save keeps its own distribution.
fn keep(
    best: &mut Option<([f64; VILLAGERS], f64)>,
    opals: [f64; VILLAGERS],
    rate: f64,
    current: &[f64; VILLAGERS],
) {
    let movement = |opals: &[f64; VILLAGERS]| {
        (0..VILLAGERS)
            .map(|villager| (opals[villager] - current[villager]).abs())
            .sum::<f64>()
    };
    let Some((best_opals, best_rate)) = best else {
        *best = Some((opals, rate));
        return;
    };
    let close = (rate - *best_rate).abs() <= best_rate.abs().max(1.0) * 1e-12;
    let better = rate > *best_rate && !close;
    // A tied plan is only swapped for one that moves fewer opals when it does
    // not lose any rate, so the best rate never slips backwards.
    let steadier = close && rate >= *best_rate && movement(&opals) < movement(best_opals);
    if better || steadier {
        *best = Some((opals, rate));
    }
}

/// A whole count without a decimal part, for the copyable plan.
fn number(value: f64) -> String {
    format!("{:.0}", whole(value))
}

fn signed(value: f64) -> String {
    if value > 0.0 {
        format!("+{}", number(value))
    } else if value < 0.0 {
        format!("-{}", number(-value))
    } else {
        "0".to_owned()
    }
}
