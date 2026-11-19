//! The account card: the skill-system values the account's skills supply.
//!
//! Until the stats dashboard decodes efficiency, EXP, AFK, Multi-Fish and the
//! speed sources, the section takes them as explicit values. The card says so,
//! so a neutral default is never mistaken for a calculation.

use super::controls::ACCOUNT_FIELDS;
use crate::state::AppState;
use crate::ui::number_field;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window};
use gpui_kit::component::alert::Alert;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone, show_number};

/// The account inputs card.
pub(crate) fn card(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let account = state.fishing.account;
    let muted = cx.theme().muted_foreground;
    let fields = &state.fishing_controls.account;
    let mut cells = Vec::with_capacity(ACCOUNT_FIELDS.len());
    for (field, input) in ACCOUNT_FIELDS.iter().zip(fields) {
        show_number(input, (field.get)(&account), field.precision, window, cx);
        cells.push(
            v_flex()
                .flex_none()
                .gap_1()
                .child(Caption::new(field.label).text_color(muted))
                .child(number_field(field.id, input, field.label))
                .into_any_element(),
        );
    }
    let second = cells.split_off(5);

    Card::new()
        .header(CardHeader::new("Account bonuses").description(
            "Entered from the game's Skills and AFK panels until the save's skill systems are decoded.",
        ))
        .child(
            v_flex()
                .id("fishing-account-inputs")
                .gap_3()
                .child(
                    Alert::new(
                        "fishing-account-notice",
                        "These values are manual: efficiency, EXP multiplier, AFK rate, Multi-Fish, speed sources, prowess and the equipment depths are not decoded yet. Neutral defaults give a lower bound.",
                    )
                    .with_variant(Tone::Info.into())
                    .banner(),
                )
                .child(h_flex().items_end().gap_4().children(cells))
                .child(h_flex().items_end().gap_4().children(second))
                .child(
                    Caption::new(
                        "The selected lure and line add their own SPEED to the speed bonus and their D0–D3 to the depths.",
                    )
                    .text_color(muted),
                ),
        )
        .into_any_element()
}
