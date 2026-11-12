//! The gpui-mcp bridge: how an agent reads and drives this window.

use crate::state::AppState;
use gpui::{Context, Window};
use gpui_mcp::{AppId, BridgeConfig, BridgeHandle};
use std::path::PathBuf;

/// Install the bridge for the window; keep the handle alive in the app state.
///
/// `endpoint_dir` isolates this instance's discovery files, so an agent lane can
/// run its own window while another lane drives a different one.
pub(crate) fn install(
    window: &mut Window,
    cx: &mut Context<AppState>,
    endpoint_dir: Option<PathBuf>,
) -> BridgeHandle {
    let config = BridgeConfig::new(
        AppId::new("idleon-optimizer").expect("valid app id"),
        "Idleon Optimizer",
    );
    let config = match endpoint_dir {
        Some(directory) => config
            .endpoint_dir(directory)
            .expect("the endpoint directory is absolute"),
        None => config,
    };
    BridgeHandle::install(window, cx, config).expect("the automation bridge installs")
}
