//! The character, toolkit and claim-length toolbar, with the saved-default reset.

use super::Page;
use super::controls::cached;
use crate::state::AppState;
use crate::ui::number_field;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, Role, Styled, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_ui::formula::FormulaTree;
use idleon_ui::widgets::{Caption, Card, CardHeader, Choice, ChoiceSelect, TEXT_MONO, show_number};

/// The character, toolkit, location and claim length, with the account card's
/// values applied to the selected toolkit.
pub(crate) fn pane(
    state: &mut AppState,
    page: &Page,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let revision = state.save_revision;
    let characters = cached(
        state,
        |controls| &mut controls.characters,
        revision,
        cx,
        |state, _| character_choices(state),
    );
    let config = &state.fishing;
    let controls = &state.fishing_controls;
    controls
        .character
        .show((*characters).clone(), Some(&config.character), window, cx);
    controls
        .lure
        .show(controls.lures.clone(), Some(&config.lure), window, cx);
    controls
        .line
        .show(controls.lines.clone(), Some(&config.line), window, cx);
    controls.location.show(
        controls.locations.clone(),
        Some(&config.location),
        window,
        cx,
    );
    show_number(&controls.hours, config.hours, 2, window, cx);
    let muted = cx.theme().muted_foreground;
    let rod = page.rod.as_ref().map(|(key, name, line)| {
        let icon = (key != "Blank").then(|| format!("combat-sim/ui/items/{key}.png"));
        v_flex()
            .id("fishing-rod")
            .role(Role::Label)
            .aria_label(name.clone())
            .gap_1()
            .child(Caption::new("Rod").text_color(muted))
            .child(
                h_flex()
                    .gap_2()
                    .when_some(icon, |cell, reference| {
                        cell.child(crate::ui::assets::asset_image(
                            "fishing-rod-icon",
                            &state.assets,
                            &reference,
                            px(28.0),
                            cx,
                        ))
                    })
                    .child(div().text_size(TEXT_MONO).child(line.clone())),
            )
    });

    Card::new()
        .header(CardHeader::new("Toolbar and character context").description(
            "The saved toolkit, spot and rod define the defaults; the claim length is hours away.",
        ))
        .child(
            v_flex()
                .id("fishing-toolbar")
                .gap_3()
                .child(
                    h_flex()
                        .items_end()
                        .gap_4()
                        .child(field(
                            "Character",
                            select(&controls.character, "fishing-character", "Character", 320.0),
                        ))
                        .child(field(
                            "Lure",
                            select(&controls.lure, "fishing-lure", "Lure", 260.0),
                        ))
                        .child(field(
                            "Line",
                            select(&controls.line, "fishing-line", "Line", 260.0),
                        )),
                )
                .child(
                    h_flex()
                        .items_end()
                        .gap_4()
                        .child(field(
                            "Fishing location",
                            select(
                                &controls.location,
                                "fishing-location",
                                "Fishing location",
                                420.0,
                            ),
                        ))
                        .child(field(
                            "Hours away",
                            number_field("fishing-hours", &controls.hours, "Hours away"),
                        ))
                        .children(rod),
                ),
        )
        .into_any_element()
}

/// The save's characters, read once per save.
fn character_choices(state: &AppState) -> Vec<Choice<usize>> {
    let save = &state.fishing_save;
    if save.characters.is_empty() {
        return vec![Choice::new(0, "Character 1 · no save")];
    }
    save.characters
        .iter()
        .map(|character| {
            Choice::new(
                character.index,
                format!(
                    "{} · Lv {} · {}{}",
                    character.name,
                    character.level,
                    character.rod_name,
                    character
                        .saved_location()
                        .map(|location| format!(" · {}", location.label))
                        .unwrap_or_default()
                ),
            )
        })
        .collect()
}

/// A small select boxed to `width`, so a row of them keeps its order.
fn select<V: Clone + PartialEq + 'static>(
    select: &ChoiceSelect<V>,
    id: &'static str,
    label: &'static str,
    width: f32,
) -> impl IntoElement {
    div().w(px(width)).child(
        Select::new(select.state())
            .id(id)
            .accessibility_label(label)
            .small(),
    )
}

fn field(label: &'static str, control: impl IntoElement) -> AnyElement {
    v_flex()
        .flex_none()
        .gap_1()
        .child(Caption::new(label))
        .child(control)
        .into_any_element()
}

/// The shared formula tree under a fixed height, so its own scroll area works.
pub(super) fn formula_card(tree: &Entity<FormulaTree>) -> AnyElement {
    Card::new()
        .header(
            CardHeader::new("Breakdown")
                .description("Catch time, split, fish, real EXP and the panel comparison."),
        )
        .child(
            div()
                .id("fishing-breakdown")
                .h(px(420.0))
                .min_h_0()
                .child(tree.clone()),
        )
        .into_any_element()
}

/// The "Reset to saved" action.
pub(crate) fn reset(cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    Button::new("fishing-reset")
        .label("Reset to saved")
        .small()
        .icon(IconName::RefreshCw)
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| state.fishing_reset(cx));
        })
        .into_any_element()
}
