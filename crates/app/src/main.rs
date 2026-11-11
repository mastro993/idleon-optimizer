//! Desktop calculator suite for Legends of Idleon.

mod assets;
mod project;
mod sections;
mod session;
mod state;
mod task;
mod ui;

use anyhow::{Context as _, Result};
use ely_gpui_component::theme::Theme;
use gpui::{
    App, AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size,
};
use state::AppState;
use std::path::PathBuf;

/// The value of a `--flag value` or `--flag=value` argument.
fn argument(name: &str) -> Option<String> {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == name {
            return arguments.next();
        }
        if let Some(value) = argument.strip_prefix(&format!("{name}=")) {
            return Some(value.to_string());
        }
    }
    None
}

/// Whether a bare `--flag` argument is present.
fn has_argument(name: &str) -> bool {
    std::env::args()
        .skip(1)
        .any(|argument| argument == name || argument.starts_with(&format!("{name}=")))
}

fn main() -> Result<()> {
    // The verification harnesses read the registry from the binary instead of
    // duplicating section ids in JavaScript.
    if has_argument("--list-sections") {
        println!("{}", sections::manifest());
        return Ok(());
    }
    let endpoint = argument("--endpoint-dir").map(PathBuf::from);
    // The session file lives under the platform app-data directory, or under a
    // private directory when a harness or a second profile names one.
    let state_dir = argument("--state-dir").map(PathBuf::from);
    // `--section <id>` opens that registry entry at launch: the harness uses it
    // to reach hidden tools, and an unknown id proves the missing-section state.
    let initial = argument("--section");
    gpui_platform::application()
        .with_assets(ely_gpui_component::Assets)
        .run(move |cx: &mut App| {
            ely_gpui_component::init(cx).expect("Ely failed to start");
            ui::shell::bind_keys(cx);
            for section in sections::SECTIONS {
                (section.keys)(cx);
            }
            Theme::set_palette(
                ely_gpui_component::theme::Mode::Dark,
                Some(ui::theme::palette()),
                cx,
            );
            Theme::set_mode_now(ely_gpui_component::theme::Mode::Dark, cx);
            if let Err(error) = open_window(cx, endpoint, initial, state_dir) {
                eprintln!("idleon-optimizer: {error:#}");
                cx.quit();
            }
        });
    Ok(())
}

fn open_window(
    cx: &mut App,
    endpoint: Option<PathBuf>,
    initial: Option<String>,
    state_dir: Option<PathBuf>,
) -> Result<()> {
    let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Idleon Research Optimizer".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(16.0), px(18.0))),
        }),
        window_min_size: Some(size(px(ui::layout::MIN_WIDTH), px(ui::layout::MIN_HEIGHT))),
        ..Default::default()
    };
    cx.open_window(options, move |window, cx| {
        cx.new(move |cx| AppState::new(window, endpoint, initial, state_dir, cx))
    })
    .context("window failed to open")?;
    cx.activate(true);
    Ok(())
}
