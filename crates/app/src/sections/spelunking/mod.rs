//! The Spelunking Simulator: expedition controls, party, elixir belt, depth
//! estimates and the expedition run.
//!
//! SP-01 covers the controls, the party and elixir belt, and the Depth limits
//! tab; SP-02 adds the seeded run with its entry, tool, reaction time and event
//! log; SP-03 adds the phased Amber policy with its dump, finisher, reserve,
//! harvest and rope reserve limits, the run result's phase record, and the
//! depth optimizer (SP-04).
//!
//! The four account multipliers that come from the account's whole stat context
//! are user inputs here (see [`idleon_spelunking::Account`]); every other
//! control and every depth estimate comes from the imported save.

mod actions;
mod belt;
mod chart;
mod controls;
mod depth;
mod events;
mod expedition;
mod optimizer;
mod party;
mod run;
pub(crate) mod state;

pub(crate) use controls::Controls;
pub(crate) use optimizer::OptimizerInputs;
pub(crate) use run::RunInputs;
pub(crate) use state::{OptimizerMode, State};

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use controls::Slots;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Div, ElementId, Entity, IntoElement, ParentElement, SharedString,
    Styled, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::Button;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::input::InputState;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::Select;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use idleon_ui::widgets::{Choice, ChoiceSelect, Tone, labeled, number_width, show_number};

/// The section: the controls, the party and belt, the account inputs and the
/// depth estimates in one scrolling page.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(save) = state.spelunking_save.as_ref() else {
        return empty(cx);
    };
    let derived = state
        .spelunking_controls
        .derived(state.save_revision, &state.spelunking, save);
    let controls = &mut state.spelunking_controls;
    controls.ensure_slots(Slots::Party, derived.party.len(), window, cx);
    controls.ensure_slots(Slots::Belt, derived.elixirs.len(), window, cx);
    controls.ensure_slots(Slots::Memory, derived.memorized.len(), window, cx);
    controls.ensure_max_stamina(derived.inputs.characters.len(), window, cx);
    let state: &AppState = state;
    let breakpoint = Breakpoint::of(window, cx);
    let page = div()
        .id("spelunking-page")
        .flex()
        .flex_col()
        .gap_3()
        .child(expedition::card(state, &derived, window, cx))
        .when_some(derived.config_error.as_ref(), |page, error| {
            page.child(
                Alert::new(
                    "spelunk-error",
                    format!("This expedition cannot run: {error}"),
                )
                .with_variant(Tone::Danger.into())
                .banner(),
            )
        })
        .children(expedition::amber_card(state, window, cx))
        .child(layout::split_pane(
            "spelunking-prep",
            breakpoint,
            party::card(state, &derived, window, cx),
            belt::card(state, &derived, window, cx),
            px(440.0),
        ))
        .child(expedition::account_card(state, window, cx))
        .child(run::card(state, &derived, window, cx))
        .child(optimizer::card(state, &derived, window, cx))
        .child(depth::card(state, &derived, window, cx));
    div()
        .id("spelunking-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(div().w_full().child(page))
                .overflow_y_scrollbar()
                .id("spelunking-pane"),
        )
        .into_any_element()
}

/// A labelled control in the toolbars.
pub(crate) fn field(label: &'static str, control: impl IntoElement) -> AnyElement {
    labeled(label, control).into_any_element()
}

/// A kit select showing `items` with `selected`, under the old control's id;
/// [`boxed`] gives it a place in a row.
pub(crate) fn select<V: Clone + PartialEq + 'static>(
    control: &ChoiceSelect<V>,
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    items: &[Choice<V>],
    selected: &V,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> Select<Vec<Choice<V>>> {
    control.show(items.to_vec(), Some(selected), window, cx);
    Select::new(control.state())
        .id(ElementId::Name(id.into()))
        .accessibility_label(label)
        .small()
}

/// A kit select fills its parent, so in a row it sits in a box of its own, as
/// wide as `control`'s longest option and never wider than the row.
pub(crate) fn boxed<V: Clone + PartialEq + 'static>(
    control: &ChoiceSelect<V>,
    select: impl IntoElement,
    window: &mut Window,
    cx: &App,
) -> Div {
    div()
        .flex_none()
        .max_w_full()
        .w(control.width(window, cx))
        .h_6()
        .child(select)
}

/// A kit number field showing `value` to `precision` decimals, under the old
/// control's id.
pub(crate) fn number(
    id: impl Into<SharedString>,
    field: &Entity<InputState>,
    label: impl Into<SharedString>,
    value: f64,
    precision: usize,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    show_number(field, value, precision, window, cx);
    crate::ui::number_field(ElementId::Name(id.into()), field, label)
        .w(number_width(field, window, cx))
        .into_any_element()
}

/// A small outlined label in `tone`.
pub(crate) fn tag(text: impl Into<SharedString>, tone: Tone) -> Tag {
    Tag::new()
        .with_variant(tone.into())
        .outline()
        .small()
        .child(text.into())
}

/// A save without the Spelunking rows has no expedition state.
fn empty(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    let muted = cx.theme().muted_foreground;
    div()
        .id("spelunking-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            div().id("spelunking-empty").child(
                Empty::new()
                    .header(
                        EmptyHeader::new()
                            .media(
                                EmptyMedia::new().child(Icon::new(IconName::Map).text_color(muted)),
                            )
                            .title(EmptyTitle::new().child("No Spelunking state"))
                            .description(EmptyDescription::new().child(
                                "The simulator reads the tunnel records, saved stamina, shop \
                                 upgrades and elixir belt from a save. Import one to choose a \
                                 party and estimate depths.",
                            )),
                    )
                    .content(
                        EmptyContent::new().child(
                            Button::new("spelunking-import")
                                .label("Import save")
                                .icon(IconName::Upload)
                                .on_click(move |_event, window, cx| {
                                    import.update(cx, |state, cx| state.open_import(window, cx));
                                }),
                        ),
                    ),
            ),
        )
        .into_any_element()
}
