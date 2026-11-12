//! The window: shell, dashboard, shared section chrome and the automation bridge.

pub(crate) mod assets;
pub(crate) mod bridge;
pub(crate) mod dashboard;
pub(crate) mod export;
pub(crate) mod import;
pub(crate) mod layout;
pub(crate) mod section;
pub(crate) mod shell;
pub(crate) mod theme;

use crate::state::AppState;
use ely_gpui_component::forms::TextInput;
use gpui::{App, AppContext as _, Context, Entity, IntoElement, Render, Window};
use idleon_ui::formula::FormulaTree;

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        shell::shell(self, window, cx)
    }
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
