//! The Scurge tab: the Self Affliction comparison and the optimized EXP
//! allocation.
//!
//! The affliction and upgrades panels over [`idleon_clamworks::scurge`], plus
//! the list of the nine Scurge upgrades the allocation spends on. The shared
//! simulator does not run Clam Work, so the rates multiply the row's EXP per
//! kill by the character's kills-per-hour stand-in, the same one the AFK claim
//! takes.

use super::controls::mode_choices;
use super::pane;
use super::table::{Col, note, table, text};
use crate::state::AppState;
use crate::ui;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Role, SharedString, Styled, Window, div,
};
use gpui_kit::component::alert::Alert;
use gpui_kit::component::select::Select;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use idleon_clamworks::Clamworks;
use idleon_clamworks::scurge::{Error, Mode, Plan, Row};
use idleon_ui::format;
use idleon_ui::widgets::{Card, CardHeader, PANEL_GAP, Tone, labeled, number_width, show_number};

pub(crate) fn scurge_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let plan = state.clamworks_scurge_plan();
    let kills = state.clamworks_kills_per_hour();
    let saved = state.clamworks_shown();
    let body = v_flex()
        .id("clamworks-scurge")
        .gap(PANEL_GAP)
        .child(controls_card(state, kills, window, cx))
        .child(match plan.as_ref() {
            Ok(plan) => comparison_card(plan, kills, cx),
            Err(error) => refused_card(*error),
        })
        .child(upgrades_card(&saved, cx));
    pane("clamworks-scurge-scroll", body)
}

/// The panel's own controls: the mode, its limit, the players on the map and the
/// kills-per-hour stand-in the rates use.
fn controls_card(
    state: &mut AppState,
    kills: f64,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let inputs = state.clamworks_scurge;
    let optimizing = inputs.mode == Mode::Upgrades;
    let character = state.save.as_ref().map_or_else(
        || "Character".to_string(),
        |save| save.character_name(state.clamworks_claim.character),
    );
    let controls = &state.clamworks_controls;
    // Values flow section state -> control; edits come back through the
    // subscriptions in `Controls::new`.
    controls
        .scurge_mode
        .show(mode_choices(), Some(&inputs.mode), window, cx);
    show_number(
        &controls.max_affliction,
        f64::from(inputs.max_affliction),
        0,
        window,
        cx,
    );
    show_number(
        &controls.other_players,
        f64::from(inputs.other_players),
        0,
        window,
        cx,
    );
    show_number(&controls.scurge_kills, kills, 0, window, cx);
    let row = h_flex()
        .id("clamworks-scurge-controls")
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(labeled(
            "Panel",
            div()
                .flex_none()
                .max_w_full()
                .w(controls.scurge_mode.width(window, cx))
                .child(
                    Select::new(controls.scurge_mode.state())
                        .id("clamworks-scurge-mode")
                        .accessibility_label("Scurge panel")
                        .small(),
                ),
        ))
        // The upgrade panel holds Self Affliction, so its limit is not an entry.
        .when(!optimizing, |row| {
            row.child(labeled(
                "Max Self Affliction",
                ui::number_field(
                    "clamworks-scurge-max-affliction",
                    &controls.max_affliction,
                    "Maximum Self Affliction level",
                )
                .w(number_width(&controls.max_affliction, window, cx)),
            ))
        })
        .child(labeled(
            "Other players",
            ui::number_field(
                "clamworks-scurge-other-players",
                &controls.other_players,
                "Other real players on the map",
            )
            .w(number_width(&controls.other_players, window, cx)),
        ))
        .child(labeled(
            "Kills / hour",
            ui::number_field(
                "clamworks-scurge-kills",
                &controls.scurge_kills,
                "Kills per hour",
            )
            .w(number_width(&controls.scurge_kills, window, cx)),
        ));
    let card = Card::new()
        .header(CardHeader::new("Scurge combat").description(
            "One Clamworks kill banks RECKONING × EVEN BETTER × SELF AFFLICTION × SWARMING Scurge EXP.",
        ))
        .child(row)
        .child(note(
            format!(
                "{character} · Scurge level {} · {}",
                state.clamworks.scurge_level,
                if optimizing {
                    "spends the unspent points, SELF AFFLICTION held"
                } else {
                    "one SELF AFFLICTION level per row, every other upgrade saved"
                }
            ),
            cx,
        ));
    card.when(kills <= 0.0, |card| {
        card.child(
            div()
                .id("clamworks-scurge-kills-warning")
                .role(Role::Label)
                .aria_label("Kills per hour is zero")
                .child(
                    Alert::new(
                        "clamworks-scurge-kills-banner",
                        "Set the character's kills per hour to read Scurge EXP an hour.",
                    )
                    .with_variant(Tone::Warning.into())
                    .banner(),
                ),
        )
    })
    .into_any_element()
}

/// The comparison table: every allocation beside the saved one.
fn comparison_card(plan: &Plan, kills: f64, cx: &App) -> AnyElement {
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
    let rows = plan
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| comparison_row(index, row, plan, kills, cx));
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
                .child(note(headline, cx)),
        )
        .child(table(
            "clamworks-scurge",
            &[
                Col::Text("Allocation"),
                Col::Number("Scurge EXP / kill"),
                Col::Number("EXP / hour"),
                Col::Number("Gain / hour"),
                Col::Number("Points left"),
                Col::Text("Changes"),
            ],
            rows,
            cx,
        ))
        .child(
            v_flex()
                .id("clamworks-scurge-notes")
                .gap_1()
                .child(note(
                    "Scurge EXP is banked per kill; the rate multiplies it by the character's kills \
                     per hour, the same stand-in the AFK claim takes, because the shared simulator \
                     does not run Clam Work.",
                    cx,
                ))
                .child(note(
                    if optimizing {
                        "Only unspent points are allocated, across the unlocked RECKONING, EVEN BETTER \
                         and SWARMING; SELF AFFLICTION and every other upgrade stay saved. Cost \
                         reductions, future level retention and promotions are not optimized."
                    } else {
                        "Only SELF AFFLICTION changes. A lower level refunds points without spending \
                         them elsewhere; a higher one uses the available point budget."
                    },
                    cx,
                ))
                .child(note(
                    "Each row holds the starting Scurge level fixed, so this is the best of the \
                     listed allocations, not a global optimum. No save or in-game allocation changes.",
                    cx,
                )),
        )
        .into_any_element()
}

fn comparison_row(index: usize, row: &Row, plan: &Plan, kills: f64, cx: &App) -> [AnyElement; 6] {
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
        v_flex()
            .id(("clamworks-scurge-row", index))
            .role(Role::Label)
            .aria_label(format!(
                "{name}: {} EXP per kill",
                format::value(row.exp_per_kill)
            ))
            .gap_1()
            .child(SharedString::from(name))
            .child(note(
                format!("Self Affliction {}", row.self_affliction()),
                cx,
            ))
            .into_any_element(),
        text(format::value(row.exp_per_kill)),
        text(format::value(row.exp_per_hour(kills))),
        text(gain),
        text(row.points_left.to_string()),
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
            div()
                .id("clamworks-scurge-error")
                .role(Role::Label)
                .aria_label(error.message())
                .child(
                    Alert::new("clamworks-scurge-error-banner", error.message())
                        .with_variant(Tone::Warning.into())
                        .banner(),
                ),
        )
        .into_any_element()
}

/// The nine Scurge upgrades with their saved levels, values and unlock levels.
fn upgrades_card(clamworks: &Clamworks, cx: &App) -> AnyElement {
    let data = &idleon_clamworks::data::clamworks().scurge;
    let rows = data.iter().enumerate().map(|(index, upgrade)| {
        let unlocked = clamworks.scurge_level as f64 >= upgrade.unlock_level;
        [
            div()
                .id(("clamworks-scurge-upgrade", index))
                .child(SharedString::from(upgrade.name.clone()))
                .into_any_element(),
            text(clamworks.scurge_upgrades[index].to_string()),
            text(format::value(upgrade.per_lv)),
            text(upgrade.unlock_level.to_string()),
            div()
                .id(("clamworks-scurge-unlock", index))
                .child(if unlocked { "Unlocked" } else { "Locked" })
                .into_any_element(),
        ]
    });
    Card::new()
        .header(
            CardHeader::new("Scurge upgrades")
                .description("The nine Scurge upgrades, in the game's order, at the saved levels."),
        )
        .child(div().id("clamworks-scurge-upgrades").child(table(
            "clamworks-scurge-upgrade-list",
            &[
                Col::Text("Scurge upgrade"),
                Col::Number("Lv"),
                Col::Number("Per Lv"),
                Col::Number("Unlocks at"),
                Col::Text("Status"),
            ],
            rows,
            cx,
        )))
        .into_any_element()
}
