//! The window: shell, dashboard, shared section chrome and the automation bridge.

pub(crate) mod assets;
#[cfg(feature = "automation")]
pub(crate) mod bridge;
pub(crate) mod dashboard;
pub(crate) mod export;
pub(crate) mod header;
pub(crate) mod import;
pub(crate) mod layout;
pub(crate) mod mark;
pub(crate) mod section;
pub(crate) mod shell;
pub(crate) mod sidebar;
pub(crate) mod theme;
pub(crate) mod views;

use crate::state::AppState;
use ely_gpui_component::forms::TextInput;
use gpui::{
    Action, App, AppContext as _, Context, Div, ElementId, Entity, InteractiveElement as _,
    IntoElement, KeyBinding, KeyBindingContextPredicate, ParentElement as _, Render, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{InputState, NumberInput};
use idleon_ui::formula::FormulaTree;
use std::rc::Rc;

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        shell::shell(self, window, cx)
    }
}

/// A key binding in `context`. `secondary-` resolves against the host OS (Cmd on
/// macOS, Ctrl elsewhere), which `KeyBinding::new` refuses since gpui 0.3.10.
pub(crate) fn key_binding<A: Action>(
    keystrokes: &str,
    action: A,
    context: &str,
    cx: &App,
) -> KeyBinding {
    let predicate = KeyBindingContextPredicate::parse(context).expect("a valid key context");
    KeyBinding::load(
        keystrokes,
        Box::new(action),
        Some(Rc::new(predicate)),
        false,
        None,
        cx,
    )
    .expect("a valid keystroke")
}

/// A formula tree whose element ids start with `id`; its owner replaces the
/// root each render.
pub(crate) fn formula_tree(
    id: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<FormulaTree> {
    cx.new(|cx| FormulaTree::new(id, window, cx))
}

/// A one-line text field with its placeholder and accessible label.
pub(crate) fn text_input(
    placeholder: &'static str,
    label: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<TextInput> {
    cx.new(|cx| {
        TextInput::new(window, cx)
            .placeholder(placeholder)
            .label(label)
    })
}

/// A one-line kit text field showing `placeholder` while empty. The render
/// line names it with `Input::aria_label`.
pub(crate) fn input_state(
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
}

/// A small kit number field under the element id the harnesses press.
/// `NumberInput` takes no id, so the wrapper carries it, and a press on the
/// wrapper focuses the field, where Up and Down step it.
pub(crate) fn number_field(
    id: impl Into<ElementId>,
    field: &Entity<InputState>,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    let focus = field.clone();
    div()
        .id(id)
        .w(px(110.0))
        .on_click(move |_event, window, cx| {
            focus.update(cx, |field, cx| field.focus(window, cx));
        })
        .child(NumberInput::new(field).accessibility_label(label).small())
}
