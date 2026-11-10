//! Landing dashboard: the searchable card grid ported from upstream `index.html`.

use crate::sections::{Group, SECTIONS, Section};
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::forms::SearchInput;
use ely_gpui_component::layout::{Card, ScrollArea, SimpleGrid};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::{ActiveTheme as _, TextSize};
use gpui::prelude::*;
use gpui::{
    Context, FontWeight, IntoElement, ParentElement, Role, SharedString, Styled, Window, div, px,
};

/// The searchable dashboard, grouped General, World 2-7.
pub fn dashboard_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let query = state.search.read(cx).text().trim().to_string();
    let entity = cx.entity();
    let search_focus = state.search.read(cx).focus().clone();
    let clear = entity.clone();
    let muted = cx.theme().colors.fg_muted;

    let mut groups = div().flex().flex_col().gap_6();
    let mut matches = 0;
    for group in Group::ALL {
        let sections: Vec<&'static Section> = SECTIONS
            .iter()
            .filter(|section| section.group == group && section.matches(&query))
            .collect();
        if sections.is_empty() {
            continue;
        }
        matches += sections.len();
        let id = group.id();
        let mut grid = SimpleGrid::new(format!("dashboard-grid-{id}"), px(300.0), px(12.0));
        for section in sections {
            grid = grid.child(section_card(section, &entity, muted));
        }
        groups = groups.child(
            div()
                .id(format!("dashboard-group-{id}"))
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .id(format!("dashboard-group-title-{id}"))
                        .role(Role::Heading)
                        .aria_label(group.label())
                        .text_size(cx.theme().text_size(TextSize::Sm))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(muted)
                        .child(group.label()),
                )
                .child(grid),
        );
    }

    div()
        .id("dashboard")
        .track_focus(&state.focus)
        .on_key_down(cx.listener(|state, event: &gpui::KeyDownEvent, _window, cx| {
            if event.keystroke.key == "escape" {
                state.clear_search(cx);
                cx.stop_propagation();
            }
        }))
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_4()
        .child(
            div()
                .id("dashboard-header")
                .flex()
                .flex_none()
                .items_end()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(cx.theme().text_size(TextSize::Xxl))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Calculators & Optimizers"),
                        )
                        .child(
                            div()
                                .text_size(cx.theme().text_size(TextSize::Md))
                                .text_color(muted)
                                .child(format!(
                                    "{} calculators and optimizers for Legends of Idleon.",
                                    SECTIONS.len()
                                )),
                        ),
                )
                .child(
                    div()
                        .id("dashboard-search")
                        .role(Role::Search)
                        .aria_label("Search calculators")
                        .flex_none()
                        .w(px(360.0))
                        .on_click(move |_event, window, cx| window.focus(&search_focus, cx))
                        .child(SearchInput::new("dashboard-search-field", &state.search)),
                ),
        )
        .child(
            ScrollArea::new("dashboard-scroll")
                .vertical()
                .shadows()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_6()
                        .pb_4()
                        .when(matches == 0, |body| {
                            body.child(
                                EmptyState::new(
                                    "dashboard-empty",
                                    IconName::SearchX,
                                    "No matching calculators",
                                )
                                .body(format!(
                                    "Nothing matches “{query}”. Try a title, description or world such as W5."
                                ))
                                .action(
                                    Button::new("dashboard-clear-search", "Clear search")
                                        .on_click(move |_event, _window, cx| {
                                            clear.update(cx, |state, cx| state.clear_search(cx));
                                        }),
                                ),
                            )
                        })
                        .when(matches > 0, |body| body.child(groups)),
                ),
        )
}

/// One clickable card: icon, title, world badge and one-line description.
fn section_card(
    section: &'static Section,
    entity: &gpui::Entity<AppState>,
    muted: gpui::Hsla,
) -> impl IntoElement {
    let entity = entity.clone();
    let label = SharedString::from(format!(
        "{} · {} — {}",
        section.title,
        section.badge(),
        section.description
    ));
    div()
        .id(SharedString::from(format!("card-{}", section.id)))
        .role(Role::Button)
        .aria_label(label)
        .cursor_pointer()
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| state.open_section(section.id, cx));
        })
        .child(
            Card::new().child(
                div()
                    .flex()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .flex_none()
                            .w(px(32.0))
                            .text_size(px(24.0))
                            .line_height(px(30.0))
                            .child(section.icon),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .flex_col()
                            .gap_1p5()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_size(px(15.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(section.title),
                                    )
                                    .child(Badge::new(section.badge()).tone(tone(section.group))),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .line_height(px(17.0))
                                    .text_color(muted)
                                    .child(section.description),
                            ),
                    ),
            ),
        )
}

/// The upstream palette's nearest Ely tone for each world badge.
fn tone(group: Group) -> Tone {
    match group {
        Group::General => Tone::Neutral,
        Group::W2 | Group::W5 => Tone::Info,
        Group::W3 => Tone::Success,
        Group::W4 => Tone::Accent,
        Group::W6 => Tone::Danger,
        Group::W7 => Tone::Warning,
    }
}
