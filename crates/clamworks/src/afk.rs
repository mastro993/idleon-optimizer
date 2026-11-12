//! The Clamworks AFK claim and the `+1` upgrade comparison beside it.
//!
//! Ports `clamworksAfkSummary` from `js/stats/systems/w7/clamworks-afk.js`, the
//! drop distribution it is built on (`afkDropRollDistribution` and the
//! Clamworks rows of `afkMonsterDropOutcomes`, both in
//! `js/stats/systems/common/combat-afk.js`) and the comparison in
//! `js/stats/systems/w7/clamworks-combat.js`.
//!
//! One claim is one ordinary AFK trip on Clamworks: every kill rolls the map's
//! pearl rows once, and every pearl is worth [`Clamworks::pearl_value`] with the
//! LUCKY DAY ten-times chance folded in. What the game derives from combat —
//! monsters killed per hour and the drop-rate stat — is an input here until the
//! combat simulator lands (`CS-02`/`CS-03`), just like the multikill total.
//!
//! Two parts of the upstream tab are not ported. The reroll-for-pure-pearls
//! policy needs the talent and guild bonuses the stats context resolves, and
//! Time Candy cannot be used on this map at all, so neither is an input.

use super::{Clamworks, PURE_PEARLS, UPGRADE_COUNT};
use crate::data::clamworks;

/// The pearl drop table a claim starts from, as the upstream page asks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// A fresh login: the map's untouched pearl rates, whatever the upgrades say.
    #[default]
    Entry,
    /// In session: the rates the scene rewrites every second.
    Settled,
}

impl Phase {
    pub const ALL: [Phase; 2] = [Phase::Entry, Phase::Settled];

    pub fn id(self) -> &'static str {
        match self {
            Phase::Entry => "entry",
            Phase::Settled => "settled",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Phase::Entry => "Fresh login",
            Phase::Settled => "In session",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|phase| phase.id() == id)
    }
}

/// What one claim reads: the hours away and the combat simulator's two stand-ins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inputs {
    pub hours: f64,
    /// Monsters killed per hour on Clamworks.
    pub kills_per_hour: f64,
    /// The character's drop-rate multiplier.
    pub drop_rate: f64,
    pub phase: Phase,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            hours: 8.0,
            kills_per_hour: 0.0,
            drop_rate: 1.0,
            phase: Phase::Entry,
        }
    }
}

/// Why a claim cannot be calculated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimError {
    /// The upstream range is 0 to 55,555 hours (`hours × 3600 <= 2e8`).
    Hours,
    /// A kill rate or drop rate that is negative or not a number.
    Rates,
}

impl ClaimError {
    /// The message the upstream page shows for the same rejected entry.
    pub fn message(self) -> &'static str {
        match self {
            ClaimError::Hours => "Enter AFK hours between 0 and 55,555.",
            ClaimError::Rates => "Enter a nonnegative kill rate and drop rate.",
        }
    }
}

impl std::fmt::Display for ClaimError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

/// One pearl type's drop distribution over the claim.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drop {
    /// The roll chance per kill: the row's rate times the drop rate.
    pub probability: f64,
    pub expected: f64,
    /// Chance of at least one drop.
    pub chance: f64,
    pub minimum: f64,
    /// Whether the claim always drops at least one.
    pub guaranteed: bool,
    /// Expected pearl currency this row is worth.
    pub pearls: f64,
}

/// One native Clamworks AFK claim.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Claim {
    pub inputs: Inputs,
    /// Whether the game credits a claim at all; it needs more than two minutes.
    pub eligible: bool,
    pub kills: f64,
    /// The kills the drops roll over, `floor(kills)`.
    pub rolls: u64,
    pub regular: Drop,
    pub pure: Drop,
    /// The pearl value the claim's pearls are worth.
    pub value: f64,
    pub lucky_chance: f64,
    pub pure_multiplier: f64,
    pub expected_pearls: f64,
    pub warnings: Vec<String>,
}

/// One row of the comparison: the current state or one `+1` upgrade.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompareRow {
    /// The upgrade the row buys; `None` for the current state.
    pub index: Option<usize>,
    pub name: String,
    pub level: u32,
    pub next_level: u32,
    /// What the purchase costs, `None` for the current state.
    pub cost: Option<f64>,
    /// Whether the saved pearl balance covers it.
    pub affordable: bool,
    pub claim: Claim,
    /// Expected pearls this row gains over the current state.
    pub gain: Option<f64>,
    /// Claims needed to pay the purchase back at this claim size.
    pub payback_claims: Option<f64>,
    pub payback_hours: Option<f64>,
}

/// The comparison rows and the candidates whose cost could not be read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Compare {
    pub rows: Vec<CompareRow>,
    pub warnings: Vec<String>,
}

impl Compare {
    /// The row with the shortest payback; the current state never competes.
    pub fn best(&self) -> Option<&CompareRow> {
        self.rows
            .iter()
            .filter(|row| row.payback_claims.is_some_and(f64::is_finite))
            .min_by(|left, right| {
                left.payback_claims
                    .unwrap_or(f64::INFINITY)
                    .total_cmp(&right.payback_claims.unwrap_or(f64::INFINITY))
            })
    }
}

impl Clamworks {
    /// `clamworksAfkSummary`: one ordinary claim's pearl drops and what they are
    /// worth.
    ///
    /// The claim starts from the selected pearl table and always keeps the same
    /// kills; a reroll would replace the claim, not add to it, which is why the
    /// payout is the one claim.
    pub fn afk_claim(&self, inputs: Inputs, multikill_total: f64) -> Result<Claim, ClaimError> {
        if !inputs.hours.is_finite() || inputs.hours < 0.0 || inputs.hours * 3600.0 > 2e8 {
            return Err(ClaimError::Hours);
        }
        if !inputs.kills_per_hour.is_finite()
            || inputs.kills_per_hour < 0.0
            || !inputs.drop_rate.is_finite()
            || inputs.drop_rate < 0.0
        {
            return Err(ClaimError::Rates);
        }
        let value = self.pearl_value(multikill_total);
        let lucky_chance = self.pearl_10x_chance();
        let pure_multiplier = self.black_pearl_value();
        // `eligible` is the game's own two-minute floor on an AFK claim.
        let eligible = inputs.hours * 3600.0 > 120.0;
        let kills = inputs.hours * inputs.kills_per_hour;
        let rolls = if eligible { kills.floor() } else { 0.0 };
        let regular = drop_row(
            rolls,
            self.pearl_rate(inputs.phase) * inputs.drop_rate,
            value,
            lucky_chance,
            1.0,
        );
        let pure = drop_row(
            rolls,
            self.pure_rate(inputs.phase) * inputs.drop_rate,
            value,
            lucky_chance,
            pure_multiplier,
        );
        let mut warnings = Vec::new();
        if !eligible {
            warnings.push("Native AFK claims need more than two minutes away.".to_string());
        }
        Ok(Claim {
            inputs,
            eligible,
            kills,
            rolls: rolls as u64,
            regular,
            pure,
            value,
            lucky_chance,
            pure_multiplier,
            expected_pearls: regular.pearls + pure.pearls,
            warnings,
        })
    }

    /// `buildClamworksAfkPlan` and `summarizeClamworksAfk`: every unlocked `+1`
    /// upgrade's claim beside the current one, with its cost and payback.
    ///
    /// The candidates are the upstream's: at most the saved unlock progress,
    /// never the two cost reducers (they change no pearl yield) and never CLAM
    /// COMRADES once it already caps the map's clams.
    pub fn afk_compare(&self, inputs: Inputs, multikill_total: f64) -> Result<Compare, ClaimError> {
        let current = self.afk_claim(inputs, multikill_total)?;
        let mut rows = vec![CompareRow {
            name: "Current loadout".to_string(),
            level: 0,
            next_level: 0,
            affordable: true,
            claim: current.clone(),
            ..CompareRow::default()
        }];
        let mut warnings = Vec::new();
        for index in self.compare_candidates() {
            let level = self.upgrades[index];
            // The upstream row costs `upgradeCostAt`'s default level, the saved
            // one, not the level the row buys.
            let cost = self.upgrade_cost_at(index, f64::from(level));
            if !cost.is_finite() || cost <= 0.0 {
                warnings.push(format!(
                    "{}: the next purchase cost is not representable; comparison skipped.",
                    self.upgrade_name(index)
                ));
                continue;
            }
            let mut next = self.clone();
            next.upgrades[index] = level + 1;
            let claim = next.afk_claim(inputs, multikill_total)?;
            let gain = claim.expected_pearls - current.expected_pearls;
            let payback_claims = (gain > 0.0).then(|| cost / gain);
            rows.push(CompareRow {
                index: Some(index),
                name: self.upgrade_name(index),
                level,
                next_level: level + 1,
                cost: Some(cost),
                affordable: self.pearl_balance >= cost,
                claim,
                gain: Some(gain),
                payback_claims,
                payback_hours: payback_claims.map(|claims| claims * inputs.hours),
            });
        }
        Ok(Compare { rows, warnings })
    }

    /// The upgrades the comparison buys: the unlocked ones, minus the two cost
    /// reducers and the clam cap that has nothing left to add.
    pub fn compare_candidates(&self) -> Vec<usize> {
        let capped = self.upgrades[1] >= 23;
        (0..UPGRADE_COUNT)
            .filter(|index| (*index as u32) <= self.unlocked)
            .filter(|index| !matches!(index, 4 | 8))
            .filter(|index| !(*index == 1 && capped))
            .collect()
    }

    fn upgrade_name(&self, index: usize) -> String {
        clamworks()
            .upgrades
            .get(index)
            .map(|upgrade| upgrade.name.clone())
            .unwrap_or_else(|| format!("Upgrade {}", index + 1))
    }

    /// The saved pearl drop rate, or the map's untouched one on a fresh login.
    fn pearl_rate(&self, phase: Phase) -> f64 {
        match phase {
            Phase::Entry => 0.001,
            Phase::Settled => self.pearl_drop_rate(),
        }
    }

    /// PURE PEARLS gates the pure pearl row entirely: unbought, the row is not
    /// in the map's table and the settled rate is zero.
    fn pure_rate(&self, phase: Phase) -> f64 {
        if self.upgrades[PURE_PEARLS] < 1 {
            return 0.0;
        }
        match phase {
            Phase::Entry => 0.000_005,
            Phase::Settled => self.black_pearl_drop_rate(),
        }
    }
}

/// One drop row over `batch` rolls: `afkDropRollDistribution`, then the row's
/// expected quantity, chance and guaranteed minimum.
fn drop_row(batch: f64, probability: f64, value: f64, lucky: f64, multiplier: f64) -> Drop {
    let distribution = roll_distribution(batch, probability);
    let expected: f64 = distribution.iter().map(|(count, p)| count * p).sum();
    let zero = distribution
        .iter()
        .find_map(|(count, p)| (*count == 0.0).then_some(*p))
        .unwrap_or(0.0);
    Drop {
        probability,
        expected,
        chance: clamp(1.0 - zero),
        minimum: distribution
            .iter()
            .map(|(count, _)| *count)
            .fold(f64::INFINITY, f64::min),
        guaranteed: zero <= 0.0,
        pearls: expected * multiplier * value * (1.0 + 9.0 * lucky),
    }
}

/// `afkDropRollDistribution`: one batch roll per row, capped at two drops until
/// its mean is strictly above two, where the game hands out a fixed quantity.
fn roll_distribution(batch: f64, probability: f64) -> Vec<(f64, f64)> {
    if batch <= 0.0 {
        return vec![(0.0, 1.0)];
    }
    if probability * batch > 2.0 {
        return vec![((probability * batch).round(), 1.0)];
    }
    let p0 = (1.0 - probability).powf(batch);
    let p01 = probability * (1.0 - probability).powf(batch - 1.0) * batch + p0;
    let zero = clamp(p0);
    let one = (clamp(p01) - zero).max(0.0);
    [(0.0, zero), (1.0, one), (2.0, 1.0 - zero - one)]
        .into_iter()
        .filter(|(_, p)| *p > 0.0)
        .collect()
}

fn clamp(probability: f64) -> f64 {
    if probability.is_nan() {
        0.0
    } else {
        probability.clamp(0.0, 1.0)
    }
}
