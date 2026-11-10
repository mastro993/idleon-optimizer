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
}

impl StatueSave {
    pub fn new(characters: Vec<Vec<Option<Progress>>>, names: Vec<String>) -> Self {
        Self { characters, names }
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
