//! The Wisdom tab: saved stats, the reward charge, milestones and the
//! permanent reward levels.

use super::{
    MilestoneFirst, bonuses_card, empty_card, metric_card, milestones_card, notes_card,
    unavailable_card,
};
use crate::state::AppState;
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_monument::{Charge, WisdomSnapshot};
use idleon_ui::format;

/// The Wisdom pane.
pub(crate) fn pane(
    state: &mut AppState,
    snapshot: Option<&WisdomSnapshot>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let body = match snapshot {
        Some(snapshot) if snapshot.available => loaded(state, snapshot, cx),
        Some(snapshot) => unavailable_card("Wisdom", &snapshot.notes, cx),
        None => empty_card(
            "Wisdom",
            "Import a decoded save to read the Wisdom training, attempts, milestones, rewards and charge.",
            cx,
        ),
    };
    div()
        .id("monument-pane-wisdom")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            ScrollArea::new("monument-wisdom-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .id("monument-wisdom-body")
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(body),
                ),
        )
        .into_any_element()
}

fn loaded(state: &AppState, snapshot: &WisdomSnapshot, cx: &mut Context<AppState>) -> AnyElement {
    let stats = &snapshot.stats;
    let cards = div()
        .id("monument-wisdom-stats")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(metric_card(
            "monument-wisdom-hours",
            "Saved training",
            format!("{} hours", format::value(stats.hours)),
            "Paying Respect; not reward levels".into(),
            cx,
        ))
        .child(metric_card(
            "monument-wisdom-count",
            "Unlocked milestones",
            format!("{} / 8", stats.milestones),
            "Milestones are saved; the next row shows its ETA".into(),
            cx,
        ))
        .child(metric_card(
            "monument-wisdom-opal",
            "Next opal offer chance",
            format::percent(stats.opal_chance),
            "Uses saved opals; each additional opal lowers later offers".into(),
            cx,
        ))
        .child(metric_card(
            "monument-wisdom-attempts",
            "Start attempts",
            format::value(stats.attempts),
            "Per story, including the study bonus".into(),
            cx,
        ))
        .child(metric_card(
            "monument-wisdom-board",
            "Attempts per board clear",
            format::value(stats.attempts_per_board),
            "From the second and seventh milestones".into(),
            cx,
        ))
        .child(metric_card(
            "monument-wisdom-instamatches",
            "Insta-matches per story",
            format::value(stats.instamatches),
            "From the fourth and eighth milestones".into(),
            cx,
        ))
        .child(metric_card(
            "monument-wisdom-match",
            "Time per match",
            format!("{} seconds", format::value(stats.seconds_per_match)),
            "One board match, as the game counts it".into(),
            cx,
        ))
        .child(metric_card(
            "monument-wisdom-reveals",
            "Reveal aids",
            reveals(stats.reveal_row, stats.reveal_square),
            "Milestone unlocks on every board".into(),
            cx,
        ));

    let mut column = div().flex().flex_col().gap_3().child(cards);
    if !snapshot.notes.is_empty() {
        column = column.child(notes_card("monument-wisdom-notes", &snapshot.notes, cx));
    }
    column = column.child(charge_card(state, &snapshot.charge, &snapshot.tomorrow, cx));
    column = column.child(milestones_card(
        &snapshot.milestones,
        MilestoneFirst::Attempts(stats.attempts),
    ));
    column = column.child(bonuses_card(&snapshot.bonuses));
    column.into_any_element()
}

fn reveals(row: bool, square: bool) -> String {
    match (row, square) {
        (true, true) => "First attempt: row; fourth attempt: square".into(),
        (true, false) => "First attempt: row".into(),
        (false, true) => "Fourth attempt: square".into(),
        (false, false) => "None".into(),
    }
}

fn charge_card(
    state: &AppState,
    charge: &Charge,
    tomorrow: &Charge,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let character = state
        .save
        .as_ref()
        .map(|save| save.character_name(state.monument.character))
        .unwrap_or_default();
    let ready = if charge.can_start {
        "story ready"
    } else {
        "needs at least 1x to start"
    };
    let sub = format!(
        "{} real hours waited for {character}; {ready}. After another 24h: {}x.",
        format::value(charge.seconds / 3600.0),
        format::value(tomorrow.multi),
    );
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .id("monument-wisdom-charge")
                .flex()
                .flex_wrap()
                .gap_3()
                .child(metric_card(
                    "monument-wisdom-charge-now",
                    "Reward charge",
                    format!("{}x", format::value(charge.multi)),
                    sub,
                    cx,
                ))
                .child(metric_card(
                    "monument-wisdom-charge-window",
                    "Linear window",
                    format!("{} days", format::value(charge.max_linear / 86_400.0)),
                    format!(
                        "Meritoc 7 +{}% · Legend 27 +{}%",
                        format::value(charge.meritoc),
                        format::value(charge.legend)
                    ),
                    cx,
                )),
        )
        .child(Caption::new(
            "Waiting is saved per monument; the linear window includes both storage schematics, Monumental Gaming, Legend 27 and Justice study ownership. Waiting beyond it still pays a diminished tail, and the charge multiplies the reward rolls a monument pays when its story restarts.",
        ))
        .into_any_element()
}
