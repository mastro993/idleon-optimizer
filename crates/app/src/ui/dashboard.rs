//! Landing dashboard: the searchable card grid of every section.

use crate::assets::Manifest;
use crate::sections::{self, Group, Section};
use crate::state::{AppState, Route};
use crate::ui::assets::section_icon;
use crate::ui::layout::{Breakpoint, workspace_width};
use crate::ui::section::GUTTER_X;
use crate::ui::theme::WorldBadge;
use gpui::Focusable as _;
use gpui::prelude::*;
use gpui::{
    BoxShadow, Context, FontWeight, IntoElement, ParentElement, Pixels, Role, SharedString, Styled,
    Window, div, point, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use idleon_ui::widgets::{Card, PANEL_GAP, TEXT_BODY, TEXT_CAPTION, TEXT_TITLE, search_input};

/// Space between cards, both ways: the Console gap between panels.
const GAP: Pixels = PANEL_GAP;

/// The searchable dashboard, grouped General, World 2-7.
pub(crate) fn dashboard_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let columns = columns(
        workspace_width(window, cx) - 2.0 * f32::from(GUTTER_X),
        Breakpoint::of(window, cx).card_min(),
    );
    let query = state.search.read(cx).value().trim().to_string();
    let entity = cx.entity();
    let search_focus = state.search.focus_handle(cx);
    let clear = entity.clone();
    let resume = state.last_section.and_then(sections::find);
    let (muted, subtle) = (
        cx.theme().muted_foreground,
        cx.theme().table_head_foreground,
    );

    let mut groups = v_flex().gap_4();
    let mut matches = 0;
    for group in Group::ALL {
        let cards: Vec<&'static Section> = sections::cards()
            .filter(|section| section.group == group && section.matches(&query))
            .collect();
        if cards.is_empty() {
            continue;
        }
        matches += cards.len();
        let id = group.id();
        let grid = div()
            .id(format!("dashboard-grid-{id}"))
            .grid()
            .grid_cols(columns)
            .gap(GAP)
            .children(
                cards
                    .into_iter()
                    .map(|section| section_card(section, &state.assets, &entity, cx)),
            );
        groups = groups.child(
            v_flex()
                .id(format!("dashboard-group-{id}"))
                .gap_2()
                .child(
                    div()
                        .id(format!("dashboard-group-title-{id}"))
                        .role(Role::Heading)
                        .aria_label(group.label())
                        .text_size(TEXT_CAPTION)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(subtle)
                        .child(group.label().to_uppercase()),
                )
                .child(grid),
        );
    }

    v_flex()
        .id("dashboard")
        .track_focus(&state.focus)
        .on_key_down(cx.listener(|state, event: &gpui::KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                state.clear_search(window, cx);
                cx.stop_propagation();
            }
        }))
        .size_full()
        .min_h_0()
        .gap_3()
        .child(
            h_flex()
                .id("dashboard-header")
                .flex_none()
                .items_end()
                .justify_between()
                .gap_4()
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_size(TEXT_TITLE)
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Calculators & Optimizers"),
                        )
                        .child(
                            div()
                                .text_size(TEXT_BODY)
                                .text_color(muted)
                                .child(format!(
                                    "{} calculators and optimizers for Legends of Idleon.",
                                    sections::card_count()
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
                        .child(search_input(&state.search).aria_label("Search calculators")),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(
                    v_flex()
                        .gap_4()
                        .pb_4()
                        .when_some(resume, |body, resume| {
                            let entity = entity.clone();
                            body.child(
                                h_flex().id("dashboard-resume-row").justify_end().child(
                                    Button::new("dashboard-resume")
                                        .label(format!("Resume {}", resume.title))
                                        .icon(IconName::ArrowRight)
                                        .small()
                                        .secondary()
                                        .on_click(move |_event, window, cx| {
                                            entity.update(cx, |state, cx| {
                                                state.resume_last_section(window, cx);
                                            });
                                        }),
                                ),
                            )
                        })
                        .when(matches == 0, |body| {
                            body.child(div().id("dashboard-empty").child(
                                Empty::new()
                                    .header(
                                        EmptyHeader::new()
                                            .media(EmptyMedia::new().child(
                                                Icon::new(IconName::SearchX).text_color(muted),
                                            ))
                                            .title(
                                                EmptyTitle::new().child("No matching calculators"),
                                            )
                                            .description(EmptyDescription::new().child(format!(
                                                "Nothing matches “{query}”. Try a title, description or world such as W5."
                                            ))),
                                    )
                                    .content(EmptyContent::new().child(
                                        Button::new("dashboard-clear-search")
                                            .label("Clear search")
                                            .on_click(move |_event, window, cx| {
                                                clear.update(cx, |state, cx| {
                                                    state.clear_search(window, cx)
                                                });
                                            }),
                                    )),
                            ))
                        })
                        .when(matches > 0, |body| body.child(groups)),
                )
                .overflow_y_scrollbar()
                .id("dashboard-scroll"),
        )
}

/// How many cards of at least `min` wide fit across `width`, gaps included.
fn columns(width: f32, min: Pixels) -> u16 {
    let (min, gap) = (f32::from(min), f32::from(GAP));
    ((width + gap) / (min + gap)).floor().max(1.0) as u16
}

/// A card's game sprite (`docs/ui-kit/design.md` § Density and shape).
const SPRITE: Pixels = px(16.0);

/// One clickable card: game sprite, title, world badge and one-line description.
///
/// The grid stretches every card in a row to the row's height. The pointer
/// draws a thin ring around the card and keyboard focus a wider one.
fn section_card(
    section: &'static Section,
    manifest: &Manifest,
    entity: &gpui::Entity<AppState>,
    cx: &Context<AppState>,
) -> impl IntoElement {
    let theme = cx.theme();
    let ring = |width: f32, opacity: f32| BoxShadow {
        color: theme.ring.opacity(opacity),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: px(width),
        inset: false,
    };
    let (hover_ring, focus_ring) = (ring(1.0, 0.6), ring(2.0, 1.0));
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
        .tab_index(0)
        .flex()
        .rounded(theme.radius)
        .cursor_pointer()
        .hover(move |style| style.shadow(vec![hover_ring]))
        .focus_visible(move |style| style.shadow(vec![focus_ring]))
        .on_click(move |_event, window, cx| {
            entity.update(cx, |state, cx| {
                state.open_place(Route::Section(section), window, cx);
            });
        })
        .child(
            Card::new().flex_1().min_w_0().child(
                h_flex()
                    .items_start()
                    .gap_2()
                    .child(section_icon(section, manifest, SPRITE))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1p5()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_size(TEXT_BODY)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(section.title),
                                    )
                                    .child(WorldBadge::new(section.group)),
                            )
                            .child(
                                div()
                                    .text_size(TEXT_CAPTION)
                                    .text_color(theme.muted_foreground)
                                    .child(section.description),
                            ),
                    ),
            ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_follow_the_width() {
        let min = px(300.0);
        assert_eq!(columns(0.0, min), 1);
        assert_eq!(columns(299.0, min), 1);
        assert_eq!(columns(608.0, min), 2);
        assert_eq!(columns(915.0, min), 2);
        assert_eq!(columns(916.0, min), 3);
        assert_eq!(columns(1920.0 - 228.0 - 40.0, min), 5);
    }
}
