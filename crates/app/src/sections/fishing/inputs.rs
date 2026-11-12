//! The account card: the skill-system values the upstream evaluator derives.
//!
//! Until the stats dashboard decodes efficiency, EXP, AFK, Multi-Fish and the
//! speed sources, the section takes them as explicit values. The card says so,
//! so a neutral default is never mistaken for a calculation.

use crate::state::AppState;
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::NumberInput;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_fishing::AccountInputs;

/// The account inputs card.
pub(crate) fn card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let account = state.fishing.account;
    let muted = cx.theme().colors.fg_muted;
    let efficiency = number(
        "fishing-input-efficiency",
        "Fishing efficiency",
        account.efficiency,
        0.0,
        1e12,
        1000.0,
        0,
        cx,
        |account, value| account.efficiency = value,
    );
    let exp = number(
        "fishing-input-exp",
        "EXP multiplier",
        account.exp_multi,
        0.0,
        1e6,
        0.05,
        2,
        cx,
        |account, value| account.exp_multi = value,
    );
    let afk = number(
        "fishing-input-afk",
        "AFK gains rate %",
        account.afk_rate * 100.0,
        0.0,
        100_000.0,
        1.0,
        2,
        cx,
        |account, value| account.afk_rate = value / 100.0,
    );
    let multi = number(
        "fishing-input-multi",
        "Multi-Fish %",
        account.multi_fish,
        0.0,
        10_000.0,
        1.0,
        2,
        cx,
        |account, value| account.multi_fish = value,
    );
    let speed = number(
        "fishing-input-speed",
        "Speed bonus %",
        account.speed_bonus,
        0.0,
        100_000.0,
        1.0,
        2,
        cx,
        |account, value| account.speed_bonus = value,
    );
    let attack = number(
        "fishing-input-attack",
        "Attack loadout factor",
        account.attack_factor,
        1.0,
        3.0,
        0.05,
        2,
        cx,
        |account, value| account.attack_factor = value,
    );
    let prowess = number(
        "fishing-input-prowess",
        "Prowess",
        account.prowess,
        0.0,
        100.0,
        0.01,
        4,
        cx,
        |account, value| account.prowess = value,
    );
    let d1 = number(
        "fishing-input-d1",
        "Equipment D1",
        account.equipment_depth[0],
        0.0,
        1e6,
        1.0,
        0,
        cx,
        |account, value| account.equipment_depth[0] = value,
    );
    let d2 = number(
        "fishing-input-d2",
        "Equipment D2",
        account.equipment_depth[1],
        0.0,
        1e6,
        1.0,
        0,
        cx,
        |account, value| account.equipment_depth[1] = value,
    );
    let d3 = number(
        "fishing-input-d3",
        "Equipment D3",
        account.equipment_depth[2],
        0.0,
        1e6,
        1.0,
        0,
        cx,
        |account, value| account.equipment_depth[2] = value,
    );

    Card::new()
        .header(CardHeader::new("Account bonuses").description(
            "Entered from the game's Skills and AFK panels; the upstream derives them from the save's skill systems.",
        ))
        .child(
            div()
                .id("fishing-account-inputs")
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(Banner::new(
                    "fishing-account-notice",
                    Severity::Info,
                    "These values are manual: efficiency, EXP multiplier, AFK rate, Multi-Fish, speed sources, prowess and the equipment depths are not decoded yet. Neutral defaults give a lower bound.",
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_end()
                        .gap_4()
                        .children([
                            efficiency, exp, afk, multi, speed, attack, prowess, d1, d2, d3,
                        ]),
                )
                .child(
                    Caption::new(
                        "The selected lure and line add their own SPEED to the speed bonus and their D0–D3 to the depths.",
                    )
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn number(
    id: &'static str,
    label: &'static str,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    precision: usize,
    cx: &mut Context<AppState>,
    set: impl Fn(&mut AccountInputs, f64) + 'static,
) -> AnyElement {
    let entity = cx.entity();
    let input = NumberInput::new(id, value)
        .range(min, max)
        .step(step)
        .precision(precision)
        .label(label)
        .on_change(move |value, _window, cx| {
            entity.update(cx, |state, cx| {
                state.set_fishing_account(|account| set(account, value), cx);
            });
        });
    let muted = cx.theme().colors.fg_muted;
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(120.0))
        .child(Caption::new(label).text_color(muted))
        .child(input)
        .into_any_element()
}
