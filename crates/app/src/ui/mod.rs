//! The window: shell, panes and the automation bridge.

pub mod bridge;
pub mod dashboard;
pub mod format;
pub mod grid_view;
pub mod inspector;
pub mod optimizer;
pub mod rates;
pub mod section;
pub mod shell;

use crate::state::AppState;
use gpui::{Context, IntoElement, Render, Window};

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        shell::shell(self, window, cx)
    }
}
