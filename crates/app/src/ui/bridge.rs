//! The gpui-mcp bridge: how an agent reads and drives this window.

use crate::state::AppState;
use gpui::{Context, Window};
use gpui_mcp::{AppId, BridgeConfig, BridgeHandle};
use std::path::PathBuf;

/// Install the bridge for the window; keep the handle alive in the app state.
///
/// `endpoint_dir` isolates this instance's discovery files, so an agent lane can
/// run its own window while another lane drives a different one. The bridge is
/// a development facility: when it cannot start, the window opens without it.
pub(crate) fn install(
    window: &mut Window,
    cx: &mut Context<AppState>,
    endpoint_dir: Option<PathBuf>,
) -> Option<BridgeHandle> {
    let config = BridgeConfig::new(
        AppId::new("idleon-optimizer").expect("valid app id"),
        "Idleon Optimizer",
    );
    let config = match endpoint_dir {
        Some(directory) => match config.endpoint_dir(directory) {
            Ok(config) => config,
            Err(error) => {
                eprintln!("idleon-optimizer: the automation bridge did not start: {error}");
                return None;
            }
        },
        None => config,
    };
    match BridgeHandle::install(window, cx, config) {
        Ok(handle) => Some(handle),
        Err(error) => {
            eprintln!("idleon-optimizer: the automation bridge did not start: {error}");
            None
        }
    }
}
