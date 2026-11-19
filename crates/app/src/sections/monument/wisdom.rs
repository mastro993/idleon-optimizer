//! The Wisdom tab: saved stats, the reward charge, milestones and the
//! permanent reward levels.

use super::{
    Metric, MilestoneFirst, bonuses_card, empty_card, metrics, milestones_card, notes_card,
    scroll_pane, unavailable_card,
};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement};
use gpui_kit::component::v_flex;
use idleon_monument::{Charge, WisdomSnapshot};
use idleon_ui::format;
use idleon_ui::widgets::Caption;

/// The Wisdom pane.
pub(crate) fn pane(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let body = match &state.monument_controls.derived.wisdom {
        Some(snapshot) if snapshot.available => loaded(state, snapshot, cx),
        Some(snapshot) => unavailable_card("Wisdom", &snapshot.notes, cx),
        None => empty_card(
            "Wisdom",
            "Import a decoded save to read the Wisdom training, attempts, milestones, rewards and charge.",
            cx,
        ),
    };
    scroll_pane(
        "monument-pane-wisdom",
        "monument-wisdom-scroll",
        v_flex().id("monument-wisdom-body").gap_3().child(body),
    )
}

fn loaded(state: &AppState, snapshot: &WisdomSnapshot, cx: &mut Context<AppState>) -> AnyElement {
    let stats = &snapshot.stats;
    let cards = metrics(
        "monument-wisdom-stats",
        vec![
            Metric::new(
                "monument-wisdom-hours",
                "Saved training",
                format!("{} hours", format::value(stats.hours)),
                "Paying Respect; not reward levels",
            ),
            Metric::new(
                "monument-wisdom-count",
                "Unlocked milestones",
                format!("{} / 8", stats.milestones),
                "Milestones are saved; the next row shows its ETA",
            ),
            Metric::new(
                "monument-wisdom-opal",
                "Next opal offer chance",
                format::percent(stats.opal_chance),
                "Uses saved opals; each additional opal lowers later offers",
            ),
            Metric::new(
                "monument-wisdom-attempts",
                "Start attempts",
                format::value(stats.attempts),
                "Per story, including the study bonus",
            ),
            Metric::new(
                "monument-wisdom-board",
                "Attempts per board clear",
                format::value(stats.attempts_per_board),
                "From the second and seventh milestones",
            ),
            Metric::new(
                "monument-wisdom-instamatches",
                "Insta-matches per story",
                format::value(stats.instamatches),
                "From the fourth and eighth milestones",
            ),
            Metric::new(
                "monument-wisdom-match",
                "Time per match",
                format!("{} seconds", format::value(stats.seconds_per_match)),
                "One board match, as the game counts it",
            ),
            Metric::new(
                "monument-wisdom-reveals",
                "Reveal aids",
                reveals(stats.reveal_row, stats.reveal_square),
                "Milestone unlocks on every board",
            ),
        ],
    );

    let mut column = v_flex().gap_3().child(cards);
    if !snapshot.notes.is_empty() {
        column = column.child(notes_card("monument-wisdom-notes", &snapshot.notes, cx));
    }
    column = column.child(charge_card(state, &snapshot.charge, &snapshot.tomorrow));
    column = column.child(milestones_card(
        &snapshot.milestones,
        MilestoneFirst::Attempts(stats.attempts),
        cx,
    ));
    column = column.child(bonuses_card(&snapshot.bonuses, cx));
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

fn charge_card(state: &AppState, charge: &Charge, tomorrow: &Charge) -> AnyElement {
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
    v_flex()
        .gap_3()
        .child(metrics(
            "monument-wisdom-charge",
            vec![
                Metric::new(
                    "monument-wisdom-charge-now",
                    "Reward charge",
                    format!("{}x", format::value(charge.multi)),
                    sub,
                ),
                Metric::new(
                    "monument-wisdom-charge-window",
                    "Linear window",
                    format!("{} days", format::value(charge.max_linear / 86_400.0)),
                    format!(
                        "Meritoc 7 +{}% · Legend 27 +{}%",
                        format::value(charge.meritoc),
                        format::value(charge.legend)
                    ),
                ),
            ],
        ))
        .child(Caption::new(
            "Waiting is saved per monument; the linear window includes both storage schematics, Monumental Gaming, Legend 27 and Justice study ownership. Waiting beyond it still pays a diminished tail, and the charge multiplies the reward rolls a monument pays when its story restarts.",
        ))
        .into_any_element()
}
