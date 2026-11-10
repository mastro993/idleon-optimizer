//! Shared section states: sections that are planned, and ids the registry lacks.
//!
//! Both render an empty state with a way back. Neither can panic: the router
//! resolves every id against the registry before the shell picks a renderer.

use crate::state::AppState;
use crate::ui::assets::asset_image;
use crate::ui::theme::WorldBadge;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::{Banner, EmptyState};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};

/// One upstream sprite per planned section that ships artwork.
///
/// The planned page shows it when `assets/upstream` is installed and a labelled
/// placeholder otherwise; sections without upstream art show no preview.
const PREVIEWS: [(&str, &str); 6] = [
    ("combat-sim", "combat-sim/player/0.png"),
    ("royal-guardian", "royal-guardian/RGres0.png"),
    ("jelly-operator", "jelly-operator/JellyUnit0.png"),
    ("sneaking", "sneaking/NjR0.png"),
    ("spelunking", "spelunking/CaveTile0.png"),
    ("worship-td", "worship-td/TowerType0.png"),
];

fn preview(section: &str) -> Option<&'static str> {
    PREVIEWS
        .iter()
        .find_map(|(id, art)| (*id == section).then_some(*art))
}

/// The no-save state every section shows until a decoded save is imported.
///
/// It names what the fallback means and offers the shared import action, so a
/// section that needs account data is never a blank page with no way forward.
pub fn no_save(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    Banner::new(
        "no-save-state",
        Severity::Info,
        "No save imported — this section uses defaults and supplements until you import a decoded save.",
    )
    .action("Import save", move |window, cx| {
        import.update(cx, |state, cx| state.open_import(window, cx));
    })
    .into_any_element()
}

/// A section the registry declares as planned: its spec exists, the module does not.
pub fn planned(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(section) = state.current_section() else {
        return div().into_any_element();
    };
    let back = cx.entity();
    let art = preview(section.id);
    div()
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_4()
        .child(
            Card::new()
                .header(
                    CardHeader::new(section.title)
                        .description(section.description)
                        .action(WorldBadge::new(section.group)),
                )
                .when_some(art, |card, reference| {
                    let muted = cx.theme().colors.fg_muted;
                    card.child(
                        div()
                            .id(format!("section-art-row-{}", section.id))
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(asset_image(
                                format!("section-art-{}", section.id),
                                &state.assets,
                                reference,
                                px(48.0),
                                cx,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(Caption::new("Upstream art"))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(muted)
                                            .child(reference),
                                    ),
                            ),
                    )
                })
                .child(
                    EmptyState::new(
                        format!("section-planned-{}", section.id),
                        IconName::Map,
                        "On the roadmap",
                    )
                    .body(format!(
                        "{} is specified in {} and has not been built yet. The research optimizer is available today.",
                        section.title, section.spec
                    ))
                    .action(
                        Button::new("section-back", "Back to dashboard")
                            .icon(IconName::ArrowLeft)
                            .on_click(move |_event, window, cx| {
                                back.update(cx, |state, cx| state.back_to_dashboard(window, cx));
                            }),
                    ),
                ),
        )
        .into_any_element()
}

/// The route's id is not in the registry: say so, name the id, and offer the way back.
pub fn missing(id: &str, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let back = entity.clone();
    div()
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_4()
        .child(
            Card::new()
                .header(CardHeader::new("Unknown section").description(id.to_string()))
                .child(
                    EmptyState::new("section-missing", IconName::SearchX, "No such section")
                        .body(format!(
                            "The registry does not declare “{id}”. Check the id, or open a calculator from the dashboard."
                        ))
                        .action(
                            Button::new("section-missing-back", "Back to dashboard")
                                .icon(IconName::ArrowLeft)
                                .on_click(move |_event, window, cx| {
                                    back.update(cx, |state, cx| {
                                        state.back_to_dashboard(window, cx);
                                    });
                                }),
                        ),
                ),
        )
        .into_any_element()
}
