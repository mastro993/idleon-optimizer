//! What the window does when the damage section is edited or run.

use crate::state::AppState;
use gpui::{AppContext as _, Context};

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

    /// Open one of the three damage layer tabs.
    pub(crate) fn set_damage_layer(
        &mut self,
        layer: idleon_damage::LayerId,
        cx: &mut Context<Self>,
    ) {
        self.damage.select_layer(layer);
        self.show_damage_trees(cx);
        cx.notify();
    }

    /// Resolve the summary a load, a restore or a context change left, off the
    /// UI thread: the page shows a skeleton until it lands.
    pub(crate) fn resolve_damage(&mut self, cx: &mut Context<Self>) {
        let Some(job) = self.damage.job() else {
            return;
        };
        let task = cx.spawn(async move |this, cx| {
            let summary = cx.background_spawn(async move { job() }).await;
            this.update(cx, |state, cx| {
                state.damage.summary = Some(summary);
                state.show_damage_trees(cx);
                cx.notify();
            })
            .ok();
        });
        self.damage.pending = Some(task);
    }

    /// Hand the summary's mastery and selected-layer trees to their cards.
    fn show_damage_trees(&mut self, cx: &mut Context<Self>) {
        if let Some(root) = self.damage.mastery_tree() {
            self.damage_mastery
                .update(cx, |tree, cx| tree.set_root(root, cx));
        }
        if let Some(root) = self.damage.layer_tree() {
            self.damage_formula
                .update(cx, |tree, cx| tree.set_root(root, cx));
        }
    }

    /// A damage context change resolves the summary again and names the new
    /// context.
    pub(crate) fn damage_context_changed(&mut self, cx: &mut Context<Self>) {
        self.resolve_damage(cx);
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
