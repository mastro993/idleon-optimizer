//! The loadout cards and the saved floor assignments.
//!
//! Ported from the loadout panel in `sneaking-calc.html`: the scenario strip, the
//! saved floor assignments (which floors the twins stand on, what each of them
//! does there) and the per-twin card with the hat, the weapon and the two charm
//! slots. Upstream also prints the jade and EXP rate of each floor; those rates
//! and their breakdown trees are SN-02, so the floor cards show what the loadout
//! itself decides: the action, the attempt time and the detection chance.

use crate::state::AppState;
use crate::ui::assets::asset_image;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Tooltip;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, Hsla, IntoElement, ParentElement, Role, SharedString, Styled,
    div, px,
};
use idleon_sneaking::{Loadout, SlotKind, Twin};
use idleon_ui::format;

/// The scenario strip: the login context, the mastery pair and the reset action.
pub(crate) fn scenario(state: &AppState, players: usize, cx: &mut Context<AppState>) -> AnyElement {
    let loadout = state
        .sneaking
        .as_ref()
        .expect("the caller checked the loadout");
    let entity = cx.entity();
    let choices: Vec<Choice> = loadout
        .twins()
        .iter()
        .take(players)
        .map(|twin| {
            Choice::new(twin.index.to_string(), twin.name.clone())
                .note(format!("Lv {} · floor {}", twin.level, twin.floor))
        })
        .collect();
    let selected = state.sneaking_state.login(players);
    let login = Select::new("sneak-login", choices)
        .label("Login context")
        .selected(selected.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(index) = value.parse::<usize>() {
                entity.update(cx, |state, cx| state.sneak_login(index, cx));
            }
        });
    let (mastery, mastery_max) = loadout.mastery();
    let reset = cx.entity();
    let changed = loadout.changed();
    let charmed = loadout.charmed();
    let colors = cx.theme().colors.clone();
    Card::new()
        .header(
            CardHeader::new("Scenario")
                .description(
                    "The loadout reads the imported save's Ninja rows. Edits stay in this window \
                     until you reset them.",
                )
                .action(
                    Badge::new(format!("Mastery {mastery} / highest {mastery_max}"))
                        .tone(Tone::Accent),
                ),
        )
        .child(
            div()
                .id("sneaking-scenario")
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(login)
                .child(chip(
                    format!("{} active twin(s)", loadout.twins().len()),
                    colors.fg,
                    colors.border,
                ))
                .child(if charmed {
                    chip("Charmed emporium perk", colors.success, colors.success)
                } else {
                    chip(
                        "No Charmed perk: no duplicate charm pair",
                        colors.warning,
                        colors.warning,
                    )
                })
                .when_some(loadout.first_batch(), |row, (name, hours)| {
                    row.child(chip(
                        format!("First ×100 batch in {} ({name})", time(hours * 3600.0)),
                        colors.fg_muted,
                        colors.border,
                    ))
                })
                .when(changed, |row| {
                    row.child(
                        div().id("sneak-reset").child(
                            ely_gpui_component::buttons::Button::new(
                                "sneak-reset-loadout",
                                "Reset loadout",
                            )
                            .on_click(move |_event, _window, cx| {
                                reset.update(cx, |state, cx| state.sneak_reset(cx));
                            }),
                        ),
                    )
                }),
        )
        .into_any_element()
}

/// The saved floor assignments: which twins stand where and what they do.
pub(crate) fn saved_floors(
    state: &AppState,
    loadout: &Loadout,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let mut floors: Vec<usize> = loadout
        .twins()
        .iter()
        .map(|twin| twin.floor)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    floors.reverse();
    let mut grid = div().id("sneaking-saved-floors").flex().flex_wrap().gap_3();
    for floor in &floors {
        let members: Vec<&Twin> = loadout
            .twins()
            .iter()
            .filter(|twin| twin.floor == *floor)
            .collect();
        grid = grid.child(floor_card(state, loadout, *floor, &members, cx));
    }
    Card::new()
        .header(CardHeader::new("Saved floor assignments").description(
            "Where the save has each twin. The Breakdowns workspace splits the Jade, EXP, door and \
             runtime formulas; this line names the action, the attempt time and the detection chance.",
        ))
        .child(grid)
        .into_any_element()
}

fn floor_card(
    state: &AppState,
    loadout: &Loadout,
    floor: usize,
    members: &[&Twin],
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let names: Vec<String> = members.iter().map(|twin| twin.name.clone()).collect();
    let mut jobs = std::collections::BTreeMap::new();
    for twin in members {
        *jobs.entry(loadout.job(twin.index)).or_insert(0usize) += 1;
    }
    let jobs: Vec<String> = jobs
        .into_iter()
        .map(|(job, count)| format!("{count} {}", job.label()))
        .collect();
    let rows: Vec<AnyElement> = members
        .iter()
        .map(|twin| {
            let detection = loadout.detection(twin.index);
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .text_size(px(11.0))
                .child(div().text_color(colors.fg_muted).child(format!(
                    "{} · {}",
                    twin.name,
                    loadout.job(twin.index).label()
                )))
                .child(
                    div()
                        .text_color(detection_color(detection, &colors))
                        .child(format!(
                            "{:.1}% det · {}",
                            detection * 100.0,
                            time(loadout.attempt_seconds(twin.index))
                        )),
                )
                .into_any_element()
        })
        .collect();
    div()
        .id(SharedString::from(format!("sneak-floor-{floor}")))
        .role(Role::Label)
        .aria_label(format!(
            "{}: {} — {}",
            if floor == 0 {
                "Training floor".to_string()
            } else {
                format!("Floor {floor}")
            },
            names.join(", "),
            jobs.join(" / ")
        ))
        .flex()
        .flex_col()
        .gap_2()
        .w(px(232.0))
        .p_3()
        .rounded(px(8.0))
        .border_1()
        .border_color(colors.border)
        .bg(colors.sunken)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(asset_image(
                    format!("sneak-floor-art-{floor}"),
                    &state.assets,
                    &format!("sneaking/NjR{floor}.png"),
                    px(28.0),
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().font_weight(FontWeight::MEDIUM).child(if floor == 0 {
                            "Training floor".to_string()
                        } else {
                            format!("Floor {floor}")
                        }))
                        .child(Caption::new(names.join(", "))),
                ),
        )
        .child(div().text_size(px(11.0)).child(jobs.join(" / ")))
        .children(rows)
        .into_any_element()
}

/// The four loadout slots of every active twin.
pub(crate) fn loadout_card(
    state: &AppState,
    loadout: &Loadout,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let target = state.sneaking_state.target(loadout.players());
    let mut cards = div().id("sneaking-loadout").flex().flex_wrap().gap_3();
    for twin in loadout.twins() {
        cards = cards.child(twin_card(state, loadout, twin, target, cx));
    }
    let colors = cx.theme().colors.clone();
    Card::new()
        .header(
            CardHeader::new("Loadout").description(
                "Pick a slot, then an inventory item, to swap it in. The stealth line is the full \
                 stack: the level base, the charm and account factors, the floor allies and the \
                 flowers. The Breakdowns workspace splits every factor.",
            ),
        )
        .child(cards)
        .child(
            div().child(Caption::new(
                "Attempt is the time between actions at the current action speed. Detection is the \
                 chance that an action costs a KO, from stealth against the floor's detection base.",
            ))
            .text_color(colors.fg_muted),
        )
        .into_any_element()
}

fn twin_card(
    state: &AppState,
    loadout: &Loadout,
    twin: &Twin,
    target: Option<(usize, usize)>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let detection = loadout.detection(twin.index);
    let mut slots = div().flex().gap_2();
    for slot in SlotKind::ALL {
        slots = slots.child(slot_button(state, twin, slot, target, cx));
    }
    let issues = loadout.twin_issues(twin.index);
    div()
        .id(SharedString::from(format!("sneak-twin-{}", twin.index)))
        .role(Role::Label)
        .aria_label(format!(
            "{}: level {}, floor {}, {:.1}% detection",
            twin.name,
            twin.level,
            twin.floor,
            detection * 100.0
        ))
        .flex()
        .flex_col()
        .gap_2()
        .w(px(280.0))
        .p_3()
        .rounded(px(8.0))
        .border_1()
        .border_color(colors.border)
        .bg(colors.sunken)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .child(twin.name.clone()),
                )
                .child(Caption::new(format!("Lv {}", twin.level)))
                .child(Caption::new(format!("Floor {}", twin.floor)))
                .when(!twin.untied, |row| {
                    row.child(Badge::new("Captive").tone(Tone::Warning))
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .text_size(px(11.0))
                .child(stat(
                    "Loadout stealth",
                    format::value(loadout.stealth(twin.index)),
                    colors.fg,
                ))
                .child(stat(
                    "Attempt",
                    time(loadout.attempt_seconds(twin.index)),
                    colors.fg_muted,
                ))
                .child(stat(
                    "KO",
                    time(loadout.ko_seconds(twin.index)),
                    colors.fg_muted,
                ))
                .child(stat(
                    "Detection",
                    format!("{:.1}%", detection * 100.0),
                    detection_color(detection, &colors),
                )),
        )
        .child(slots)
        .when_some(bonus_totals(loadout, twin.index), |card, bonuses| {
            card.child(
                div()
                    .text_size(px(10.0))
                    .text_color(colors.fg_muted)
                    .child(bonuses),
            )
        })
        .children(issues.into_iter().map(|issue| {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(Badge::new("Invalid").tone(Tone::Danger))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(colors.danger)
                        .child(issue.message()),
                )
        }))
        .into_any_element()
}

/// The charm bonus totals a twin's equipped charms give, as one line.
fn bonus_totals(loadout: &Loadout, twin: usize) -> Option<String> {
    let bonuses = loadout.bonuses(twin);
    let parts: Vec<String> = bonuses
        .nonzero()
        .map(|(bonus_type, value)| {
            format!(
                "{} {:.1}",
                idleon_sneaking::charm_bonus_label(bonus_type),
                value
            )
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn slot_button(
    state: &AppState,
    twin: &Twin,
    slot: SlotKind,
    target: Option<(usize, usize)>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let item = twin.slot(slot);
    let selected = target == Some((twin.index, slot.index()));
    let entity = cx.entity();
    let twin_index = twin.index;
    let slot_index = slot.index();
    let changed = twin.changed(slot);
    let border = if selected {
        colors.accent
    } else if changed {
        colors.warning
    } else {
        colors.border
    };
    let fallback = item.key.clone();
    let art = item
        .def()
        .map(|_| format!("sneaking/{}.png", item.key))
        .unwrap_or(fallback);
    let label = if item.is_blank() {
        slot.label().to_string()
    } else {
        format!("{} +{}", item.name(), item.level)
    };
    let tooltip = match (item.is_blank(), idleon_sneaking::equip_info(item)) {
        (false, Some(info)) => format!(
            "{} · {} +{}\n{}\n{}",
            slot.label(),
            info.name,
            item.level,
            info.label,
            info.detail
        ),
        _ => format!("{} (empty)", slot.label()),
    };
    let mut button = div()
        .id(SharedString::from(format!(
            "sneak-slot-{twin_index}-{slot_index}"
        )))
        .role(Role::Button)
        .aria_label(format!("{}: {label}", slot.label()))
        .tab_index(0)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_1()
        .w(px(62.0))
        .h(px(72.0))
        .p_1()
        .rounded(px(6.0))
        .border_1()
        .border_color(border)
        .bg(if selected {
            colors.active
        } else {
            colors.surface
        })
        .hover(|style| style.bg(colors.hover))
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.sneak_select(twin_index, slot_index, cx)
            });
        })
        .tooltip(Tooltip::text(tooltip));
    if !item.is_blank() {
        button = button.child(asset_image(
            format!("sneak-slot-art-{twin_index}-{slot_index}"),
            &state.assets,
            &art,
            px(22.0),
            cx,
        ));
    }
    button
        .child(
            div()
                .text_size(px(9.0))
                .text_color(colors.fg_muted)
                .child(slot.label()),
        )
        .child(
            div()
                .text_size(px(10.0))
                .truncate()
                .child(if item.is_blank() {
                    "Empty".to_string()
                } else {
                    format!("+{}", item.level)
                }),
        )
        .into_any_element()
}

fn chip(text: impl Into<SharedString>, fg: Hsla, border: Hsla) -> AnyElement {
    div()
        .flex()
        .items_center()
        .px_2()
        .py_1()
        .rounded(px(6.0))
        .border_1()
        .border_color(border)
        .text_size(px(11.0))
        .text_color(fg)
        .child(text.into())
        .into_any_element()
}

fn stat(name: &str, value: String, color: Hsla) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .child(div().text_color(color).child(name.to_string()))
        .child(div().font_weight(FontWeight::MEDIUM).child(value))
        .into_any_element()
}

fn detection_color(detection: f64, colors: &ely_gpui_component::theme::Palette) -> Hsla {
    if detection <= 0.05 {
        colors.success
    } else if detection <= 0.3 {
        colors.warning
    } else {
        colors.danger
    }
}

/// Seconds as `12.3s`, minutes as `4m 5s` and hours as `6.3h`.
fn time(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "—".into();
    }
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else if seconds < 3600.0 {
        format!("{:.0}m {:.0}s", (seconds / 60.0).floor(), seconds % 60.0)
    } else {
        format!("{:.1}h", seconds / 3600.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_like_the_upstream_panel() {
        assert_eq!(time(12.34), "12.3s");
        assert_eq!(time(245.0), "4m 5s");
        assert_eq!(time(22_700.0), "6.3h");
        assert_eq!(time(f64::INFINITY), "—");
    }
}
