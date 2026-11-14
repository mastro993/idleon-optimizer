//! The Spelunking section's controls, party, elixir belt and depth window.
//!
//! The state holds what the save cannot: the chosen cave, mode, policy,
//! full-clear and speed, the party slots, the belt and memorized elixirs, and
//! the account inputs the upstream stat context would supply (power, extra
//! exits and each character's maximum stamina). Importing a save adopts its
//! records and saved selections; everything else stays as the user left it.

use idleon_spelunking::{
    Account, AmberPolicy, Config, ConfigError, DepthRange, Enabled, Entry, Inputs, Objective,
    Policy, Preset, PresetKind, ReferenceTool, Requested, ScopeRequest, StaminaMode, ToolChoice,
    normalize,
};

/// Which optimizer search the section runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum OptimizerMode {
    #[default]
    Configuration,
    Amber,
}

impl OptimizerMode {
    pub(crate) fn id(self) -> &'static str {
        match self {
            OptimizerMode::Configuration => "configuration",
            OptimizerMode::Amber => "amber",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        match id {
            "configuration" => Some(OptimizerMode::Configuration),
            "amber" => Some(OptimizerMode::Amber),
            _ => None,
        }
    }
}

/// The section's visible state, saved with the project.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    /// The logged-in character whose row the section reads first.
    pub(crate) character: usize,
    pub(crate) cave: usize,
    pub(crate) manic: bool,
    pub(crate) policy: Policy,
    pub(crate) full_clear: bool,
    pub(crate) speed: u32,
    /// Saved stamina or a fully recovered hypothetical.
    pub(crate) starting: StaminaMode,
    /// One slot per cave capacity; `-1` is empty.
    pub(crate) party: Vec<i32>,
    /// One slot per unlocked belt slot; `-1` is empty.
    pub(crate) elixirs: Vec<i32>,
    /// One slot per memory slot; `-1` is empty.
    pub(crate) memorized: Vec<i32>,
    /// The account values the save does not derive yet.
    pub(crate) account: Account,
    pub(crate) depth_from: usize,
    pub(crate) depth_to: usize,
    pub(crate) depth_tool: ReferenceTool,
    /// The depth the inspector shows.
    pub(crate) inspect: usize,
    /// The seed the next expedition run uses.
    pub(crate) sim_seed: u32,
    /// The manual reaction time, in milliseconds.
    pub(crate) sim_reaction_ms: f64,
    /// The manual tool: adaptive, or one unlocked tool.
    pub(crate) sim_tool: ToolChoice,
    /// Where the run starts.
    pub(crate) sim_entry: Entry,
    /// The phased Amber policy's limits, read only by `amber-farm`.
    pub(crate) amber: AmberPolicy,
    /// The optimizer's search mode and objective.
    pub(crate) optimizer_mode: OptimizerMode,
    pub(crate) optimizer_objective: Objective,
    pub(crate) optimizer_screening: usize,
    pub(crate) optimizer_trials: usize,
    pub(crate) optimizer_seed: u32,
    /// Which dimensions the configuration search may change.
    pub(crate) optimizer_scope: Enabled,
    /// The Amber search's dump-depth window and rope filter.
    pub(crate) optimizer_from: usize,
    pub(crate) optimizer_to: usize,
    pub(crate) optimizer_min_rope: usize,
    pub(crate) optimizer_max_trials: usize,
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
            sim_seed: idleon_spelunking::DEFAULT_SEED,
            sim_reaction_ms: 350.0,
            sim_tool: ToolChoice::Adaptive,
            sim_entry: Entry::Shortcut,
            amber: AmberPolicy::default(),
            optimizer_mode: OptimizerMode::Configuration,
            optimizer_objective: Objective::Depth,
            optimizer_screening: 1,
            optimizer_trials: 2,
            optimizer_seed: idleon_spelunking::DEFAULT_SEED,
            optimizer_scope: Enabled {
                party: false,
                modes: true,
                clears: true,
                speeds: false,
                elixirs: false,
                memorized: false,
                tools: false,
                entry: false,
            },
            optimizer_from: 1,
            optimizer_to: 100,
            optimizer_min_rope: 0,
            optimizer_max_trials: 32,
        }
    }
}

impl State {
    /// Adopt a save's records and saved selections, keeping nothing stale.
    pub(crate) fn adopt(&mut self, inputs: &Inputs) {
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
            ..Account::neutral()
        };
        self.account.max_stamina = inputs
            .characters
            .iter()
            .map(|character| character.stamina)
            .collect();
        self.reset_range(inputs);
    }

    /// Fit an imported configuration to the save without discarding it.
    ///
    /// A project from another save may name more party, belt or memory slots,
    /// a higher speed or a locked mode; those are clamped, the rest is kept.
    pub(crate) fn clamp(&mut self, inputs: &Inputs) {
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
        if let ToolChoice::Tool(id) = self.sim_tool
            && !inputs.tools.iter().any(|tool| tool.id == id)
        {
            self.sim_tool = ToolChoice::Adaptive;
        }
        self.sim_reaction_ms = self.sim_reaction_ms.clamp(100.0, 5_000.0);
        self.set_amber_policy(self.amber);
        self.optimizer_screening = self.optimizer_screening.clamp(1, 8);
        self.optimizer_trials = self.optimizer_trials.clamp(2, 64);
        self.optimizer_max_trials = self.optimizer_max_trials.clamp(self.optimizer_trials, 256);
        self.optimizer_from = self.optimizer_from.clamp(1, 10_000);
        self.optimizer_to = self.optimizer_to.clamp(self.optimizer_from, 10_000);
        self.optimizer_min_rope = self.optimizer_min_rope.min(100);
        self.clamp_range();
    }

    /// The request the section's depth estimate runs.
    pub(crate) fn requested(&self) -> Requested {
        self.run_requested()
    }

    /// The request the expedition run reads: the estimate's fields plus the
    /// run's own entry, tool and reaction time.
    pub(crate) fn run_requested(&self) -> Requested {
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
            tool: Some(self.sim_tool),
            entry: Some(self.sim_entry),
            full_clear: Some(self.full_clear),
            amber: (self.policy == Policy::AmberFarm).then_some(self.amber),
            reaction_ms: Some(self.sim_reaction_ms),
            stamina: Some(self.starting),
        }
    }

    /// The normalised configuration, or the reason the controls cannot run.
    pub(crate) fn config(&self, inputs: &Inputs) -> Result<Config, ConfigError> {
        normalize(inputs, &self.requested())
    }

    /// The configuration search's request: the run's fields with the selected
    /// dimensions left to the search.
    pub(crate) fn optimizer_requested(&self) -> ScopeRequest {
        ScopeRequest {
            enabled: Some(self.optimizer_scope),
            ..ScopeRequest::default()
        }
    }

    /// Apply a recommendation to the controls.
    pub(crate) fn apply_config(&mut self, config: &Config, inputs: &Inputs) {
        self.cave = config.cave.min(inputs.caves.len().saturating_sub(1));
        self.manic = config.manic;
        self.policy = config.policy;
        self.speed = config.speed;
        self.full_clear = config.full_clear;
        self.starting = config.stamina;
        self.party = config.party.iter().map(|id| *id as i32).collect();
        let capacity = inputs.caves.get(self.cave).map_or(0, |cave| cave.capacity);
        self.party.resize(capacity, -1);
        self.elixirs = pad(&config.elixirs, inputs.slots);
        self.memorized = pad(
            &config
                .memorized
                .iter()
                .map(|id| *id as i32)
                .collect::<Vec<_>>(),
            inputs.memory_slots,
        );
        self.sim_tool = config.tool;
        self.sim_entry = config.entry;
        self.sim_reaction_ms = config.reaction_ms;
        if let Some(amber) = config.amber {
            self.set_amber_policy(amber);
        }
        if let Some(character) = config.party.first() {
            self.character = *character;
        }
    }

    /// The depth window the estimate covers.
    pub(crate) fn range(&self) -> DepthRange {
        DepthRange {
            from: self.depth_from,
            to: self.depth_to,
            tool: self.depth_tool,
        }
    }

    /// The chosen party member ids, in slot order.
    pub(crate) fn party_ids(&self) -> Vec<usize> {
        self.party
            .iter()
            .filter(|id| **id >= 0)
            .map(|id| *id as usize)
            .collect()
    }

    /// Whether `id` already fills another party slot.
    pub(crate) fn in_party(&self, slot: usize, id: usize) -> bool {
        self.party
            .iter()
            .enumerate()
            .any(|(index, value)| index != slot && *value == id as i32)
    }

    /// Set one slot and fit the party to the cave capacity.
    pub(crate) fn set_party_slot(&mut self, slot: usize, id: i32, inputs: &Inputs) {
        let capacity = inputs.caves.get(self.cave).map_or(0, |cave| cave.capacity);
        self.party.resize(capacity, -1);
        if slot < capacity {
            self.party[slot] = id;
        }
    }

    /// Change the cave, keeping the party slots that still fit.
    pub(crate) fn set_cave(&mut self, cave: usize, inputs: &Inputs) {
        self.cave = cave.min(inputs.caves.len().saturating_sub(1));
        let capacity = inputs.caves.get(self.cave).map_or(0, |cave| cave.capacity);
        self.party.resize(capacity, -1);
        self.reset_range(inputs);
    }

    /// Change the Normal/Manic mode and re-anchor the depth window.
    pub(crate) fn set_manic(&mut self, manic: bool, inputs: &Inputs) {
        self.manic = manic && inputs.manic;
        self.reset_range(inputs);
    }

    /// Change the policy; amber farming always clears the floor.
    pub(crate) fn set_policy(&mut self, policy: Policy) {
        self.policy = policy;
        if policy == Policy::AmberFarm {
            self.full_clear = true;
        }
    }

    /// Set the Amber policy fields, clamped to the ranges the game accepts.
    pub(crate) fn set_amber_policy(&mut self, amber: AmberPolicy) {
        self.amber = AmberPolicy {
            dump_depth: amber.dump_depth.clamp(1, 10_000),
            finishers: amber.finishers.clamp(1, 2),
            reserve_pct: amber.reserve_pct.clamp(25, 100),
            harvest_depths: amber.harvest_depths.min(100),
            rope_reserve_pct: amber.rope_reserve_pct.clamp(1, 100),
        };
    }

    /// Set one belt slot.
    pub(crate) fn set_elixir(&mut self, slot: usize, id: i32, inputs: &Inputs) {
        self.elixirs.resize(inputs.slots, -1);
        if slot < inputs.slots {
            self.elixirs[slot] = id;
        }
    }

    /// Set one memorized slot.
    pub(crate) fn set_memorized(&mut self, slot: usize, id: i32, inputs: &Inputs) {
        self.memorized.resize(inputs.memory_slots, -1);
        if slot < inputs.memory_slots {
            self.memorized[slot] = id;
        }
    }

    /// The belt or memory a preset would fill, and why it cannot.
    pub(crate) fn preset_issues(&self, inputs: &Inputs, preset: &Preset) -> Vec<String> {
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
    pub(crate) fn apply_preset(&mut self, inputs: &Inputs, preset: &Preset) {
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
    pub(crate) fn set_power(&mut self, power: f64) {
        if power.is_finite() {
            self.account.power = power.max(0.0);
        }
    }

    /// Set the account extra-exit bonus, the manual `SpelunkyBonus(1)` input.
    pub(crate) fn set_holes_bonus(&mut self, bonus: f64) {
        if bonus.is_finite() {
            self.account.holes_bonus = bonus.max(0.0);
        }
    }

    /// Set the account Amber multiplier.
    pub(crate) fn set_amber(&mut self, amber: f64) {
        if amber.is_finite() {
            self.account.amber = amber.max(0.0);
        }
    }

    /// Set the account stamina regeneration per hour.
    pub(crate) fn set_regen(&mut self, regen: f64) {
        if regen.is_finite() {
            self.account.regen = regen.max(0.0);
        }
    }

    /// Set the Grand Discovery chance from the percentage the user reads.
    pub(crate) fn set_grand_pct(&mut self, percent: f64) {
        if percent.is_finite() {
            self.account.grand = (percent / 100.0).clamp(0.0, 1.0);
        }
    }

    /// Override one character's maximum stamina.
    pub(crate) fn set_max_stamina(&mut self, character: usize, value: f64) {
        if !value.is_finite() || value < 0.0 {
            return;
        }
        if self.account.max_stamina.len() <= character {
            self.account.max_stamina.resize(character + 1, 0.0);
        }
        self.account.max_stamina[character] = value;
    }

    /// Anchor the depth window around the current cave's record, as upstream does.
    pub(crate) fn reset_range(&mut self, inputs: &Inputs) {
        let record = records(inputs, self.manic)
            .get(self.cave)
            .copied()
            .unwrap_or(0.0);
        self.depth_from = (record as usize).saturating_sub(100).max(1);
        self.depth_to = (record as usize + 100).clamp(150, 10_000);
        self.inspect = self.depth_from;
    }

    /// Clamp the inspected depth into the window.
    pub(crate) fn clamp_inspect(&mut self) {
        self.inspect = self
            .inspect
            .clamp(self.depth_from, self.depth_to.max(self.depth_from));
    }

    /// Clamp the window into the bounds the profile accepts.
    pub(crate) fn clamp_range(&mut self) {
        self.depth_from = self.depth_from.clamp(1, 10_000);
        self.depth_to = self
            .depth_to
            .clamp(self.depth_from, (self.depth_from + 1_000).min(10_000));
        self.clamp_inspect();
    }
}

/// The depth records for the current mode.
pub(crate) fn records(inputs: &Inputs, manic: bool) -> &[f64] {
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
    use idleon_spelunking::{Account, Save};

    fn inputs() -> Inputs {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/data.json"
        ))
        .expect("the fixture is committed");
        let imported = idleon_import::import(&text).expect("the fixture imports");
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
    fn the_memory_and_the_run_choices_survive_a_project_round_trip() {
        let state = State {
            memorized: vec![7, -1],
            elixirs: vec![3, 3, 6, -1, -1],
            sim_seed: 42,
            sim_tool: ToolChoice::Tool(2),
            sim_entry: Entry::Surface,
            sim_reaction_ms: 500.0,
            amber: AmberPolicy {
                dump_depth: 12,
                finishers: 2,
                reserve_pct: 80,
                harvest_depths: 4,
                rope_reserve_pct: 40,
            },
            ..State::default()
        };
        let text = serde_json::to_string(&state).expect("the state serialises");
        let restored: State = serde_json::from_str(&text).expect("the state decodes");
        assert_eq!(restored, state);
    }

    #[test]
    fn the_optimizer_controls_survive_a_project_round_trip_and_clamp() {
        let state = State {
            optimizer_mode: OptimizerMode::Amber,
            optimizer_objective: Objective::SustainableAmber,
            optimizer_screening: 3,
            optimizer_trials: 9,
            optimizer_seed: 77,
            optimizer_scope: Enabled {
                party: true,
                entry: true,
                ..Enabled::default()
            },
            optimizer_from: 6,
            optimizer_to: 40,
            optimizer_min_rope: 25,
            optimizer_max_trials: 64,
            ..State::default()
        };
        let text = serde_json::to_string(&state).expect("the state serialises");
        let restored: State = serde_json::from_str(&text).expect("the state decodes");
        assert_eq!(restored, state);
        let mut clamped = State {
            optimizer_screening: 0,
            optimizer_trials: 0,
            optimizer_max_trials: 0,
            optimizer_from: 0,
            optimizer_to: 0,
            optimizer_min_rope: 200,
            ..State::default()
        };
        let inputs = inputs();
        clamped.clamp(&inputs);
        assert_eq!(clamped.optimizer_screening, 1);
        assert_eq!(clamped.optimizer_trials, 2);
        assert_eq!(clamped.optimizer_max_trials, 2);
        assert_eq!(clamped.optimizer_from, 1);
        assert_eq!(clamped.optimizer_to, 1);
        assert_eq!(clamped.optimizer_min_rope, 100);
    }

    #[test]
    fn a_recommendation_fills_the_controls() {
        let inputs = inputs();
        let mut state = State::default();
        state.adopt(&inputs);
        let config = state.config(&inputs).expect("the fixture defaults run");
        state.apply_config(&config, &inputs);
        assert_eq!(state.party_ids(), config.party);
        assert_eq!(state.policy, config.policy);
        assert_eq!(state.cave, config.cave);
        assert_eq!(state.manic, config.manic);
        assert_eq!(state.speed, config.speed);
        assert_eq!(state.sim_tool, config.tool);
    }

    #[test]
    fn the_amber_policy_is_requested_only_for_amber_farming() {
        let mut state = State {
            policy: Policy::Dive,
            ..State::default()
        };
        assert_eq!(state.run_requested().amber, None);
        state.set_policy(Policy::AmberFarm);
        assert_eq!(state.run_requested().amber, Some(AmberPolicy::default()));
        state.set_amber_policy(AmberPolicy {
            dump_depth: 0,
            finishers: 9,
            reserve_pct: 1,
            harvest_depths: 500,
            rope_reserve_pct: 0,
        });
        assert_eq!(
            state.amber,
            AmberPolicy {
                dump_depth: 1,
                finishers: 2,
                reserve_pct: 25,
                harvest_depths: 100,
                rope_reserve_pct: 1,
            }
        );
    }

    #[test]
    fn a_project_without_the_run_choices_starts_from_the_defaults() {
        let restored: State =
            serde_json::from_str(r#"{"cave":2}"#).expect("an older project decodes");
        assert_eq!(restored.cave, 2);
        assert_eq!(restored.sim_seed, idleon_spelunking::DEFAULT_SEED);
        assert_eq!(restored.sim_tool, ToolChoice::Adaptive);
        assert_eq!(restored.sim_entry, Entry::Shortcut);
        assert_eq!(restored.sim_reaction_ms, 350.0);
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
