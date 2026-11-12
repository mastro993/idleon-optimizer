//! What the window does when the combat sim section is edited or run.

use crate::sections::combat_sim::Tab as CombatTab;
use crate::state::AppState;
use gpui::Context;

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
}
