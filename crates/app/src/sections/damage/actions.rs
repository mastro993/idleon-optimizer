//! What the window does when the damage section is edited or run.

use crate::state::AppState;
use gpui::Context;

impl AppState {
    /// Pick the damage context's character; the map follows its current map.
    pub(crate) fn set_damage_character(&mut self, character: usize, cx: &mut Context<Self>) {
        self.damage.select_character(character);
        self.damage_context_changed(cx);
    }

    /// Pick the damage context's map, from the all-maps list.
    pub(crate) fn set_damage_map(&mut self, map: usize, cx: &mut Context<Self>) {
        self.damage.select_map(map);
        self.damage_context_changed(cx);
    }

    /// A damage context change refreshes the summary the page shows.
    pub(crate) fn damage_context_changed(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = self.damage.snapshot.as_ref() else {
            self.set_status("Load a save to resolve the damage context.", cx);
            return;
        };
        let class = idleon_stats::classify(self.damage.context.map);
        self.set_status(
            format!(
                "Damage context: {} · {} · {}",
                snapshot.name(self.damage.context.character),
                class.name,
                class.category.label()
            ),
            cx,
        );
    }
}
