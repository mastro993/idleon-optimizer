//! The loadout cards and the saved floor assignments.
//!
//! The scenario strip, the saved floor assignments (which floors the twins
//! stand on, what each of them does there) and the per-twin card with the hat,
//! the weapon and the two charm slots. The jade and EXP rate of each floor and
//! their breakdown trees are SN-02, so the floor cards show what the loadout
//! itself decides: the action, the attempt time and the detection chance.

use super::controls::Figures;
use crate::state::AppState;
use crate::ui::assets::asset_image;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, FontWeight, Hsla, IntoElement, ParentElement, Role, SharedString,
    Styled, Window, div, px,
};
use gpui_kit::component::button::Button;
use gpui_kit::component::select::Select;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use idleon_sneaking::{Loadout, SlotKind, Twin};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_CAPTION, Tone};

/// The scenario strip: the login context, the mastery pair and the reset action.
pub(crate) fn scenario(
    state: &AppState,
    loadout: &Loadout,
    figures: &Figures,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let controls = &state.sneaking_controls;
    let selected = state.sneaking_state.login(loadout.players());
    controls
        .login
        .show(figures.twin_choices.clone(), Some(&selected), window, cx);
    let login = div().flex_none().w(px(260.0)).h_6().child(
        Select::new(controls.login.state())
            .id("sneak-login")
            .accessibility_label("Login context")
            .small(),
    );
    let (mastery, mastery_max) = loadout.mastery();
    let reset = cx.entity();
    Card::new()
        .header(
            CardHeader::new("Scenario")
                .description(
                    "The loadout reads the imported save's Ninja rows. Edits stay in this window \
                     until you reset them.",
                )
                .action(tag(
                    format!("Mastery {mastery} / highest {mastery_max}"),
                    Tone::Accent,
                )),
        )
        .child(
            div()
                .id("sneaking-scenario")
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(login)
                .child(tag(
                    format!("{} active twin(s)", loadout.twins().len()),
                    Tone::Neutral,
                ))
                .child(if figures.charmed {
                    tag("Charmed emporium perk", Tone::Success)
                } else {
                    tag("No Charmed perk: no duplicate charm pair", Tone::Warning)
                })
                .when_some(figures.first_batch.clone(), |row, (name, hours)| {
                    row.child(tag(
                        format!("First ×100 batch in {} ({name})", time(hours * 3600.0)),
                        Tone::Neutral,
                    ))
                })
                .when(loadout.changed(), |row| {
                    row.child(
                        div().id("sneak-reset").child(
                            Button::new("sneak-reset-loadout")
                                .label("Reset loadout")
                                .small()
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
    figures: &Figures,
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
        grid = grid.child(floor_card(state, figures, *floor, &members, cx));
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
    figures: &Figures,
    floor: usize,
    members: &[&Twin],
    cx: &mut Context<AppState>,
) -> AnyElement {
    let theme = cx.theme();
    let (border, muted, sunken, radius) = (
        theme.border,
        theme.muted_foreground,
        theme.muted,
        theme.radius_lg,
    );
    let names: Vec<String> = members.iter().map(|twin| twin.name.clone()).collect();
    let mut jobs = std::collections::BTreeMap::new();
    for twin in members {
        *jobs.entry(figures.twins[twin.index].job).or_insert(0usize) += 1;
    }
    let jobs: Vec<String> = jobs
        .into_iter()
        .map(|(job, count)| format!("{count} {}", job.label()))
        .collect();
    let rows: Vec<AnyElement> = members
        .iter()
        .map(|twin| {
            let twin_figures = &figures.twins[twin.index];
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .text_size(TEXT_CAPTION)
                .child(div().text_color(muted).child(format!(
                    "{} · {}",
                    twin.name,
                    twin_figures.job.label()
                )))
                .child(
                    div()
                        .text_color(detection_color(twin_figures.detection, cx))
                        .child(format!(
                            "{:.1}% det · {}",
                            twin_figures.detection * 100.0,
                            time(twin_figures.attempt)
                        )),
                )
                .into_any_element()
        })
        .collect();
    let title = if floor == 0 {
        "Training floor".to_string()
    } else {
        format!("Floor {floor}")
    };
    div()
        .id(SharedString::from(format!("sneak-floor-{floor}")))
        .role(Role::Label)
        .aria_label(format!(
            "{title}: {} — {}",
            names.join(", "),
            jobs.join(" / ")
        ))
        .flex()
        .flex_col()
        .gap_2()
        .w(px(232.0))
        .p_3()
        .rounded(radius)
        .border_1()
        .border_color(border)
        .bg(sunken)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().flex_none().child(asset_image(
                    format!("sneak-floor-art-{floor}"),
                    &state.assets,
                    &format!("sneaking/NjR{floor}.png"),
                    px(28.0),
                    cx,
                )))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(div().font_weight(FontWeight::MEDIUM).child(title))
                        .child(Caption::new(names.join(", "))),
                ),
        )
        .child(div().text_size(TEXT_CAPTION).child(jobs.join(" / ")))
        .children(rows)
        .into_any_element()
}

/// The four loadout slots of every active twin.
pub(crate) fn loadout_card(
    state: &AppState,
    loadout: &Loadout,
    figures: &Figures,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let target = state.sneaking_state.target(loadout.players());
    let mut cards = div().id("sneaking-loadout").flex().flex_wrap().gap_3();
    for twin in loadout.twins() {
        cards = cards.child(twin_card(state, figures, twin, target, cx));
    }
    Card::new()
        .header(CardHeader::new("Loadout").description(
            "Pick a slot, then an inventory item, to swap it in. The stealth line is the full \
                 stack: the level base, the charm and account factors, the floor allies and the \
                 flowers. The Breakdowns workspace splits every factor.",
        ))
        .child(cards)
        .child(Caption::new(
            "Attempt is the time between actions at the current action speed. Detection is the \
             chance that an action costs a KO, from stealth against the floor's detection base.",
        ))
        .into_any_element()
}

fn twin_card(
    state: &AppState,
    figures: &Figures,
    twin: &Twin,
    target: Option<(usize, usize)>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let theme = cx.theme();
    let (border, sunken, radius, fg, muted, danger) = (
        theme.border,
        theme.muted,
        theme.radius_lg,
        theme.foreground,
        theme.muted_foreground,
        theme.danger,
    );
    let twin_figures = &figures.twins[twin.index];
    let detection = twin_figures.detection;
    let mut slots = div().flex().gap_2();
    for slot in SlotKind::ALL {
        slots = slots.child(slot_button(state, twin, slot, target, cx));
    }
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
        .rounded(radius)
        .border_1()
        .border_color(border)
        .bg(sunken)
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
                .when(!twin.untied, |row| row.child(tag("Captive", Tone::Warning))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .text_size(TEXT_CAPTION)
                .child(stat(
                    "Loadout stealth",
                    format::value(twin_figures.stealth),
                    fg,
                ))
                .child(stat("Attempt", time(twin_figures.attempt), muted))
                .child(stat("KO", time(twin_figures.ko), muted))
                .child(stat(
                    "Detection",
                    format!("{:.1}%", detection * 100.0),
                    detection_color(detection, cx),
                )),
        )
        .child(slots)
        .when_some(twin_figures.bonuses.clone(), |card, bonuses| {
            card.child(Caption::new(bonuses))
        })
        .children(twin_figures.issues.iter().map(|issue| {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(tag("Invalid", Tone::Danger))
                .child(
                    div()
                        .text_size(TEXT_CAPTION)
                        .text_color(danger)
                        .child(issue.clone()),
                )
        }))
        .into_any_element()
}

fn slot_button(
    state: &AppState,
    twin: &Twin,
    slot: SlotKind,
    target: Option<(usize, usize)>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let theme = cx.theme();
    let item = twin.slot(slot);
    let selected = target == Some((twin.index, slot.index()));
    let entity = cx.entity();
    let twin_index = twin.index;
    let slot_index = slot.index();
    let border = if selected {
        theme.primary
    } else if twin.changed(slot) {
        theme.warning
    } else {
        theme.border
    };
    let background = if selected {
        theme.secondary_active
    } else {
        theme.secondary
    };
    let (hover, muted, radius) = (theme.secondary_hover, theme.muted_foreground, theme.radius);
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
    let tooltip = SharedString::from(match (item.is_blank(), idleon_sneaking::equip_info(item)) {
        (false, Some(info)) => format!(
            "{} · {} +{}\n{}\n{}",
            slot.label(),
            info.name,
            item.level,
            info.label,
            info.detail
        ),
        _ => format!("{} (empty)", slot.label()),
    });
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
        .rounded(radius)
        .border_1()
        .border_color(border)
        .bg(background)
        .hover(move |style| style.bg(hover))
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.sneak_select(twin_index, slot_index, cx)
            });
        })
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx));
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
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child(slot.label()),
        )
        .child(
            div()
                .text_size(TEXT_CAPTION)
                .truncate()
                .child(if item.is_blank() {
                    "Empty".to_string()
                } else {
                    format!("+{}", item.level)
                }),
        )
        .into_any_element()
}

/// A small outlined label in `tone`.
pub(super) fn tag(text: impl Into<SharedString>, tone: Tone) -> Tag {
    Tag::new()
        .with_variant(tone.into())
        .outline()
        .small()
        .child(text.into())
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

fn detection_color(detection: f64, cx: &App) -> Hsla {
    let tone = if detection <= 0.05 {
        Tone::Success
    } else if detection <= 0.3 {
        Tone::Warning
    } else {
        Tone::Danger
    };
    tone.color(cx)
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
    fn times_read_in_seconds_minutes_or_hours() {
        assert_eq!(time(12.34), "12.3s");
        assert_eq!(time(245.0), "4m 5s");
        assert_eq!(time(22_700.0), "6.3h");
        assert_eq!(time(f64::INFINITY), "—");
    }
}
