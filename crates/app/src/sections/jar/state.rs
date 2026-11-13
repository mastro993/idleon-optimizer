//! The enchant simulator's controls and the history the project keeps.
//!
//! The knobs are the upstream `esim-*` inputs: how many enchants a simulation
//! distributes, how many simulations to run, the strategy and its focus, the
//! level each collectible is aimed at and the seed. The history is the
//! `_saveSnapshot` spread from `jar-calc.html`, saved with the project instead
//! of `localStorage`.

use idleon_jar::{COLLECTIBLES, EnchantSnapshot, HISTORY_LIMIT, SimInputs, Strategy};
use std::collections::BTreeMap;

/// The page's limits on the enchant count and the simulation count, which the
/// controls and a run both apply.
pub(crate) const ENCHANTS: (u32, u32) = (1, 1_000);
pub(crate) const SIMS: (u32, u32) = (100, 10_000);
/// The highest target level the targets grid accepts.
pub(crate) const TARGET_MAX: u32 = 100;

/// The simulator's controls and the saved spreads.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    /// `esim-target`: the enchants each simulation distributes.
    pub(crate) enchants: u32,
    /// `esim-count`: how many simulations to run.
    pub(crate) sims: u32,
    pub(crate) strategy: Strategy,
    /// `esim-focus`: the collectible a focused enchant targets.
    pub(crate) focus: Option<usize>,
    /// `esim-tgt`: the level each collectible is aimed at.
    pub(crate) targets: BTreeMap<usize, u32>,
    /// The run's seed, which every result records.
    pub(crate) seed: u64,
    /// The saved spreads, oldest first.
    pub(crate) history: Vec<EnchantSnapshot>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            enchants: 50,
            sims: 2_000,
            strategy: Strategy::None,
            focus: None,
            targets: BTreeMap::new(),
            seed: 1,
            history: Vec::new(),
        }
    }
}

impl State {
    /// The state with every field inside the range the page accepts and the
    /// save can satisfy: a target or focus on a collectible the save has not
    /// found is dropped, as the page never offers one.
    pub(crate) fn normalized(mut self, levels: Option<[u32; COLLECTIBLES]>) -> Self {
        self.enchants = self.enchants.clamp(ENCHANTS.0, ENCHANTS.1);
        self.sims = self.sims.clamp(SIMS.0, SIMS.1);
        self.seed = self.seed.max(1);
        self.targets
            .retain(|index, target| *index < COLLECTIBLES && *target > 0 && *target <= TARGET_MAX);
        if let Some(levels) = levels {
            self.targets.retain(|index, _| levels[*index] >= 1);
            if self
                .focus
                .is_some_and(|index| levels.get(index).copied().unwrap_or(0) < 1)
            {
                self.focus = None;
            }
        }
        let excess = self.history.len().saturating_sub(HISTORY_LIMIT);
        if excess > 0 {
            self.history.drain(..excess);
        }
        self
    }

    /// The inputs a run started now reads: the page's own ranges, so a value
    /// half-typed into a field still runs at the limit.
    pub(crate) fn inputs(&self, levels: [u32; COLLECTIBLES]) -> SimInputs {
        SimInputs {
            levels,
            enchants: self.enchants.clamp(ENCHANTS.0, ENCHANTS.1),
            sims: self.sims.clamp(SIMS.0, SIMS.1),
            strategy: self.strategy,
            focus: self.focus,
            targets: self
                .targets
                .iter()
                .map(|(index, target)| (*index, *target))
                .collect(),
            seed: self.seed.max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found() -> [u32; COLLECTIBLES] {
        let mut levels = [0u32; COLLECTIBLES];
        levels[0] = 3;
        levels[5] = 1;
        levels
    }

    #[test]
    fn the_controls_are_normalized_to_the_page_limits() {
        let state = State {
            enchants: 0,
            sims: 5,
            seed: 0,
            targets: [(0, 40), (1, 5), (5, 0), (6, 500), (99, 1)]
                .into_iter()
                .collect(),
            focus: Some(9),
            ..State::default()
        }
        .normalized(Some(found()));
        assert_eq!(state.enchants, 1);
        assert_eq!(state.sims, 100);
        assert_eq!(state.seed, 1);
        // Only the found collectible's target survives: one is not found, one
        // is zero, one is past the limit and one is not a collectible.
        assert_eq!(state.targets, [(0, 40)].into_iter().collect());
        // The focus names a collectible the save has not found.
        assert_eq!(state.focus, None);
    }

    #[test]
    fn a_state_survives_the_project_document() {
        let state = State {
            enchants: 120,
            sims: 500,
            strategy: Strategy::Smart,
            focus: Some(5),
            targets: [(5, 3)].into_iter().collect(),
            seed: 42,
            history: vec![EnchantSnapshot {
                at: 1_700_000_000,
                total: 4,
                spread: vec![0, 0, 0, 0, 0, 4],
            }],
        };
        let saved = serde_json::to_value(&state).expect("the state saves");
        assert_eq!(saved["strategy"], "smart");
        assert_eq!(saved["seed"], 42);
        let back: State = serde_json::from_value(saved).expect("the state reads back");
        assert_eq!(back, state);
        // A config from before JR-04 keeps the page's defaults.
        let legacy: State = serde_json::from_value(serde_json::json!({})).expect("defaults read");
        assert_eq!(legacy, State::default());
    }

    #[test]
    fn the_inputs_carry_every_control() {
        let state = State {
            strategy: Strategy::Fixed,
            focus: Some(5),
            targets: [(5, 2)].into_iter().collect(),
            ..State::default()
        };
        let inputs = state.inputs(found());
        assert_eq!(inputs.levels, found());
        assert_eq!(inputs.focus, Some(5));
        assert_eq!(inputs.targets, vec![(5, 2)]);
        assert_eq!(inputs.focus_chance(), 1.0);
    }
}
