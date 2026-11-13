//! What the window does when the combat sim section is edited or run.

use crate::sections::combat_sim::Tab as CombatTab;
use crate::state::AppState;
use gpui::{App, ClipboardItem, Context};
use idleon_combat::Loadout;
use idleon_combat::compare::{self, Comparison, Plan};
use idleon_combat::encounter::{self, Options, Outcome, Player};
use idleon_ui::runner::TaskState;

impl AppState {
    /// Pick the loadout editor's tab.
    pub(crate) fn set_combat_tab(&mut self, tab: CombatTab, cx: &mut Context<Self>) {
        self.combat.tab = tab;
        cx.notify();
    }

    /// Switch the loadout editor to another character of the loaded save.
    pub(crate) fn set_combat_character(&mut self, character: usize, cx: &mut Context<Self>) {
        let Some(save) = self.save.as_ref() else {
            return;
        };
        if character >= save.character_count() {
            return;
        }
        self.combat.load(save, character);
        self.set_status(
            format!("Editing {}.", self.combat.loadout.character.name),
            cx,
        );
    }

    /// Select the equipment slot the detail panel edits.
    pub(crate) fn set_combat_slot(&mut self, slot: usize, cx: &mut Context<Self>) {
        if slot < self.combat.loadout.slots.len() {
            self.combat.selected = slot;
            cx.notify();
        }
    }

    /// Equip an item in one slot; an empty key unequips it.
    pub(crate) fn set_combat_item(
        &mut self,
        row: usize,
        slot: usize,
        key: &str,
        cx: &mut Context<Self>,
    ) {
        self.combat.loadout.set_item(row, slot, key);
        if let Some(entry) = self.combat.loadout.slot(row, slot) {
            self.status = format!("{}: {}.", entry.name(), entry.item_name());
        }
        cx.notify();
    }

    /// Set one talent's level, clamped to its saved maximum.
    pub(crate) fn set_combat_talent(&mut self, id: u32, level: u32, cx: &mut Context<Self>) {
        self.combat.loadout.set_talent(id, level);
        cx.notify();
    }

    /// Put every slot and talent back to the imported save.
    pub(crate) fn reset_combat_loadout(&mut self, cx: &mut Context<Self>) {
        self.combat.loadout.reset();
        self.set_status("Loadout reset to the save.", cx);
    }

    /// Pick a mechanic's mode; `None` is Saved, which clears its override.
    pub(crate) fn set_combat_override_mode(
        &mut self,
        id: &str,
        mode: Option<idleon_combat::Mode>,
        cx: &mut Context<Self>,
    ) {
        match mode {
            Some(mode) => {
                let current = self.combat.overrides.rule(id);
                let rule = idleon_combat::Rule::seeded(mode, current.as_ref(), None);
                self.combat.overrides.set(id, rule);
                let name = idleon_combat::toggle(id).map_or(id, |toggle| toggle.label.as_str());
                self.status = format!("{name}: {} override.", mode.label());
            }
            None => {
                self.combat.overrides.remove(id);
                self.status = format!("Override cleared for {id}.");
            }
        }
        cx.notify();
    }

    /// Set the value a Set or Scale rule uses.
    pub(crate) fn set_combat_override_value(
        &mut self,
        id: &str,
        value: f64,
        cx: &mut Context<Self>,
    ) {
        let Some(rule) = self.combat.overrides.rule(id) else {
            return;
        };
        if !matches!(
            rule.mode,
            idleon_combat::Mode::Set | idleon_combat::Mode::Scale
        ) {
            return;
        }
        self.combat
            .overrides
            .set(id, idleon_combat::Rule::with_value(rule.mode, value));
        cx.notify();
    }

    /// Forget one mechanic's override, putting its input back to the save.
    pub(crate) fn remove_combat_override(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.combat.overrides.rule(id).is_some() {
            self.combat.overrides.remove(id);
            cx.notify();
        }
    }

    /// Put every mechanic back to its saved value.
    pub(crate) fn reset_combat_overrides(&mut self, cx: &mut Context<Self>) {
        self.combat.overrides.reset();
        self.set_status("Overrides reset: every mechanic uses the save.", cx);
    }

    /// Filter the mechanics pane by group; empty means every group.
    pub(crate) fn set_combat_override_group(&mut self, group: &str, cx: &mut Context<Self>) {
        self.combat.override_group = group.to_string();
        cx.notify();
    }

    /// Whether the mechanics pane lists only overridden mechanics.
    pub(crate) fn set_combat_overrides_only_active(&mut self, only: bool, cx: &mut Context<Self>) {
        self.combat.override_only_active = only;
        cx.notify();
    }

    /// Save the working loadout under the typed name, replacing one with the
    /// same name for the same character, like the upstream storage key does.
    pub(crate) fn save_combat_preset(&mut self, cx: &mut Context<Self>) {
        let name = self.combat.preset_name(cx);
        match self.combat.presets.store(&self.combat.loadout, &name) {
            Ok(message) => {
                self.combat
                    .name
                    .update(cx, |input, cx| input.set_text("", cx));
                self.status = message;
            }
            Err(message) => self.status = message,
        }
        cx.notify();
    }

    /// Apply a named loadout to its character.
    pub(crate) fn load_combat_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        let line = self.combat.presets.apply(index, &mut self.combat.loadout);
        self.set_status(line, cx);
    }

    /// Forget one named loadout.
    pub(crate) fn delete_combat_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(preset) = self.combat.presets.delete(index) {
            self.set_status(format!("Deleted the loadout “{}”.", preset.name), cx);
        }
    }

    /// Pick the map the next encounter run reads.
    pub(crate) fn set_combat_encounter_map(&mut self, index: usize, cx: &mut Context<Self>) {
        if encounter::map(index).is_some() {
            self.combat.sim_map = index;
        }
        cx.notify();
    }

    /// Choose the idle player instead of the fixed kill interval.
    pub(crate) fn set_combat_encounter_idle(&mut self, idle: bool, cx: &mut Context<Self>) {
        self.combat.sim_stand_in = !idle;
        cx.notify();
    }

    /// Set the stand-in's kill interval, snapped to the 10 ms step.
    pub(crate) fn set_combat_encounter_kill_every(
        &mut self,
        milliseconds: f64,
        cx: &mut Context<Self>,
    ) {
        if milliseconds.is_finite() {
            let snapped = (milliseconds.max(10.0) / 10.0).round() * 10.0;
            self.combat.sim_kill_every_ms = snapped as u64;
        }
        cx.notify();
    }

    /// Set the simulated duration, clamped to the upstream presets' range.
    pub(crate) fn set_combat_encounter_duration(&mut self, seconds: u64, cx: &mut Context<Self>) {
        self.combat.sim_duration_s = seconds.clamp(10, 86_400);
        cx.notify();
    }

    /// The options a run started now would read.
    pub(crate) fn combat_encounter_options(&self) -> Options {
        Options {
            map: self.combat.sim_map,
            player: if self.combat.sim_stand_in {
                Player::StandIn {
                    kill_every_ms: self.combat.sim_kill_every_ms,
                }
            } else {
                Player::Idle
            },
            duration_ms: self.combat.sim_duration_s * 1000,
        }
    }

    /// Run the encounter off the UI thread through the shared task runner.
    pub(crate) fn run_combat_encounter(&mut self, cx: &mut Context<Self>) {
        let options = self.combat_encounter_options();
        if let Err(error) = encounter::validate(&options) {
            self.set_status(error, cx);
            return;
        }
        let line = format!(
            "Running map {} with {}…",
            options.map,
            options.player.label()
        );
        self.start_run(
            self.combat_encounter.clone(),
            line,
            options,
            move |options, report| encounter::run(options, report).ok().flatten(),
            cx,
        );
    }

    /// Ask a running encounter to stop; the previous result stays.
    pub(crate) fn cancel_combat_encounter(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.combat_encounter.clone(),
            "Cancelling the encounter…",
            cx,
        );
    }

    /// The stored encounter run and whether its options changed since the run.
    pub(crate) fn combat_encounter(&self, cx: &App) -> TaskState<Outcome> {
        self.combat_encounter
            .read(cx)
            .state(&self.combat_encounter_options())
    }

    /// Add or remove a character from the comparison.
    pub(crate) fn set_combat_compare_character(
        &mut self,
        character: usize,
        selected: bool,
        cx: &mut Context<Self>,
    ) {
        if selected {
            if !self.combat.compare_characters.contains(&character) {
                self.combat.compare_characters.push(character);
                self.combat.compare_characters.sort_unstable();
            }
        } else {
            self.combat
                .compare_characters
                .retain(|index| *index != character);
        }
        cx.notify();
    }

    /// Add or remove one of a character's loadouts from the comparison.
    pub(crate) fn set_combat_compare_loadout(
        &mut self,
        character: usize,
        key: &str,
        selected: bool,
        cx: &mut Context<Self>,
    ) {
        let mut keys = self.combat.compare_loadout_keys(character);
        if selected {
            if !keys.iter().any(|current| current == key) {
                keys.push(key.to_string());
            }
        } else {
            keys.retain(|current| current != key);
        }
        self.combat.compare_loadouts.insert(character, keys);
        cx.notify();
    }

    /// Add a map to the comparison.
    pub(crate) fn add_combat_compare_map(&mut self, map: usize, cx: &mut Context<Self>) {
        if encounter::map(map).is_none() {
            return;
        }
        if !self.combat.compare_maps.contains(&map) {
            self.combat.compare_maps.push(map);
            self.combat.compare_maps.sort_unstable();
        }
        cx.notify();
    }

    /// Remove a map from the comparison.
    pub(crate) fn remove_combat_compare_map(&mut self, map: usize, cx: &mut Context<Self>) {
        self.combat.compare_maps.retain(|current| *current != map);
        cx.notify();
    }

    /// The loadouts one character compares, in the picker's order: the imported
    /// save, the working edits, then the character's named presets.
    fn combat_compare_setups(&self, character: usize) -> Vec<(String, Loadout)> {
        let Some(save) = self.save.as_ref() else {
            return Vec::new();
        };
        let keys = self.combat.compare_loadout_keys(character);
        let mut setups = Vec::new();
        if keys.iter().any(|key| key == "saved") {
            setups.push((
                "Imported save".to_string(),
                Loadout::from_save(save, character),
            ));
        }
        if character == self.combat.character && keys.iter().any(|key| key == "edited") {
            setups.push((
                format!("Working edits ({} changes)", self.combat.loadout.changed()),
                self.combat.loadout.clone(),
            ));
        }
        for preset in self
            .combat
            .presets
            .items
            .iter()
            .filter(|preset| preset.character == character)
        {
            let key = format!("preset:{}", preset.name);
            if !keys.contains(&key) {
                continue;
            }
            let mut loadout = Loadout::from_save(save, character);
            if preset.apply_to(&mut loadout).is_ok() {
                setups.push((preset.name.clone(), loadout));
            }
        }
        setups
    }

    /// Every selected character, loadout and map as one run plan. An empty save
    /// or selection gives an empty plan, which the run refuses.
    pub(crate) fn combat_comparison_plan(&self) -> Plan {
        let options = self.combat_encounter_options();
        let mut rows = Vec::new();
        if let Some(save) = self.save.as_ref() {
            for character in &self.combat.compare_characters {
                if *character >= save.character_count() {
                    continue;
                }
                for (loadout, setup) in self.combat_compare_setups(*character) {
                    let totals = setup.totals();
                    for map in &self.combat.compare_maps {
                        rows.push(compare::RowSpec {
                            character: *character,
                            character_name: setup.character.name.clone(),
                            loadout: loadout.clone(),
                            map: *map,
                            totals: totals.clone(),
                        });
                    }
                }
            }
        }
        Plan {
            player: options.player,
            duration_ms: options.duration_ms,
            rows,
        }
    }

    /// Run the comparison off the UI thread through the shared task runner.
    pub(crate) fn run_combat_comparison(&mut self, cx: &mut Context<Self>) {
        let plan = self.combat_comparison_plan();
        if let Err(error) = plan.validate() {
            self.set_status(error, cx);
            return;
        }
        let line = format!("Comparing {} setups…", plan.rows.len());
        self.start_run(
            self.combat_compare.clone(),
            line,
            plan,
            move |plan, report| compare::run(&plan, report).ok().flatten(),
            cx,
        );
    }

    /// Ask a running comparison to stop; the previous result stays.
    pub(crate) fn cancel_combat_comparison(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.combat_compare.clone(),
            "Cancelling the comparison…",
            cx,
        );
    }

    /// The stored comparison and whether its plan changed since the run.
    pub(crate) fn combat_comparison(&self, cx: &App) -> TaskState<Comparison> {
        self.combat_compare
            .read(cx)
            .state(&self.combat_comparison_plan())
    }

    /// Copy the last comparison as the section's JSON export.
    pub(crate) fn copy_combat_comparison(&mut self, cx: &mut Context<Self>) {
        let Some(comparison) = self.combat_compare.read(cx).result().cloned() else {
            self.set_status("Run a comparison before exporting it.", cx);
            return;
        };
        let json = compare::document_json(&comparison);
        cx.write_to_clipboard(ClipboardItem::new_string(json.clone()));
        self.set_status(
            format!(
                "Copied the comparison JSON to the clipboard ({} bytes).",
                json.len()
            ),
            cx,
        );
    }
}
