//! SN-04: the Floor & Mastery workspace.
//!
//! Ported from the floor table and decision cards in `sneaking-calc.html`: the
//! active door with the saved placement's ETA, the next captive with its untie
//! ETA, and one row per floor and mastery level with the floor's mastery values
//! and the loadout's own detection, Jade, EXP, door and untie rates.
//!
//! A floor with no twin is a state, not a blank row: it says so, its detection
//! is a dash, and its door names the floor that locks it. The rows read the
//! loadout as edited, so a charm or weapon swap moves every figure it decides.

use crate::state::AppState;
use crate::ui::assets::asset_image_sized;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, div, px};
use idleon_sneaking::{ActiveDoor, FloorMastery, Job, NextCaptive};
use idleon_ui::format;

/// The Floor & Mastery pane: the two decision cards and the table.
pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(loadout) = state.sneaking.as_ref() else {
        return div().into_any_element();
    };
    let door = loadout.active_door();
    let captive = loadout.next_captive();
    let rows = loadout.floor_mastery();
    let muted = cx.theme().colors.fg_muted;
    div()
        .id("sneaking-floors")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .id("sneaking-floors-decisions")
                .flex()
                .flex_wrap()
                .gap_3()
                .child(door_card(state, door.as_ref(), cx))
                .child(captive_card(state, captive.as_ref(), cx)),
        )
        .child(table_card(&rows, cx))
        .child(
            Caption::new(
                "A projected mastery is a what-if: the game relocks the floors and clears the \
                 flower stacks, so the row resets both. The saved mastery reads the placement as \
                 it stands. Door and untie ETAs come from the twins standing on that floor, so a \
                 Nunchaku or Kunai swap moves them.",
            )
            .text_color(muted),
        )
        .into_any_element()
}

/// The one unfinished door and the saved placement's time to clear it.
fn door_card(
    state: &AppState,
    door: Option<&ActiveDoor>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let (title, eta, detail, floor) = match door {
        Some(door) => (
            format!("Floor {}", door.floor),
            if door.eta.is_some() {
                hours(door.eta)
            } else if door.damage_per_hour > 0.0 {
                "No finite ETA".to_string()
            } else {
                "No door hitter".to_string()
            },
            format!(
                "{} of {} HP left · {} damage/hr · {} hits/hr",
                hp(door.remaining_hp),
                hp(door.max_hp),
                format::value(door.damage_per_hour.max(0.0)),
                format::value(door.hits_per_hour.max(0.0))
            ),
            Some(door.floor),
        ),
        None => (
            "No active door".to_string(),
            "—".to_string(),
            "The saved mastery has no undefeated door left to hit.".to_string(),
            None,
        ),
    };
    let mut body = div().id("sneak-active-door-card").flex().flex_col().gap_2();
    body = match floor {
        Some(floor) => body.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(asset_image_sized(
                    format!("sneak-active-door-art-{floor}"),
                    &state.assets,
                    &format!("sneaking/NjR{floor}.png"),
                    px(140.0),
                    px(24.0),
                    cx,
                ))
                .child(
                    div().flex().flex_col().child(title).child(
                        Caption::new(format!("Active door · floor {floor}"))
                            .text_color(colors.fg_muted),
                    ),
                ),
        ),
        None => body.child(title),
    };
    body = body.child(
        div()
            .id("sneak-active-door")
            .role(Role::Label)
            .aria_label(eta.clone())
            .font_weight(gpui::FontWeight::MEDIUM)
            .child(eta),
    );
    Card::new()
        .header(
            CardHeader::new("Active Door")
                .description("Saved placement against the one unfinished door"),
        )
        .child(body.child(Caption::new(detail).text_color(colors.fg_muted)))
        .into_any_element()
}

/// The next captive the save holds and the untie progress on its floor.
fn captive_card(
    state: &AppState,
    captive: Option<&NextCaptive>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let (title, eta, detail, floor) = match captive {
        Some(captive) => (
            format!("Twin {} · floor {}", captive.twin + 1, captive.floor),
            if captive.eta.is_some() {
                hours(captive.eta)
            } else {
                "No Kunai on this floor".to_string()
            },
            format!(
                "{} progress left · {} progress/hr",
                format::value(captive.remaining.max(0.0)),
                format::value(captive.progress_per_hour.max(0.0))
            ),
            Some(captive.floor),
        ),
        None => (
            "All twins untied".to_string(),
            "No captive".to_string(),
            "The save holds no captive twin to free.".to_string(),
            None,
        ),
    };
    let mut body = div()
        .id("sneak-next-captive-card")
        .flex()
        .flex_col()
        .gap_2();
    body = match floor {
        Some(floor) => body.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(asset_image_sized(
                    format!("sneak-next-captive-art-{floor}"),
                    &state.assets,
                    &format!("sneaking/NjR{floor}.png"),
                    px(140.0),
                    px(24.0),
                    cx,
                ))
                .child(title),
        ),
        None => body.child(title),
    };
    body = body.child(
        div()
            .id("sneak-next-captive")
            .role(Role::Label)
            .aria_label(eta.clone())
            .font_weight(gpui::FontWeight::MEDIUM)
            .child(eta),
    );
    Card::new()
        .header(
            CardHeader::new("Next Captive")
                .description("The first captive in roster order and its untie ETA"),
        )
        .child(body.child(Caption::new(detail).text_color(colors.fg_muted)))
        .into_any_element()
}

/// One row per floor and mastery level.
fn table_card(rows: &[FloorMastery], cx: &mut Context<AppState>) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let mut table = Table::new([
        "Floor",
        "Mastery",
        "Flowers",
        "Detection",
        "Jade / hr",
        "EXP / hr",
        "Door",
        "Door ETA",
        "Captive",
        "Untie ETA",
    ]);
    for row in rows {
        let empty = row.twins.is_empty();
        let floor = div()
            .id(SharedString::from(format!(
                "sneak-floor-{}-{}",
                row.floor, row.mastery
            )))
            .flex()
            .flex_col()
            .gap_0p5()
            .child(if row.floor == 0 {
                "Training floor".to_string()
            } else {
                format!("Floor {}", row.floor)
            })
            .child(if empty {
                div()
                    .id(SharedString::from(format!(
                        "sneak-floor-empty-{}-{}",
                        row.floor, row.mastery
                    )))
                    .child(Caption::new("Empty floor").text_color(colors.warning))
                    .into_any_element()
            } else {
                Caption::new(format!(
                    "{} twin(s) · {}",
                    row.twins.len(),
                    jobs(row).join(" / ")
                ))
                .text_color(colors.fg_muted)
                .into_any_element()
            })
            .into_any_element();
        let mastery = div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(format!("M{}", row.mastery))
            .child(if row.current {
                Badge::new("current").tone(Tone::Accent).into_any_element()
            } else {
                Caption::new("projected")
                    .text_color(colors.fg_muted)
                    .into_any_element()
            })
            .into_any_element();
        let flowers = div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(if row.current {
                format::value(row.flowers)
            } else {
                "—".to_string()
            })
            .child(
                Caption::new(if row.current {
                    format!("{:.2}x funeral", 1.0 + row.funeral_percent / 100.0)
                } else {
                    "fresh reset".to_string()
                })
                .text_color(colors.fg_muted),
            )
            .into_any_element();
        let detection = div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(row.best_detection.map_or_else(
                || "—".to_string(),
                |chance| format!("{:.1}%", chance * 100.0),
            ))
            .child(
                Caption::new(format!("base {}", format::value(row.detection_base)))
                    .text_color(colors.fg_muted),
            )
            .into_any_element();
        let door_eta = div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(hours(row.door_eta.filter(|hours| *hours > 0.0)))
            .when(
                row.door_eta.is_none() && matches!(row.door_state.as_str(), "Active" | "Fresh"),
                |cell| {
                    cell.child(
                        Caption::new(if row.door_damage_per_hour > 0.0 {
                            "no finite ETA"
                        } else {
                            "no door hitter"
                        })
                        .text_color(colors.fg_muted),
                    )
                },
            )
            .into_any_element();
        let untie = div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(if row.captive.is_some() {
                if row.untie_eta.is_some() {
                    hours(row.untie_eta)
                } else {
                    "No Kunai".to_string()
                }
            } else {
                "—".to_string()
            })
            .when(row.captive.is_some() && row.untie_eta.is_none(), |cell| {
                cell.child(Caption::new("need a Kunai").text_color(colors.fg_muted))
            })
            .into_any_element();
        table = table.row([
            floor,
            mastery,
            flowers,
            detection,
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(rate(row.jade_per_hour))
                .child(
                    Caption::new(format!("base {}", format::value(row.jade_base)))
                        .text_color(colors.fg_muted),
                )
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(rate(row.exp_per_hour))
                .child(
                    Caption::new(format!("base {}", format::value(row.exp_base)))
                        .text_color(colors.fg_muted),
                )
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(row.door_state.clone())
                .child(
                    Caption::new(if row.door_damage_per_hour > 0.0 {
                        format!(
                            "{} HP · {} dmg/hr",
                            hp(row.door_hp),
                            format::value(row.door_damage_per_hour)
                        )
                    } else {
                        format!("{} HP", hp(row.door_hp))
                    })
                    .text_color(colors.fg_muted),
                )
                .into_any_element(),
            door_eta,
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    row.captive
                        .map_or_else(|| "—".to_string(), |twin| format!("Twin {}", twin + 1)),
                )
                .child(
                    Caption::new(if row.captive.is_some() {
                        format!("{} left", format::value(row.untie_remaining))
                    } else {
                        "no captive".to_string()
                    })
                    .text_color(colors.fg_muted),
                )
                .into_any_element(),
            untie,
        ]);
    }
    Card::new()
        .header(CardHeader::new("Floor × Mastery").description(
            "Every floor at the saved mastery and its neighbours, with the saved placement's \
                 detection, Jade, EXP, door and untie figures.",
        ))
        .child(
            div()
                .id("sneaking-floor-mastery")
                .flex()
                .flex_col()
                .gap_1()
                .child(table)
                .child(
                    Caption::new(format!("{} floor rows", rows.len())).text_color(colors.fg_muted),
                ),
        )
        .into_any_element()
}

/// How many twins take each action on the floor.
fn jobs(row: &FloorMastery) -> Vec<String> {
    let mut counts: Vec<(Job, usize)> = Vec::new();
    for job in &row.jobs {
        match counts.iter_mut().find(|(known, _)| known == job) {
            Some((_, count)) => *count += 1,
            None => counts.push((*job, 1)),
        }
    }
    counts
        .into_iter()
        .map(|(job, count)| format!("{count} {}", job.label()))
        .collect()
}

/// A door HP, with the mastery overflow shown as infinity rather than `inf`.
fn hp(value: f64) -> String {
    if value.is_infinite() {
        "∞".to_string()
    } else {
        format::value(value)
    }
}

/// An hourly figure, or a dash when the floor has no twin doing that action.
fn rate(value: f64) -> String {
    if value > 0.0 {
        format::value(value)
    } else {
        "—".to_string()
    }
}

/// Hours as the upstream `timeStr` shape, or a dash when there is no estimate.
fn hours(eta: Option<f64>) -> String {
    match eta {
        Some(hours) if hours > 0.0 && hours.is_finite() => format::duration(hours * 3600.0),
        _ => "—".to_string(),
    }
}
