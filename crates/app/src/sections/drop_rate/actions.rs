//! What the window does when the drop rate section is edited or run.

use crate::state::AppState;
use gpui::Context;

impl AppState {
    /// Pick a character and map for the drop-rate page, exactly like upstream
    /// `switchChar`/`switchMap`.
    pub(crate) fn set_drop_rate_character(&mut self, character: usize, cx: &mut Context<Self>) {
        let Some(snapshot) = self.drop_rate.snapshot.as_ref() else {
            return;
        };
        let mut selection = self.drop_rate.selection;
        selection.select_character(character, snapshot);
        self.drop_rate.selection = selection;
        self.drop_rate_changed(cx);
    }

    /// Pick the drop-rate page's map from the selector's rows.
    pub(crate) fn set_drop_rate_map(&mut self, map: usize, cx: &mut Context<Self>) {
        let Some(snapshot) = self.drop_rate.snapshot.as_ref() else {
            return;
        };
        let mut selection = self.drop_rate.selection;
        selection.select_map(map, snapshot);
        self.drop_rate.selection = selection;
        self.drop_rate_changed(cx);
    }

    /// A context change refreshes the total the page derives from it.
    pub(crate) fn drop_rate_changed(&mut self, cx: &mut Context<Self>) {
        let selection = self.drop_rate.selection;
        let character = self
            .drop_rate
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.name(selection.character))
            .unwrap_or_default();
        let map = idleon_stats::classify(selection.map);
        let run = if idleon_drop_rate::is_dungeon_map(selection.map) {
            " · live dungeon inputs"
        } else {
            ""
        };
        self.set_status(
            if character.is_empty() {
                "Load a save to resolve the drop rate.".into()
            } else {
                format!("Drop rate: {character} · {}{run}", map.name)
            },
            cx,
        );
    }
}
