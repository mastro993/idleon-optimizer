//! The Outposts tab: the world map with its outposts and resource nodes, the
//! table every outpost the world lists reports, and the selected outpost's
//! connections.
//!
//! The map follows the upstream page's own coordinate space: an 811×433 image
//! (`royal-guardian/UImap<world>.png`) whose anchors are offset by 75 and 27
//! pixels. The connection lines are drawn rather than stamped from the game's
//! line art, because a rotated image edge would blur the 1-pixel upstream line;
//! the colour still tells the three families apart.

use super::overview::{number_cell, rate_cell, selected_outpost, type_label};
use super::world_rail;
use crate::assets::{Artwork, Manifest};
use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Hsla, IntoElement, Styled, canvas, div, img, point, px};
use idleon_royal_guardian::RoyalGuardian;
use idleon_royal_guardian::{self as royal_guardian, Endpoint};
use idleon_ui::format;

/// Resolve one upstream artwork reference, or `None` when the import skipped it.
fn artwork(manifest: &Manifest, reference: &str) -> Option<std::path::PathBuf> {
    match manifest.artwork(reference) {
        Artwork::Ready(path) => Some(path.to_path_buf()),
        Artwork::Missing => None,
    }
}

/// The map image's own coordinate space, as the upstream page draws it.
const MAP_WIDTH: f64 = 811.0;
const MAP_HEIGHT: f64 = 433.0;
/// The image's anchor offset: upstream `mapPoint` subtracts these.
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
    div()
        .id("royal-outposts")
        .flex()
        .flex_col()
        .gap_3()
        .child(world_rail(world, cx))
        .child(map(royal, world, selected, &state.assets, cx))
        .child(outpost_table(royal, world, selected, cx))
        .child(selected_outpost_card(royal, selected))
        .into_any_element()
}

/// The map board: the world image, its connection lines, its outposts and the
/// resource nodes that pay a currency.
fn map(
    royal: &RoyalGuardian,
    world: usize,
    selected: usize,
    manifest: &Manifest,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let entity = cx.entity();
    let width = (MAP_WIDTH * f64::from(MAP_SCALE)) as f32;
    let height = (MAP_HEIGHT * f64::from(MAP_SCALE)) as f32;
    let mut board = div()
        .id("royal-map")
        .role(gpui::Role::Group)
        .aria_label(format!(
            "World {} outpost map · {} outposts · {} resource nodes",
            world + 1,
            royal.world_outposts(world).len(),
            royal
                .world_resources(world)
                .iter()
                .filter(|node| node.currency_slot >= 0)
                .count()
        ))
        .relative()
        .flex_none()
        .w(px(width))
        .h(px(height))
        .rounded(px(8.0))
        .overflow_hidden()
        .bg(colors.bg)
        .border_1()
        .border_color(colors.border);
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

    // The connection lines sit under the markers, like the upstream layers.
    let lines = connection_lines(royal, world);
    let palette = (colors.success, colors.info, colors.warning);
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

    for node in royal
        .world_resources(world)
        .into_iter()
        .filter(|node| node.currency_slot >= 0)
    {
        let Some((x, y)) = royal_guardian::resource_position(node.resource) else {
            continue;
        };
        let (left, top) = board_point(x, y);
        let reachable = royal
            .reachable_outposts(node.resource)
            .iter()
            .any(|outpost| outpost.reachable);
        let ring = if node.drained {
            colors.warning
        } else if reachable {
            colors.success
        } else {
            colors.border
        };
        let resource = node.resource;
        let click = entity.clone();
        board = board.child(
            div()
                .id(format!("royal-map-resource-{resource}"))
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
                .bg(colors.bg.opacity(0.72))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(8.0))
                .text_color(colors.fg)
                .child(format!("{}", node.currency_slot))
                .on_click(move |_event, _window, cx| {
                    click.update(cx, |state, cx| {
                        state.set_royal_resource(resource, cx);
                        state.set_royal_tab(super::Tab::Resources, cx);
                    });
                }),
        );
    }

    for outpost in royal.world_outposts(world) {
        let Some((x, y)) = RoyalGuardian::outpost_position(outpost.map) else {
            continue;
        };
        let (left, top) = board_point(x, y);
        let kind = royal.outpost_type(outpost.map);
        let fill = if outpost.built {
            match kind as i64 {
                1 => colors.info,
                2 => colors.warning,
                _ => colors.success,
            }
        } else {
            colors.fg_muted
        };
        let picked = outpost.map == selected;
        let map_idx = outpost.map;
        let units = royal.total_units(map_idx);
        let click = entity.clone();
        board = board.child(
            div()
                .id(format!("royal-map-outpost-{map_idx}"))
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
                .border_color(if picked { colors.accent } else { fill })
                .bg(if outpost.built {
                    fill.opacity(0.65)
                } else {
                    colors.bg.opacity(0.35)
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
            div()
                .id("royal-map-pane")
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
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
struct Line {
    from: (f64, f64),
    to: (f64, f64),
    family: LineFamily,
}

/// Every connection a built outpost in this world saves.
fn connection_lines(royal: &RoyalGuardian, world: usize) -> Vec<Line> {
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
    let muted = cx.theme().colors.fg_muted;
    let colors = cx.theme().colors.clone();
    let swatch = |color: Hsla| div().w(px(9.0)).h(px(9.0)).rounded(px(2.0)).bg(color);
    div()
        .id("royal-map-legend")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(swatch(colors.success))
                .child(Caption::new("Normal connection")),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(swatch(colors.info))
                .child(Caption::new("Support connection")),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(swatch(colors.warning))
                .child(Caption::new("Savage connection or drained node")),
        )
        .child(Caption::new("Click an outpost or a node to inspect it.").text_color(muted))
        .into_any_element()
}

/// Every outpost the world page lists, built or not.
fn outpost_table(
    royal: &RoyalGuardian,
    world: usize,
    selected: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let rows = royal.world_outposts(world);
    let mut table = Table::new([
        "Outpost",
        "Type",
        "Range",
        "Units",
        "Links",
        "Supports",
        "Collection /hr",
        "State",
    ]);
    for outpost in &rows {
        let map = outpost.map;
        let pick = cx.entity();
        let picked = map == selected;
        let kind = royal.outpost_type(map);
        let rate = royal.outpost_collection_rate(map);
        let connections = royal.outpost_connections(map);
        let links = connections
            .iter()
            .filter(|connection| connection.endpoint != Endpoint::Empty)
            .map(|connection| match connection.endpoint {
                Endpoint::Resource(resource) => format!("R{}", resource + 1),
                Endpoint::Map(target) => format!("W{}", target + 1),
                Endpoint::Empty => String::new(),
            })
            .collect::<Vec<_>>()
            .join(" + ");
        table = table.row([
            div()
                .id(format!("royal-outpost-row-{map}"))
                .role(gpui::Role::Button)
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .when(picked, |name| name.text_color(cx.theme().colors.accent))
                        .child(outpost.name.replace('_', " ")),
                )
                .child(
                    Caption::new(match outpost.reason {
                        Some(reason) => reason.to_owned(),
                        None => format!("Map {map}"),
                    })
                    .text_color(muted),
                )
                .on_click(move |_event, _window, cx| {
                    pick.update(cx, |state, cx| state.set_royal_outpost(map, cx));
                })
                .into_any_element(),
            div()
                .child(if outpost.built {
                    type_label(kind)
                } else {
                    "—"
                })
                .into_any_element(),
            number_cell(if outpost.built { royal.range(map) } else { 0.0 }),
            number_cell(if outpost.built {
                royal.total_units(map)
            } else {
                0.0
            }),
            div()
                .child(if links.is_empty() {
                    "—".to_owned()
                } else {
                    links
                })
                .into_any_element(),
            number_cell(royal.support_count(map) as f64),
            rate_cell(if outpost.built { rate.value } else { 0.0 }),
            div()
                .child(if outpost.built {
                    Badge::new("Built").tone(Tone::Accent)
                } else {
                    Badge::new(outpost.reason.unwrap_or("Unavailable")).tone(Tone::Danger)
                })
                .into_any_element(),
        ]);
    }
    let card = Card::new().header(
        CardHeader::new("Outposts")
            .description("Every outpost the world page lists, with its saved links and rate"),
    );
    if rows.is_empty() {
        return card
            .child(
                div()
                    .id("royal-outposts-empty")
                    .child(Caption::new("This world carries no outpost.").text_color(muted)),
            )
            .into_any_element();
    }
    card.child(div().id("royal-outpost-table").child(table))
        .into_any_element()
}

/// The selected outpost's links and what they reach.
fn selected_outpost_card(royal: &RoyalGuardian, map: usize) -> AnyElement {
    let outpost = royal.outpost(map);
    let mut body = div()
        .id("royal-outpost-detail")
        .role(gpui::Role::Label)
        .flex()
        .flex_col()
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
    let mut links = div().id("royal-outpost-links").flex().flex_wrap().gap_2();
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
        links = links.child(Badge::new(label).tone(tone));
    }
    body = body.child(
        div()
            .flex()
            .flex_col()
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
