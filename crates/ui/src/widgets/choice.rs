//! Options for the kit `Select`, and a select whose items and pick mirror
//! section state.

use gpui::{App, AppContext as _, Entity, SharedString, Window};
use gpui_kit::component::IndexPath;
use gpui_kit::component::searchable_list::{SearchableListDelegate as _, SearchableListItem};
use gpui_kit::component::select::SelectState;
use std::cell::RefCell;

/// One option of a kit `Select`: the value a pick reports and the label it
/// shows. The kit's own items (`String`, `&str`) report their label, so a
/// typed value such as a villager index needs this.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice<V> {
    value: V,
    label: SharedString,
    disabled: bool,
}

impl<V> Choice<V> {
    /// An option showing `label` that reports `value`.
    pub fn new(value: V, label: impl Into<SharedString>) -> Self {
        Self {
            value,
            label: label.into(),
            disabled: false,
        }
    }

    /// Shown but not pickable.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<V: Clone + PartialEq> SearchableListItem for Choice<V> {
    type Value = V;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &V {
        &self.value
    }

    fn disabled(&self) -> bool {
        self.disabled
    }
}

/// A kit `SelectState` over [`Choice`]s that a section's render keeps in step
/// with the section's own value. Picks still reach the section through
/// `SelectEvent::Confirm`; [`Self::show`] is the way back, for a value or a
/// list that changed elsewhere (an import, a config, a reset).
pub struct ChoiceSelect<V: Clone + PartialEq + 'static> {
    state: Entity<SelectState<Vec<Choice<V>>>>,
    /// The items the state holds, so an unchanged list is never pushed again.
    shown: RefCell<Vec<Choice<V>>>,
}

impl<V: Clone + PartialEq + 'static> ChoiceSelect<V> {
    /// A select over `items` showing `selected`.
    pub fn new(
        items: Vec<Choice<V>>,
        selected: Option<&V>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let index = selected.and_then(|value| items.position(value));
        let shown = RefCell::new(items.clone());
        let state = cx.new(|cx| SelectState::new(items, index, window, cx));
        Self { state, shown }
    }

    /// The state the kit `Select` renders and the section subscribes to.
    pub fn state(&self) -> &Entity<SelectState<Vec<Choice<V>>>> {
        &self.state
    }

    /// Show `items` with `selected` picked, touching the state only where it
    /// shows something else. Writing emits no `SelectEvent::Confirm`.
    pub fn show(
        &self,
        items: Vec<Choice<V>>,
        selected: Option<&V>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if *self.shown.borrow() != items {
            self.shown.replace(items.clone());
            self.state
                .update(cx, |state, cx| state.set_items(items, window, cx));
        }
        if self.state.read(cx).selected_value() == selected {
            return;
        }
        let index: Option<IndexPath> =
            selected.and_then(|value| self.shown.borrow().position(value));
        self.state
            .update(cx, |state, cx| state.set_selected_index(index, window, cx));
    }
}

#[cfg(test)]
mod tests {
    use super::{Choice, ChoiceSelect};
    use gpui::{Context, IntoElement, Render, Subscription, TestAppContext, Window};
    use gpui_kit::component::select::{Select, SelectEvent};

    /// A view owning a villager pick that its select mirrors.
    struct Target {
        unlocked: usize,
        target: Option<usize>,
        select: ChoiceSelect<Option<usize>>,
        confirmed: Vec<Option<usize>>,
        _subscription: Subscription,
    }

    fn items(unlocked: usize) -> Vec<Choice<Option<usize>>> {
        std::iter::once(Choice::new(None, "All villagers"))
            .chain(
                (0..unlocked).map(|villager| Choice::new(Some(villager), format!("V{villager}"))),
            )
            .collect()
    }

    impl Render for Target {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.select
                .show(items(self.unlocked), Some(&self.target), window, cx);
            Select::new(self.select.state())
        }
    }

    #[gpui::test]
    fn the_select_follows_the_owner_without_reporting_a_pick(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(|window, cx| {
            let select = ChoiceSelect::new(items(1), Some(&None), window, cx);
            let subscription = cx.subscribe(
                select.state(),
                |view: &mut Target, _, event: &SelectEvent<Vec<Choice<Option<usize>>>>, _| {
                    let SelectEvent::Confirm(value) = event;
                    view.confirmed.push(value.flatten());
                },
            );
            Target {
                unlocked: 1,
                target: None,
                select,
                confirmed: Vec::new(),
                _subscription: subscription,
            }
        });
        let state = view.read_with(cx, |view, _| view.select.state().clone());
        assert_eq!(
            state.read_with(cx, |state, _| state.selected_value().cloned()),
            Some(None)
        );

        // A new save unlocks a villager and an applied config targets it.
        view.update(cx, |view, cx| {
            view.unlocked = 3;
            view.target = Some(2);
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(
            state.read_with(cx, |state, _| state.selected_value().cloned()),
            Some(Some(2))
        );
        assert!(view.read_with(cx, |view, _| view.confirmed.is_empty()));

        // A target the list no longer has clears the pick.
        view.update(cx, |view, cx| {
            view.unlocked = 1;
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(
            state.read_with(cx, |state, _| state.selected_value().cloned()),
            None
        );
    }

    #[test]
    fn a_choice_reports_its_value_and_shows_its_label() {
        use gpui_kit::component::searchable_list::SearchableListItem;
        let choice = Choice::new(4_usize, "Bob (★2)").disabled(true);
        assert_eq!(*choice.value(), 4);
        assert_eq!(choice.title(), "Bob (★2)");
        assert!(SearchableListItem::disabled(&choice));
    }
}
