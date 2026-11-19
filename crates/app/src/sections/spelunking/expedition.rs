//! The expedition controls above the party, belt and depth estimate.
//!
//! Every control the toolbar carries: the logged-in character, the cave (with
//! its Normal/Manic records), Normal/Manic mode, the policy, the Auto
//! full-clear flag and the Auto speed. The account card carries the four values
//! that come from the account's whole stat context, which the save decode does
//! not cover yet.

use super::controls::{Derived, finisher_choices};
use super::{boxed, field, number, select, tag};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Styled, Window, div};
use gpui_kit::component::Disableable as _;
use gpui_kit::component::checkbox::Checkbox;
use idleon_spelunking::Policy;
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone};

/// The toolbar card: character, cave, mode, policy, full-clear and speed.
pub(crate) fn card(
    state: &AppState,
    derived: &Derived,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let spelunking = &state.spelunking;
    let controls = &state.spelunking_controls;
    let auto = spelunking.policy == Policy::Auto;
    let clear = cx.entity();
    let full_clear = Checkbox::new("spelunk-full-clear")
        .label("Auto full clear")
        .checked(spelunking.full_clear)
        .disabled(!auto)
        .on_click(move |checked, _window, cx| {
            let checked = *checked;
            clear.update(cx, |state, cx| state.set_spelunk_full_clear(checked, cx));
        });
    let character = select(
        &controls.character,
        "spelunk-character",
        "Logged-in character",
        &derived.characters,
        &spelunking.character,
        window,
        cx,
    );
    let cave = select(
        &controls.cave,
        "spelunk-cave",
        "Cave",
        &derived.caves,
        &spelunking.cave,
        window,
        cx,
    );
    let mode = select(
        &controls.mode,
        "spelunk-mode",
        "Expedition",
        &derived.modes,
        &spelunking.manic,
        window,
        cx,
    );
    let policy = select(
        &controls.policy,
        "spelunk-policy",
        "Policy",
        &derived.policies,
        &spelunking.policy,
        window,
        cx,
    );
    let speed = select(
        &controls.speed,
        "spelunk-speed",
        "Auto speed",
        &derived.speeds,
        &spelunking.speed,
        window,
        cx,
    )
    .disabled(!auto);

    Card::new()
        .header(CardHeader::new("Expedition controls").description(
            "The save supplies each character's stamina, the caves and their records, and the \
             available modes, speeds and elixirs.",
        ))
        .child(
            div()
                .id("spelunk-controls")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_4()
                .child(field("Character", boxed(character)))
                .child(field("Cave", boxed(cave)))
                .child(field("Mode", boxed(mode)))
                .child(field("Policy", boxed(policy)))
                .child(field(
                    "Descent",
                    div().h_6().flex().items_center().child(full_clear),
                ))
                .child(field("Auto speed", boxed(speed)))
                .when(spelunking.policy == Policy::AmberFarm, |row| {
                    row.child(tag("Amber farming always clears the floor", Tone::Warning))
                }),
        )
        .into_any_element()
}

/// The account values the account's stat context would supply.
pub(crate) fn account_card(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let account = &state.spelunking.account;
    let controls = &state.spelunking_controls;
    Card::new()
        .header(CardHeader::new("Account inputs").description(
            "Spelunking Power, the extra-exit and Amber multipliers, stamina regeneration and the \
             Grand Discovery chance are account values the save does not decode yet: read them in \
             game. The save fixes everything else.",
        ))
        .child(
            div()
                .id("spelunk-account-inputs")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_4()
                .child(field(
                    "Spelunking Power",
                    number(
                        "spelunk-power",
                        &controls.power,
                        "Spelunking Power",
                        account.power,
                        4,
                        window,
                        cx,
                    ),
                ))
                .child(field(
                    "Extra exits (%)",
                    number(
                        "spelunk-extra-exits",
                        &controls.exits,
                        "Extra exits",
                        account.holes_bonus,
                        2,
                        window,
                        cx,
                    ),
                ))
                .child(field(
                    "Amber multiplier",
                    number(
                        "spelunk-amber",
                        &controls.amber,
                        "Amber multiplier",
                        account.amber,
                        4,
                        window,
                        cx,
                    ),
                ))
                .child(field(
                    "Stamina regen / hr",
                    number(
                        "spelunk-regen",
                        &controls.regen,
                        "Stamina regeneration",
                        account.regen,
                        2,
                        window,
                        cx,
                    ),
                ))
                .child(field(
                    "Grand Discovery (%)",
                    number(
                        "spelunk-grand",
                        &controls.grand,
                        "Grand Discovery chance (%)",
                        100.0 * account.grand,
                        4,
                        window,
                        cx,
                    ),
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Caption::new("The saved belt and memory are the defaults.")),
                ),
        )
        .into_any_element()
}

/// The phased Amber policy's limits, shown for `amber-farm` only.
pub(crate) fn amber_card(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> Option<AnyElement> {
    if state.spelunking.policy != Policy::AmberFarm {
        return None;
    }
    let amber = state.spelunking.amber;
    let controls = &state.spelunking_controls;
    let finishers = select(
        &controls.finishers,
        "spelunk-amber-finishers",
        "Finishing characters",
        &finisher_choices(),
        &amber.finishers,
        window,
        cx,
    );
    Some(
        Card::new()
            .header(CardHeader::new("Amber policy").description(
                "Manual targeting throughout: full clear and drink teals as they come, saving the \
                 oranges, then burn stamina on visible objects from the dump depth. The first \
                 finisher keeps the reserve; later finishers stay untouched until needed. Drink \
                 the oranges when the reserve is reached, harvest within the remaining stamina \
                 budget, and take the first untouched rope after the harvest target or below the \
                 rope reserve.",
            ))
            .child(
                div()
                    .id("spelunk-amber-policy")
                    .flex()
                    .flex_wrap()
                    .items_end()
                    .gap_4()
                    .child(field(
                        "Dump depth",
                        number(
                            "spelunk-amber-dump-depth",
                            &controls.dump,
                            "Amber dump depth",
                            f64::from(amber.dump_depth),
                            0,
                            window,
                            cx,
                        ),
                    ))
                    .child(field("Finishing characters", boxed(finishers)))
                    .child(field(
                        "Finisher reserve (%)",
                        number(
                            "spelunk-amber-reserve",
                            &controls.reserve,
                            "First finisher's stamina to keep (%)",
                            f64::from(amber.reserve_pct),
                            0,
                            window,
                            cx,
                        ),
                    ))
                    .child(field(
                        "Harvest floors",
                        number(
                            "spelunk-amber-harvest",
                            &controls.harvest,
                            "Floors after oranges before seeking rope",
                            f64::from(amber.harvest_depths),
                            0,
                            window,
                            cx,
                        ),
                    ))
                    .child(field(
                        "Rope reserve (%)",
                        number(
                            "spelunk-amber-rope-reserve",
                            &controls.rope,
                            "Seek rope below finishing stamina (%)",
                            f64::from(amber.rope_reserve_pct),
                            0,
                            window,
                            cx,
                        ),
                    )),
            )
            .into_any_element(),
    )
}
