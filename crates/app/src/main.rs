//! Desktop research-grid optimizer for Legends of Idleon.

mod state;
mod ui;

use anyhow::{Context as _, Result};
use ely_gpui_component::theme::Theme;
use gpui::{App, AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size};
use state::AppState;

fn main() -> Result<()> {
    gpui_platform::application()
        .with_assets(ely_gpui_component::Assets)
        .run(|cx: &mut App| {
            ely_gpui_component::init(cx).expect("Ely failed to start");
            ui::grid_view::bind_keys(cx);
            Theme::set_mode_now(ely_gpui_component::theme::Mode::Dark, cx);
            if let Err(error) = open_window(cx) {
                eprintln!("idleon-optimizer: {error:#}");
                cx.quit();
            }
        });
    Ok(())
}

fn open_window(cx: &mut App) -> Result<()> {
    let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Idleon Research Optimizer".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(16.0), px(18.0))),
        }),
        window_min_size: Some(size(px(1080.0), px(680.0))),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        cx.new(|cx| AppState::new(window, cx))
    })
    .context("window failed to open")?;
    cx.activate(true);
    Ok(())
}
