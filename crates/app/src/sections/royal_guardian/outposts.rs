//! The Outposts tab: the world map with its outposts and resource nodes, the
//! table every outpost the world lists reports, and the selected outpost's
//! connections.
//!
//! The map follows the game's own coordinate space: an 811×433 image
//! (`royal-guardian/UImap<world>.png`) whose anchors are offset by 75 and 27
//! pixels. The connection lines are drawn rather than stamped from the game's
//! line art, because a rotated image edge would blur the 1-pixel line; the
//! colour still tells the three families apart.

use super::controls::WorldMap;
use super::overview::{number_cell, rate_cell, selected_outpost, tag, type_label};
use super::table::{Col, table, text};
use super::world_rail;
use crate::assets::{Artwork, Manifest};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Hsla, IntoElement, SharedString, Styled, canvas, div, img, point, px,
};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use idleon_royal_guardian::RoyalGuardian;
use idleon_royal_guardian::{self as royal_guardian, Endpoint};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, PANEL_GAP, Tone};

/// Resolve one game artwork reference, or `None` when the import skipped it.
fn artwork(manifest: &Manifest, reference: &str) -> Option<std::path::PathBuf> {
    match manifest.artwork(reference) {
        Artwork::Ready(path) => Some(path.to_path_buf()),
        Artwork::Missing => None,
    }
}

/// The map image's own coordinate space.
const MAP_WIDTH: f64 = 811.0;
const MAP_HEIGHT: f64 = 433.0;
/// The image's anchor offset, subtracted from every map point.
const MAP_LEFT: f64 = 75.0;
const MAP_TOP: f64 = 27.0;
/// How much of the image's pixel space the board shows.
const MAP_SCALE: f32 = 0.92;

/// The Outposts pane.
pub(crate) fn outposts(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let royal = state
        .royal
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let world = state.royal_state.world.min(royal_guardian::WORLDS - 1);
    let selected = selected_outpost(royal, world, state.royal_state.outpost);
    let rows = state.royal_controls.derived.world(royal, world);
    v_flex()
        .id("royal-outposts")
        .gap(PANEL_GAP)
        .child(world_rail(world, cx))
        .child(map(royal, rows, world, selected, &state.assets, cx))
        .child(outpost_table(royal, rows, selected, cx))
        .child(selected_outpost_card(royal, selected))
        .into_any_element()
}

/// The map board: the world image, its connection lines, its outposts and the
/// resource nodes that pay a currency.
fn map(
    royal: &RoyalGuardian,
    rows: &WorldMap,
    world: usize,
    selected: usize,
    manifest: &Manifest,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let theme = cx.theme();
    let (background, border, foreground, muted, accent) = (
        theme.background,
        theme.border,
        theme.foreground,
        theme.muted_foreground,
        theme.primary,
    );
    let (success, info, warning, radius) = (theme.success, theme.info, theme.warning, theme.radius);
    let entity = cx.entity();
    let width = (MAP_WIDTH * f64::from(MAP_SCALE)) as f32;
    let height = (MAP_HEIGHT * f64::from(MAP_SCALE)) as f32;
    let mut board = div()
        .id("royal-map")
        .role(gpui::Role::Group)
        .aria_label(format!(
            "World {} outpost map · {} outposts · {} resource nodes",
            world + 1,
            rows.outposts.len(),
            rows.nodes.len()
        ))
        .relative()
        .flex_none()
        .w(px(width))
        .h(px(height))
        .rounded(radius)
        .overflow_hidden()
        .bg(background)
        .border_1()
        .border_color(border);
    if let Some(path) = artwork(manifest, &format!("royal-guardian/UImap{}.png", world + 1)) {
        board = board.child(
            img(path)
                .absolute()
                .left_0()
                .top_0()
                .w(px(width))
                .h(px(height)),
        );
    }

    // The connection lines sit under the markers.
    let lines = rows.lines.clone();
    let palette = (success, info, warning);
    board = board.child(
        canvas(
            move |_bounds, _window, _cx| (lines, palette),
            |bounds, (lines, palette), window, _cx| {
                for line in &lines {
                    let (x0, y0) = board_point(line.from.0, line.from.1);
                    let (x1, y1) = board_point(line.to.0, line.to.1);
                    let mut path = gpui::PathBuilder::stroke(px(2.0));
                    path.move_to(point(bounds.origin.x + px(x0), bounds.origin.y + px(y0)));
                    path.line_to(point(bounds.origin.x + px(x1), bounds.origin.y + px(y1)));
                    if let Ok(path) = path.build() {
                        let color = match line.family {
                            LineFamily::Resource => palette.0,
                            LineFamily::Map => palette.1,
                            LineFamily::Savage => palette.2,
                        };
                        window.paint_path(path, color.opacity(0.75));
                    }
                }
            },
        )
        .absolute()
        .left_0()
        .top_0()
        .w(px(width))
        .h(px(height)),
    );

    for &(resource, reachable) in &rows.nodes {
        let Some((x, y)) = royal_guardian::resource_position(resource) else {
            continue;
        };
        let node = royal.resource_node(resource);
        let (left, top) = board_point(x, y);
        let ring = if node.drained {
            warning
        } else if reachable {
            success
        } else {
            border
        };
        let click = entity.clone();
        board = board.child(
            div()
                .id(SharedString::from(format!("royal-map-resource-{resource}")))
                .role(gpui::Role::Button)
                .aria_label(format!(
                    "Resource {}, grade {}, {} of {} left, currency slot {}",
                    resource + 1,
                    format::value(node.grade),
                    format::value(node.remaining),
                    format::value(node.capacity),
                    node.currency_slot
                ))
                .absolute()
                .left(px(left - 7.0))
                .top(px(top - 7.0))
                .w(px(14.0))
                .h(px(14.0))
                .rounded_full()
                .border_2()
                .border_color(ring)
                .bg(background.opacity(0.72))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(8.0))
                .text_color(foreground)
                .child(format!("{}", node.currency_slot))
                .on_click(move |_event, _window, cx| {
                    click.update(cx, |state, cx| {
                        state.set_royal_resource(resource, cx);
                        state.set_royal_tab(super::Tab::Resources, cx);
                    });
                }),
        );
    }

    for row in &rows.outposts {
        let Some((x, y)) = RoyalGuardian::outpost_position(row.map) else {
            continue;
        };
        let outpost = royal.outpost(row.map);
        let (left, top) = board_point(x, y);
        let kind = row.kind;
        let fill = if outpost.built {
            match kind as i64 {
                1 => info,
                2 => warning,
                _ => success,
            }
        } else {
            muted
        };
        let picked = row.map == selected;
        let map_idx = row.map;
        let units = row.units;
        let click = entity.clone();
        board = board.child(
            div()
                .id(SharedString::from(format!("royal-map-outpost-{map_idx}")))
                .role(gpui::Role::Button)
                .aria_label(format!(
                    "{} outpost, {}, {} units",
                    outpost.name.replace('_', " "),
                    if outpost.built {
                        type_label(kind)
                    } else {
                        outpost.reason.unwrap_or("unavailable")
                    },
                    format::value(units)
                ))
                .absolute()
                .left(px(left - 8.0))
                .top(px(top - 8.0))
                .w(px(16.0))
                .h(px(16.0))
                .rounded(px(3.0))
                .border_2()
                .border_color(if picked { accent } else { fill })
                .bg(if outpost.built {
                    fill.opacity(0.65)
                } else {
                    background.opacity(0.35)
                })
                .on_click(move |_event, _window, cx| {
                    click.update(cx, |state, cx| state.set_royal_outpost(map_idx, cx));
                }),
        );
    }

    Card::new()
        .header(
            CardHeader::new(format!("World {} map", world + 1)).description(
                "Every outpost the world page lists, the nodes it pays, and the saved connections \
             between them",
            ),
        )
        .child(
            v_flex()
                .id("royal-map-pane")
                .gap_2()
                .child(board)
                .child(map_legend(cx)),
        )
        .into_any_element()
}

/// The three connection families the map draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineFamily {
    /// A Normal outpost's resource link.
    Resource,
    /// A Support outpost's map link.
    Map,
    /// A Savage outpost's resource link.
    Savage,
}

/// One drawn connection, in the map image's coordinate space.
#[derive(Clone)]
pub(crate) struct Line {
    from: (f64, f64),
    to: (f64, f64),
    family: LineFamily,
}

/// Every connection a built outpost in this world saves.
pub(super) fn connection_lines(royal: &RoyalGuardian, world: usize) -> Vec<Line> {
    let mut lines = Vec::new();
    for outpost in royal.world_outposts(world).iter().filter(|row| row.built) {
        let Some((x, y)) = RoyalGuardian::outpost_position(outpost.map) else {
            continue;
        };
        let kind = royal.outpost_type(outpost.map);
        for connection in royal.outpost_connections(outpost.map) {
            let (to, family) = match connection.endpoint {
                Endpoint::Map(target) => {
                    let Some(position) = RoyalGuardian::outpost_position(target) else {
                        continue;
                    };
                    (position, LineFamily::Map)
                }
                Endpoint::Resource(resource) => {
                    let Some(position) = royal_guardian::resource_position(resource) else {
                        continue;
                    };
                    let family = if kind == 2.0 {
                        LineFamily::Savage
                    } else {
                        LineFamily::Resource
                    };
                    (position, family)
                }
                Endpoint::Empty => continue,
            };
            lines.push(Line {
                from: (x + 1.0, y - 1.0),
                to,
                family,
            });
        }
    }
    lines
}

/// A map-space anchor as board pixels.
fn board_point(x: f64, y: f64) -> (f32, f32) {
    (
        ((x - MAP_LEFT) * f64::from(MAP_SCALE)) as f32,
        ((y - MAP_TOP) * f64::from(MAP_SCALE)) as f32,
    )
}

/// The colours the map uses, spelled out for the harness.
fn map_legend(cx: &mut Context<AppState>) -> AnyElement {
    let theme = cx.theme();
    let (success, info, warning, radius) = (theme.success, theme.info, theme.warning, theme.radius);
    let entry = |color: Hsla, label: &'static str| {
        h_flex()
            .gap_1()
            .child(div().size(px(9.0)).rounded(radius).bg(color))
            .child(Caption::new(label))
    };
    h_flex()
        .id("royal-map-legend")
        .flex_wrap()
        .gap_3()
        .child(entry(success, "Normal connection"))
        .child(entry(info, "Support connection"))
        .child(entry(warning, "Savage connection or drained node"))
        .child(Caption::new("Click an outpost or a node to inspect it."))
        .into_any_element()
}

/// Every outpost the world page lists, built or not.
fn outpost_table(
    royal: &RoyalGuardian,
    rows: &WorldMap,
    selected: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let card = Card::new().header(
        CardHeader::new("Outposts")
            .description("Every outpost the world page lists, with its saved links and rate"),
    );
    if rows.outposts.is_empty() {
        return card
            .child(
                div()
                    .id("royal-outposts-empty")
                    .child(Caption::new("This world carries no outpost.")),
            )
            .into_any_element();
    }
    let accent = cx.theme().primary;
    let cells = rows.outposts.iter().map(|row| {
        let map = row.map;
        let outpost = royal.outpost(map);
        let pick = cx.entity();
        vec![
            v_flex()
                .id(SharedString::from(format!("royal-outpost-row-{map}")))
                .role(gpui::Role::Button)
                .cursor_pointer()
                .gap_1()
                .child(
                    div()
                        .when(map == selected, |name| name.text_color(accent))
                        .child(outpost.name.replace('_', " ")),
                )
                .child(Caption::new(match outpost.reason {
                    Some(reason) => reason.to_owned(),
                    None => format!("Map {map}"),
                }))
                .on_click(move |_event, _window, cx| {
                    pick.update(cx, |state, cx| state.set_royal_outpost(map, cx));
                })
                .into_any_element(),
            text(if outpost.built {
                type_label(row.kind)
            } else {
                "—"
            }),
            number_cell(if outpost.built { row.range } else { 0.0 }),
            number_cell(if outpost.built { row.units } else { 0.0 }),
            text(if row.links.is_empty() {
                "—".to_owned()
            } else {
                row.links.clone()
            }),
            number_cell(row.supports as f64),
            rate_cell(if outpost.built { row.rate } else { 0.0 }),
            if outpost.built {
                tag("Built", Tone::Accent).into_any_element()
            } else {
                tag(outpost.reason.unwrap_or("Unavailable"), Tone::Danger).into_any_element()
            },
        ]
    });
    let table = table(
        "royal-outpost",
        &[
            Col::Text("Outpost"),
            Col::Text("Type"),
            Col::Number("Range"),
            Col::Number("Units"),
            Col::Text("Links"),
            Col::Number("Supports"),
            Col::Number("Collection /hr"),
            Col::Text("State"),
        ],
        cells,
        cx,
    );
    card.child(div().id("royal-outpost-table").child(table))
        .into_any_element()
}

/// The selected outpost's links and what they reach.
fn selected_outpost_card(royal: &RoyalGuardian, map: usize) -> AnyElement {
    let outpost = royal.outpost(map);
    let mut body = v_flex()
        .id("royal-outpost-detail")
        .role(gpui::Role::Label)
        .gap_2();
    let title = format!(
        "{} · {}",
        outpost.name.replace('_', " "),
        if outpost.built {
            type_label(royal.outpost_type(map))
        } else {
            "not built"
        }
    );
    body = body
        .aria_label(title.clone())
        .child(div().text_size(px(15.0)).child(title));
    if !outpost.built {
        return Card::new()
            .header(CardHeader::new("Selected outpost"))
            .child(body)
            .into_any_element();
    }
    let mut links = h_flex().id("royal-outpost-links").flex_wrap().gap_2();
    for connection in royal.outpost_connections(map) {
        let label = match connection.endpoint {
            Endpoint::Empty => "Empty slot".to_owned(),
            Endpoint::Resource(resource) => {
                let reach = royal
                    .reachable_resources(map)
                    .into_iter()
                    .find(|node| node.resource == resource);
                match reach {
                    Some(node) => format!(
                        "Resource {} · {} apart",
                        resource + 1,
                        format::value(node.distance)
                    ),
                    None => format!("Resource {}", resource + 1),
                }
            }
            Endpoint::Map(target) => format!(
                "Map {} · {} apart",
                target + 1,
                format::value(RoyalGuardian::outpost_distance(map, target))
            ),
        };
        let tone = match connection.endpoint {
            Endpoint::Empty => Tone::Neutral,
            Endpoint::Resource(_) => Tone::Success,
            Endpoint::Map(_) => Tone::Accent,
        };
        links = links.child(tag(label, tone));
    }
    body = body.child(
        v_flex()
            .gap_1()
            .child(Caption::new("Connections"))
            .child(links),
    );
    let targets = royal.support_targets(map);
    if !targets.is_empty() {
        let linked: Vec<String> = targets
            .iter()
            .filter(|target| target.linked_count > 0)
            .map(|target| format!("Map {}", target.map + 1))
            .collect();
        body = body.child(Caption::new(format!(
            "Links {} of {} maps in reach",
            if linked.is_empty() {
                "none".to_owned()
            } else {
                linked.join(", ")
            },
            targets.iter().filter(|target| target.reachable).count()
        )));
    }
    let reachable = royal
        .reachable_resources(map)
        .into_iter()
        .filter(|node| node.reachable)
        .count();
    body = body.child(format!(
        "Range {} · {} of {} nodes in reach · {} support links",
        format::value(royal.range(map)),
        reachable,
        royal.reachable_resources(map).len(),
        royal.support_count(map)
    ));
    Card::new()
        .header(
            CardHeader::new("Selected outpost")
                .description("What the saved connections link, and how far the outpost reaches"),
        )
        .child(body)
        .into_any_element()
}
