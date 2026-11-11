//! Batch target entry: Set all targets and Next level for all, with the plan
//! they rewrite and the one undo step a batch action leaves.
//!
//! Ported from `statue-calc.html`: the `#apply-all` handler reads the entry as a
//! whole nonnegative level, costs that level for every statue and only then
//! writes the plan, so an entry the save cannot cost changes nothing.
//! `#next-level` clears the explicit targets, so every row falls back to its
//! saved level plus one. Upstream keeps no history; the acceptance asks for
//! undoable batch actions, so the plan remembers the targets it replaced.

use super::{Leveling, StatueSave, Unavailable, count, info, target_cost};

/// The largest whole level a target can hold: the plan stores a `u32`.
const MAX_LEVEL: f64 = u32::MAX as f64;

/// Why a batch entry left the plan alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchError {
    /// Not a whole nonnegative level.
    InvalidLevel,
    /// One statue's target cannot be costed.
    Unavailable(usize, Unavailable),
}

impl BatchError {
    /// The message the set-all field shows under the buttons.
    pub fn message(self) -> String {
        match self {
            BatchError::InvalidLevel => "Enter a nonnegative whole target level.".to_string(),
            BatchError::Unavailable(statue, Unavailable::NoRow(gate)) => {
                match (info(statue), info(gate)) {
                    (Some(statue), Some(gate)) => format!(
                        "{} rounds its EXP on the saved level of {}, which this save lacks.",
                        statue.name, gate.name
                    ),
                    _ => "Statue EXP rounding needs a statue row this save lacks.".to_string(),
                }
            }
            BatchError::Unavailable(_, Unavailable::OutOfRange) => {
                "Statue level exceeds the finite EXP calculation range.".to_string()
            }
            BatchError::Unavailable(_, Unavailable::TargetOutOfRange) => {
                "Statue target EXP exceeds the finite calculation range.".to_string()
            }
            BatchError::Unavailable(statue, _) => match info(statue) {
                Some(info) => format!("{} cannot be planned from this save.", info.name),
                None => "That statue is not in the game's table.".to_string(),
            },
        }
    }
}

/// The Set all entry as the upstream handler reads it: a whole number at or
/// above zero, and no larger than a target can hold.
pub fn parse_level(text: &str) -> Result<u32, BatchError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(BatchError::InvalidLevel);
    }
    let value: f64 = text.parse().map_err(|_| BatchError::InvalidLevel)?;
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > MAX_LEVEL {
        return Err(BatchError::InvalidLevel);
    }
    Ok(value as u32)
}

/// The targets a batch action replaced, and the action's name.
#[derive(Clone, Debug, PartialEq)]
struct Undo {
    label: &'static str,
    targets: Vec<Option<u32>>,
}

/// The plan inputs the table edits: the explicit targets and the raw
/// statues/hour entries for the selected character, with the targets the last
/// batch action replaced kept for undo.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    targets: Vec<Option<u32>>,
    rates: Vec<String>,
    undo: Option<Undo>,
}

impl Default for Plan {
    fn default() -> Self {
        Self {
            targets: vec![None; count()],
            rates: vec![String::new(); count()],
            undo: None,
        }
    }
}

impl Plan {
    pub fn new() -> Self {
        Self::default()
    }

    /// One statue's entered target; `None` follows the saved level plus one.
    pub fn target(&self, statue: usize) -> Option<u32> {
        self.targets.get(statue).copied().flatten()
    }

    /// Every statue's entered target, one entry per game statue.
    pub fn targets(&self) -> &[Option<u32>] {
        &self.targets
    }

    /// One statue's raw statues/hour entry, trimmed.
    pub fn rate(&self, statue: usize) -> &str {
        self.rates
            .get(statue)
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// Every statue's raw statues/hour entry.
    pub fn rates(&self) -> &[String] {
        &self.rates
    }

    /// Set one statue's target. A hand edit ends the undo step, because the
    /// batch action no longer describes what is on screen.
    pub fn set_target(&mut self, statue: usize, target: u32) {
        if let Some(slot) = self.targets.get_mut(statue) {
            *slot = Some(target);
            self.undo = None;
        }
    }

    /// Set one statue's raw statues/hour entry.
    pub fn set_rate(&mut self, statue: usize, text: impl Into<String>) {
        if let Some(slot) = self.rates.get_mut(statue) {
            *slot = text.into();
        }
    }

    /// Replace both entries from a project config, sized to the table and
    /// without an undo step: the config is a fresh plan.
    pub fn reset(&mut self, targets: Vec<Option<u32>>, rates: Vec<String>) {
        self.targets = targets;
        self.targets.resize(count(), None);
        self.rates = rates;
        self.rates.resize(count(), String::new());
        for rate in &mut self.rates {
            *rate = rate.trim().to_string();
        }
        self.undo = None;
    }

    /// Clear both entries, as an import or a character switch does.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Set every statue's target to the level in `text`, after costing that
    /// level for each statue, and hand the level back for the status line.
    /// Statues without a readable row are skipped, as the upstream handler
    /// leaves their rows unavailable rather than rejecting the batch.
    pub fn set_all(
        &mut self,
        save: &StatueSave,
        character: usize,
        text: &str,
    ) -> Result<u32, BatchError> {
        let level = parse_level(text)?;
        let leveling: &Leveling = save.leveling();
        for statue in 0..count() {
            if save.progress(character, statue).is_none() {
                continue;
            }
            if let Err(error) = target_cost(save, leveling, character, statue, level) {
                return Err(BatchError::Unavailable(statue, error));
            }
        }
        self.remember("set all targets");
        self.targets = vec![Some(level); count()];
        Ok(level)
    }

    /// Next level for all: clear the entered targets, so every row follows its
    /// saved level plus one.
    pub fn next_level(&mut self) {
        self.remember("next level for all");
        self.targets = vec![None; count()];
    }

    /// Restore the targets the last batch action replaced.
    pub fn undo(&mut self) -> Option<&'static str> {
        let undo = self.undo.take()?;
        self.targets = undo.targets;
        Some(undo.label)
    }

    /// The batch action an undo would reverse, for the button's label.
    pub fn undo_label(&self) -> Option<&'static str> {
        self.undo.as_ref().map(|undo| undo.label)
    }

    fn remember(&mut self, label: &'static str) {
        self.undo = Some(Undo {
            label,
            targets: self.targets.clone(),
        });
    }
}
