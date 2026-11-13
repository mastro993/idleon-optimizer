//! The Scurge tab: the Self Affliction comparison and the optimized EXP
//! allocation.
//!
//! Ports the two panels of `js/ui/clamworks-scurge.js` (`mode: 'affliction'` and
//! `mode: 'upgrades'`) over [`idleon_clamworks::scurge`], plus the list of the
//! nine Scurge upgrades the allocation spends on. Upstream compares simulated
//! runs on Clam Work; the shared simulator does not run that map, so the rates
//! multiply the row's EXP per kill by the character's kills-per-hour stand-in,
//! the same one the AFK claim takes.

use crate::state::AppState;
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::{Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, Window, div, px,
};
use idleon_clamworks::Clamworks;
use idleon_clamworks::scurge::{Error, MAX_AFFLICTION, MAX_OTHER_PLAYERS, Mode, Plan, Row};
use idleon_ui::format;

pub(crate) fn scurge_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let plan = state.clamworks_scurge_plan();
    let kills = state.clamworks_kills_per_hour();
    let saved = state.clamworks_view();
    let body = div()
        .id("clamworks-scurge")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(controls_card(state, kills, muted, cx))
        .child(match plan {
            Ok(plan) => comparison_card(&plan, kills, muted),
            Err(error) => refused_card(error),
        })
        .child(upgrades_card(&saved));
    ScrollArea::new("clamworks-scurge-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// The panel's own controls: the mode, its limit, the players on the map and the
/// kills-per-hour stand-in the rates use.
fn controls_card(
    state: &mut AppState,
    kills: f64,
    muted: gpui::Hsla,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let inputs = state.clamworks_scurge;
    let optimizing = inputs.mode == Mode::Upgrades;
    let character = state.save.as_ref().map_or_else(
        || "Character".to_string(),
        |save| save.character_name(state.clamworks_claim.character),
    );
    let set_mode = cx.entity();
    let set_max = cx.entity();
    let set_players = cx.entity();
    let set_kills = cx.entity();
    let mut controls = div()
        .id("clamworks-scurge-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new("Panel").text_color(muted))
                .child(
                    Select::new(
                        "clamworks-scurge-mode",
                        Mode::ALL
                            .into_iter()
                            .map(|mode| Choice::new(mode.id(), mode.label()))
                            .collect::<Vec<_>>(),
                    )
                    .label("Scurge panel")
                    .selected(inputs.mode.id())
                    .on_change(move |value, _window, cx| {
                        if let Some(mode) = Mode::from_id(value.as_ref()) {
                            set_mode.update(cx, |state, cx| {
                                state.set_clamworks_scurge_mode(mode, cx);
                            });
                        }
                    }),
                ),
        );
    // The upgrade panel holds Self Affliction, so its limit is not an entry.
    if !optimizing {
        controls = controls.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new("Max Self Affliction").text_color(muted))
                .child(
                    NumberInput::new(
                        "clamworks-scurge-max-affliction",
                        f64::from(inputs.max_affliction),
                    )
                    .label("Maximum Self Affliction level")
                    .range(0.0, f64::from(MAX_AFFLICTION))
                    .step(1.0)
                    .precision(0)
                    .size(ControlSize::Sm)
                    .on_change(move |value, _window, cx| {
                        set_max.update(cx, |state, cx| {
                            state.set_clamworks_scurge_max_affliction(value as u32, cx);
                        });
                    }),
                ),
        );
    }
    let controls = controls
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new("Other players").text_color(muted))
                .child(
                    NumberInput::new(
                        "clamworks-scurge-other-players",
                        f64::from(inputs.other_players),
                    )
                    .label("Other real players on the map")
                    .range(0.0, f64::from(MAX_OTHER_PLAYERS))
                    .step(1.0)
                    .precision(0)
                    .size(ControlSize::Sm)
                    .on_change(move |value, _window, cx| {
                        set_players.update(cx, |state, cx| {
                            state.set_clamworks_scurge_other_players(value as u32, cx);
                        });
                    }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new("Kills / hour").text_color(muted))
                .child(
                    NumberInput::new("clamworks-scurge-kills", kills)
                        .label("Kills per hour")
                        .range(0.0, 1_000_000.0)
                        .step(100.0)
                        .precision(0)
                        .size(ControlSize::Sm)
                        .on_change(move |value, _window, cx| {
                            set_kills.update(cx, |state, cx| {
                                state.set_clamworks_kills_per_hour(value, cx);
                            });
                        }),
                ),
        );
    let mut card = Card::new().header(CardHeader::new("Scurge combat").description(
        "One Clamworks kill banks RECKONING × EVEN BETTER × SELF AFFLICTION × SWARMING Scurge EXP.",
    ));
    card = card.child(
        div().p_3().flex().flex_col().gap_3().child(controls).child(
            Caption::new(format!(
                "{character} · Scurge level {} · {}",
                state.clamworks.scurge_level,
                if optimizing {
                    "spends the unspent points, SELF AFFLICTION held"
                } else {
                    "one SELF AFFLICTION level per row, every other upgrade saved"
                }
            ))
            .text_color(muted),
        ),
    );
    if kills <= 0.0 {
        card = card.child(
            div().px_3().pb_3().child(
                div()
                    .id("clamworks-scurge-kills-warning")
                    .role(Role::Label)
                    .aria_label("Kills per hour is zero")
                    .child(Banner::new(
                        "clamworks-scurge-kills-banner",
                        Severity::Warning,
                        "Set the character's kills per hour to read Scurge EXP an hour.",
                    )),
            ),
        );
    }
    card.into_any_element()
}

/// The upstream comparison table: every allocation beside the saved one.
fn comparison_card(plan: &Plan, kills: f64, muted: gpui::Hsla) -> AnyElement {
    // Every row shares one kills-per-hour entry, so the best allocation is the
    // same either way; the headline names the unit the entry actually reads.
    let best = plan.best();
    let headline = if kills > 0.0 {
        format!(
            "Best Scurge EXP an hour: {} · {}",
            best.kind.label(),
            format::value(best.exp_per_hour(kills))
        )
    } else {
        format!(
            "Best Scurge EXP a kill: {} · {}",
            best.kind.label(),
            format::value(best.exp_per_kill)
        )
    };
    let mut table = Table::new([
        "Allocation",
        "Scurge EXP / kill",
        "EXP / hour",
        "Gain / hour",
        "Points left",
        "Changes",
    ]);
    for (index, row) in plan.rows.iter().enumerate() {
        table = table.row(comparison_row(index, row, plan, kills, muted));
    }
    let optimizing = plan.inputs.mode == Mode::Upgrades;
    Card::new()
        .header(CardHeader::new("Comparison").description(format!(
            "{} · other players on the map: {}",
            plan.inputs.mode.label(),
            plan.inputs.other_players
        )))
        .child(
            div()
                .id("clamworks-scurge-best")
                .role(Role::Label)
                .aria_label(headline.clone())
                .px_3()
                .child(Caption::new(headline).text_color(muted)),
        )
        .child(div().p_3().child(div().id("clamworks-scurge-table").child(table)))
        .child(
            div()
                .id("clamworks-scurge-notes")
                .flex()
                .flex_col()
                .gap_1()
                .px_3()
                .pb_3()
                .text_size(px(11.0))
                .text_color(muted)
                .child(
                    "Scurge EXP is banked per kill; the rate multiplies it by the character's kills \
                     per hour, the same stand-in the AFK claim takes, because the shared simulator \
                     does not run Clam Work.",
                )
                .child(if optimizing {
                    "Only unspent points are allocated, across the unlocked RECKONING, EVEN BETTER \
                     and SWARMING; SELF AFFLICTION and every other upgrade stay saved. Cost \
                     reductions, future level retention and promotions are not optimized."
                } else {
                    "Only SELF AFFLICTION changes. A lower level refunds points without spending \
                     them elsewhere; a higher one uses the available point budget."
                })
                .child(
                    "Each row holds the starting Scurge level fixed, so this is the best of the \
                     listed allocations, not a global optimum. No save or in-game allocation changes.",
                ),
        )
        .into_any_element()
}

fn comparison_row(
    index: usize,
    row: &Row,
    plan: &Plan,
    kills: f64,
    muted: gpui::Hsla,
) -> [AnyElement; 6] {
    let name = row.kind.label();
    let changes = if row.changes.is_empty() {
        "Keep saved allocation".to_string()
    } else {
        row.changes
            .iter()
            .map(|change| format!("{} {} → {}", change.name(), change.from, change.to))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let gain = plan.gain(row, kills);
    let gain = if gain > 0.0 {
        format!("+{}", format::value(gain))
    } else {
        format::value(gain)
    };
    [
        div()
            .id(("clamworks-scurge-row", index))
            .role(Role::Label)
            .aria_label(format!(
                "{name}: {} EXP per kill",
                format::value(row.exp_per_kill)
            ))
            .flex()
            .flex_col()
            .gap_1()
            .child(SharedString::from(name))
            .child(
                Caption::new(format!("Self Affliction {}", row.self_affliction()))
                    .text_color(muted),
            )
            .into_any_element(),
        div()
            .child(format::value(row.exp_per_kill))
            .into_any_element(),
        div()
            .child(format::value(row.exp_per_hour(kills)))
            .into_any_element(),
        div().child(gain).into_any_element(),
        div().child(row.points_left.to_string()).into_any_element(),
        div()
            .id(("clamworks-scurge-changes", index))
            .min_w_0()
            .child(changes)
            .into_any_element(),
    ]
}

/// Why the panel cannot compare anything: the save has no Scurge menu, its
/// upgrades overspend its level, or an entry is out of range.
fn refused_card(error: Error) -> AnyElement {
    Card::new()
        .header(CardHeader::new("Comparison"))
        .child(
            div().p_3().child(
                div()
                    .id("clamworks-scurge-error")
                    .role(Role::Label)
                    .aria_label(error.message())
                    .child(Banner::new(
                        "clamworks-scurge-error-banner",
                        Severity::Warning,
                        error.message(),
                    )),
            ),
        )
        .into_any_element()
}

/// The nine Scurge upgrades with their saved levels, values and unlock levels.
fn upgrades_card(clamworks: &Clamworks) -> AnyElement {
    let data = &idleon_clamworks::data::clamworks().scurge;
    let mut table = Table::new(["Scurge upgrade", "Lv", "Per Lv", "Unlocks at", "Status"]);
    for (index, upgrade) in data.iter().enumerate() {
        let unlocked = clamworks.scurge_level as f64 >= upgrade.unlock_level;
        table = table.row([
            div()
                .id(("clamworks-scurge-upgrade", index))
                .child(SharedString::from(upgrade.name.clone()))
                .into_any_element(),
            div()
                .child(clamworks.scurge_upgrades[index].to_string())
                .into_any_element(),
            div()
                .child(format::value(upgrade.per_lv))
                .into_any_element(),
            div()
                .child(upgrade.unlock_level.to_string())
                .into_any_element(),
            div()
                .id(("clamworks-scurge-unlock", index))
                .child(if unlocked { "Unlocked" } else { "Locked" })
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Scurge upgrades")
                .description("The nine Scurge upgrades, in the game's order, at the saved levels."),
        )
        .child(
            div()
                .id("clamworks-scurge-upgrades")
                .p_3()
                .child(div().min_w_0().child(table)),
        )
        .into_any_element()
}
