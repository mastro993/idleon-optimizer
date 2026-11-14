//! What the window does when the combat sim section is edited or run.

use crate::sections::combat_sim::{ResultsTab, RunResult, Tab as CombatTab};
use crate::state::AppState;
use gpui::{App, ClipboardItem, Context};
use idleon_combat::Loadout;
use idleon_combat::compare::{self, Comparison, Plan};
use idleon_combat::drops::{self, Inputs, Snapshot};
use idleon_combat::encounter::{self, Options, Player};
use idleon_combat::overrides::Entries;
use idleon_ui::runner::TaskState;
use std::time::{Duration, Instant};

/// How often the replay loop advances the playhead.
const REPLAY_FRAME: Duration = Duration::from_millis(16);

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

    /// Set the seed every drop roll draws from.
    pub(crate) fn set_combat_encounter_seed(&mut self, seed: f64, cx: &mut Context<Self>) {
        if seed.is_finite() {
            self.combat.sim_seed = seed.max(1.0) as u64;
        }
        cx.notify();
    }

    /// Pick the results tab the Encounter pane shows.
    pub(crate) fn set_combat_results_tab(&mut self, tab: ResultsTab, cx: &mut Context<Self>) {
        self.combat.results = tab;
        cx.notify();
    }

    /// The saved card and obol state a run's drops start from.
    ///
    /// The character's Drop Rate and Card Drop Chance come from the mechanics
    /// overrides: `off` is neutral, `set` and `scale` apply, and the saved
    /// values `scale` would need are not wired yet.
    fn combat_drop_state(&self) -> (Snapshot, Inputs) {
        let snapshot = self.save.as_ref().map(drops::snapshot).unwrap_or_default();
        let overrides = self.combat.overrides.build(&Entries::new());
        (snapshot, Inputs::from_overrides(&overrides))
    }

    /// The options a run started now would read.
    pub(crate) fn combat_encounter_options(&self) -> Options {
        let (drops, drop_inputs) = self.combat_drop_state();
        let overrides = self.combat.overrides.build(&Entries::new());
        let read = |key: &str, neutral: f64| match overrides.get(key) {
            Some(idleon_combat::overrides::Value::Number(value)) => *value,
            _ => neutral,
        };
        let map = self.combat.sim_map;
        let duration_ms = self.combat.sim_duration_s * 1000;
        Options {
            map,
            player: if self.combat.sim_stand_in {
                Player::StandIn {
                    kill_every_ms: self.combat.sim_kill_every_ms,
                }
            } else {
                Player::Idle
            },
            duration_ms,
            seed: self.combat.sim_seed,
            drops,
            drop_inputs,
            progress: self.combat.progress.clone(),
            grants: idleon_combat::progress::Grants {
                drop_rate: drop_inputs.drop_rate,
                kill_credit: read("killPerKill", 1.0) + read("multiKillPlay", 0.0) / 100.0,
                map,
                scene: idleon_combat::encounter::map(map)
                    .map(|map| map.scene.clone())
                    .unwrap_or_default(),
            },
            // Every run records its frames, so a replay never needs a rerun.
            replay: Some(idleon_combat::replay::interval_for(duration_ms)),
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
        // The Killroy practice re-runs the room's timer at the kill rate the
        // encounter measures, and the Den projects its hound waves at the same
        // rate, so both read the same save and seed.
        let save = self.save.clone();
        let character = self.combat.character;
        let den = self.combat.den.clone();
        let progress_warnings = self.combat.progress_warnings.clone();
        let source = self.combat.pending_source.take();
        self.start_run(
            self.combat_encounter.clone(),
            line,
            options,
            move |options, report| {
                let run = encounter::run(options.clone(), report).ok().flatten()?;
                let killroy = save.as_ref().and_then(|save| {
                    let seconds = run.simulated_ms as f64 / 1000.0;
                    if run.kills == 0 || seconds <= 0.0 {
                        return None;
                    }
                    let inputs =
                        idleon_killroy::trial_inputs(save, character, run.kills as f64 / seconds);
                    Some(idleon_killroy::trial(&inputs, options.seed))
                });
                let den = match (den, options.player) {
                    (Some(config), Player::StandIn { kill_every_ms }) => {
                        Some(idleon_combat::den::project(
                            &config,
                            kill_every_ms,
                            options.duration_ms,
                            options.seed,
                        ))
                    }
                    _ => None,
                };
                Some(RunResult {
                    run,
                    killroy,
                    den,
                    progress_warnings,
                    source,
                })
            },
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
    pub(crate) fn combat_encounter(&self, cx: &App) -> TaskState<RunResult> {
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
            progress: options.progress,
            grants: options.grants,
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

    /// The stored run's replay size and frame interval, when it has one.
    fn combat_replay_info(&self, cx: &App) -> Option<(usize, u64)> {
        let runner = self.combat_encounter.read(cx);
        let replay = runner.result()?.run.replay.as_ref()?;
        Some((replay.len(), replay.interval_ms()))
    }

    /// Start or pause the replay. Playing from the last frame restarts it.
    pub(crate) fn combat_replay_set_playing(&mut self, playing: bool, cx: &mut Context<Self>) {
        let Some((len, _)) = self.combat_replay_info(cx) else {
            self.set_status("Run the encounter before playing its replay.", cx);
            return;
        };
        let last = len.saturating_sub(1) as f64;
        if playing && self.combat.replay_pos >= last {
            self.combat.replay_pos = 0.0;
        }
        self.combat.replay_playing = playing;
        self.combat.replay_generation += 1;
        self.combat.replay_pumping = false;
        self.combat.replay_at = None;
        if playing {
            self.combat_replay_pump(cx);
        }
        cx.notify();
    }

    /// Move the scrubber to a frame, pausing the loop.
    pub(crate) fn combat_replay_seek(&mut self, frame: f64, cx: &mut Context<Self>) {
        let Some((len, _)) = self.combat_replay_info(cx) else {
            return;
        };
        self.combat.replay_playing = false;
        self.combat.replay_generation += 1;
        self.combat.replay_pumping = false;
        self.combat.replay_at = None;
        self.combat.replay_pos = frame.clamp(0.0, len.saturating_sub(1) as f64);
        cx.notify();
    }

    /// Move the playhead to the first frame at or after `t_ms`; the Den's wave
    /// controls read their boundaries from the projected attempt.
    pub(crate) fn combat_replay_seek_time(&mut self, t_ms: u64, cx: &mut Context<Self>) {
        let index = {
            let runner = self.combat_encounter.read(cx);
            runner
                .result()
                .and_then(|result| result.run.replay.as_ref())
                .map(|replay| replay.index_at(t_ms as f64))
        };
        if let Some(index) = index {
            self.combat_replay_seek(index as f64, cx);
        }
    }

    /// Pick a playback speed; it applies from the current playhead.
    pub(crate) fn combat_replay_set_speed(&mut self, speed: f64, cx: &mut Context<Self>) {
        if speed.is_finite() && speed > 0.0 {
            self.combat.replay_speed = speed;
        }
        cx.notify();
    }

    /// The playback loop: advance the playhead by the elapsed wall time scaled
    /// by the speed, and stop on the last frame.
    pub(crate) fn combat_replay_pump(&mut self, cx: &mut Context<Self>) {
        if !self.combat.replay_playing || self.combat.replay_pumping {
            return;
        }
        self.combat.replay_pumping = true;
        self.combat.replay_at = None;
        let generation = self.combat.replay_generation;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(REPLAY_FRAME).await;
                let keep = this
                    .update(cx, |state, cx| state.combat_replay_frame(generation, cx))
                    .unwrap_or(false);
                if !keep {
                    return;
                }
            }
        })
        .detach();
    }

    /// One playback frame; false once the loop should stop.
    fn combat_replay_frame(&mut self, generation: u64, cx: &mut Context<Self>) -> bool {
        if generation != self.combat.replay_generation || !self.combat.replay_playing {
            self.combat.replay_pumping = false;
            return false;
        }
        let Some((len, interval)) = self.combat_replay_info(cx) else {
            self.combat.replay_playing = false;
            self.combat.replay_pumping = false;
            return false;
        };
        let last = len.saturating_sub(1) as f64;
        let now = Instant::now();
        let elapsed = self
            .combat
            .replay_at
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0)
            .min(0.25);
        // Speed is simulated seconds per second: a frame covers `interval` ms.
        self.combat.replay_pos +=
            elapsed * self.combat.replay_speed * 1000.0 / interval.max(1) as f64;
        if self.combat.replay_pos >= last {
            self.combat.replay_pos = last;
            self.combat.replay_playing = false;
            self.combat.replay_pumping = false;
        }
        cx.notify();
        self.combat.replay_playing
    }

    /// Capture the stored Killroy practice as a replay document: the imported
    /// save's map, duration and seed, ready to be opened again.
    pub(crate) fn capture_combat_killroy_replay(&mut self, cx: &mut Context<Self>) {
        let Some(save) = self.save.as_ref() else {
            self.set_status("Import a save before capturing a Killroy replay.", cx);
            return;
        };
        let raw = save.root().clone();
        let character = self.combat.character;
        let run = {
            let runner = self.combat_encounter.read(cx);
            match runner.result() {
                Some(result) if result.killroy.is_some() => Some(result.run.clone()),
                _ => None,
            }
        };
        let Some(run) = run else {
            self.set_status(
                "A Killroy replay needs a finished practice: run a map with kills first.",
                cx,
            );
            return;
        };
        let label = format!(
            "Killroy practice · {}",
            idleon_combat::encounter::map(run.map)
                .map(idleon_combat::encounter::map_name)
                .unwrap_or_else(|| format!("Map {}", run.map))
        );
        // A practice reads the run's kill rate, not its loadout, so a capture
        // carries no setup patches: the imported save is exactly what it ran.
        let replay = idleon_combat::replay::KillroyReplay::capture(
            raw,
            character,
            run.map,
            run.simulated_ms as f64 / 1000.0,
            run.seed as i64,
            label,
            false,
            false,
            serde_json::json!({}),
            serde_json::json!({}),
        );
        if let Err(error) = replay.validate() {
            self.set_status(error, cx);
            return;
        }
        self.combat.killroy_replay = Some(replay);
        self.set_status("Captured the Killroy replay.", cx);
        cx.notify();
    }

    /// Open the captured Killroy replay: restore its character, map, duration
    /// and seed, then run it through the encounter.
    pub(crate) fn open_combat_killroy_replay(&mut self, cx: &mut Context<Self>) {
        let Some(replay) = self.combat.killroy_replay.clone() else {
            return;
        };
        if let Err(error) = replay.validate() {
            self.set_status(error, cx);
            return;
        }
        let character = replay.char_idx.max(0) as usize;
        let count = self.save.as_ref().map_or(0, |save| save.character_count());
        if character < count && character != self.combat.character {
            let save = self.save.clone();
            if let Some(save) = save {
                self.combat.load(&save, character);
            }
        }
        self.combat.sim_map = replay.map_idx.max(0) as usize;
        self.combat.sim_duration_s = replay.seconds.round().clamp(1.0, 86_400.0) as u64;
        self.combat.sim_seed = replay.seed.max(1) as u64;
        self.combat.sim_stand_in = true;
        self.combat.replay_pos = 0.0;
        self.combat.pending_source = Some(replay);
        self.run_combat_encounter(cx);
    }

    /// Drop the captured Killroy replay.
    pub(crate) fn forget_combat_killroy_replay(&mut self, cx: &mut Context<Self>) {
        self.combat.killroy_replay = None;
        self.set_status("Forgot the captured Killroy replay.", cx);
        cx.notify();
    }
}
