//! Everything the Opal Optimizer keeps: the imported villagers, the visible
//! selections, the plan runner and the formula tree, with the section's import
//! step and project config.
//!
//! `AppState` holds one [`State`]; `load_imported` calls [`State::on_import`],
//! and the project's `configs`/`assign` call [`State::config`] and
//! [`State::apply_config`]. See `docs/section-state.md`.

use super::{MAX_OPALS, Tab};
use crate::state::{AppState, ProgressLine, RunStatus, report_run};
use gpui::{App, Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use idleon_opal::{NAMES, Opal, Plan, PlanInput, PlanTarget, VILLAGERS};
use idleon_ui::formula::FormulaTree;
use idleon_ui::runner::{TaskRunner, TaskState};
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value};

/// The section's project config: the villager the breakdown is switched to, the
/// optimizer's tab, target and total, and the manual simulator's counts. The
/// villagers themselves come from the save; a document from before the
/// optimizer has no `total`, and one from before the simulator no `manual`, so
/// both fall back to the save's own.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Config {
    selected: Option<usize>,
    tab: Tab,
    target: Option<usize>,
    total: Option<f64>,
    manual: Option<[f64; VILLAGERS]>,
}

/// The section's state, kept across dashboard visits.
pub(crate) struct State {
    /// The villagers and account bonuses, when the save has them.
    pub(crate) save: Option<Opal>,
    /// The villager the breakdown is switched to, or the total.
    pub(crate) selected: Option<usize>,
    pub(crate) tab: Tab,
    /// The villager the optimizer maximizes, or `None` for the total.
    pub(crate) target: Option<usize>,
    /// The opal total the optimizer distributes, seeded from the save.
    pub(crate) total: f64,
    /// The manual simulator's opal counts, seeded from and reset to the save.
    pub(crate) manual: [f64; VILLAGERS],
    /// The distribution plan run.
    pub(crate) run: Entity<TaskRunner<Plan, PlanInput>>,
    /// The formula tree on the overview.
    pub(crate) breakdown: Entity<FormulaTree>,
    /// The save revision and villager the tree was last rooted at.
    tree_key: Option<(u64, Option<usize>)>,
    /// The optimizer's target, total and the manual simulator's counts: kit
    /// controls that show the values above and edit them through the setters.
    pub(crate) target_select: ChoiceSelect<Option<usize>>,
    pub(crate) total_field: Entity<InputState>,
    pub(crate) manual_fields: [Entity<InputState>; VILLAGERS],
    _subscriptions: Vec<Subscription>,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let mut subscriptions = Vec::new();
        let run = report_run::<Plan, PlanInput>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "opal",
                progress: ProgressLine::Message("Optimizing the opal distribution…"),
                cancelled_kept: "Opal plan cancelled; the previous plan is unchanged.",
                cancelled_empty: "Opal plan cancelled; nothing was planned.",
            },
            |_, plan| {
                Some(format!(
                    "Opal plan ready: {} opals across the villagers.",
                    idleon_ui::format::count(plan.spent() as u64)
                ))
            },
        );
        let target_select = ChoiceSelect::new(target_choices(None), Some(&None), window, cx);
        let total_field = number_state(0.0, f64::MAX, 1.0, window, cx);
        let manual_fields = std::array::from_fn(|_| number_state(0.0, f64::MAX, 1.0, window, cx));
        // Edits flow control -> section value, through the section's setters.
        subscriptions.push(cx.subscribe(
            target_select.state(),
            |state, _, event: &SelectEvent<Vec<Choice<Option<usize>>>>, cx| {
                if let SelectEvent::Confirm(Some(target)) = event {
                    state.opal.set_target(*target, cx);
                }
            },
        ));
        subscriptions.push(cx.subscribe(&total_field, |state, field, event, cx| {
            if matches!(event, InputEvent::Change)
                && let Some(total) = number_value(field.read(cx))
            {
                let total = total.min(state.opal.max_opals(state.opal.total));
                state.opal.set_total(total, cx);
            }
        }));
        for (villager, field) in manual_fields.iter().enumerate() {
            subscriptions.push(cx.subscribe(field, move |state, field, event, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(opals) = number_value(field.read(cx))
                {
                    let saved = state.opal.saved_total();
                    let opals = opals.min(state.opal.max_opals(saved));
                    state.opal.set_manual(villager, opals, cx);
                }
            }));
        }
        Self {
            save: None,
            selected: None,
            tab: Tab::default(),
            target: None,
            total: 0.0,
            manual: [0.0; VILLAGERS],
            run,
            breakdown: crate::ui::formula_tree("opal-formula", window, cx),
            tree_key: None,
            target_select,
            total_field,
            manual_fields,
            _subscriptions: subscriptions,
        }
    }

    /// The most opals a typed count takes: the static ceiling, or `current`
    /// when a save already holds more.
    fn max_opals(&self, current: f64) -> f64 {
        MAX_OPALS.max(current)
    }

    /// Root the formula tree at the selected villager or the total, once per
    /// change of the save or the selection.
    pub(crate) fn sync_tree(&mut self, save_revision: u64, cx: &mut Context<AppState>) {
        let key = (save_revision, self.selected);
        if self.tree_key == Some(key) {
            return;
        }
        let Some(opal) = self.save.as_ref() else {
            return;
        };
        self.tree_key = Some(key);
        let root = opal.breakdown(self.selected);
        self.breakdown
            .update(cx, |tree, cx| tree.set_root(root, cx));
    }

    /// Take a new save's villagers. The selections go back to the total, the
    /// plan's default total is the new save's invested opals, and the manual
    /// simulator starts on that save's own split. A stored plan stays: the
    /// runner marks it stale once its inputs no longer match.
    pub(crate) fn on_import(&mut self, opal: Option<Opal>) {
        self.save = opal;
        self.selected = None;
        self.target = None;
        self.total = self.saved_total();
        self.manual = self.saved_split();
    }

    /// The config a project export writes.
    pub(crate) fn config(&self) -> Config {
        Config {
            selected: self.selected,
            tab: self.tab,
            target: self.target,
            total: Some(self.total),
            manual: Some(self.manual),
        }
    }

    /// Apply a decoded config over the loaded save; a villager the save has
    /// not unlocked falls back to the total.
    pub(crate) fn apply_config(&mut self, config: Config, cx: &mut Context<AppState>) {
        self.set_villager(config.selected, cx);
        self.set_tab(config.tab, cx);
        self.set_target(config.target, cx);
        match config.total {
            Some(total) => self.set_total(total, cx),
            None => self.use_saved_total(cx),
        }
        match config.manual {
            Some(manual) => {
                self.manual = manual.map(idleon_opal::whole);
                cx.notify();
            }
            None => self.reset_manual(cx),
        }
    }

    /// `villager` when the save has it unlocked.
    fn unlocked(&self, villager: Option<usize>) -> Option<usize> {
        villager.filter(|villager| {
            self.save
                .as_ref()
                .is_some_and(|opal| opal.unlocked(*villager))
        })
    }

    pub(crate) fn saved_total(&self) -> f64 {
        self.save.as_ref().map_or(0.0, Opal::total_opals)
    }

    fn saved_split(&self) -> [f64; VILLAGERS] {
        self.save.as_ref().map_or([0.0; VILLAGERS], Opal::opals)
    }

    /// Switch the breakdown between the total and one villager.
    ///
    /// A locked villager (or no save) falls back to the total, so a stale
    /// selection never points at a villager this save does not have.
    pub(crate) fn set_villager(&mut self, villager: Option<usize>, cx: &mut Context<AppState>) {
        let selected = self.unlocked(villager);
        if self.selected != selected {
            self.selected = selected;
            cx.notify();
        }
    }

    /// Switch the optimizer between the total and one villager.
    pub(crate) fn set_target(&mut self, villager: Option<usize>, cx: &mut Context<AppState>) {
        let target = self.unlocked(villager);
        if self.target != target {
            self.target = target;
            cx.notify();
        }
    }

    pub(crate) fn set_tab(&mut self, tab: Tab, cx: &mut Context<AppState>) {
        if self.tab != tab {
            self.tab = tab;
            cx.notify();
        }
    }

    /// Set the opal total the optimizer splits; whole opals only.
    pub(crate) fn set_total(&mut self, total: f64, cx: &mut Context<AppState>) {
        let total = idleon_opal::whole(total);
        if self.total != total {
            self.total = total;
            cx.notify();
        }
    }

    /// Put the optimizer back on the save's own opal total.
    pub(crate) fn use_saved_total(&mut self, cx: &mut Context<AppState>) {
        self.set_total(self.saved_total(), cx);
    }

    /// Set one villager's manual opal count; whole opals only.
    pub(crate) fn set_manual(&mut self, villager: usize, opals: f64, cx: &mut Context<AppState>) {
        let Some(count) = self.manual.get_mut(villager) else {
            return;
        };
        let opals = idleon_opal::whole(opals);
        if *count != opals {
            *count = opals;
            cx.notify();
        }
    }

    /// Put the manual simulator back on the save's own opal split.
    pub(crate) fn reset_manual(&mut self, cx: &mut Context<AppState>) {
        let saved = self.saved_split();
        if self.manual != saved {
            self.manual = saved;
            cx.notify();
        }
    }

    /// The plan's inputs: the selected target, the total and the save's split.
    pub(crate) fn plan_input(&self) -> PlanInput {
        PlanInput {
            target: self
                .unlocked(self.target)
                .map_or(PlanTarget::Total, PlanTarget::Villager),
            total: self.total,
            current: self.saved_split(),
        }
    }

    /// The plan run's progress, result and staleness.
    pub(crate) fn task(&self, cx: &App) -> TaskState<Plan> {
        self.run.read(cx).state(&self.plan_input())
    }
}

/// The optimizer's target list: the total, then each unlocked villager.
pub(crate) fn target_choices(opal: Option<&Opal>) -> Vec<Choice<Option<usize>>> {
    let villagers = opal.into_iter().flat_map(|opal| {
        (0..VILLAGERS)
            .filter(|villager| opal.unlocked(*villager))
            .map(|villager| {
                Choice::new(
                    Some(villager),
                    format!(
                        "{} (★{})",
                        NAMES[villager], opal.villagers[villager].stars as i64
                    ),
                )
            })
    });
    std::iter::once(Choice::new(None, "All villagers (total EXP)"))
        .chain(villagers)
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::state::headless;

    #[test]
    fn an_import_seeds_the_section_from_the_save_and_drops_the_old_selections() {
        let dir = std::env::temp_dir().join(format!("idleon-opal-import-{}", std::process::id()));
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
        let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
        let imported = idleon_import::import(&text).expect("the fixture imports");
        let mut cx = gpui::TestAppContext::single();
        let window = headless(&mut cx, &dir);
        window
            .update(&mut cx, |state, _, cx| {
                state.opal.selected = Some(0);
                state.opal.target = Some(0);
                state.opal.total = 1.0;
                state.load_imported(imported, cx);
                let opal = state.opal.save.clone().expect("the fixture has villagers");
                assert!(opal.unlocked_count() > 0);
                assert_eq!(state.opal.selected, None);
                assert_eq!(state.opal.target, None);
                assert_eq!(state.opal.total, opal.total_opals());
                assert_eq!(state.opal.manual, opal.opals());
            })
            .expect("the window is open");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
