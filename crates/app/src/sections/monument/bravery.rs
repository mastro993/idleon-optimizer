//! The Bravery tab: saved stats, the run-state inputs, sword odds, milestones
//! and the permanent reward levels.

use super::{clean_text, metric_card};
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::forms::NumberInput;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div, px};
use idleon_core::monument::{BonusRow, Milestone, Odds, Snapshot};

/// The Bravery pane.
pub fn pane(
    state: &mut AppState,
    snapshot: Option<&Snapshot>,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let body = match snapshot {
        Some(snapshot) if snapshot.available => loaded(state, snapshot, cx),
        Some(snapshot) => unavailable(snapshot, cx),
        None => empty(window, cx),
    };
    div()
        .id("monument-pane-bravery")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            ScrollArea::new("monument-bravery-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .id("monument-bravery-body")
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(body),
                ),
        )
        .into_any_element()
}

fn empty(_window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    Card::new()
        .header(CardHeader::new("Bravery"))
        .child(
            EmptyState::new("monument-empty", IconName::Upload, "Load a save")
                .body("Import a decoded save to read the Bravery training, milestones and permanent rewards.")
                .action(
                    Button::new("monument-import", "Import save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            entity.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}

fn unavailable(snapshot: &Snapshot, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut card = Card::new()
        .header(CardHeader::new("Bravery").description("The save has no Monument rows."))
        .child(Caption::new(
            "This save is missing Monument charge, training or reward data.",
        ));
    if !snapshot.notes.is_empty() {
        card = card.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(snapshot.notes.iter().map(|note| {
                    div()
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(note.clone())
                })),
        );
    }
    card.into_any_element()
}

fn loaded(state: &mut AppState, snapshot: &Snapshot, cx: &mut Context<AppState>) -> AnyElement {
    let stats = &snapshot.stats;
    let cards = div()
        .id("monument-bravery-stats")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(metric_card(
            "monument-hours",
            "Saved training",
            format!("{} hours", format::value(stats.hours)),
            "Paying Respect; not reward levels".into(),
            cx,
        ))
        .child(metric_card(
            "monument-milestones",
            "Unlocked milestones",
            format!("{} / 8", stats.milestones),
            "Milestones are saved; the next row shows its ETA".into(),
            cx,
        ))
        .child(metric_card(
            "monument-opal",
            "Next opal offer chance",
            format::percent(stats.opal_chance),
            "Uses saved opals; each additional opal lowers later offers".into(),
            cx,
        ))
        .child(metric_card(
            "monument-swords",
            "Swords",
            format::value(f64::from(stats.swords)),
            "Swords this story can throw".into(),
            cx,
        ))
        .child(metric_card(
            "monument-damage",
            "Initial sword damage",
            format!(
                "{} - {}",
                format::value(stats.min_dmg),
                format::value(stats.max_dmg)
            ),
            "Fresh story: zero boosts, full rethrows".into(),
            cx,
        ))
        .child(metric_card(
            "monument-reroll-summary",
            "Rethrows / retellings",
            format!("{} / {}", stats.max_rerolls, stats.retellings),
            "Per story".into(),
            cx,
        ));

    let mut column = div().flex().flex_col().gap_3().child(cards);
    if !snapshot.notes.is_empty() {
        column = column.child(notes_card(snapshot, cx));
    }
    column = column.child(controls_card(state, snapshot, cx));
    match state.monument_odds() {
        Some(Ok(odds)) => {
            column = column.child(odds_cards(&odds, cx));
        }
        Some(Err(error)) => {
            column = column.child(error_line(error.to_string(), cx));
        }
        None => {}
    }
    column = column.child(
        Card::new()
            .header(CardHeader::new("Right-most sword damage boost").description(
                "Run state is not saved. Enter the current enemy and boost count. For the finishing blow, let every other sword land before the right-most sword; an earlier kill ends the fight before a later sword can grant a boost.",
            ))
            .child(
                Caption::new(
                    "Fresh-roll odds use continuous uniform damage and no rethrows. A rethrow replaces the last sword's old damage; it does not add another hit. The optional HP should exclude that sword's old hit.",
                ),
            ),
    );
    column = column.child(milestones_card(snapshot));
    column = column.child(bonuses_card(snapshot));
    column.into_any_element()
}

fn notes_card(snapshot: &Snapshot, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    Card::new()
        .header(CardHeader::new("Partial save").description("Some sources cannot be read"))
        .child(
            div()
                .id("monument-bravery-notes")
                .flex()
                .flex_col()
                .gap_1()
                .children(snapshot.notes.iter().map(|note| {
                    div()
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(note.clone())
                })),
        )
        .into_any_element()
}

fn controls_card(
    state: &mut AppState,
    snapshot: &Snapshot,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let entity = cx.entity();
    let wave = entity.clone();
    let boosts = entity.clone();
    let rerolls = entity.clone();
    let reset_rerolls = entity.clone();
    let hp = entity.clone();
    let clear_hp = entity.clone();

    let reroll_value = state.monument.rerolls.unwrap_or(snapshot.stats.max_rerolls) as f64;
    let hp_value = state.monument.remaining_hp.unwrap_or(0.0);
    let hp_input = NumberInput::new("monument-hp", hp_value)
        .range(0.000001, 1.0e300)
        .precision(0)
        .label("HP before last sword")
        .on_change(move |value, _window, cx| {
            hp.update(cx, |state, cx| state.set_monument_hp(value, cx));
        });
    let hp_input = if state.monument.remaining_hp.is_none() {
        hp_input.blank("Unknown")
    } else {
        hp_input
    };

    Card::new()
        .header(
            CardHeader::new("Run state")
                .description("Enemy number, boost stacks and the rethrows left in this story."),
        )
        .child(
            div()
                .id("monument-controls")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(number_field(
                    "monument-wave",
                    "Enemy number",
                    f64::from(state.monument.wave),
                    (1.0, 1001.0),
                    1.0,
                    move |value, _window, cx| {
                        wave.update(cx, |state, cx| state.set_monument_wave(value, cx));
                    },
                ))
                .child(number_field(
                    "monument-boosts",
                    "Current boost stacks",
                    f64::from(state.monument.boosts),
                    (0.0, 100_000.0),
                    1.0,
                    move |value, _window, cx| {
                        boosts.update(cx, |state, cx| state.set_monument_boosts(value, cx));
                    },
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Rethrows remaining"))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    NumberInput::new("monument-rerolls", reroll_value)
                                        .range(0.0, snapshot.stats.max_rerolls as f64)
                                        .precision(0)
                                        .label("Rethrows remaining")
                                        .on_change(move |value, _window, cx| {
                                            rerolls.update(cx, |state, cx| {
                                                state.set_monument_rerolls(value, cx);
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("monument-rerolls-max", "Max")
                                        .size(ControlSize::Sm)
                                        .disabled(state.monument.rerolls.is_none())
                                        .on_click(move |_event, _window, cx| {
                                            reset_rerolls.update(cx, |state, cx| {
                                                state.reset_monument_rerolls(cx);
                                            });
                                        }),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("HP before last sword (optional)"))
                        .child(
                            div().flex().items_center().gap_2().child(hp_input).child(
                                Button::new("monument-hp-clear", "Clear")
                                    .size(ControlSize::Sm)
                                    .disabled(state.monument.remaining_hp.is_none())
                                    .on_click(move |_event, _window, cx| {
                                        clear_hp.update(cx, |state, cx| {
                                            state.clear_monument_hp(cx);
                                        });
                                    }),
                            ),
                        ),
                ),
        )
        .into_any_element()
}

fn number_field(
    id: &'static str,
    label: &'static str,
    value: f64,
    range: (f64, f64),
    step: f64,
    on_change: impl Fn(f64, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label))
        .child(
            NumberInput::new(id, value)
                .range(range.0, range.1)
                .step(step)
                .precision(0)
                .label(label)
                .on_change(on_change),
        )
        .into_any_element()
}

fn error_line(message: String, cx: &mut Context<AppState>) -> AnyElement {
    let danger = cx.theme().colors.danger;
    div()
        .id("monument-bravery-error")
        .role(Role::Alert)
        .text_color(danger)
        .child(message)
        .into_any_element()
}

fn odds_cards(odds: &Odds, cx: &mut Context<AppState>) -> AnyElement {
    let boost_sub = if odds.automatic {
        "Study guarantees the skip and grants a stack"
    } else if odds.stats.rightmost {
        "Others do not kill; the right-most finishes"
    } else {
        "The right-most boost schematic is not owned"
    };
    let mut cards = div()
        .id("monument-bravery-odds")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(metric_card(
            "monument-odds-hp",
            "Enemy HP",
            format::value(odds.hp),
            format!("Enemy number {}", odds.wave + 1),
            cx,
        ))
        .child(metric_card(
            "monument-odds-damage",
            "This roll damage",
            format!(
                "{} - {}",
                format::value(odds.stats.min_dmg),
                format::value(odds.stats.max_dmg)
            ),
            "Per sword; based on the entered run state".into(),
            cx,
        ))
        .child(metric_card(
            "monument-odds-kill",
            "Fresh full-volley kill",
            format::percent(odds.kill),
            "Independent rolls; before any rethrows".into(),
            cx,
        ))
        .child(metric_card(
            "monument-odds-boost",
            "Right-most finishing / boost",
            format::percent(odds.boost_chance),
            boost_sub.into(),
            cx,
        ));
    if let Some(chance) = odds.last_sword_chance {
        cards = cards.child(metric_card(
            "monument-odds-last",
            "One last-sword kill",
            format::percent(chance),
            "Conditional on the HP entered".into(),
            cx,
        ));
    }
    cards = cards.child(metric_card(
        "monument-odds-next",
        "Next stack damage gain",
        format!("+{:.2}%", (odds.next_damage_factor - 1.0) * 100.0),
        "10 percentage points added to the boost factor".into(),
        cx,
    ));
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(cards)
        .child(Caption::new(
            "Boost odds exclude early kills. They assume the right-most sword lands strictly later, not simultaneous animation completions. Existing stacks multiply maximum damage by 1 + 0.1 × stacks.",
        ))
        .into_any_element()
}

fn milestones_card(snapshot: &Snapshot) -> AnyElement {
    let mut table = Table::new([
        "Milestone",
        "Training hours required",
        "Saved status",
        "One-character ETA",
    ]);
    for (index, row) in snapshot.milestones.iter().enumerate() {
        table = table.row(milestone_row(snapshot, index, row));
    }
    Card::new()
        .header(CardHeader::new("Paying Respect milestones"))
        .child(table)
        .into_any_element()
}

fn milestone_row(snapshot: &Snapshot, index: usize, row: &Milestone) -> [AnyElement; 4] {
    let mut text = clean_text(&row.text);
    if index == 0 {
        text = text
            .replace('{', &format::value(snapshot.stats.min_dmg))
            .replace('}', &format::value(snapshot.stats.max_dmg));
    }
    [
        div().min_w_0().child(text).into_any_element(),
        div().child(format::value(row.hours)).into_any_element(),
        div().child(milestone_status(row)).into_any_element(),
        div().child(milestone_eta(row)).into_any_element(),
    ]
}

fn milestone_status(row: &Milestone) -> &'static str {
    if row.unlocked {
        "Unlocked"
    } else if row.ready {
        "Ready: unlock in game"
    } else {
        "Not reached"
    }
}

fn milestone_eta(row: &Milestone) -> String {
    if row.unlocked || row.ready {
        "-".to_string()
    } else {
        format!("{} real hours", format::value(row.real_hours))
    }
}

fn bonuses_card(snapshot: &Snapshot) -> AnyElement {
    let mut table = Table::new([
        "Reward",
        "Saved level",
        "Current effect",
        "Effect at +1 level",
    ]);
    for row in &snapshot.bonuses {
        table = table.row(bonus_row(row));
    }
    Card::new()
        .header(CardHeader::new("Permanent reward levels & total effects"))
        .child(table)
        .into_any_element()
}

fn bonus_row(row: &BonusRow) -> [AnyElement; 4] {
    let text = clean_text(&row.text);
    [
        div().min_w_0().child(bonus_label(&text)).into_any_element(),
        div().child(format::value(row.level)).into_any_element(),
        div()
            .child(effect_text(&text, row.value))
            .into_any_element(),
        div().child(effect_text(&text, row.next)).into_any_element(),
    ]
}

fn bonus_label(text: &str) -> String {
    let mut label: String = text
        .chars()
        .filter(|character| !matches!(character, '+' | '{' | '}' | '%'))
        .collect();
    label = label.trim().to_string();
    if let Some(rest) = label.strip_prefix("x ") {
        label = rest.trim().to_string();
    }
    label
}

fn effect_text(text: &str, value: f64) -> String {
    text.replace('{', &format::value(value))
        .replace('}', &format::value(1.0 + value / 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn milestone(unlocked: bool, ready: bool, real_hours: f64) -> Milestone {
        Milestone {
            text: String::new(),
            hours: 0.0,
            unlocked,
            ready,
            hours_needed: 0.0,
            real_hours,
        }
    }

    #[test]
    fn reward_labels_and_effects_follow_the_upstream_cleanup() {
        assert_eq!(bonus_label("+{% Mining Efficiency"), "Mining Efficiency");
        assert_eq!(bonus_label("x Bucket Fill Rate"), "Bucket Fill Rate");
        assert_eq!(
            bonus_label("Chance for Bravery Opals"),
            "Chance for Bravery Opals"
        );
        assert_eq!(
            effect_text("+{% Mining Efficiency", 63_443.52),
            "+63.4K% Mining Efficiency"
        );
        assert_eq!(
            effect_text("}x Bucket Fill Rate", 108_812.16),
            "1.1Kx Bucket Fill Rate"
        );
    }

    #[test]
    fn milestone_status_and_eta_read_the_saved_row() {
        assert_eq!(milestone_status(&milestone(true, true, 0.0)), "Unlocked");
        assert_eq!(
            milestone_status(&milestone(false, true, 0.0)),
            "Ready: unlock in game"
        );
        assert_eq!(
            milestone_status(&milestone(false, false, 12.0)),
            "Not reached"
        );
        assert_eq!(milestone_eta(&milestone(true, true, 0.0)), "-");
        assert_eq!(
            milestone_eta(&milestone(false, false, 12.0)),
            "12.0 real hours"
        );
    }
}
