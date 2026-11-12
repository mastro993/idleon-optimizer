//! The trial map: platform art, the path, slots, mobs, towers and its ranges.

use crate::assets::{Artwork, Manifest};
use crate::state::AppState;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::theme::Palette;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Hsla, IntoElement, Pixels, Point, Styled, Window, canvas, div, img, point,
    px,
};
use idleon_worship::{self as worship, EffectKind, RunView, TowerView};

/// One point of a shape in scene coordinates.
type ScenePoint = (f64, f64);

/// The map pane: the whole trial scene, scaled to the window.
pub(crate) fn map(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(trial) = worship::trial(state.worship.trial) else {
        return div().into_any_element();
    };
    let scale = scale_for(window, trial);
    let width = trial.scene.w as f32 * scale;
    let height = trial.scene.h as f32 * scale;
    let view = state.worship.display_view();
    let replaying = state.worship.replaying();
    let slots = if replaying { Vec::new() } else { slots(state) };
    let selected = state.worship.selected;
    let ranges = state.worship.ranges;
    let manifest = state.assets.clone();
    let colors = cx.theme().colors.clone();
    let entity = cx.entity();

    let mut board = div()
        .id("worship-map")
        .role(gpui::Role::Group)
        .aria_label(format!(
            "{} map · {}wave {} · {} mobs · {} towers",
            trial.mob_def.name,
            if replaying { "replay · " } else { "" },
            view.wave,
            view.mobs.len(),
            view.towers.len()
        ))
        .relative()
        .flex_none()
        .w(px(width))
        .h(px(height))
        .rounded(px(10.0))
        .overflow_hidden()
        .bg(colors.bg)
        .border_1()
        .border_color(colors.border);

    // Cover the scene with the parallax layer, bottom-aligned like the game.
    if let Some(path) = artwork(&manifest, &format!("worship-td/{}bg.png", trial.background)) {
        let (bg_w, bg_h) = (1200.0, 675.0);
        let k = (trial.scene.w / bg_w).max(trial.scene.h / bg_h);
        let (draw_w, draw_h) = (bg_w * k, bg_h * k);
        let left = (trial.scene.w - draw_w) / 2.0;
        let top = trial.scene.h - draw_h;
        board = board.child(
            img(path)
                .absolute()
                .left(px(left as f32 * scale))
                .top(px(top as f32 * scale))
                .w(px(draw_w as f32 * scale))
                .h(px(draw_h as f32 * scale)),
        );
    }
    if let Some(path) = artwork(
        &manifest,
        &format!("worship-td/Plat{}.png", trial.scene.plat),
    ) {
        board = board.child(
            img(path)
                .absolute()
                .left_0()
                .top_0()
                .w(px(width))
                .h(px(height)),
        );
    }
    if let Some(path) = artwork(&manifest, "worship-td/WorshipBattleHeart.png") {
        board = board.child(
            img(path)
                .absolute()
                .left(px((trial.totem_x - 27.0) as f32 * scale))
                .top(px((trial.totem_y - 21.0) as f32 * scale))
                .w(px(55.0 * scale))
                .h(px(42.0 * scale)),
        );
    }

    board = board.child(overlay(&view, trial, scale, ranges, selected, &colors));

    for mob in &view.mobs {
        board = board.child(mob_marker(mob, trial, scale, view.tick, &manifest, &colors));
    }
    for tower in &view.towers {
        board = board.child(tower_marker(
            tower,
            &manifest,
            scale,
            selected == Some(tower.id),
            (!replaying).then(|| entity.clone()),
            &colors,
        ));
    }
    for (node, x) in slots {
        let y = worship::geometry(trial)
            .and_then(|geom| geom.nodes.get(node))
            .map(|node| node.y)
            .unwrap_or(0.0);
        let picked = entity.clone();
        board = board.child(
            div()
                .id(format!("worship-slot-{node}-{}", x as i64))
                .role(gpui::Role::Button)
                .aria_label(format!("Place tower on platform {node} at {x:.0}"))
                .absolute()
                .left(px(x as f32 * scale - 8.0))
                .top(px(y as f32 * scale - 8.0))
                .size(px(16.0))
                .rounded_full()
                .border_1()
                .border_color(colors.fg_muted.opacity(0.35))
                .bg(colors.accent.opacity(0.10))
                .cursor_pointer()
                .tooltip(ely_gpui_component::primitives::Tooltip::text(format!(
                    "Place a tower here (platform {node}, x {x:.0})"
                )))
                .on_click(move |_event, _window, cx| {
                    picked.update(cx, |state, cx| state.worship_place(node, x, cx));
                }),
        );
    }

    div()
        .id("worship-map-pane")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .items_center()
        .justify_center()
        .gap_2()
        .child(board)
        .child(
            Caption::new(format!(
                "{}×{} scene at {:.0}% · {}",
                trial.scene.w as u32,
                trial.scene.h as u32,
                scale * 100.0,
                if replaying {
                    "recorded replay, no placement".to_string()
                } else {
                    "dots are open slots".to_string()
                }
            ))
            .text_color(colors.fg_muted),
        )
        .into_any_element()
}

fn scale_for(window: &Window, trial: &worship::Trial) -> f32 {
    let viewport = window.viewport_size();
    let available_w = (f32::from(viewport.width) - 380.0).max(320.0);
    let available_h = (f32::from(viewport.height) - 430.0).max(240.0);
    (available_w / trial.scene.w as f32)
        .min(available_h / trial.scene.h as f32)
        .clamp(0.18, 1.0)
}

fn slots(state: &AppState) -> Vec<(usize, f64)> {
    let Some(run) = state.worship.run.as_ref() else {
        return Vec::new();
    };
    let trial = run.trial();
    let mut out = Vec::new();
    for spot in worship::legal_slots(trial, 62.0) {
        if run.snapped(spot.node, spot.x) == Some(spot.x) {
            out.push((spot.node, spot.x));
        }
    }
    out.truncate(120);
    out
}

fn artwork(manifest: &Manifest, reference: &str) -> Option<std::path::PathBuf> {
    match manifest.artwork(reference) {
        Artwork::Ready(path) => Some(path.to_path_buf()),
        Artwork::Missing => None,
    }
}

fn mob_marker(
    mob: &idleon_worship::MobView,
    trial: &worship::Trial,
    scale: f32,
    tick: u64,
    manifest: &Manifest,
    colors: &Palette,
) -> AnyElement {
    let w = (trial.mob_def.frame_w * mob.size * f64::from(scale)).clamp(10.0, 96.0) as f32;
    let h = (trial.mob_def.frame_h * mob.size * f64::from(scale)).clamp(10.0, 112.0) as f32;
    let ring = if mob.frozen {
        Some(colors.info)
    } else if mob.healing {
        Some(colors.success)
    } else {
        None
    };
    let sprite = &trial.sprite;
    let range = if mob.walking {
        sprite.walk
    } else {
        sprite.idle
    };
    let len = (range[1].saturating_sub(range[0]) + 1).max(1);
    // Idle and walk frames last 100 ms, like the game's animation.
    let frame = range[0] + (tick as usize / 10) % len;
    let across = sprite.across.max(1);
    let down = sprite.down.max(1);
    let column = frame % across;
    let row = frame / across;
    let sheet = format!("worship-td/mob-{}.png", trial.mob_def.key);

    let mut marker = div()
        .absolute()
        .left(px(mob.x as f32 * scale - w / 2.0))
        .top(px(mob.y as f32 * scale - h / 2.0))
        .w(px(w))
        .h(px(h))
        .overflow_hidden()
        .when_some(ring, |marker, ring| marker.border_2().border_color(ring))
        .child(
            div()
                .absolute()
                .left_0()
                .bottom_0()
                .w(px(w * mob.hp_frac.clamp(0.0, 1.0) as f32))
                .h(px(3.0))
                .bg(colors.success),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .bottom_0()
                .w(px(w))
                .h(px(3.0))
                .bg(colors.danger.opacity(0.4)),
        );
    if let Some(sheet) = artwork(manifest, &sheet) {
        marker = marker.child(
            img(sheet)
                .absolute()
                .left(px(-(column as f32) * w))
                .top(px(-(row as f32) * h))
                .w(px(across as f32 * w))
                .h(px(down as f32 * h))
                .grayscale(mob.mods == 0),
        );
    } else {
        marker = marker.child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .w(px(w))
                .h(px(h))
                .rounded_full()
                .bg(colors.warning.opacity(0.85)),
        );
    }
    if mob.mods > 0 {
        marker = marker.child(
            div()
                .absolute()
                .top_0()
                .right_0()
                .text_size(px(9.0))
                .text_color(colors.bg)
                .child(format!("{}", mob.mods)),
        );
    }
    marker.into_any_element()
}

fn tower_marker(
    tower: &TowerView,
    manifest: &Manifest,
    scale: f32,
    selected: bool,
    entity: Option<gpui::Entity<AppState>>,
    colors: &Palette,
) -> AnyElement {
    let size = 44.0 * scale * 1.15;
    let icon = artwork(
        manifest,
        &format!("worship-td/TowerType{}.png", tower.type_index),
    );
    let id = format!("worship-tower-{}-{}", tower.node, tower.x as i64);
    let tower_id = tower.id;
    let mut marker = div()
        .id(id)
        .role(gpui::Role::Button)
        .aria_label(format!(
            "{} level {} at platform {} x {:.0}",
            worship::data().towers[tower.type_index].name,
            tower.level,
            tower.node,
            tower.x
        ))
        .absolute()
        .left(px(tower.cx as f32 * scale - size / 2.0))
        .top(px(tower.cy as f32 * scale - size + 6.0))
        .w(px(size))
        .h(px(size))
        .rounded(px(6.0))
        .when(selected, |marker| {
            marker.border_2().border_color(colors.accent)
        })
        .child(
            div()
                .absolute()
                .left_0()
                .bottom_0()
                .px(px(3.0))
                .rounded(px(4.0))
                .bg(colors.bg.opacity(0.75))
                .text_size(px(10.0))
                .text_color(colors.fg)
                .child(format!("{}", tower.level)),
        );
    if let Some(entity) = entity {
        marker = marker
            .cursor_pointer()
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| state.worship_select(Some(tower_id), cx));
            });
    }
    if let Some(icon) = icon {
        marker = marker.child(img(icon).w(px(size)).h(px(size)));
    }
    marker.into_any_element()
}

/// One canvas layer for the path, ranges and projectiles.
fn overlay(
    view: &RunView,
    trial: &worship::Trial,
    scale: f32,
    ranges: bool,
    selected: Option<u64>,
    colors: &Palette,
) -> AnyElement {
    let path: Vec<(f64, f64, f64, f64)> = trial
        .path
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            let geom = worship::geometry(trial)?;
            let def = geom.nodes.get(*node)?;
            let reverse = trial.flags.get(index).copied().unwrap_or(0) == 1;
            let a = if reverse { def.length } else { def.x };
            let b = if reverse { def.x } else { def.length };
            Some((a, def.y - 4.0, b, def.y - 4.0))
        })
        .collect();
    let ranges: Vec<(f64, f64, f64)> = view
        .towers
        .iter()
        .filter(|tower| ranges || selected == Some(tower.id))
        .map(|tower| (tower.wx, tower.wy, tower.range))
        .collect();
    let selected_range = view
        .towers
        .iter()
        .find(|tower| selected == Some(tower.id))
        .map(|tower| (tower.wx, tower.wy, tower.range));
    let boulders: Vec<ScenePoint> = view.boulders.iter().map(|b| (b.x, b.y - 30.0)).collect();
    let clouds: Vec<ScenePoint> = view.clouds.iter().map(|cloud| (cloud.x, cloud.y)).collect();
    let krakens: Vec<ScenePoint> = view
        .krakens
        .iter()
        .map(|kraken| (kraken.x, kraken.y))
        .collect();
    let effects: Vec<(EffectKind, f64)> = view
        .effects
        .iter()
        .map(|effect| {
            let span = (effect.end.saturating_sub(effect.at)).max(1) as f64;
            let progress = ((view.tick.saturating_sub(effect.at)) as f64 / span).clamp(0.0, 1.0);
            (effect.kind.clone(), progress)
        })
        .collect();
    let totem = (trial.totem_x, trial.totem_y);
    let palette = (colors.warning, colors.accent, colors.danger);
    canvas(
        move |_bounds, _window, _cx| PaintData {
            scale,
            path,
            ranges,
            selected_range,
            boulders,
            clouds,
            krakens,
            effects,
            totem,
            palette,
        },
        |bounds, data, window, _cx| {
            let point_at = |x: f64, y: f64| {
                point(
                    bounds.origin.x + px(x as f32 * data.scale),
                    bounds.origin.y + px(y as f32 * data.scale),
                )
            };
            for (x0, y0, x1, y1) in &data.path {
                let mut path = gpui::PathBuilder::stroke(px(3.0)).dash_array(&[px(9.0), px(6.0)]);
                path.move_to(point_at(*x0, *y0));
                path.line_to(point_at(*x1, *y1));
                if let Ok(path) = path.build() {
                    window.paint_path(path, data.palette.0.opacity(0.65));
                }
            }
            let mut paint_range = |x: f64, y: f64, range: f64, color: Hsla| {
                let axis = range / worship::distance(1.0, 0.0);
                let diag = range / worship::distance(1.0, 1.0);
                let points: Vec<Point<Pixels>> = [
                    (axis, 0.0),
                    (diag, diag),
                    (0.0, axis),
                    (-diag, diag),
                    (-axis, 0.0),
                    (-diag, -diag),
                    (0.0, -axis),
                    (diag, -diag),
                ]
                .iter()
                .map(|(dx, dy)| point_at(x + dx, y + dy))
                .collect();
                let mut path = gpui::PathBuilder::stroke(px(2.0));
                path.add_polygon(&points, true);
                if let Ok(path) = path.build() {
                    window.paint_path(path, color.opacity(0.55));
                }
            };
            for (x, y, range) in &data.ranges {
                paint_range(*x, *y, *range, data.palette.1);
            }
            if let Some((x, y, range)) = data.selected_range {
                paint_range(x, y, range, data.palette.0);
            }
            for (x, y) in &data.boulders {
                paint_circle(
                    window,
                    point_at(*x, *y),
                    9.0 * data.scale as f64,
                    data.palette.0,
                    0.9,
                );
            }
            for (x, y) in &data.clouds {
                paint_circle(
                    window,
                    point_at(*x, *y),
                    22.0 * data.scale as f64,
                    data.palette.1,
                    0.25,
                );
            }
            for (x, y) in &data.krakens {
                paint_circle(
                    window,
                    point_at(*x, *y),
                    7.0 * data.scale as f64,
                    data.palette.0,
                    0.9,
                );
            }
            for (effect, progress) in &data.effects {
                paint_effect(window, effect, *progress, &point_at, data.palette);
            }
            // The totem's own footprint, so a leak reads against the base.
            paint_circle(
                window,
                point_at(data.totem.0, data.totem.1),
                26.0 * data.scale as f64,
                data.palette.2,
                0.18,
            );
        },
    )
    .absolute()
    .left_0()
    .top_0()
    .w(px(trial.scene.w as f32 * scale))
    .h(px(trial.scene.h as f32 * scale))
    .into_any_element()
}

struct PaintData {
    scale: f32,
    path: Vec<(f64, f64, f64, f64)>,
    ranges: Vec<(f64, f64, f64)>,
    selected_range: Option<(f64, f64, f64)>,
    boulders: Vec<ScenePoint>,
    clouds: Vec<ScenePoint>,
    krakens: Vec<ScenePoint>,
    effects: Vec<(EffectKind, f64)>,
    totem: ScenePoint,
    palette: (Hsla, Hsla, Hsla),
}

fn paint_circle(
    window: &mut Window,
    center: Point<Pixels>,
    radius: f64,
    color: Hsla,
    opacity: f32,
) {
    let radius = radius as f32;
    let points: Vec<Point<Pixels>> = (0..16)
        .map(|step| {
            let angle = std::f64::consts::TAU * f64::from(step) / 16.0;
            point(
                center.x + px(angle.cos() as f32 * radius),
                center.y + px(angle.sin() as f32 * radius),
            )
        })
        .collect();
    let mut path = gpui::PathBuilder::fill();
    path.add_polygon(&points, true);
    if let Ok(path) = path.build() {
        window.paint_path(path, color.opacity(opacity));
    }
}

fn paint_effect(
    window: &mut Window,
    effect: &EffectKind,
    progress: f64,
    point_at: &impl Fn(f64, f64) -> Point<Pixels>,
    palette: (Hsla, Hsla, Hsla),
) {
    match effect {
        EffectKind::Pulse { x0, y0, x, y, .. } => {
            let mut path = gpui::PathBuilder::stroke(px(2.5));
            path.move_to(point_at(*x0, *y0));
            path.line_to(point_at(*x, *y));
            if let Ok(path) = path.build() {
                window.paint_path(path, palette.0.opacity((1.0 - progress) as f32));
            }
            paint_circle(window, point_at(*x, *y), 5.0, palette.1, 0.8);
        }
        EffectKind::Fireball { x0, y0, x, y, .. } => {
            let arc = (std::f64::consts::PI * progress).sin() * 70.0;
            let fx = x0 + (x - x0) * progress;
            let fy = y0 + (y - 30.0 - y0) * progress - arc;
            paint_circle(window, point_at(fx, fy), 8.0, palette.2, 0.9);
        }
        EffectKind::Boom { x, y, radius, .. } => {
            paint_circle(
                window,
                point_at(*x, *y - 20.0),
                *radius,
                palette.2,
                (1.0 - progress) as f32 * 0.5,
            );
        }
        EffectKind::Storm { x, y, .. } => {
            let mut path = gpui::PathBuilder::stroke(px(3.0));
            path.move_to(point_at(*x - 6.0, *y));
            path.line_to(point_at(*x, *y + 18.0));
            path.line_to(point_at(*x + 6.0, *y + 34.0));
            if let Ok(path) = path.build() {
                window.paint_path(path, palette.0.opacity((1.0 - progress) as f32));
            }
        }
        EffectKind::Ring { x, y, radius, .. } => {
            paint_circle(
                window,
                point_at(*x, *y),
                *radius,
                palette.1,
                (1.0 - progress) as f32 * 0.45,
            );
        }
        EffectKind::Damage { x, y, crit, .. } => {
            paint_circle(
                window,
                point_at(*x, *y),
                if *crit { 5.0 } else { 3.0 },
                palette.0,
                (1.0 - progress) as f32,
            );
        }
        EffectKind::Stun { x, y } => {
            paint_circle(
                window,
                point_at(*x, *y),
                6.0,
                palette.1,
                (1.0 - progress) as f32,
            );
        }
    }
}
