//! What the window does when the stats all section is edited or run.

use crate::state::AppState;
use gpui::Context;
use idleon_stats::{ConstructionMode, MapVisibility, Skill};

impl AppState {
    /// Pick the stats context's character; the map follows its current map and
    /// then the map list, exactly like an upstream character switch.
    pub(crate) fn set_stats_character(&mut self, character: usize, cx: &mut Context<Self>) {
        let Some(snapshot) = self.stats_all.snapshot.clone() else {
            return;
        };
        if character >= snapshot.characters() {
            return;
        }
        self.stats_all
            .context
            .select_character(character, &snapshot);
        self.stats_context_changed(cx);
    }

    /// Pick the stats context's map, from the current map list.
    pub(crate) fn set_stats_map(&mut self, map: usize, cx: &mut Context<Self>) {
        if !self
            .stats_all
            .options()
            .iter()
            .any(|option| option.class.map == map)
        {
            return;
        }
        self.stats_all.context.map = map;
        self.stats_context_changed(cx);
    }

    /// Switch the map list level; the selected map follows it when it can.
    pub(crate) fn set_stats_map_visibility(
        &mut self,
        level: MapVisibility,
        cx: &mut Context<Self>,
    ) {
        self.stats_all.context.map_visibility = level;
        self.stats_all.clamp(&mut Vec::new());
        self.stats_context_changed(cx);
    }

    /// Switch the skill the skill-scoped descriptors read.
    pub(crate) fn set_stats_skill(&mut self, skill: Skill, cx: &mut Context<Self>) {
        self.stats_all.context.skill = skill;
        self.stats_context_changed(cx);
    }

    /// Switch construction mode on or off.
    pub(crate) fn set_stats_construction(
        &mut self,
        mode: ConstructionMode,
        cx: &mut Context<Self>,
    ) {
        self.stats_all.context.construction = mode;
        self.stats_context_changed(cx);
    }

    /// Switch the breakdown tab and select the first row of the new category.
    pub(crate) fn set_stats_category(
        &mut self,
        category: idleon_stats::BreakdownCategory,
        cx: &mut Context<Self>,
    ) {
        self.stats_all.category = category;
        self.stats_all.select_first();
        cx.notify();
    }

    /// Switch the breakdown row the formula tree shows.
    pub(crate) fn set_stats_descriptor(&mut self, id: String, cx: &mut Context<Self>) {
        if self
            .stats_all
            .sheet
            .as_ref()
            .is_some_and(|sheet| sheet.get(&id).is_some())
        {
            self.stats_all.selected = Some(id);
            cx.notify();
        }
    }

    /// Filter the formula explorer to one category, or to every category.
    pub(crate) fn set_stats_explorer_category(
        &mut self,
        category: Option<idleon_stats::BreakdownCategory>,
        cx: &mut Context<Self>,
    ) {
        self.stats_all.explorer.category = category;
        cx.notify();
    }

    /// Filter the formula explorer by whether a formula is computed here.
    pub(crate) fn set_stats_explorer_status(
        &mut self,
        status: idleon_stats::ExplorerStatus,
        cx: &mut Context<Self>,
    ) {
        self.stats_all.explorer.status = status;
        cx.notify();
    }

    /// Show one formula in the explorer's card.
    pub(crate) fn set_stats_formula(&mut self, id: String, cx: &mut Context<Self>) {
        if self
            .stats_all
            .sheet
            .as_ref()
            .is_some_and(|sheet| sheet.get(&id).is_some())
        {
            self.stats_all.explorer.selected = Some(id);
            cx.notify();
        }
    }

    /// Record a hypothetical edit of one game input and re-resolve the page.
    ///
    /// Upstream applies the edit to a copy of the save, recalculates and keeps
    /// the message when the value is outside the input's own checks; the same
    /// refusal keeps the previous value and shows the error in the ledger.
    pub(crate) fn set_stats_input(&mut self, id: String, value: f64, cx: &mut Context<Self>) {
        let outcome = match self.stats_all.sandbox.as_mut() {
            Some(sandbox) => sandbox.set(&id, value),
            None => return,
        };
        match outcome {
            Ok(()) => self.stats_all.input_error.clear(),
            Err(error) => {
                self.stats_all.input_error = error;
                cx.notify();
                return;
            }
        }
        self.rebuild_stats_breakdowns();
        self.stats_sandbox_changed(cx);
    }

    /// Put one game input back to the value the imported save carries.
    pub(crate) fn reset_stats_input(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(sandbox) = self.stats_all.sandbox.as_mut() {
            sandbox.reset(&id);
        }
        self.stats_all.input_error.clear();
        self.rebuild_stats_breakdowns();
        self.stats_sandbox_changed(cx);
    }

    /// Drop every pending edit, which is upstream's `Reset all edits`.
    pub(crate) fn reset_stats_inputs(&mut self, cx: &mut Context<Self>) {
        if let Some(sandbox) = self.stats_all.sandbox.as_mut() {
            sandbox.reset_all();
        }
        self.stats_all.input_error.clear();
        self.rebuild_stats_breakdowns();
        self.stats_sandbox_changed(cx);
    }

    /// Show one descriptor from the affected-formula list: the tab follows it,
    /// then the row is selected, like upstream's link into the breakdowns.
    pub(crate) fn select_stats_descriptor(&mut self, id: String, cx: &mut Context<Self>) {
        let category = match self
            .stats_all
            .sheet
            .as_ref()
            .and_then(|sheet| sheet.get(&id))
        {
            Some(row) => row.category,
            None => return,
        };
        self.stats_all.category = category;
        self.stats_all.selected = Some(id);
        cx.notify();
    }

    /// Re-resolve the breakdowns from the save and the context.
    pub(crate) fn rebuild_stats_breakdowns(&mut self) {
        let Some(save) = self.save.as_ref() else {
            return;
        };
        self.stats_all
            .rebuild(save, &self.simulation, &self.context);
    }

    /// The status line a sandbox edit leaves: what is hypothetical and how far
    /// it reaches.
    fn stats_sandbox_changed(&mut self, cx: &mut Context<Self>) {
        let (edited, affected) = match self.stats_all.sandbox.as_ref() {
            Some(sandbox) => (sandbox.count(), self.stats_all.affected.len()),
            None => (0, 0),
        };
        self.set_status(
            match edited {
                0 => "Stats sandbox: no edits.".to_string(),
                1 => format!("Stats sandbox: 1 edited game input · {affected} affected formulas"),
                count => {
                    format!(
                        "Stats sandbox: {count} edited game inputs · {affected} affected formulas"
                    )
                }
            },
            cx,
        );
    }

    /// A context change refreshes every value the page derives from it.
    pub(crate) fn stats_context_changed(&mut self, cx: &mut Context<Self>) {
        self.rebuild_stats_breakdowns();
        let class = idleon_stats::classify(self.stats_all.context.map);
        let character = self
            .stats_all
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.name(self.stats_all.context.character))
            .unwrap_or_default();
        self.set_status(
            if character.is_empty() {
                "Load a save to resolve the stats context.".into()
            } else {
                format!(
                    "Stats context: {character} · {} · {} · Construction {}",
                    class.name,
                    self.stats_all.context.skill.label(),
                    self.stats_all.context.construction.label()
                )
            },
            cx,
        );
    }
}
