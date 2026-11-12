//! The Monument (W5) calculator: the Bravery and Wisdom tabs, the Justice
//! response advisor, the shared training pool and the reward charge all three
//! monuments pay out.
//!
//! Ported from the upstream `js/stats/systems/w5/monument.js` and the helper
//! systems those tabs read: hole upgrades, the fountain, jar measurements and
//! collectibles, summoning victories, arcade, achievements, the compass,
//! Minehead, Meritoc voting, sushi tiers and the gaming superbits. The static
//! tables come from `script/extract-monument-data.mjs`; the Justice worksheet is
//! [`justice`].
//!
//! The formulas follow the upstream order of operations exactly, including its
//! rounding and flooring, because the calculator reports the game's displayed
//! values. A save missing the monument rows reports [`Snapshot::available`]
//! false instead of guessing.

mod data;
pub mod justice;
mod save;

pub use save::Save;

use data::data;
use idleon_common::curve::log;
use serde_json::Value;

/// A Bravery input the save cannot fix: the run state the player is in.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Runtime {
    /// Rethrows left; `None` uses the saved maximum.
    pub rerolls: Option<u32>,
    /// Damage-boost stacks already earned this story.
    pub boosts: u32,
}

/// One additive training source.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub name: String,
    pub value: f64,
}

/// Paying Respect training: the additive pool, its Minehead multiplier and the
/// hours a 24-hour claim grants.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Training {
    pub sources: Vec<Source>,
    pub pct: f64,
    pub multi: f64,
    pub rate: f64,
    pub per_day: f64,
}

/// The Bravery tab's saved and run-dependent stats.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraveryStats {
    pub hours: f64,
    pub milestones: u32,
    pub opal_chance: f64,
    pub min_dmg: f64,
    pub max_dmg: f64,
    pub rerolls: u32,
    pub max_rerolls: u32,
    pub boosts: u32,
    pub swords: u32,
    pub retellings: u32,
    pub rightmost: bool,
    pub auto_kill: bool,
}

/// The sword odds for one enemy and run state.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Odds {
    pub stats: BraveryStats,
    pub wave: u32,
    pub hp: f64,
    pub automatic: bool,
    pub kill: f64,
    pub boost_chance: f64,
    pub last_sword_chance: Option<f64>,
    pub next_damage_factor: f64,
}

/// One Paying Respect milestone row.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Milestone {
    pub text: String,
    pub hours: f64,
    pub unlocked: bool,
    pub ready: bool,
    pub hours_needed: f64,
    pub real_hours: f64,
}

/// One permanent reward level and its effect now and at the next level.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BonusRow {
    pub level: f64,
    pub text: String,
    pub value: f64,
    pub next: f64,
}

/// Everything the Bravery tab renders for a save.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub available: bool,
    pub notes: Vec<String>,
    pub training: Training,
    pub stats: BraveryStats,
    pub milestones: Vec<Milestone>,
    pub bonuses: Vec<BonusRow>,
}

/// The Wisdom tab's saved stats.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WisdomStats {
    pub hours: f64,
    pub milestones: u32,
    pub opal_chance: f64,
    pub attempts: f64,
    pub attempts_per_board: f64,
    pub instamatches: f64,
    pub reveal_row: bool,
    pub reveal_square: bool,
    pub seconds_per_match: f64,
}

/// `monumentRewardCharge`: the reward multiplier a wait has built up.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Charge {
    /// Real seconds waited: the saved `Holes[11][11 + monument]` row plus the extra wait.
    pub seconds: f64,
    /// The linear window in seconds; waiting beyond it only adds a diminished tail.
    pub max_linear: f64,
    /// The wait's own multiplier, before the Meritoc and Legend factors.
    pub base: f64,
    /// `computeMeritocBonusz(7)`. Zero unless vote 7 is active and its map is cleared.
    pub meritoc: f64,
    /// `legendPTSbonus(27)`.
    pub legend: f64,
    pub multi: f64,
    pub can_start: bool,
}

/// Everything the Wisdom tab renders for a save.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WisdomSnapshot {
    pub available: bool,
    pub notes: Vec<String>,
    pub training: Training,
    pub stats: WisdomStats,
    pub milestones: Vec<Milestone>,
    pub bonuses: Vec<BonusRow>,
    /// The charge saved waiting has built so far.
    pub charge: Charge,
    /// The charge after another 24 hours, the upstream "tomorrow" preview.
    pub tomorrow: Charge,
}

/// The Justice tab's saved training and fresh-story start.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JusticeStats {
    pub hours: f64,
    pub milestones: u32,
    pub opal_chance: f64,
    pub coins: i64,
    pub health: i64,
    pub popularity: i64,
    pub dismissals: i64,
}

/// Everything the Justice tab renders for a save.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JusticeSnapshot {
    pub available: bool,
    pub notes: Vec<String>,
    pub training: Training,
    pub stats: JusticeStats,
    pub milestones: Vec<Milestone>,
    pub bonuses: Vec<BonusRow>,
}

/// Why a Bravery input was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Enemies defeated must be at most 1000.
    WaveTooLarge,
    /// Remaining rethrows exceed the saved maximum.
    RerollsExceedMaximum,
    /// Remaining HP must be greater than zero.
    RemainingHp,
    /// The monument must be 0-2 and the extra wait a finite, nonnegative time.
    InvalidCharge,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::WaveTooLarge => write!(formatter, "Enemies defeated must be at most 1000."),
            Error::RerollsExceedMaximum => {
                write!(formatter, "Remaining rethrows exceed the saved maximum.")
            }
            Error::RemainingHp => write!(formatter, "Remaining HP must be greater than zero."),
            Error::InvalidCharge => {
                write!(formatter, "Invalid monument or additional waiting time.")
            }
        }
    }
}

impl std::error::Error for Error {}

/// Paying Respect training for one character.
pub fn training(save: &Save, char_idx: usize) -> Training {
    let names = ["Bravery", "Justice", "Wisdom"];
    let mut sources: Vec<Source> = names
        .iter()
        .enumerate()
        .map(|(monument, name)| Source {
            name: format!("{name}: All Monument AFK Gain"),
            value: monument_rog_bonus(save, monument, 8),
        })
        .collect();
    sources.push(Source {
        name: "Summoning victories".into(),
        value: win_bonus(save, 25, char_idx),
    });
    sources.push(Source {
        name: "Jar collectible: Monument AFK gain".into(),
        value: collectible_bonus(save, 19, legend_pts_bonus(save, 29)),
    });
    sources.push(Source {
        name: "Rock Smart".into(),
        value: compute_bupg(save, 81, 20.0),
    });
    sources.push(Source {
        name: "Monument AFK measurement".into(),
        value: measurement_bonus(save, 11),
    });
    sources.push(Source {
        name: "Arcade: Monument AFK gain".into(),
        value: arcade_bonus(save, 42),
    });
    sources.push(Source {
        name: "Hug from Timmy".into(),
        value: 10.0 * achieve_status(save, 311),
    });
    sources.push(Source {
        name: "Compass: Monument AFK gain".into(),
        value: compass_bonus(save, 55),
    });

    let pct = sources.iter().map(|source| source.value).sum::<f64>();
    let multi = 1.0 + minehead_bonus_qty(19, state_r7_floor(save)) / 100.0;
    let rate = (1.0 + pct / 100.0) * multi;
    Training {
        sources,
        pct,
        multi,
        rate,
        per_day: (24.0 * rate).floor(),
    }
}

/// The Bravery stats for one run state.
pub fn bravery_stats(save: &Save, runtime: Runtime) -> Result<BraveryStats, Error> {
    let hours = save.hole(14, 0);
    let milestones = save.hole(14, 1).max(0.0) as u32;
    let hr = |index: u32| milestones > index;
    let opal_chance =
        (0.5_f64.powf(save.hole(7, 3)) * (1.0 + monument_rog_bonus(save, 0, 5) / 100.0)).min(0.5);
    let max_rerolls = (5.0 * f64::from(hr(2)) + 10.0 * f64::from(hr(6))).round() as u32;
    let rerolls = runtime.rerolls.unwrap_or(max_rerolls);
    if rerolls > max_rerolls {
        return Err(Error::RerollsExceedMaximum);
    }
    let boosts = runtime.boosts;
    let max_dmg = (25.0 + 10.0 * (hours / 6.0).floor() * compute_bupg(save, 24, 1.0))
        * (1.0 + measurement_bonus(save, 1) / 100.0)
        * (1.0 + compute_bupg(save, 40, 10.0) * f64::from(rerolls) / 100.0)
        * (1.0 + 10.0 * f64::from(boosts) / 100.0);
    let min_dmg = 3.0
        + (hours / 6.0).floor() * compute_bupg(save, 24, 1.0)
        + study(save, 3) / 100.0 * max_dmg;
    Ok(BraveryStats {
        hours,
        milestones,
        opal_chance,
        min_dmg,
        max_dmg,
        rerolls,
        max_rerolls,
        boosts,
        swords: (3 + 2 * u32::from(hr(1)) + u32::from(hr(3)) + u32::from(hr(5)) + u32::from(hr(7)))
            .min(9),
        retellings: u32::from(hr(4)),
        rightmost: compute_bupg(save, 25, 1.0) == 1.0,
        auto_kill: save.hole(26, 3) >= 1.0,
    })
}

/// The sword odds for `wave` enemies defeated, where the UI's enemy number 1 is
/// wave 0.
pub fn bravery_odds(
    save: &Save,
    wave: u32,
    runtime: Runtime,
    remaining_hp: Option<f64>,
) -> Result<Odds, Error> {
    if wave > 1000 {
        return Err(Error::WaveTooLarge);
    }
    // The run stats come first so a run with two bad inputs reports the same
    // error the upstream calculator does.
    let stats = bravery_stats(save, runtime)?;
    if remaining_hp.is_some_and(|hp| !hp.is_finite() || hp <= 0.0) {
        return Err(Error::RemainingHp);
    }
    let hp = (10.0 + 15.0 * f64::from(wave)) * 1.3_f64.powi(wave as i32);
    let cdf = |threshold: f64, count: u32| sum_cdf(threshold, count, stats.min_dmg, stats.max_dmg);
    let automatic = stats.auto_kill && stats.min_dmg >= hp;
    let kill = if automatic {
        1.0
    } else {
        1.0 - cdf(hp, stats.swords)
    };
    let finish = if automatic {
        1.0
    } else {
        (cdf(hp, stats.swords.saturating_sub(1)) - cdf(hp, stats.swords)).max(0.0)
    };
    Ok(Odds {
        wave,
        hp,
        automatic,
        kill,
        boost_chance: if automatic {
            1.0
        } else if stats.rightmost {
            finish
        } else {
            0.0
        },
        last_sword_chance: remaining_hp.map(|hp| 1.0 - cdf(hp, 1)),
        next_damage_factor: (1.0 + 10.0 * f64::from(stats.boosts + 1) / 100.0)
            / (1.0 + 10.0 * f64::from(stats.boosts) / 100.0),
        stats,
    })
}

/// The eight Bravery milestones, with their one-character ETA at `rate`.
pub fn milestones(save: &Save, rate: f64) -> Vec<Milestone> {
    let stats = bravery_stats(save, Runtime::default()).unwrap_or_default();
    milestone_rows(0, stats.hours, stats.milestones, rate)
}

/// The eight Wisdom milestones, with their one-character ETA at `rate`.
pub fn wisdom_milestones(save: &Save, rate: f64) -> Vec<Milestone> {
    let stats = wisdom_stats(save);
    milestone_rows(2, stats.hours, stats.milestones, rate)
}

/// The eight Justice milestones, with their one-character ETA at `rate`.
pub fn justice_milestones(save: &Save, rate: f64) -> Vec<Milestone> {
    let stats = justice_stats(save);
    milestone_rows(1, stats.hours, stats.milestones, rate)
}

/// One monument's eight Paying Respect milestones.
fn milestone_rows(monument: usize, hours: f64, unlocked: u32, rate: f64) -> Vec<Milestone> {
    data()
        .holes
        .milestone_hours
        .iter()
        .enumerate()
        .map(|(index, hours_required)| {
            let hours_required = *hours_required;
            let is_unlocked = unlocked > index as u32;
            let ready = hours >= hours_required;
            let hours_needed = (hours_required - hours).max(0.0);
            let real_hours = if ready {
                0.0
            } else {
                1.0_f64.max(hours_needed.ceil() / rate)
            };
            Milestone {
                text: data()
                    .holes
                    .milestone_text
                    .get(8 * monument + index)
                    .cloned()
                    .unwrap_or_default(),
                hours: hours_required,
                unlocked: is_unlocked,
                ready,
                hours_needed,
                real_hours,
            }
        })
        .collect()
}

/// The ten Bravery permanent reward levels and their effects.
pub fn bonuses(save: &Save) -> Vec<BonusRow> {
    bonuses_for(save, 0)
}

/// The ten permanent reward levels of `monument` and their effects.
pub fn bonuses_for(save: &Save, monument: usize) -> Vec<BonusRow> {
    (0..10)
        .map(|index| {
            let slot = 10 * monument + index;
            BonusRow {
                level: save.hole(15, slot),
                text: data().bonus_text(slot).to_string(),
                value: monument_rog_bonus(save, monument, index),
                next: monument_rog_bonus_with_level(
                    save,
                    monument,
                    index,
                    Some(save.hole(15, slot) + 1.0),
                ),
            }
        })
        .collect()
}

/// Everything the Bravery tab needs, or an unavailable marker when the save
/// lacks the Monument rows.
pub fn snapshot(save: &Save, char_idx: usize) -> Snapshot {
    if !has_rows(save) {
        return Snapshot::default();
    }
    let training = training(save, char_idx);
    let stats = bravery_stats(save, Runtime::default()).unwrap_or_default();
    Snapshot {
        available: true,
        notes: notes(save, char_idx),
        milestones: milestones(save, training.rate),
        bonuses: bonuses(save),
        training,
        stats,
    }
}

/// The Wisdom saved stats for a save.
pub fn wisdom_stats(save: &Save) -> WisdomStats {
    let hours = save.hole(14, 4);
    let milestones = save.hole(14, 5).max(0.0) as u32;
    let unlocked = |index: u32| milestones > index;
    WisdomStats {
        hours,
        milestones,
        opal_chance: (0.5_f64.powf(save.hole(7, 12))
            * (1.0 + monument_rog_bonus(save, 2, 5) / 100.0))
            .min(0.5),
        attempts: (5.0
            + study(save, 12)
            + (4.0 * f64::from(unlocked(4)) + (2.0 * log(hours)).floor()))
        .round(),
        attempts_per_board: (2.0 * f64::from(unlocked(1)) + f64::from(unlocked(6))).round(),
        instamatches: (4.0 * f64::from(unlocked(3)) + 5.0 * f64::from(unlocked(7))).round(),
        reveal_row: unlocked(2),
        reveal_square: unlocked(5),
        seconds_per_match: 12.6,
    }
}

/// Everything the Wisdom tab needs, or an unavailable marker when the save
/// lacks the Monument rows.
pub fn wisdom_snapshot(save: &Save, char_idx: usize) -> WisdomSnapshot {
    if !has_rows(save) {
        return WisdomSnapshot::default();
    }
    let training = training(save, char_idx);
    WisdomSnapshot {
        available: true,
        notes: notes(save, char_idx),
        stats: wisdom_stats(save),
        milestones: wisdom_milestones(save, training.rate),
        bonuses: bonuses_for(save, 2),
        charge: reward_charge(save, 2, char_idx, 0.0).unwrap_or_default(),
        tomorrow: reward_charge(save, 2, char_idx, 86_400.0).unwrap_or_default(),
        training,
    }
}

/// The Justice saved stats for a save.
pub fn justice_stats(save: &Save) -> JusticeStats {
    let hours = save.hole(14, 2);
    let milestones = save.hole(14, 3).max(0.0) as u32;
    let unlocked = |index: u32| f64::from(milestones > index);
    JusticeStats {
        hours,
        milestones,
        opal_chance: (0.5_f64.powf(save.hole(7, 9))
            * (1.0 + monument_rog_bonus(save, 1, 5) / 100.0)
            * (1.0 + study(save, 9) / 100.0))
            .min(0.5),
        coins: ((5.0 + hours.max(1.0).log2() * compute_bupg(save, 61, 1.0))
            * (0.5 * unlocked(3) + 1.5 * unlocked(6) + 1.0))
            .round() as i64,
        health: (1.0 + unlocked(1) + unlocked(4) + 2.0 * unlocked(7)).round() as i64,
        popularity: (3.0 + 7.0 * unlocked(5)).round() as i64,
        dismissals: (unlocked(2) + unlocked(4) + 2.0 * unlocked(7)).round() as i64,
    }
}

/// Everything the Justice tab needs, or an unavailable marker when the save
/// lacks the Monument rows.
pub fn justice_snapshot(save: &Save, char_idx: usize) -> JusticeSnapshot {
    if !has_rows(save) {
        return JusticeSnapshot::default();
    }
    let training = training(save, char_idx);
    JusticeSnapshot {
        available: true,
        notes: notes(save, char_idx),
        stats: justice_stats(save),
        milestones: justice_milestones(save, training.rate),
        bonuses: bonuses_for(save, 1),
        training,
    }
}

/// The saved Monument rows every tab reads.
fn has_rows(save: &Save) -> bool {
    [11, 14, 15]
        .iter()
        .all(|row| save.holes.get(*row).is_some_and(Value::is_array))
}

/// The partial-save warnings the upstream snapshot prints for every tab.
fn notes(save: &Save, char_idx: usize) -> Vec<String> {
    let mut notes = Vec::new();
    if !save.companion_available {
        notes.push(
            "Companion ownership is missing; reward levels and companion-dependent sources are partial."
                .into(),
        );
    }
    if !save.state_r7_available {
        notes.push(
            "Minehead progress is missing; Clutch Overtime Block is unknown and shown without its bonus."
                .into(),
        );
    }
    if save.option(453) == 7.0 && save.kla_map(char_idx, 250).is_none() {
        notes.push(
            "Selected character Summoning access is missing; the active Monument Meritoc bonus is unknown."
                .into(),
        );
    }
    notes
}

/// `monumentRewardCharge`: the reward multiplier a monument's saved wait pays.
///
/// `monument` is the upstream `t` (Bravery 0, Justice 1, Wisdom 2) and
/// `extra_seconds` the additional wait the caller previews. Waiting past the
/// linear window keeps paying out a diminished cube-root tail.
pub fn reward_charge(
    save: &Save,
    monument: usize,
    char_idx: usize,
    extra_seconds: f64,
) -> Result<Charge, Error> {
    if monument > 2 || !extra_seconds.is_finite() || extra_seconds < 0.0 {
        return Err(Error::InvalidCharge);
    }
    let legend = legend_pts_bonus(save, 27);
    let meritoc = meritoc(save, 7, char_idx);
    let b70 = compute_bupg(save, 70, 2.0);
    let b96 = compute_bupg(save, 96, 2.0);
    let gaming = 10.0 * superbit(save, 31);
    let linear = b70 + b96 + gaming;
    let max_linear = 86_400.0
        * if monument == 1 {
            2.0 + linear + (14.0 * f64::from(save.hole(26, 9) >= 1.0) + legend / 24.0)
        } else {
            2.0 + (linear + legend / 24.0)
        };
    let seconds = save.hole(11, 11 + monument) + extra_seconds;
    let base = seconds.min(max_linear) / 72_000.0
        + ((1.0 + (seconds - max_linear).max(0.0) / 72_000.0).powf(0.3) - 1.0);
    let multi = base * (1.0 + meritoc / 100.0) * (1.0 + legend / 100.0);
    Ok(Charge {
        seconds,
        max_linear,
        base,
        meritoc,
        legend,
        multi,
        can_start: multi >= 1.0,
    })
}

/// `MonumentROGbonuses(t, i)`: a permanent reward level's effect.
fn monument_rog_bonus(save: &Save, monument: usize, index: usize) -> f64 {
    monument_rog_bonus_with_level(save, monument, index, None)
}

fn monument_rog_bonus_with_level(
    save: &Save,
    monument: usize,
    index: usize,
    level: Option<f64>,
) -> f64 {
    let slot = 10 * monument + index;
    let level = level.unwrap_or_else(|| save.hole(15, slot));
    if level <= 0.0 {
        return 0.0;
    }
    let bonus_info = data().bonus_per_level(slot);
    if bonus_info <= 0.0 {
        return 0.0;
    }
    let mut holeoz = 1.0;
    if index != 9 {
        holeoz =
            1.0 + monument_rog_bonus(save, monument, 9) / 100.0 + cosmo_bonus(save, 0, 0) / 100.0;
    }
    holeoz *= 1.0 + fountain_bon_tot(save, monument) / 100.0;
    if bonus_info < 30.0 {
        level * bonus_info * holeoz.max(1.0)
    } else {
        0.1 * (level / (250.0 + level) * 10.0 * bonus_info * holeoz.max(1.0)).ceil()
    }
}

/// `CosmoBonus(tier, idx)`: the cosmo upgrade base times its saved level.
fn cosmo_bonus(save: &Save, tier: usize, index: usize) -> f64 {
    let level = save.hole(4 + tier, index);
    if level <= 0.0 {
        return 0.0;
    }
    let base = match (tier, index) {
        (0, 0) => data().cosmo.monument,
        (1, 3) => data().cosmo.measurement,
        _ => return 0.0,
    };
    (base * level).floor()
}

/// `Fountain_BonTOT(t, 13)`: the monument boost from the fountain upgrades.
fn fountain_bon_tot(save: &Save, water: usize) -> f64 {
    let level = save.hole_nested(31, water, 13);
    let marble = save.hole_nested(32, water, 13);
    let per_level = data().fountain_per_level.get(water).copied().unwrap_or(0.0);
    let marble_bonus = if marble <= 0.0 {
        1.0
    } else {
        1.5 + 0.5 * marble
    };
    (marble_bonus * level * per_level).round()
}

/// `computeBUpg`: a cavern upgrade's value, gated on the building.
fn compute_bupg(save: &Save, index: usize, value: f64) -> f64 {
    if save.hole(13, index) < 1.0 {
        return 0.0;
    }
    match index {
        48 => 10.0 * save.hole(11, 26),
        52 => 60.0 * log(save.hole(9, 0)).floor(),
        49 => {
            value
                * (save.hole(11, 1)
                    + save.hole(11, 3) * compute_bupg(save, 50, 1.0)
                    + save.hole(11, 5) * compute_bupg(save, 79, 1.0)
                    + save.hole(11, 7) * compute_bupg(save, 98, 1.0))
        }
        _ => value,
    }
}

/// `computeMeasurementBonus`: a measurement's total bonus.
fn measurement_bonus(save: &Save, index: usize) -> f64 {
    let level = save.hole(22, index);
    if level <= 0.0 {
        return 0.0;
    }
    let cosmo = cosmo_bonus(save, 1, 3);
    let base_text = data().measurement_base(index);
    let base = if base_text.contains("TOT") {
        (1.0 + cosmo / 100.0) * (parse_base(base_text) * level / (100.0 + level))
    } else {
        (1.0 + cosmo / 100.0) * parse_base(base_text) * level
    };
    base * measurement_multi(save, data().measurement_type(index) as usize)
}

/// `HolesInfo[55]` is text such as `"5TOT"`; only the number matters.
fn parse_base(text: &str) -> f64 {
    text.trim_end_matches("TOT").trim().parse().unwrap_or(0.0)
}

/// `MeasurementMulti(type)`. The monument formulas only reach types 0, 4, 5, 7,
/// 9 and 10; the remaining upstream types read systems outside this section.
fn measurement_multi(save: &Save, kind: usize) -> f64 {
    let quantity = match kind {
        4 => {
            let skills = save.account_skills();
            skills / 5000.0 + (skills - 18000.0).max(0.0) / 1500.0
        }
        5 => 0.0,
        9 => {
            let count = save
                .holes
                .get(26)
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            (0..count).map(|index| save.hole(26, index)).sum::<f64>() / 6.0
        }
        10 => {
            let found = save.hole(11, 63);
            if found > 0.0 {
                (log(found) - 2.0).max(0.0)
            } else {
                0.0
            }
        }
        0 => {
            let found = save.hole(11, 28);
            if found > 0.0 { log(found) } else { 0.0 }
        }
        7 => {
            let high = save.task(0, 1, 0);
            if high > 0.0 { log(high) / 2.0 } else { 0.0 }
        }
        _ => 0.0,
    };
    if quantity < 5.0 {
        1.0 + 18.0 * quantity / 100.0
    } else {
        1.0 + (18.0 * quantity + 8.0 * (quantity - 5.0)) / 100.0
    }
}

/// `ArcadeBonus`: a shop upgrade's value with its max-level and companion
/// multipliers.
fn arcade_bonus(save: &Save, index: usize) -> f64 {
    let Some(params) = data().arcade(index) else {
        return 0.0;
    };
    let level = save.arcade(index);
    if level <= 0.0 {
        return 0.0;
    }
    let kind = params.get(3).and_then(Value::as_str).unwrap_or("");
    let base = params.get(1).map(number).unwrap_or(0.0);
    let denominator = params.get(2).map(number).unwrap_or(0.0);
    let raw = if kind == "add" {
        if denominator != 0.0 {
            ((base + denominator) / denominator + 0.5 * (level - 1.0)) / (base / denominator)
                * level
                * base
        } else {
            base * level
        }
    } else {
        base * level / (level + denominator)
    };
    let maxed = if level == 101.0 {
        2.0
    } else if level == 102.0 {
        6.0
    } else {
        1.0
    };
    let companion = if companion_bonus_for_save(save, 27) == 1.0 {
        2.0
    } else {
        1.0
    };
    maxed * companion * raw
}

/// `achieveStatus`: a completed achievement's fixed bonus.
fn achieve_status(save: &Save, index: usize) -> f64 {
    if save.achievement(index) != -1.0 {
        return 0.0;
    }
    match index {
        4 | 27 | 37 | 44 | 107 | 109 | 117 => 5.0,
        108 => 10.0,
        99 | 104 | 112 => 20.0,
        _ => 1.0,
    }
}

/// `CompassBonus`: a compass upgrade's value.
fn compass_bonus(save: &Save, index: usize) -> f64 {
    let level = save.compass(0, index);
    if level <= 0.0 {
        return 0.0;
    }
    let Some(upgrade) = data().compass(index) else {
        return 0.0;
    };
    let per_level = upgrade.get(5).map(number).unwrap_or(0.0);
    let kind = upgrade.get(9).map(number).unwrap_or(0.0);
    if kind == 1.0 {
        (1.0 + (compass_bonus(save, 39) + compass_bonus(save, 80)) / 100.0) * level * per_level
    } else if index == 45 {
        level * per_level * 2.0_f64.powf((level / 50.0).floor())
    } else {
        level * per_level
    }
}

/// `collectibleBonus`: a jar collectible's level times its per-level effect.
fn collectible_bonus(save: &Save, index: usize, legend_pts: f64) -> f64 {
    let level = save.hole(24, index);
    let per_level = if index == 19 {
        data().collectible19
    } else {
        0.0
    };
    level * per_level * (1.0 + legend_pts / 100.0)
}

/// `legendPTSbonus`: a Legend talent's points times its per-point effect.
fn legend_pts_bonus(save: &Save, index: usize) -> f64 {
    (save.spelunk(18, index) * data().legend_per_point(index)).round()
}

/// `computeMeritocBonusz`: the bonus of the active Meritoc vote. A vote only
/// pays once its map is cleared, and a missing clear row leaves it unknown.
fn meritoc(save: &Save, option: usize, char_idx: usize) -> f64 {
    if save.options.is_empty() {
        return 0.0;
    }
    if save.option(453) != option as f64 {
        return 0.0;
    }
    let Some(kills_left) = save.kla_map(char_idx, 250) else {
        return 0.0;
    };
    if kills_left > 0.0 {
        return 0.0;
    }
    let base = data().meritoc_base(option);
    if base <= 0.0 {
        return 0.0;
    }
    let can_vote = save.option(472) == 1.0;
    let clam_work3 = if save.option(464) > 3.0 { 1.0 } else { 0.0 };
    let add = 5.0 * clam_work3
        + companion_bonus_for_save(save, 39)
        + legend_pts_bonus(save, 24)
        + arcade_bonus(save, 59)
        + 20.0 * event_shop(save, 23)
        + rog_bonus_qty(save, 51)
        + jelly_reward(save, 33);
    let multi = if can_vote { 1.0 } else { 0.25 } + add / 100.0;
    base * (1.0 + companion_bonus_for_save(save, 161) / 100.0) * multi
}

/// `eventShopOwned`: whether an event shop item's letter is in `OptLacc[311]`.
fn event_shop(save: &Save, index: usize) -> f64 {
    let Some(letter) = data().encoding_letter(index) else {
        return 0.0;
    };
    if save.option_text(311).contains(letter) {
        1.0
    } else {
        0.0
    }
}

/// `superBitType`: whether a gaming superbit's letter is in `Gaming[12]`.
fn superbit(save: &Save, index: usize) -> f64 {
    let Some(letter) = data().encoding_letter(index) else {
        return 0.0;
    };
    if save.gaming_text(12).contains(letter) {
        1.0
    } else {
        0.0
    }
}

/// `rogBonusQTY`: a sushi tier's bonus, paid once that many tiers are found.
fn rog_bonus_qty(save: &Save, index: usize) -> f64 {
    if save.unique_sushi() > index {
        data().rog_bonus_qty(index)
    } else {
        0.0
    }
}

/// `mineheadBonusQTY`: a Minehead floor reward's bonus.
fn minehead_bonus_qty(index: usize, floor: f64) -> f64 {
    if floor > index as f64 {
        data().minehead_bonus(index)
    } else {
        0.0
    }
}

fn state_r7_floor(save: &Save) -> f64 {
    save.state_r7.get(4).map(number).unwrap_or(0.0)
}

/// `computeWinBonus`: a summoning winner bonus, including its account
/// multipliers.
fn win_bonus(save: &Save, index: usize, char_idx: usize) -> f64 {
    let wins = summon_win_bonus(save);
    let raw = wins.get(index).copied().unwrap_or(0.0);
    if raw <= 0.0 {
        return 0.0;
    }
    if matches!(index, 20 | 22 | 24 | 31) {
        return raw;
    }
    let pristine = if save.ninja(107, 8) == 1.0 { 30.0 } else { 0.0 };
    let gems = save.gem_item(11);
    let artifact_rarity = save.sailing(3, 32);
    let artifact = if artifact_rarity > 0.0 {
        data().artifact32 * artifact_rarity
    } else {
        0.0
    };
    let task = save.task(2, 5, 4).min(10.0);
    let winner_31 = wins.get(31).copied().unwrap_or(0.0);
    let emperor = emperor_bonus(save, 8);
    let godshard = get_set_bonus(save, "GODSHARD_SET", char_idx);
    let achievements = achieve_status(save, 379) + achieve_status(save, 373);
    let bundle = if save.bundle("ban_i") { 50.0 } else { 0.0 };
    let base_mult = if (20..=33).contains(&index) { 1.0 } else { 3.5 };
    let mut winner_sum = artifact + task + achievements + godshard + bundle;
    if index != 19 {
        winner_sum += winner_31 + emperor;
    }
    base_mult
        * raw
        * (1.0 + pristine / 100.0)
        * (1.0 + 10.0 * gems / 100.0)
        * (1.0 + winner_sum / 100.0)
}

/// `computeSummWinBonus`: the 32 winner bonus types from normal and endless
/// victories.
fn summon_win_bonus(save: &Save) -> [f64; 32] {
    let mut bonus = [0.0; 32];
    for name in save.summon_wins() {
        let Some(name) = name.as_str() else {
            continue;
        };
        if name.starts_with("rift") {
            continue;
        }
        if let Some([kind, value]) = data().summon.normal.get(name) {
            let index = (*kind - 1.0).round();
            if (0.0..32.0).contains(&index) {
                bonus[index as usize] += *value;
            }
        }
    }
    let endless = save.option(319).max(0.0) as usize;
    for win in 0..endless {
        let slot = win % 40;
        let kind = data().summon.endless_type.get(slot).copied().unwrap_or(0.0) - 1.0;
        if (0.0..32.0).contains(&kind) {
            bonus[kind as usize] += data().summon.endless_val.get(slot).copied().unwrap_or(0.0);
        }
    }
    bonus
}

/// `computeEmperorBon`: the emperor-kill bonus of one type.
fn emperor_bonus(save: &Save, index: usize) -> f64 {
    let kills = save.option(369).max(0.0) as usize;
    let mut sum = 0.0;
    for kill in 0..kills {
        let slot = kill % 48;
        if (data().emperor_type(slot) - index as f64).abs() < f64::EPSILON {
            sum += data().emperor_value(index);
        }
    }
    let multi =
        1.0 + (arcane_48_bonus(save) + arcade_bonus(save, 51) + jelly_reward(save, 28)) / 100.0;
    (sum * multi).floor()
}

/// `arcaneUpgBonus(48)`; index 48 skips the mastery multiplier.
fn arcane_48_bonus(save: &Save) -> f64 {
    let level = save.arcane(48);
    if level <= 0.0 {
        0.0
    } else {
        level * data().arcane48_per_level
    }
}

/// `jellyRewardBonus`: a jelly reward is active once its obstruction is cleared.
fn jelly_reward(save: &Save, index: usize) -> f64 {
    if save.research(7, 9) > index as f64 {
        data().jelly_reward(index)
    } else {
        0.0
    }
}

/// `getSetBonus`: a permanent set or the same set actively equipped.
fn get_set_bonus(save: &Save, set_name: &str, char_idx: usize) -> f64 {
    let Some(set) = data().godshard_set() else {
        return 0.0;
    };
    if set_name != "GODSHARD_SET" {
        return 0.0;
    }
    if save.option_text(379).contains(set_name) || set_equipped(save, &set, char_idx) {
        set.bonus
    } else {
        0.0
    }
}

/// The upstream `checkSetEquipped` fallback for the permanent set check.
fn set_equipped(save: &Save, set: &data::GodshardSet<'_>, char_idx: usize) -> bool {
    let Some(equipment) = save.equipment(char_idx) else {
        return false;
    };
    let required = set.armor.len() + set.tools_cap + set.special_cap;
    let row0 = save::at(equipment, 0);
    let mut equipped = 0;
    for slot in 0..16 {
        if let Some(item) = row0
            .and_then(|row| save::at(row, slot))
            .and_then(Value::as_str)
            && set.armor.iter().any(|piece| piece.as_str() == Some(item))
        {
            equipped += 1;
        }
    }
    if set.tools_cap > 0 {
        let row1 = save::at(equipment, 1);
        let mut found = 0;
        for slot in 0..8 {
            if found >= set.tools_cap {
                break;
            }
            if let Some(item) = row1
                .and_then(|row| save::at(row, slot))
                .and_then(Value::as_str)
                && set.tools.iter().any(|piece| piece.as_str() == Some(item))
            {
                equipped += 1;
                found += 1;
            }
        }
    }
    if set.special_cap == 1 {
        for slot in 0..16 {
            if let Some(item) = row0
                .and_then(|row| save::at(row, slot))
                .and_then(Value::as_str)
                && set.special.iter().any(|piece| piece.as_str() == Some(item))
            {
                equipped += 1;
                break;
            }
        }
    }
    equipped >= required
}

/// `companionBonusForSave`: a companion's bonus, preferring its level-two value.
fn companion_bonus_for_save(save: &Save, index: usize) -> f64 {
    if !save.owns_companion(index as u32) {
        return 0.0;
    }
    let Some([base, enhanced]) = data().companion(index) else {
        return 0.0;
    };
    if save.companion_enhanced(index as u32) {
        enhanced
    } else {
        base
    }
}

/// `_study`: the Bolaia study bonus at `index`.
fn study(save: &Save, index: usize) -> f64 {
    let level = save.hole(26, index);
    if index == 9 {
        if level >= 1.0 {
            50.0 + level * data().bolaia_per_level(9)
        } else {
            0.0
        }
    } else if index == 3 {
        if level >= 1.0 {
            32.0_f64.min(12.0 + level * data().bolaia_per_level(3))
        } else {
            0.0
        }
    } else {
        level * data().bolaia_per_level(index)
    }
}

/// The Irwin-Hall CDF, symmetric so the upper tail does not cancel.
fn sum_cdf(x: f64, count: u32, low: f64, high: f64) -> f64 {
    if count == 0 {
        return if x <= 0.0 { 0.0 } else { 1.0 };
    }
    let (low, high) = if low <= high {
        (low, high)
    } else {
        (high, low)
    };
    if (low - high).abs() < f64::EPSILON {
        return if x <= f64::from(count) * low {
            0.0
        } else {
            1.0
        };
    }
    let count_f = f64::from(count);
    let z = (x - count_f * low) / (high - low);
    if z <= 0.0 {
        return 0.0;
    }
    if z >= count_f {
        return 1.0;
    }
    if z > count_f / 2.0 {
        return 1.0 - sum_cdf(count_f - z, count, 0.0, 1.0);
    }
    let mut sum = 0.0;
    let mut choose = 1.0;
    let mut factorial = 1.0;
    for k in 2..=count {
        factorial *= f64::from(k);
    }
    for k in 0..=(z.floor() as u32) {
        if k > 0 {
            choose = choose * f64::from(count - k + 1) / f64::from(k);
        }
        let term = choose * (z - f64::from(k)).powi(count as i32);
        sum += if k % 2 == 1 { -term } else { term };
    }
    (sum / factorial).clamp(0.0, 1.0)
}

fn number(value: &Value) -> f64 {
    save::number(value)
}

#[cfg(test)]
mod tests;
