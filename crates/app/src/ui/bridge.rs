//! The gpui-mcp bridge: how an agent reads and drives this window.

use crate::state::AppState;
use gpui::{Context, Window};
use gpui_mcp::{AppId, BridgeConfig, BridgeHandle};

/// Install the bridge for the window; keep the handle alive in the app state.
pub fn install(window: &mut Window, cx: &mut Context<AppState>) -> BridgeHandle {
    BridgeHandle::install(
        window,
        cx,
        BridgeConfig::new(AppId::new("idleon-optimizer").expect("valid app id"), "Idleon Optimizer"),
    )
    .expect("the automation bridge installs")
}
