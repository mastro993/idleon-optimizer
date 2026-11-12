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

    /// Re-resolve the breakdowns from the save and the context.
    pub(crate) fn rebuild_stats_breakdowns(&mut self) {
        let Some(save) = self.save.as_ref() else {
            return;
        };
        self.stats_all
            .rebuild(save, &self.simulation, &self.context);
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
