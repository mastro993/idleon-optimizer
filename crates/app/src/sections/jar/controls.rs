//! The enchant simulator's kit controls, and the figures the tabs read from the
//! save, built once.
//!
//! The persisted values stay on [`State`](super::State) (`docs/section-state.md`).
//! These entities only show them: edits flow control → value through the
//! subscriptions made here, and values flow back in render through
//! `show`/`show_number` (`docs/ui-kit/forms.md`). [`Figures`] holds what the
//! tabs list from the save (the groups, rankings, plans and schematics) and is
//! rebuilt only when another save is applied.

use super::state::{ENCHANTS, SIMS, TARGET_MAX};
use crate::state::AppState;
use gpui::{Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use idleon_jar::{
    COLLECTIBLES, CollectibleGroup, EnchantPriority, Jar, MilestoneRow, Schematic, Strategy,
    TierProjection, UpgradePlan, collectible_name,
};
use idleon_ui::formula::FormulaTree;
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value};

/// The largest seed the field takes.
const SEED_MAX: f64 = 4_294_967_295.0;

/// What the tabs list from one save, worked out when it is applied.
pub(crate) struct Figures {
    pub(super) groups: Vec<CollectibleGroup>,
    pub(super) priority: Vec<EnchantPriority>,
    pub(super) schematics: Vec<Schematic>,
    pub(super) upgrade_plan: UpgradePlan,
    pub(super) milestones: Vec<MilestoneRow>,
    pub(super) projections: Vec<TierProjection>,
    /// `—`, then every collectible the save has found.
    pub(super) focus: Vec<Choice<Option<usize>>>,
    /// The found collectibles, which the targets grid lists.
    pub(super) found: Vec<usize>,
}

impl Figures {
    fn of(jar: &Jar) -> Self {
        let found: Vec<usize> = (0..COLLECTIBLES)
            .filter(|index| jar.collectible(*index).is_some_and(|item| item.level >= 1))
            .collect();
        Self {
            groups: jar.collectible_groups(),
            priority: jar.enchant_priority(),
            schematics: jar.schematics(),
            upgrade_plan: jar.upgrade_plan(),
            milestones: jar.milestone_ranking(),
            projections: jar.tier_projections(),
            focus: std::iter::once(Choice::new(None, "—"))
                .chain(
                    found
                        .iter()
                        .map(|index| Choice::new(Some(*index), collectible_name(*index))),
                )
                .collect(),
            found,
        }
    }
}

/// The simulator's controls and the save's figures.
pub(crate) struct Controls {
    pub(super) enchants: Entity<InputState>,
    pub(super) sims: Entity<InputState>,
    pub(super) strategy: ChoiceSelect<Strategy>,
    pub(super) focus: ChoiceSelect<Option<usize>>,
    pub(super) seed: Entity<InputState>,
    /// One target field per collectible.
    pub(super) targets: Vec<Entity<InputState>>,
    revision: Option<u64>,
    pub(super) figures: Option<Figures>,
    _subscriptions: Vec<Subscription>,
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let enchants = number_state(
            f64::from(ENCHANTS.0),
            f64::from(ENCHANTS.1),
            1.0,
            window,
            cx,
        );
        let sims = number_state(f64::from(SIMS.0), f64::from(SIMS.1), 100.0, window, cx);
        let strategy = ChoiceSelect::new(strategy_choices(), None, window, cx);
        let focus = ChoiceSelect::new(Vec::new(), None, window, cx);
        let seed = number_state(1.0, SEED_MAX, 1.0, window, cx);
        let targets: Vec<Entity<InputState>> = (0..COLLECTIBLES)
            .map(|_| number_state(0.0, f64::from(TARGET_MAX), 1.0, window, cx))
            .collect();
        let mut subscriptions = vec![
            on_number(cx, &enchants, |state, value| {
                state.enchants = value.clamp(f64::from(ENCHANTS.0), f64::from(ENCHANTS.1)) as u32;
            }),
            on_number(cx, &sims, |state, value| {
                state.sims = value.clamp(f64::from(SIMS.0), f64::from(SIMS.1)) as u32;
            }),
            on_number(cx, &seed, |state, value| {
                state.seed = value.clamp(1.0, SEED_MAX) as u64;
            }),
            cx.subscribe(
                strategy.state(),
                |state, _, event: &SelectEvent<Vec<Choice<Strategy>>>, cx| {
                    let SelectEvent::Confirm(Some(strategy)) = event else {
                        return;
                    };
                    let strategy = *strategy;
                    state.edit_jar_state(cx, |jar| jar.strategy = strategy);
                },
            ),
            cx.subscribe(
                focus.state(),
                |state, _, event: &SelectEvent<Vec<Choice<Option<usize>>>>, cx| {
                    let SelectEvent::Confirm(Some(focus)) = event else {
                        return;
                    };
                    let focus = *focus;
                    state.edit_jar_state(cx, |jar| jar.focus = focus);
                },
            ),
        ];
        for (index, field) in targets.iter().enumerate() {
            subscriptions.push(on_number(cx, field, move |state, value| {
                let target = value.clamp(0.0, f64::from(TARGET_MAX)) as u32;
                if target == 0 {
                    state.targets.remove(&index);
                } else {
                    state.targets.insert(index, target);
                }
            }));
        }
        Self {
            enchants,
            sims,
            strategy,
            focus,
            seed,
            targets,
            revision: None,
            figures: None,
            _subscriptions: subscriptions,
        }
    }

    /// Bring the figures in line with the applied save: they and the rupie
    /// value tree are rebuilt only when another save is applied.
    pub(super) fn sync(
        &mut self,
        revision: u64,
        jar: &Jar,
        breakdown: &Entity<FormulaTree>,
        cx: &mut Context<AppState>,
    ) {
        if self.revision == Some(revision) {
            return;
        }
        self.revision = Some(revision);
        self.figures = Some(Figures::of(jar));
        let root = jar.rupie_value_tree();
        breakdown.update(cx, |tree, cx| tree.set_root(root, cx));
    }

    /// The figures [`Self::sync`] last built.
    pub(super) fn figures(&self) -> &Figures {
        self.figures
            .as_ref()
            .expect("the section syncs its figures before drawing a tab")
    }
}

/// A typed number reaches the simulator's state once it parses.
fn on_number(
    cx: &mut Context<AppState>,
    field: &Entity<InputState>,
    apply: impl Fn(&mut super::State, f64) + 'static,
) -> Subscription {
    cx.subscribe(field, move |state, field, event: &InputEvent, cx| {
        if matches!(event, InputEvent::Change)
            && let Some(value) = number_value(field.read(cx))
        {
            state.edit_jar_state(cx, |jar| apply(jar, value));
        }
    })
}

pub(super) fn strategy_choices() -> Vec<Choice<Strategy>> {
    Strategy::ALL
        .into_iter()
        .map(|strategy| Choice::new(strategy, strategy.label()))
        .collect()
}
