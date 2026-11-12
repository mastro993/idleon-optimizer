//! The Spelunking section's controls, party, elixir belt and depth window.
//!
//! The state holds what the save cannot: the chosen cave, mode, policy,
//! full-clear and speed, the party slots, the belt and memorized elixirs, and
//! the account inputs the upstream stat context would supply (power, extra
//! exits and each character's maximum stamina). Importing a save adopts its
//! records and saved selections; everything else stays as the user left it.

use idleon_core::spelunking::{
    Account, Config, ConfigError, DepthRange, Entry, Inputs, Policy, Preset, PresetKind,
    ReferenceTool, Requested, StaminaMode, ToolChoice, normalize,
};

/// The section's visible state, saved with the project.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    /// The logged-in character whose row the section reads first.
    pub character: usize,
    pub cave: usize,
    pub manic: bool,
    pub policy: Policy,
    pub full_clear: bool,
    pub speed: u32,
    /// Saved stamina or a fully recovered hypothetical.
    pub starting: StaminaMode,
    /// One slot per cave capacity; `-1` is empty.
    pub party: Vec<i32>,
    /// One slot per unlocked belt slot; `-1` is empty.
    pub elixirs: Vec<i32>,
    /// One slot per memory slot; `-1` is empty.
    pub memorized: Vec<i32>,
    /// The account values the save does not derive yet.
    pub account: Account,
    pub depth_from: usize,
    pub depth_to: usize,
    pub depth_tool: ReferenceTool,
    /// The depth the inspector shows.
    pub inspect: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            character: 0,
            cave: 0,
            manic: false,
            policy: Policy::Auto,
            full_clear: false,
            speed: 0,
            starting: StaminaMode::Saved,
            party: Vec::new(),
            elixirs: Vec::new(),
            memorized: Vec::new(),
            account: Account::neutral(),
            depth_from: 1,
            depth_to: 150,
            depth_tool: ReferenceTool::Best,
            inspect: 1,
        }
    }
}

impl State {
    /// Adopt a save's records and saved selections, keeping nothing stale.
    pub fn adopt(&mut self, inputs: &Inputs) {
        self.character = self
            .character
            .min(inputs.characters.len().saturating_sub(1));
        self.cave = self.cave.min(inputs.caves.len().saturating_sub(1));
        self.manic = inputs.saved_manic && inputs.manic;
        self.policy = if inputs.auto {
            Policy::Auto
        } else if self.policy == Policy::Auto {
            Policy::Dive
        } else {
            self.policy
        };
        self.speed = inputs.speed_max;
        self.full_clear = inputs.full_clear;
        self.starting = StaminaMode::Saved;
        self.party = default_party(inputs);
        self.elixirs = pad(&inputs.saved_elixirs, inputs.slots);
        self.memorized = pad(
            &inputs
                .memorized
                .iter()
                .map(|id| *id as i32)
                .collect::<Vec<_>>(),
            inputs.memory_slots,
        );
        self.account = Account {
            power: 1.0,
            holes_bonus: 0.0,
            max_stamina: inputs
                .characters
                .iter()
                .map(|character| character.stamina)
                .collect(),
        };
        self.reset_range(inputs);
    }

    /// Fit an imported configuration to the save without discarding it.
    ///
    /// A project from another save may name more party, belt or memory slots,
    /// a higher speed or a locked mode; those are clamped, the rest is kept.
    pub fn clamp(&mut self, inputs: &Inputs) {
        self.character = self
            .character
            .min(inputs.characters.len().saturating_sub(1));
        self.cave = self.cave.min(inputs.caves.len().saturating_sub(1));
        let capacity = inputs.caves.get(self.cave).map_or(0, |cave| cave.capacity);
        self.party.resize(capacity, -1);
        self.elixirs.resize(inputs.slots, -1);
        self.memorized.resize(inputs.memory_slots, -1);
        self.speed = self.speed.min(inputs.speed_max);
        self.manic = self.manic && inputs.manic;
        if self.policy == Policy::Auto && !inputs.auto {
            self.policy = Policy::Dive;
        }
        while self.account.max_stamina.len() < inputs.characters.len() {
            let id = self.account.max_stamina.len();
            self.account.max_stamina.push(inputs.characters[id].stamina);
        }
        self.account.max_stamina.truncate(inputs.characters.len());
        self.clamp_range();
    }

    /// The request the section's depth estimate runs.
    pub fn requested(&self) -> Requested {
        Requested {
            cave: Some(self.cave),
            party: Some(self.party_ids()),
            manic: Some(self.manic),
            policy: Some(self.policy),
            speed: Some(self.speed),
            elixirs: Some(self.elixirs.clone()),
            memorized: Some(
                self.memorized
                    .iter()
                    .filter(|id| **id >= 0)
                    .map(|id| *id as usize)
                    .collect(),
            ),
            tool: None::<ToolChoice>,
            entry: None::<Entry>,
            full_clear: Some(self.full_clear),
            amber: None,
            reaction_ms: None,
            stamina: Some(self.starting),
        }
    }

    /// The normalised configuration, or the reason the controls cannot run.
    pub fn config(&self, inputs: &Inputs) -> Result<Config, ConfigError> {
        normalize(inputs, &self.requested())
    }

    /// The depth window the estimate covers.
    pub fn range(&self) -> DepthRange {
        DepthRange {
            from: self.depth_from,
            to: self.depth_to,
            tool: self.depth_tool,
        }
    }

    /// The chosen party member ids, in slot order.
    pub fn party_ids(&self) -> Vec<usize> {
        self.party
            .iter()
            .filter(|id| **id >= 0)
            .map(|id| *id as usize)
            .collect()
    }

    /// Whether `id` already fills another party slot.
    pub fn in_party(&self, slot: usize, id: usize) -> bool {
        self.party
            .iter()
            .enumerate()
            .any(|(index, value)| index != slot && *value == id as i32)
    }

    /// Set one slot and fit the party to the cave capacity.
    pub fn set_party_slot(&mut self, slot: usize, id: i32, inputs: &Inputs) {
        let capacity = inputs.caves.get(self.cave).map_or(0, |cave| cave.capacity);
        self.party.resize(capacity, -1);
        if slot < capacity {
            self.party[slot] = id;
        }
    }

    /// Change the cave, keeping the party slots that still fit.
    pub fn set_cave(&mut self, cave: usize, inputs: &Inputs) {
        self.cave = cave.min(inputs.caves.len().saturating_sub(1));
        let capacity = inputs.caves.get(self.cave).map_or(0, |cave| cave.capacity);
        self.party.resize(capacity, -1);
        self.reset_range(inputs);
    }

    /// Change the Normal/Manic mode and re-anchor the depth window.
    pub fn set_manic(&mut self, manic: bool, inputs: &Inputs) {
        self.manic = manic && inputs.manic;
        self.reset_range(inputs);
    }

    /// Change the policy; amber farming always clears the floor.
    pub fn set_policy(&mut self, policy: Policy) {
        self.policy = policy;
        if policy == Policy::AmberFarm {
            self.full_clear = true;
        }
    }

    /// Set one belt slot.
    pub fn set_elixir(&mut self, slot: usize, id: i32, inputs: &Inputs) {
        self.elixirs.resize(inputs.slots, -1);
        if slot < inputs.slots {
            self.elixirs[slot] = id;
        }
    }

    /// Set one memorized slot.
    pub fn set_memorized(&mut self, slot: usize, id: i32, inputs: &Inputs) {
        self.memorized.resize(inputs.memory_slots, -1);
        if slot < inputs.memory_slots {
            self.memorized[slot] = id;
        }
    }

    /// The belt or memory a preset would fill, and why it cannot.
    pub fn preset_issues(&self, inputs: &Inputs, preset: &Preset) -> Vec<String> {
        let memory = preset.kind == PresetKind::Memory;
        let slots = if memory {
            inputs.memory_slots
        } else {
            inputs.slots
        };
        let mut issues = Vec::new();
        if preset.elixirs.len() > slots {
            issues.push(format!(
                "Requires {} {} slots; {slots} unlocked.",
                preset.elixirs.len(),
                if memory { "memorization" } else { "belt" }
            ));
        }
        let mut counts: std::collections::BTreeMap<usize, usize> =
            std::collections::BTreeMap::new();
        for id in &preset.elixirs {
            *counts.entry(*id).or_insert(0) += 1;
        }
        for (id, count) in counts {
            let name = inputs
                .elixirs
                .get(id)
                .map_or("Unknown elixir", |elixir| elixir.name.as_str());
            if id >= inputs.elixir_owned {
                issues.push(format!("{name} is locked."));
            }
            if !memory && count > inputs.dupes {
                issues.push(format!(
                    "Requires {count} copies of {name}; duplicate limit {}.",
                    inputs.dupes
                ));
            }
        }
        issues
    }

    /// Fill the preset's half of the belt when it fits.
    pub fn apply_preset(&mut self, inputs: &Inputs, preset: &Preset) {
        if !self.preset_issues(inputs, preset).is_empty() {
            return;
        }
        let ids: Vec<i32> = preset.elixirs.iter().map(|id| *id as i32).collect();
        match preset.kind {
            PresetKind::Belt => self.elixirs = pad(&ids, inputs.slots),
            PresetKind::Memory => self.memorized = pad(&ids, inputs.memory_slots),
        }
    }

    /// Set the account power, the manual `Spelunking Power` input.
    pub fn set_power(&mut self, power: f64) {
        if power.is_finite() {
            self.account.power = power.max(0.0);
        }
    }

    /// Set the account extra-exit bonus, the manual `SpelunkyBonus(1)` input.
    pub fn set_holes_bonus(&mut self, bonus: f64) {
        if bonus.is_finite() {
            self.account.holes_bonus = bonus.max(0.0);
        }
    }

    /// Override one character's maximum stamina.
    pub fn set_max_stamina(&mut self, character: usize, value: f64) {
        if !value.is_finite() || value < 0.0 {
            return;
        }
        if self.account.max_stamina.len() <= character {
            self.account.max_stamina.resize(character + 1, 0.0);
        }
        self.account.max_stamina[character] = value;
    }

    /// Anchor the depth window around the current cave's record, as upstream does.
    pub fn reset_range(&mut self, inputs: &Inputs) {
        let record = records(inputs, self.manic)
            .get(self.cave)
            .copied()
            .unwrap_or(0.0);
        self.depth_from = (record as usize).saturating_sub(100).max(1);
        self.depth_to = (record as usize + 100).clamp(150, 10_000);
        self.inspect = self.depth_from;
    }

    /// Clamp the inspected depth into the window.
    pub fn clamp_inspect(&mut self) {
        self.inspect = self
            .inspect
            .clamp(self.depth_from, self.depth_to.max(self.depth_from));
    }

    /// Clamp the window into the bounds the profile accepts.
    pub fn clamp_range(&mut self) {
        self.depth_from = self.depth_from.clamp(1, 10_000);
        self.depth_to = self
            .depth_to
            .clamp(self.depth_from, (self.depth_from + 1_000).min(10_000));
        self.clamp_inspect();
    }
}

/// The depth records for the current mode.
pub fn records(inputs: &Inputs, manic: bool) -> &[f64] {
    if manic {
        &inputs.manic_depths
    } else {
        &inputs.depths
    }
}

fn default_party(inputs: &Inputs) -> Vec<i32> {
    let capacity = inputs.caves.first().map_or(0, |cave| cave.capacity);
    let mut party = vec![-1; capacity];
    for (slot, character) in inputs
        .characters
        .iter()
        .filter(|character| character.has_half_stamina())
        .take(capacity)
        .enumerate()
    {
        party[slot] = character.id as i32;
    }
    party
}

fn pad(values: &[i32], slots: usize) -> Vec<i32> {
    let mut padded = values.to_vec();
    padded.resize(slots, -1);
    padded
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_core::spelunking::{Account, Save};

    fn inputs() -> Inputs {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/data.json"
        ))
        .expect("the fixture is committed");
        let imported = idleon_core::save::import(&text).expect("the fixture imports");
        Save::from_parts(&imported.save)
            .expect("the fixture carries Spelunk rows")
            .inputs(&Account::neutral())
    }

    #[test]
    fn adopting_the_fixture_fills_the_party_and_belt_from_the_save() {
        let inputs = inputs();
        let mut state = State {
            party: Vec::new(),
            ..State::default()
        };
        state.adopt(&inputs);
        assert_eq!(state.cave, 0);
        assert_eq!(state.party, vec![0, 1, 2]);
        assert_eq!(state.elixirs, inputs.saved_elixirs);
        assert_eq!(state.memorized, vec![7, -1]);
        assert_eq!(state.speed, inputs.speed_max);
        assert_eq!(state.policy, Policy::Auto);
        assert_eq!(state.account.max_stamina.len(), inputs.characters.len());
        assert_eq!(state.depth_from, 1);
        assert_eq!(state.depth_to, 201);
        assert!(state.config(&inputs).is_ok());
    }

    #[test]
    fn a_smaller_cave_truncates_the_party() {
        let inputs = inputs();
        let mut state = State::default();
        state.adopt(&inputs);
        state.party = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        state.set_cave(0, &inputs);
        assert_eq!(state.party.len(), inputs.caves[0].capacity);
        assert_eq!(state.party, vec![0, 1, 2]);
    }

    #[test]
    fn amber_farming_forces_a_full_clear() {
        let mut state = State::default();
        state.set_policy(Policy::AmberFarm);
        assert!(state.full_clear);
    }

    #[test]
    fn a_duplicate_slot_is_reported_not_hidden() {
        let inputs = inputs();
        let mut state = State::default();
        state.adopt(&inputs);
        state.set_party_slot(1, 0, &inputs);
        let error = state.config(&inputs).expect_err("duplicates cannot run");
        assert_eq!(
            error.to_string(),
            "Choose a unique, nonempty party within this cave's capacity."
        );
    }
}
