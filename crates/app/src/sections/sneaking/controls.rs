//! The kit controls the section owns (`docs/ui-kit/forms.md`) and the figures
//! render reads.
//!
//! The three selects are built once here; an edit comes back through a
//! subscription that calls the section's setter, and render pushes the
//! section's values back into them. The loadout figures, the floor table and
//! the breakdown tree are derived once per change of the save, the loadout or
//! the selection, never per frame.

use super::state::{FormulaTab, State};
use crate::state::AppState;
use gpui::{Context, Entity, Subscription, Window};
use gpui_kit::component::select::SelectEvent;
use idleon_common::FormulaNode;
use idleon_sneaking::{
    ActiveDoor, FloorMastery, Job, Loadout, NextCaptive, Objective, Outcome, Request, SlotKind,
    breakdown,
};
use idleon_ui::runner::{Ending, TaskEvent, TaskRunner};
use idleon_ui::widgets::{Choice, ChoiceSelect};
use std::rc::Rc;

/// What the figures are derived from: the save, the loadout edits since it
/// loaded, and the section's own selection.
pub(crate) type Key = (u64, u64, State);

pub(crate) struct Controls {
    pub(crate) login: ChoiceSelect<usize>,
    pub(crate) breakdown_twin: ChoiceSelect<usize>,
    pub(crate) objective: ChoiceSelect<Objective>,
    /// Bumped by every loadout edit; with the save revision it keys the caches.
    pub(crate) edits: u64,
    figures: Option<(Key, Rc<Figures>)>,
    floors: Option<((u64, u64), Rc<Floors>)>,
    /// What the breakdown tree was last given, so its root is built only when
    /// the loadout, the twin or the formula tab changes.
    breakdown: Option<(u64, u64, usize, FormulaTab)>,
    /// The stored optimizer outcome, copied once when the run stores it.
    pub(crate) outcome: Option<Rc<Outcome>>,
    _subscriptions: Vec<Subscription>,
}

impl Controls {
    pub(crate) fn new(
        optimizer: &Entity<TaskRunner<Outcome, Request>>,
        window: &mut Window,
        cx: &mut Context<AppState>,
    ) -> Self {
        let login = ChoiceSelect::new(Vec::new(), None, window, cx);
        let breakdown_twin = ChoiceSelect::new(Vec::new(), None, window, cx);
        let objective =
            ChoiceSelect::new(objective_choices(), Some(&Objective::default()), window, cx);
        let subscriptions = vec![
            cx.subscribe(
                login.state(),
                |state, _, event: &SelectEvent<Vec<Choice<usize>>>, cx| {
                    if let SelectEvent::Confirm(Some(index)) = event {
                        state.sneak_login(*index, cx);
                    }
                },
            ),
            cx.subscribe(
                breakdown_twin.state(),
                |state, _, event: &SelectEvent<Vec<Choice<usize>>>, cx| {
                    if let SelectEvent::Confirm(Some(index)) = event {
                        state.sneak_breakdown_twin(*index, cx);
                    }
                },
            ),
            cx.subscribe(
                objective.state(),
                |state, _, event: &SelectEvent<Vec<Choice<Objective>>>, cx| {
                    if let SelectEvent::Confirm(Some(objective)) = event {
                        state.sneak_objective(*objective, cx);
                    }
                },
            ),
            // Keep the charm search's result once, when the run stores it.
            cx.subscribe(optimizer, |state, runner, event: &TaskEvent, cx| {
                if matches!(event, TaskEvent::Finished(Ending::Stored)) {
                    state.sneaking_controls.outcome =
                        runner.read(cx).result().cloned().map(Rc::new);
                }
            }),
        ];
        Self {
            login,
            breakdown_twin,
            objective,
            edits: 0,
            figures: None,
            floors: None,
            breakdown: None,
            outcome: None,
            _subscriptions: subscriptions,
        }
    }

    /// The loadout's figures for the save, its edits and the selection.
    pub(crate) fn figures(&mut self, key: Key, loadout: &Loadout) -> Rc<Figures> {
        if let Some((known, figures)) = &self.figures
            && *known == key
        {
            return figures.clone();
        }
        let figures = Rc::new(Figures::of(loadout, &key.2));
        self.figures = Some((key, figures.clone()));
        figures
    }

    /// The Floor & Mastery figures for the save and its edits.
    pub(crate) fn floors(&mut self, key: (u64, u64), loadout: &Loadout) -> Rc<Floors> {
        if let Some((known, floors)) = &self.floors
            && *known == key
        {
            return floors.clone();
        }
        let floors = Rc::new(Floors {
            door: loadout.active_door(),
            captive: loadout.next_captive(),
            rows: loadout.floor_mastery(),
        });
        self.floors = Some((key, floors.clone()));
        floors
    }

    /// The formula tree for `twin` and `tab`, or `None` while the tree already
    /// shows it.
    pub(crate) fn breakdown_root(
        &mut self,
        key: (u64, u64, usize, FormulaTab),
        loadout: &Loadout,
    ) -> Option<FormulaNode> {
        if self.breakdown == Some(key) {
            return None;
        }
        self.breakdown = Some(key);
        let (_, _, twin, tab) = key;
        Some(match tab {
            FormulaTab::Stealth => breakdown::stealth(loadout, twin),
            FormulaTab::Jade => breakdown::jade(loadout, twin),
            FormulaTab::Exp => breakdown::exp(loadout, twin),
            FormulaTab::Door => breakdown::door(loadout, twin),
            FormulaTab::Runtime => breakdown::runtime(loadout, twin, 1.0),
        })
    }
}

/// What the loadout workspace reads from the calculator.
pub(crate) struct Figures {
    /// The twin pickers' options: each active twin with its level and floor.
    pub(crate) twin_choices: Vec<Choice<usize>>,
    pub(crate) charmed: bool,
    pub(crate) first_batch: Option<(String, f64)>,
    /// One per twin, in roster order.
    pub(crate) twins: Vec<TwinFigures>,
    /// One per inventory row.
    pub(crate) inventory: Vec<SlotFigures>,
}

pub(crate) struct TwinFigures {
    pub(crate) job: Job,
    pub(crate) stealth: f64,
    pub(crate) attempt: f64,
    pub(crate) ko: f64,
    pub(crate) detection: f64,
    /// The charm bonus totals, as one line.
    pub(crate) bonuses: Option<String>,
    pub(crate) issues: Vec<String>,
}

#[derive(Clone, Copy)]
pub(crate) struct SlotFigures {
    /// The Gold charm the game's cache selects for its bonus type.
    pub(crate) best: bool,
    /// Whether the item fits the slot the inventory is aimed at.
    pub(crate) fits: bool,
    pub(crate) symbol: f64,
}

impl Figures {
    fn of(loadout: &Loadout, state: &State) -> Self {
        let players = loadout.players();
        let twin_choices = loadout
            .twins()
            .iter()
            .take(players)
            .map(|twin| {
                Choice::new(
                    twin.index,
                    format!("{} · Lv {} · floor {}", twin.name, twin.level, twin.floor),
                )
            })
            .collect();
        let twins = loadout
            .twins()
            .iter()
            .map(|twin| TwinFigures {
                job: loadout.job(twin.index),
                stealth: loadout.stealth(twin.index),
                attempt: loadout.attempt_seconds(twin.index),
                ko: loadout.ko_seconds(twin.index),
                detection: loadout.detection(twin.index),
                bonuses: bonus_totals(loadout, twin.index),
                issues: loadout
                    .twin_issues(twin.index)
                    .iter()
                    .map(|issue| issue.message())
                    .collect(),
            })
            .collect();
        let gold = loadout.gold_bonuses();
        let target = state.target(players);
        let inventory = loadout
            .inventory()
            .iter()
            .enumerate()
            .map(|(row, item)| SlotFigures {
                best: !item.is_blank()
                    && item.def().is_some_and(|def| {
                        gold.selected(def.bonus_type)
                            .is_some_and(|pick| pick.row == row)
                    }),
                fits: !item.is_blank()
                    && target.is_some_and(|(twin, slot)| {
                        let slot = SlotKind::of(slot).unwrap_or(SlotKind::Hat);
                        loadout.check_slot(twin, slot, item).is_ok()
                    }),
                symbol: loadout.inventory_symbol(row),
            })
            .collect();
        Self {
            twin_choices,
            charmed: loadout.charmed(),
            first_batch: loadout.first_batch(),
            twins,
            inventory,
        }
    }
}

/// The Floor & Mastery workspace's decision cards and table.
pub(crate) struct Floors {
    pub(crate) door: Option<ActiveDoor>,
    pub(crate) captive: Option<NextCaptive>,
    pub(crate) rows: Vec<FloorMastery>,
}

/// The charm bonus totals a twin's equipped charms give, as one line.
fn bonus_totals(loadout: &Loadout, twin: usize) -> Option<String> {
    let bonuses = loadout.bonuses(twin);
    let parts: Vec<String> = bonuses
        .nonzero()
        .map(|(bonus_type, value)| {
            format!(
                "{} {:.1}",
                idleon_sneaking::charm_bonus_label(bonus_type),
                value
            )
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

pub(crate) fn objective_choices() -> Vec<Choice<Objective>> {
    Objective::ALL
        .into_iter()
        .map(|objective| Choice::new(objective, objective.label()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loadout() -> Loadout {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/data.json"
        ))
        .expect("the fixture is committed");
        idleon_import::import(&text)
            .expect("the fixture imports")
            .sneaking
            .expect("the fixture carries Ninja rows")
    }

    #[test]
    fn the_figures_read_the_loadout_and_the_aimed_slot() {
        let loadout = loadout();
        let mut state = State::default();
        let figures = Figures::of(&loadout, &state);
        assert_eq!(figures.twins.len(), loadout.twins().len());
        for twin in loadout.twins() {
            let read = &figures.twins[twin.index];
            assert_eq!(read.detection, loadout.detection(twin.index));
            assert_eq!(read.stealth, loadout.stealth(twin.index));
        }
        assert_eq!(figures.inventory.len(), loadout.inventory().len());
        // Nothing fits until a slot is aimed at.
        assert!(figures.inventory.iter().all(|slot| !slot.fits));

        state.select(0, 3);
        let aimed = Figures::of(&loadout, &state);
        for (row, item) in loadout.inventory().iter().enumerate() {
            let fits = !item.is_blank() && loadout.check_slot(0, SlotKind::Charm(1), item).is_ok();
            assert_eq!(aimed.inventory[row].fits, fits, "row {row}");
        }
        assert!(aimed.inventory.iter().any(|slot| slot.fits));
    }
}
