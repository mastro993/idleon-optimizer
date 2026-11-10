//! The window: shell, dashboard, shared section chrome and the automation bridge.

pub mod bridge;
pub mod dashboard;
pub mod format;
pub mod import;
pub mod layout;
pub mod section;
pub mod shell;
pub mod theme;

use crate::state::AppState;
use gpui::{Context, IntoElement, Render, Window};

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        shell::shell(self, window, cx)
    }
}
