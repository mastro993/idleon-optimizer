//! Statue levels, EXP requirements and availability.
//!
//! Ported from the upstream statue calculator:
//! `js/stats/data/common/statues.js` (the 32-row table) and
//! `js/stats/systems/common/statue-leveling.js` (`statueExpRequirement`).
//! The per-level EXP is `level^1.17 * 1.35^(level/10) + 1`, rounded unless the
//! rounding gate statue is above level 300. Statues 10-19, 20-29 and 30-31 round
//! with the saved level of statue 1, 2 and 3; single-digit statues gate on the
//! level being computed.

use serde::Deserialize;
use std::sync::OnceLock;

const STATUES_JSON: &str = include_str!("../data/statues.json");

/// One row of the upstream `StatueInfo` table.
#[derive(Clone, Debug, Deserialize)]
pub struct StatueInfo {
    pub index: usize,
    pub name: String,
    pub effect: String,
    pub percent: bool,
    #[serde(rename = "basePerLevel")]
    pub base_per_level: f64,
}

#[derive(Debug, Deserialize)]
struct Statues {
    statues: Vec<StatueInfo>,
}

/// The bundled statue table, parsed once.
fn statues() -> &'static Vec<StatueInfo> {
    static STATUES: OnceLock<Vec<StatueInfo>> = OnceLock::new();
    STATUES.get_or_init(|| {
        serde_json::from_str::<Statues>(STATUES_JSON)
            .expect("bundled statue data is valid JSON")
            .statues
    })
}

/// How many statues the game has.
pub fn count() -> usize {
    statues().len()
}

/// One statue's static data, by game order.
pub fn info(index: usize) -> Option<&'static StatueInfo> {
    statues().get(index)
}

/// Where a statue stands: its saved level and the EXP banked towards the next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    pub level: u32,
    pub exp: f64,
}

/// The saved statue state for every character a save carries.
#[derive(Clone, Debug, Default)]
pub struct StatueSave {
    /// Indexed by character, then by statue; a missing or unreadable row is `None`.
    characters: Vec<Vec<Option<Progress>>>,
    /// Character names when the save carries them, e.g. from `it.json`.
    names: Vec<String>,
    /// The account multipliers a deposit carries; zero when the save has none.
    leveling: Leveling,
}

impl StatueSave {
    pub fn new(characters: Vec<Vec<Option<Progress>>>, names: Vec<String>) -> Self {
        Self {
            characters,
            names,
            leveling: Leveling::default(),
        }
    }

    /// Attach the account multipliers decoded from the same save.
    pub fn set_leveling(&mut self, leveling: Leveling) {
        self.leveling = leveling;
    }

    /// The STARTUE bubble, vial, tier multiplier and Flair levels behind a deposit.
    pub fn leveling(&self) -> &Leveling {
        &self.leveling
    }

    /// Whether any character carries a readable statue row.
    pub fn is_empty(&self) -> bool {
        self.character_indices().next().is_none()
    }

    /// Whether `character` carries at least one readable statue row.
    pub fn has_character(&self, character: usize) -> bool {
        self.characters
            .get(character)
            .is_some_and(|rows| rows.iter().any(Option::is_some))
    }

    /// The characters a selector can offer: those with at least one statue row.
    pub fn character_indices(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.characters.len()).filter(|index| self.has_character(*index))
    }

    /// The first character with statue data, if any.
    pub fn first_character(&self) -> Option<usize> {
        self.character_indices().next()
    }

    /// A character's saved name, or `Character N` when the save carries none.
    pub fn name(&self, character: usize) -> String {
        self.names
            .get(character)
            .filter(|name| !name.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| format!("Character {}", character + 1))
    }

    /// One statue's saved level and EXP.
    pub fn progress(&self, character: usize, statue: usize) -> Option<Progress> {
        self.characters
            .get(character)?
            .get(statue)
            .copied()
            .flatten()
    }

    /// How many of the game's statues have no readable row for `character`.
    pub fn missing_rows(&self, character: usize) -> usize {
        let Some(rows) = self.characters.get(character) else {
            return count();
        };
        (0..count())
            .filter(|statue| rows.get(*statue).copied().flatten().is_none())
            .count()
    }
}

/// Why a statue's requirement cannot be computed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// The statue itself, or its rounding gate, has no saved level.
    NoRow(usize),
    /// The index is not a game statue.
    NoStatue(usize),
    /// The level is past the finite range of the formula.
    OutOfRange,
}

/// The saved statue and the EXP its next level needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NextLevel {
    pub level: u32,
    pub exp: f64,
    /// The full bar that triggers the level-up, from zero.
    pub required: f64,
    /// The EXP still missing from the saved progress.
    pub remaining: f64,
}

/// The upstream `statueExpRequirement`: the EXP bar for `level` on `statue`.
///
/// `level` is the level being computed. For single-digit statues the rounding
/// gate is that same level; for 10-19, 20-29 and 30-31 it is the saved level of
/// statue 1, 2 and 3. A gate above 300 keeps the unrounded value.
pub fn exp_requirement(
    save: &StatueSave,
    character: usize,
    statue: usize,
    level: u32,
) -> Result<f64, Unavailable> {
    if statue >= count() {
        return Err(Unavailable::NoStatue(statue));
    }
    let gate = if statue < 10 { statue } else { statue / 10 };
    let gate_level = if gate == statue {
        level
    } else {
        save.progress(character, gate)
            .ok_or(Unavailable::NoRow(gate))?
            .level
    };
    let level = f64::from(level);
    let raw = level.powf(1.17) * 1.35f64.powf(level / 10.0) + 1.0;
    if !raw.is_finite() {
        return Err(Unavailable::OutOfRange);
    }
    Ok(if gate_level > 300 { raw } else { raw.round() })
}

/// The next level's bar and what remains of the current one.
pub fn next_level(
    save: &StatueSave,
    character: usize,
    statue: usize,
) -> Result<NextLevel, Unavailable> {
    let progress = save
        .progress(character, statue)
        .ok_or(Unavailable::NoRow(statue))?;
    let required = exp_requirement(save, character, statue, progress.level)?;
    Ok(NextLevel {
        level: progress.level,
        exp: progress.exp,
        required,
        remaining: (required - progress.exp).max(0.0),
    })
}

/// The account multipliers a statue deposit carries.
///
/// Ported from `statueLevelingInputs` in
/// `js/stats/systems/common/statue-leveling.js`: the STARTUE EXP bubble lowers
/// the EXP a level consumes, the Skinny 0 Cal vial doubles deposits, the Onyx
/// and Zenith statue tiers multiply them, and Royal Guardian Flair multiplies
/// them per statue.
#[derive(Clone, Debug, PartialEq)]
pub struct Leveling {
    /// The bubble's bonus, `25 × level / (level + 60)` times its Prisma bonus.
    pub bubble_pct: f64,
    /// The share of a level's EXP still consumed, capped at 50%.
    pub cost_factor: f64,
    /// `(1 - cost_factor) × 100`, the bubble's effective retention.
    pub retention_pct: f64,
    /// The Skinny 0 Cal vial's chance to double one deposit, as a fraction.
    pub double_chance: f64,
    /// Statues at Onyx tier or above.
    pub onyx: u32,
    /// Statues at Zenith tier.
    pub zenith: u32,
    /// The shared deposit multiplier: `1 + 10% × Onyx + 15% × Zenith`.
    pub tier_multiplier: f64,
    /// Royal Guardian Flair level per statue; empty when the save has no row.
    pub flair: Vec<u32>,
    /// Whether the save carries Royal Guardian data at all.
    pub flair_available: bool,
}

impl Default for Leveling {
    fn default() -> Self {
        Self {
            bubble_pct: 0.0,
            cost_factor: 1.0,
            retention_pct: 0.0,
            double_chance: 0.0,
            onyx: 0,
            zenith: 0,
            tier_multiplier: 1.0,
            flair: Vec::new(),
            flair_available: false,
        }
    }
}

impl Leveling {
    /// The inputs from their decoded parts; the bubble, vial and tier
    /// multipliers are clamped exactly as `statueLevelingInputs` does.
    pub fn new(
        bubble_pct: f64,
        double_chance: f64,
        onyx: u32,
        zenith: u32,
        flair: Vec<u32>,
        flair_available: bool,
    ) -> Self {
        let bubble_pct = if bubble_pct.is_finite() {
            bubble_pct.max(0.0)
        } else {
            0.0
        };
        let cost_factor = (1.0 - bubble_pct / 100.0).max(0.5);
        let double_chance = if double_chance.is_finite() {
            double_chance.clamp(0.0, 1.0)
        } else {
            0.0
        };
        Self {
            bubble_pct,
            cost_factor,
            retention_pct: (1.0 - cost_factor) * 100.0,
            double_chance,
            onyx,
            zenith,
            tier_multiplier: 1.0 + (0.1 * f64::from(onyx) + 0.15 * f64::from(zenith)).max(0.0),
            flair,
            flair_available,
        }
    }

    /// One statue's saved Flair level.
    pub fn flair_level(&self, statue: usize) -> u32 {
        self.flair.get(statue).copied().unwrap_or(0)
    }

    /// One statue's deposit multiplier from its Flair.
    pub fn flair_multiplier(&self, statue: usize) -> f64 {
        flair_multiplier(self.flair_level(statue))
    }
}

/// `statueFlairMultiplier`: Flair 0 / 1 / 2 / 3 / 4 gives 1× / 7× / 15× / 25× / 33×.
pub fn flair_multiplier(level: u32) -> f64 {
    let level = f64::from(level);
    let bonus =
        2.0 * (300.0 * level + 100.0 * (level / 2.0).floor() + 200.0 * (level / 3.0).floor());
    1.0 + bonus / 100.0
}

/// Which deposit estimate the table shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Estimate {
    /// Every deposit counts once unless the vial chance reaches 100%.
    #[default]
    Guaranteed,
    /// The long-run mean with the vial's double chance.
    Average,
}

impl Estimate {
    pub const ALL: [Estimate; 2] = [Estimate::Guaranteed, Estimate::Average];

    /// The EXP one deposited statue gives under this estimate.
    pub fn deposit(self, cost: &TargetCost) -> f64 {
        match self {
            Estimate::Guaranteed => cost.guaranteed_exp,
            Estimate::Average => cost.average_exp,
        }
    }

    /// The statues a target needs under this estimate.
    pub fn statues(self, cost: &TargetCost) -> u64 {
        match self {
            Estimate::Guaranteed => cost.statues_guaranteed,
            Estimate::Average => cost.statues_average,
        }
    }
}

/// The EXP one target level costs from the saved state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetCost {
    /// The saved level and its banked EXP.
    pub level: u32,
    pub exp: f64,
    /// The saved level's own bar, as [`NextLevel::required`].
    pub required: f64,
    /// The level the target asks for.
    pub target: u32,
    /// EXP still missing from the saved progress, retention included.
    pub exp_needed: f64,
    pub flair_level: u32,
    pub flair_multiplier: f64,
    /// EXP per deposited statue without the vial's doubles.
    pub guaranteed_exp: f64,
    /// EXP per deposited statue with the vial's double chance.
    pub average_exp: f64,
    pub statues_guaranteed: u64,
    pub statues_average: u64,
}

/// `statueTargetCost`: the EXP a target level needs and how many statues that
/// is, with the STARTUE bubble's retention applied. A target at or below the
/// saved level needs nothing, as the upstream loop reads.
pub fn target_cost(
    save: &StatueSave,
    leveling: &Leveling,
    character: usize,
    statue: usize,
    target: u32,
) -> Result<TargetCost, Unavailable> {
    let progress = save
        .progress(character, statue)
        .ok_or(Unavailable::NoRow(statue))?;
    let required = exp_requirement(save, character, statue, progress.level)?;
    let flair_level = leveling.flair_level(statue);
    let flair_multiplier = leveling.flair_multiplier(statue);
    let base = leveling.tier_multiplier * flair_multiplier;
    let guaranteed_exp = base
        * if leveling.double_chance >= 1.0 {
            2.0
        } else {
            1.0
        };
    let average_exp = base * (1.0 + leveling.double_chance);
    if !guaranteed_exp.is_finite()
        || guaranteed_exp <= 0.0
        || !average_exp.is_finite()
        || average_exp <= 0.0
    {
        return Err(Unavailable::OutOfRange);
    }
    let mut consumed = 0.0_f64;
    let mut threshold = 0.0_f64;
    for level in progress.level..target {
        let requirement = exp_requirement(save, character, statue, level)?;
        // Retention lowers the EXP consumed, not the full bar that triggers it.
        threshold = threshold.max(consumed + requirement);
        consumed += requirement * leveling.cost_factor;
        if !threshold.is_finite() || !consumed.is_finite() {
            return Err(Unavailable::OutOfRange);
        }
    }
    let exp_needed = (threshold - progress.exp).max(0.0);
    Ok(TargetCost {
        level: progress.level,
        exp: progress.exp,
        required,
        target,
        exp_needed,
        flair_level,
        flair_multiplier,
        guaranteed_exp,
        average_exp,
        statues_guaranteed: statue_count(exp_needed, guaranteed_exp),
        statues_average: statue_count(exp_needed, average_exp),
    })
}

/// `Math.ceil(needed / deposit)`, saturating instead of wrapping.
fn statue_count(needed: f64, deposit: f64) -> u64 {
    let count = (needed / deposit).ceil();
    if !count.is_finite() || count <= 0.0 {
        return 0;
    }
    count.min(u64::MAX as f64) as u64
}

/// The farm time for `statues` at `rate` raw statues per hour: zero when the
/// target is met, endless when no rate is entered, as the upstream `time()`.
pub fn hours(statues: u64, rate: f64) -> f64 {
    if statues == 0 {
        0.0
    } else if rate.is_finite() && rate > 0.0 {
        statues as f64 / rate
    } else {
        f64::INFINITY
    }
}

/// Why a farming-rate entry could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateError {
    /// Not a nonnegative number with an optional `k`/`m`/`b`/`t` suffix.
    Invalid,
    /// The scaled rate is past `f64`'s finite range.
    TooLarge,
}

impl RateError {
    /// The message the UI shows, as the upstream rate parser writes it.
    pub fn message(self) -> &'static str {
        match self {
            RateError::Invalid => "Enter a nonnegative rate, such as 500K, 1.2M or 1e9.",
            RateError::TooLarge => "Farming rate is too large.",
        }
    }
}

/// The upstream `parseRate`: a nonnegative number with an optional exponent and
/// `K`, `M`, `B` or `T` suffix. An empty entry is no rate (zero).
pub fn parse_rate(text: &str) -> Result<f64, RateError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(0.0);
    }
    let (number, suffix) = match text.chars().last() {
        Some(suffix) if suffix.is_ascii_alphabetic() => {
            (&text[..text.len() - suffix.len_utf8()], Some(suffix))
        }
        _ => (text, None),
    };
    let number = number.trim_end();
    let scale = match suffix.map(|suffix| suffix.to_ascii_lowercase()) {
        None => 1.0,
        Some('k') => 1e3,
        Some('m') => 1e6,
        Some('b') => 1e9,
        Some('t') => 1e12,
        Some(_) => return Err(RateError::Invalid),
    };
    if !rate_number(number) {
        return Err(RateError::Invalid);
    }
    let value = number.parse::<f64>().map_err(|_| RateError::Invalid)? * scale;
    if !value.is_finite() {
        return Err(RateError::TooLarge);
    }
    Ok(value)
}

/// The upstream rate regex, without the regex: `\d+(\.\d*)?|\.\d+`, then an
/// optional signed exponent, and nothing else.
fn rate_number(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut digits = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
        digits += 1;
    }
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return false;
    }
    if index < bytes.len() && (bytes[index] == b'e' || bytes[index] == b'E') {
        index += 1;
        if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
            index += 1;
        }
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == start {
            return false;
        }
    }
    index == bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(level: u32, exp: f64) -> Option<Progress> {
        Some(Progress { level, exp })
    }

    fn sample(rows: Vec<Option<Progress>>) -> StatueSave {
        StatueSave::new(vec![rows], vec!["main".into()])
    }

    #[test]
    fn the_bundled_table_covers_every_statue() {
        assert_eq!(count(), 32);
        assert_eq!(info(0).unwrap().name, "POWER");
        assert_eq!(info(0).unwrap().base_per_level, 3.0);
        assert_eq!(info(1).unwrap().effect, "MOVE SPEED");
        assert!(info(1).unwrap().percent);
        assert_eq!(info(31).unwrap().name, "CORAL");
        assert!(info(32).is_none());
    }

    #[test]
    fn single_digit_statues_round_on_their_own_target_level() {
        let save = sample(vec![row(10, 0.0); count()]);
        assert_eq!(exp_requirement(&save, 0, 0, 1).unwrap(), 2.0);
        assert_eq!(exp_requirement(&save, 0, 5, 141).unwrap(), 22_507.0);
        assert_eq!(exp_requirement(&save, 0, 9, 249).unwrap(), 1_119_089.0);
    }

    #[test]
    fn higher_statues_round_on_their_gate_statue() {
        let mut rows = vec![row(0, 0.0); count()];
        rows[1] = row(141, 0.0);
        let save = sample(rows.clone());
        // Statue 10 rounds on statue 1's saved level (141), not its own level.
        assert_eq!(exp_requirement(&save, 0, 10, 432).unwrap(), 517_529_111.0);
        // Past gate level 300 the raw value is kept.
        rows[1] = row(301, 0.0);
        let save = sample(rows);
        let raw = exp_requirement(&save, 0, 10, 432).unwrap();
        assert_ne!(raw, raw.round());
    }

    #[test]
    fn a_missing_gate_row_is_unavailable() {
        let mut rows = vec![row(10, 0.0); count()];
        rows[1] = None;
        let save = sample(rows);
        assert_eq!(
            exp_requirement(&save, 0, 10, 432),
            Err(Unavailable::NoRow(1))
        );
        assert_eq!(next_level(&save, 0, 10), Err(Unavailable::NoRow(1)));
    }

    #[test]
    fn next_level_reports_the_full_bar_and_the_remainder() {
        let mut rows = vec![row(10, 0.0); count()];
        rows[3] = row(141, 100.0);
        let save = sample(rows);
        let next = next_level(&save, 0, 3).unwrap();
        assert_eq!(next.level, 141);
        assert_eq!(next.exp, 100.0);
        assert_eq!(next.required, 22_507.0);
        assert_eq!(next.remaining, 22_407.0);
    }

    #[test]
    fn an_unknown_character_or_statue_is_unavailable() {
        let save = sample(vec![row(10, 0.0); count()]);
        assert_eq!(next_level(&save, 1, 0), Err(Unavailable::NoRow(0)));
        assert_eq!(
            next_level(&save, 0, count()),
            Err(Unavailable::NoRow(count()))
        );
        assert_eq!(
            exp_requirement(&save, 0, count(), 1),
            Err(Unavailable::NoStatue(count()))
        );
    }

    #[test]
    fn the_startue_bubble_caps_retention_at_half() {
        let none = Leveling::new(0.0, 0.0, 0, 0, Vec::new(), false);
        assert_eq!(none.cost_factor, 1.0);
        assert_eq!(none.retention_pct, 0.0);
        assert_eq!(none.tier_multiplier, 1.0);

        let part = Leveling::new(20.0, 0.0, 0, 0, Vec::new(), false);
        assert!((part.cost_factor - 0.8).abs() < 1e-12);
        assert!((part.retention_pct - 20.0).abs() < 1e-12);

        let full = Leveling::new(68.4, 0.0, 0, 0, Vec::new(), false);
        assert_eq!(full.cost_factor, 0.5, "retention is capped at 50%");
        assert_eq!(full.retention_pct, 50.0);
        let negative = Leveling::new(-30.0, 0.0, 0, 0, Vec::new(), false);
        assert_eq!(negative.bubble_pct, 0.0, "a negative bubble reads as none");
    }

    #[test]
    fn the_tier_multiplier_adds_onyx_and_zenith() {
        let leveling = Leveling::new(0.0, 0.0, 10, 4, Vec::new(), false);
        assert!((leveling.tier_multiplier - (1.0 + 0.1 * 10.0 + 0.15 * 4.0)).abs() < 1e-12);
        assert_eq!(leveling.double_chance, 0.0);
        let capped = Leveling::new(0.0, 3.0, 0, 0, Vec::new(), false);
        assert_eq!(
            capped.double_chance, 1.0,
            "the vial chance is clamped to 100%"
        );
    }

    #[test]
    fn flair_multipliers_are_the_saved_tiers() {
        assert_eq!(flair_multiplier(0), 1.0);
        assert_eq!(flair_multiplier(1), 7.0);
        assert_eq!(flair_multiplier(2), 15.0);
        assert_eq!(flair_multiplier(3), 25.0);
        assert_eq!(flair_multiplier(4), 33.0);
    }

    #[test]
    fn the_bubble_changes_the_statues_a_target_needs() {
        let save = sample(vec![row(100, 0.0); count()]);
        let none = Leveling::default();
        let bubbled = Leveling::new(50.0, 0.0, 0, 0, Vec::new(), true);
        let plain = target_cost(&save, &none, 0, 0, 105).unwrap();
        let cheap = target_cost(&save, &bubbled, 0, 0, 105).unwrap();
        assert!(
            cheap.exp_needed < plain.exp_needed,
            "retention lowers the EXP a target needs: {} vs {}",
            cheap.exp_needed,
            plain.exp_needed
        );
        assert!(cheap.statues_guaranteed < plain.statues_guaranteed);
    }

    #[test]
    fn retention_lowers_consumed_exp_but_keeps_the_triggering_bar() {
        let save = sample(vec![row(10, 0.0); count()]);
        let leveling = Leveling::new(50.0, 0.0, 0, 0, Vec::new(), true);
        let cost = target_cost(&save, &leveling, 0, 0, 13).unwrap();
        let bar = |level| exp_requirement(&save, 0, 0, level).unwrap();
        // Levels 10 and 11 consume half; level 12 is the bar that triggers the
        // last level-up, so it is required in full.
        let expected = 0.5 * (bar(10) + bar(11)) + bar(12);
        assert!((cost.exp_needed - expected).abs() <= expected * 1e-12);
    }

    #[test]
    fn a_target_at_or_below_the_saved_level_needs_nothing() {
        let save = sample(vec![row(42, 7.0); count()]);
        let leveling = Leveling::default();
        for target in [0, 41, 42] {
            let cost = target_cost(&save, &leveling, 0, 0, target).unwrap();
            assert_eq!(cost.exp_needed, 0.0, "target {target}");
            assert_eq!(cost.statues_guaranteed, 0);
        }
    }

    #[test]
    fn the_estimate_switches_between_guaranteed_and_average_deposits() {
        let save = sample(vec![row(10, 0.0); count()]);
        let leveling = Leveling::new(0.0, 1.0, 0, 0, Vec::new(), true);
        let cost = target_cost(&save, &leveling, 0, 0, 20).unwrap();
        assert_eq!(cost.flair_level, 0, "a save without a Flair row assumes 1x");
        assert_eq!(cost.flair_multiplier, 1.0);
        assert_eq!(
            cost.guaranteed_exp, 2.0,
            "a full vial chance doubles deposits"
        );
        assert_eq!(cost.average_exp, 2.0);
        assert_eq!(
            Estimate::Guaranteed.statues(&cost),
            Estimate::Average.statues(&cost)
        );
        assert_eq!(Estimate::Guaranteed.deposit(&cost), 2.0);

        let partial = Leveling::new(0.0, 0.25, 0, 0, Vec::new(), true);
        let cost = target_cost(&save, &partial, 0, 1, 20).unwrap();
        assert_eq!(cost.guaranteed_exp, 1.0);
        assert_eq!(cost.average_exp, 1.25);
        assert!(cost.statues_average < cost.statues_guaranteed);
    }

    #[test]
    fn a_flair_row_multiplies_that_statue_only() {
        let save = sample(vec![row(10, 0.0); count()]);
        let leveling = Leveling::new(0.0, 0.0, 0, 0, vec![0, 2], true);
        let plain = target_cost(&save, &leveling, 0, 0, 20).unwrap();
        let flaired = target_cost(&save, &leveling, 0, 1, 20).unwrap();
        assert_eq!(plain.flair_multiplier, 1.0);
        assert_eq!(flaired.flair_level, 2);
        assert_eq!(flaired.flair_multiplier, 15.0);
        assert!(flaired.guaranteed_exp > plain.guaranteed_exp);
        assert!(flaired.statues_guaranteed < plain.statues_guaranteed);
    }

    #[test]
    fn a_missing_row_cannot_be_costed() {
        let mut rows = vec![row(10, 0.0); count()];
        rows[5] = None;
        let save = sample(rows);
        assert_eq!(
            target_cost(&save, &Leveling::default(), 0, 5, 12),
            Err(Unavailable::NoRow(5))
        );
        assert_eq!(
            target_cost(&save, &Leveling::default(), 0, count(), 12),
            Err(Unavailable::NoRow(count()))
        );
    }

    #[test]
    fn a_rate_reads_suffixes_exponents_and_empty_entries() {
        assert_eq!(parse_rate(""), Ok(0.0));
        assert_eq!(parse_rate("   "), Ok(0.0));
        assert_eq!(parse_rate("500"), Ok(500.0));
        assert_eq!(parse_rate("500K"), Ok(500_000.0));
        assert_eq!(parse_rate("1.2m"), Ok(1_200_000.0));
        assert_eq!(parse_rate(" 2 B "), Ok(2_000_000_000.0));
        assert_eq!(parse_rate(".5"), Ok(0.5));
        assert_eq!(parse_rate("1e9"), Ok(1e9));
        assert_eq!(parse_rate("1.5e-3"), Ok(0.0015));
        assert_eq!(parse_rate("2.5 T"), Ok(2.5e12));
    }

    #[test]
    fn an_invalid_rate_names_the_entry() {
        for text in ["abc", "-1", "1.2.3", "1e", "K", "1 K M", "NaN", "inf"] {
            assert_eq!(
                parse_rate(text),
                Err(RateError::Invalid),
                "the rate {text:?} is rejected"
            );
        }
        assert_eq!(parse_rate("1e999"), Err(RateError::TooLarge));
        assert_eq!(
            RateError::Invalid.message(),
            "Enter a nonnegative rate, such as 500K, 1.2M or 1e9."
        );
    }

    #[test]
    fn farm_time_waits_on_a_rate_and_reads_ready_at_zero() {
        assert_eq!(hours(0, 500.0), 0.0);
        assert_eq!(hours(10, 2.0), 5.0);
        assert!(hours(10, 0.0).is_infinite());
        assert!(hours(10, -1.0).is_infinite());
        assert!(hours(10, f64::INFINITY).is_infinite());
    }

    #[test]
    fn characters_without_a_readable_row_are_not_offered() {
        let save = StatueSave::new(
            vec![vec![None; count()], vec![row(1, 0.0); count()]],
            vec![],
        );
        assert!(!save.is_empty());
        assert_eq!(save.first_character(), Some(1));
        assert_eq!(save.name(1), "Character 2");
        assert_eq!(save.missing_rows(0), count());
        assert_eq!(save.missing_rows(1), 0);
    }
}
