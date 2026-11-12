//! The Justice tab: the saved start, the undoable worksheet, the case search and
//! the response advisor.
//!
//! The advisor is a local worksheet, not a live-game connection. The player
//! records the case they actually received and the outcome that happened; the
//! tab never predicts random visitors and never changes the save.

use super::{
    MilestoneFirst, bonuses_card, clean_text, empty_card, error_line, metric_card, milestones_card,
    notes_card, unavailable_card,
};
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::forms::{Checkbox, Choice, NumberInput, SearchInput, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, div, px};
use idleon_monument::JusticeSnapshot;
use idleon_monument::justice::{self, Response, State};
pub(crate) use idleon_monument::justice::{Action, Effect};
use idleon_ui::format;

/// One of the worksheet's six stats, in the upstream order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Round,
    Health,
    Coins,
    Popularity,
    Dismissals,
    Chests,
}

impl Field {
    pub(crate) const ALL: [Field; 6] = [
        Field::Round,
        Field::Health,
        Field::Coins,
        Field::Popularity,
        Field::Dismissals,
        Field::Chests,
    ];

    /// The element id suffix and the editor order key.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Field::Round => "round",
            Field::Health => "health",
            Field::Coins => "coins",
            Field::Popularity => "popularity",
            Field::Dismissals => "dismissals",
            Field::Chests => "chests",
        }
    }

    /// The stat's label, as the upstream worksheet names it.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Field::Round => "Case number",
            Field::Health => "Mental health",
            Field::Coins => "Coins",
            Field::Popularity => "Popularity",
            Field::Dismissals => "Dismissals",
            Field::Chests => "Retirement chests",
        }
    }

    /// The value the editor shows; the case number is the round plus one.
    pub(crate) fn value(self, state: &State) -> f64 {
        match self {
            Field::Round => (state.round + 1) as f64,
            Field::Health => state.health as f64,
            Field::Coins => state.coins as f64,
            Field::Popularity => state.popularity as f64,
            Field::Dismissals => state.dismissals as f64,
            Field::Chests => state.chests as f64,
        }
    }

    /// Writes a typed value, clamped the way the input is; the case number is
    /// the round plus one, as upstream shows it.
    pub(crate) fn set(self, state: &mut State, value: f64) {
        match self {
            Field::Round => state.round = (value.round() - 1.0).max(0.0) as i64,
            Field::Popularity => state.popularity = value.round() as i64,
            Field::Health => state.health = value.round().max(0.0) as i64,
            Field::Coins => state.coins = value.round().max(0.0) as i64,
            Field::Dismissals => state.dismissals = value.round().max(0.0) as i64,
            Field::Chests => state.chests = value.round().max(0.0) as i64,
        }
    }
}

/// The Justice pane.
pub(crate) fn pane(
    state: &mut AppState,
    snapshot: Option<&JusticeSnapshot>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let body = match snapshot {
        Some(snapshot) if snapshot.available => loaded(state, snapshot, cx),
        Some(snapshot) => unavailable_card("Justice", &snapshot.notes, cx),
        None => empty_card(
            "Justice",
            "Import a decoded save to read the Justice start, milestones, rewards and the response advisor.",
            cx,
        ),
    };
    div()
        .id("monument-pane-justice")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            ScrollArea::new("monument-justice-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .id("monument-justice-body")
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(body),
                ),
        )
        .into_any_element()
}

fn loaded(
    state: &mut AppState,
    snapshot: &JusticeSnapshot,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let stats = &snapshot.stats;
    let cards = div()
        .id("monument-justice-stats")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(metric_card(
            "monument-justice-hours",
            "Saved training",
            format!("{} hours", format::value(stats.hours)),
            "Paying Respect; the story start reads it".into(),
            cx,
        ))
        .child(metric_card(
            "monument-justice-milestones",
            "Unlocked milestones",
            format!("{} / 8", stats.milestones),
            "Each adds coins, health, popularity or dismissals".into(),
            cx,
        ))
        .child(metric_card(
            "monument-justice-opal",
            "Next opal offer chance",
            format::percent(stats.opal_chance),
            "Uses saved opals; Justice study raises it".into(),
            cx,
        ))
        .child(metric_card(
            "monument-justice-start",
            "Fresh-story start",
            format!(
                "{} coins · {} health · {} popularity · {} dismissals",
                format::value(stats.coins as f64),
                format::value(stats.health as f64),
                format::value(stats.popularity as f64),
                format::value(stats.dismissals as f64)
            ),
            "A normal story; practice mode has no Justice".into(),
            cx,
        ));
    let mut column = div().flex().flex_col().gap_3().child(cards);
    if !snapshot.notes.is_empty() {
        column = column.child(notes_card("monument-justice-notes", &snapshot.notes, cx));
    }
    column
        .child(worksheet_card(state, cx))
        .child(effects_card(state, cx))
        .child(advisor_card(state, cx))
        .child(delayed_card(state))
        .child(milestones_card(
            &snapshot.milestones,
            MilestoneFirst::Coins(stats.coins as f64),
        ))
        .child(bonuses_card(&snapshot.bonuses))
        .into_any_element()
}

/// The six stats and the worksheet's reset, undo and next-case buttons.
fn worksheet_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let worksheet = &state.monument.worksheet;
    let can_undo = worksheet.can_undo();
    let awaiting = worksheet.awaiting && worksheet.state.health > 0;
    let entity = cx.entity();
    let mut fields = div()
        .id("monument-justice-fields")
        .flex()
        .flex_wrap()
        .gap_3();
    for field in Field::ALL {
        let entity = entity.clone();
        let value = field.value(&state.monument.worksheet.state);
        let range = if field == Field::Popularity {
            (-1.0e9, 1.0e9)
        } else if field == Field::Round {
            (1.0, 1.0e9)
        } else {
            (0.0, 1.0e9)
        };
        fields = fields.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new(field.label()))
                .child(
                    NumberInput::new(format!("monument-justice-{}", field.id()), value)
                        .range(range.0, range.1)
                        .precision(0)
                        .label(field.label())
                        .on_commit(move |value, _window, cx| {
                            entity.update(cx, |state, cx| {
                                state.set_monument_justice_field(field, value, cx);
                            });
                        }),
                ),
        );
    }

    let reset = cx.entity();
    let undo = cx.entity();
    let next = cx.entity();
    let mut buttons = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(
            Button::new("monument-justice-reset", "Fresh story from save")
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    reset.update(cx, |state, cx| state.reset_monument_justice(cx));
                }),
        )
        .child(
            Button::new("monument-justice-undo", "Undo worksheet step")
                .size(ControlSize::Sm)
                .disabled(!can_undo)
                .on_click(move |_event, _window, cx| {
                    undo.update(cx, |state, cx| state.undo_monument_justice(cx));
                }),
        );
    if awaiting {
        buttons = buttons.child(
            Button::new("monument-justice-next", "Open selected next case")
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    next.update(cx, |state, cx| state.open_monument_justice(cx));
                }),
        );
    }

    Card::new()
        .header(CardHeader::new("Justice worksheet").description(
            "Use the values after this visitor's arrival effects. Nothing here changes your save.",
        ))
        .child(
            div()
                .id("monument-justice-controls")
                .flex()
                .flex_col()
                .gap_3()
                .child(buttons)
                .child(fields)
                .child(Caption::new(
                    "Recording a response, opening the next case and every stat edit are each one undoable step.",
                )),
        )
        .into_any_element()
}

/// The deals, promises and investments the worksheet carries.
fn effects_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let mut rows = div()
        .id("monument-justice-effects")
        .flex()
        .flex_wrap()
        .gap_3();
    for effect in Effect::ALL {
        let entity = cx.entity();
        let active = state.monument.worksheet.state.effects.get(&effect).copied();
        let checked = active.is_some();
        let mut row = div()
            .w(px(240.0))
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div().flex().items_center().justify_between().gap_2().child(
                    Checkbox::new(format!("monument-justice-effect-{}", effect.key()), checked)
                        .label(effect.name())
                        .on_change(move |on, _window, cx| {
                            entity.update(cx, |state, cx| {
                                state.toggle_monument_justice_effect(effect, on, cx);
                            });
                        }),
                ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(cx.theme().colors.fg_muted)
                    .child(effect.help()),
            );
        if let Some(value) = active
            && !effect.flag()
        {
            let entity = cx.entity();
            row = row.child(
                NumberInput::new(
                    format!("monument-justice-effect-value-{}", effect.key()),
                    value as f64,
                )
                .range(0.0, 1.0e9)
                .precision(0)
                .label(effect.name())
                .on_commit(move |value, _window, cx| {
                    entity.update(cx, |state, cx| {
                        state.set_monument_justice_effect(effect, value, cx);
                    });
                }),
            );
        }
        rows = rows.child(row);
    }
    Card::new()
        .header(CardHeader::new("Active deals, promises & investments").description(
            "Unchecked effects are absent. A checked numeric effect may legitimately contain zero.",
        ))
        .child(rows)
        .into_any_element()
}

/// The search, the observed case, the three responses and the heuristic.
fn advisor_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let query = state.monument_search.read(cx).text().trim().to_lowercase();
    let search_focus = state.monument_search.read(cx).focus().clone();
    let worksheet = &state.monument.worksheet;
    let case_id = worksheet.case_id;
    let state_now = worksheet.state.clone();

    // The upstream search matches the visitor and the request wording, keeps the
    // selected case in the list, and de-duplicates rows with the same wording.
    let mut visible: Vec<usize> = Vec::new();
    for case in justice::cases() {
        let haystack = format!("{} {}", case.name, case.text).to_lowercase();
        if query.is_empty() || haystack.contains(&query) {
            let duplicate = visible.iter().any(|id| {
                justice::cases()
                    .get(*id)
                    .is_some_and(|kept| kept.visitor == case.visitor && kept.text == case.text)
            });
            if !duplicate {
                visible.push(case.id);
            }
        }
    }
    if !visible.contains(&case_id) {
        visible.insert(0, case_id);
    }
    let mut choices = Vec::with_capacity(visible.len());
    for id in &visible {
        let row =
            justice::case_text(&state_now, *id).unwrap_or_else(|_| justice::cases()[*id].clone());
        choices.push(Choice::new(
            id.to_string(),
            format!("{} - {}", row.name, clean_text(&row.text)),
        ));
    }
    let matching = visible.len();
    let searching = !query.is_empty();
    let count_label = if searching {
        format!("{matching} matching cases")
    } else {
        format!("{} cases", justice::cases().len())
    };
    let case_focus = entity.clone();
    let select = Select::new("monument-justice-case", choices)
        .selected(case_id.to_string())
        .on_change(move |value, _window, cx| {
            if let Ok(id) = value.as_ref().parse::<usize>() {
                case_focus.update(cx, |state, cx| state.select_monument_justice_case(id, cx));
            }
        });

    let mut body = div()
        .id("monument-justice-advisor")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Find case"))
                        .child(
                            div()
                                .id("monument-justice-search")
                                .role(Role::Search)
                                .aria_label("Find a Justice case")
                                .w(px(320.0))
                                .on_click(move |_event, window, cx| {
                                    window.focus(&search_focus, cx);
                                })
                                .child(
                                    SearchInput::new(
                                        "monument-justice-search-field",
                                        &state.monument_search,
                                    )
                                    .size(ControlSize::Sm),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Observed case"))
                        .child(select),
                )
                .child(
                    div()
                        .id("monument-justice-match-count")
                        .role(Role::Label)
                        .aria_label(count_label.clone())
                        .text_size(px(12.0))
                        .text_color(cx.theme().colors.fg_muted)
                        .child(count_label),
                ),
        );

    match justice::responses(&state_now, case_id) {
        Ok(report) => {
            body = body.child(
                div()
                    .id("monument-justice-request")
                    .role(Role::Label)
                    .aria_label(format!("{}: {}", report.row.name, report.row.text))
                    .child(div().child(format!(
                        "{}: {}",
                        report.row.name,
                        clean_text(&report.row.text)
                    ))),
            );
            if !state.monument.notice.is_empty() {
                body = body.child(notice_line(state, cx));
            }
            let mut responses = div()
                .id("monument-justice-responses")
                .flex()
                .flex_wrap()
                .gap_3();
            for response in &report.responses {
                responses =
                    responses.child(response_card(&state_now, response, report.recommended, cx));
            }
            body = body.child(responses).child(
                div()
                    .id("monument-justice-heuristic")
                    .role(Role::Label)
                    .aria_label(format!(
                        "{}{}",
                        report.heuristic,
                        if report.ambiguous {
                            " Grumblo uses identical wording for hidden gifts. These outcomes use their equally likely native case variants; record only the actual result."
                        } else {
                            ""
                        }
                    ))
                    .text_size(px(12.0))
                    .text_color(cx.theme().colors.fg_muted)
                    .child(format!(
                        "{}{}",
                        report.heuristic,
                        if report.ambiguous {
                            " Grumblo uses identical wording for hidden gifts. These outcomes use their equally likely native case variants; record only the actual result."
                        } else {
                            ""
                        }
                    )),
            );
        }
        Err(error) => {
            if !state.monument.notice.is_empty() {
                body = body.child(notice_line(state, cx));
            }
            body = body.child(error_line("monument-justice-error", error.to_string(), cx));
        }
    }
    Card::new()
        .header(CardHeader::new("Response advisor").description(
            "Select the case you actually received; the advisor does not predict random visitors.",
        ))
        .child(body)
        .into_any_element()
}

/// What the last step did, under the search.
fn notice_line(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    div()
        .id("monument-justice-notice")
        .role(Role::Label)
        .aria_label(state.monument.notice.clone())
        .text_size(px(12.0))
        .text_color(muted)
        .child(state.monument.notice.clone())
        .into_any_element()
}

fn response_card(
    current: &State,
    response: &Response,
    recommended: Option<Action>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let suggested = recommended == Some(response.action);
    let mut card = div()
        .id(format!(
            "monument-justice-response-{}",
            response.action.id()
        ))
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(260.0))
        .gap_1()
        .p_2()
        .rounded(px(6.0))
        .border_1()
        .border_color(if suggested {
            cx.theme().colors.accent
        } else {
            cx.theme().colors.border
        })
        .child(
            div()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(response.action.label()),
        );
    if suggested {
        card = card.child(
            div()
                .id("monument-justice-recommended")
                .role(Role::Label)
                .aria_label("Suggested: the survival-first heuristic picks this response")
                .text_size(px(11.0))
                .text_color(cx.theme().colors.accent)
                .child("Suggested — survival first, then the heuristically best state"),
        );
    }
    match &response.blocked {
        Some(reason) => {
            card = card.child(
                div()
                    .id(format!("monument-justice-blocked-{}", response.action.id()))
                    .role(Role::Label)
                    .aria_label(format!("{}: {reason}", response.action.label()))
                    .text_color(cx.theme().colors.danger)
                    .child(reason.clone()),
            );
        }
        None => {
            card = card.child(
                div()
                    .id(format!(
                        "monument-justice-survival-{}",
                        response.action.id()
                    ))
                    .role(Role::Label)
                    .aria_label(format!(
                        "Survive this response: {}",
                        format::percent(response.survival)
                    ))
                    .text_size(px(12.0))
                    .text_color(cx.theme().colors.fg_muted)
                    .child(format!(
                        "Survive this response: {}",
                        format::percent(response.survival)
                    )),
            );
            let mut outcomes = div().flex().flex_col().gap_2();
            for (index, outcome) in response.outcomes.iter().enumerate() {
                let entity = cx.entity();
                let action = response.action;
                let changes: Vec<String> = Field::ALL
                    .into_iter()
                    .filter(|field| *field != Field::Round)
                    .map(|field| {
                        let value = field.value(&outcome.state);
                        let change = value - field.value(current);
                        format!(
                            "{}: {} ({}{})",
                            field.label(),
                            format::value(value),
                            if change >= 0.0 { "+" } else { "" },
                            format::value(change)
                        )
                    })
                    .collect();
                let mut outcome_card = div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .pt_2()
                    .border_t_1()
                    .border_color(cx.theme().colors.border)
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(format!(
                                "{} - {}",
                                outcome.label,
                                format::percent(outcome.probability)
                            )),
                    )
                    .child(div().text_size(px(12.0)).child(changes.join(" / ")));
                if outcome.state.health <= 0 {
                    outcome_card = outcome_card.child(
                        div()
                            .text_size(px(12.0))
                            .text_color(cx.theme().colors.danger)
                            .child(
                                "Story ends; existing retirement chests remain available to open.",
                            ),
                    );
                }
                outcome_card = outcome_card.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().colors.fg_muted)
                        .child(effect_changes(current, &outcome.state)),
                );
                let label = if response.outcomes.len() > 1 {
                    format!("Record {} - {}", action.label(), outcome.label)
                } else {
                    format!("Record {}", action.label())
                };
                outcome_card = outcome_card.child(
                    Button::new(
                        format!("monument-justice-record-{}-{index}", action.id()),
                        label,
                    )
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        entity.update(cx, |state, cx| {
                            state.record_monument_justice(action, index, cx);
                        });
                    }),
                );
                outcomes = outcomes.child(outcome_card);
            }
            card = card.child(outcomes);
        }
    }
    card.into_any_element()
}

/// What changes in the deals and promises: gains, raises and endings.
fn effect_changes(current: &State, next: &State) -> String {
    let mut parts = Vec::new();
    for effect in Effect::ALL {
        let before = current.effects.get(&effect);
        let after = next.effects.get(&effect);
        if before == after {
            continue;
        }
        parts.push(match after {
            Some(value) => format!("{}: {value}. {}", effect.name(), effect.help()),
            None => format!("{} ends.", effect.name()),
        });
    }
    parts.join(" ")
}

/// The effects pending right now, and what settles them.
fn delayed_card(state: &mut AppState) -> AnyElement {
    let effects = justice::delayed_effects(&state.monument.worksheet.state);
    let body = if effects.is_empty() {
        div()
            .id("monument-justice-delayed")
            .role(Role::Label)
            .aria_label("No active deals or investments recorded.")
            .child(Caption::new("No active deals or investments recorded."))
            .into_any_element()
    } else {
        let names: Vec<&str> = effects.iter().map(|effect| effect.name).collect();
        let mut table = Table::new(["Effect", "Value", "When / consequence"]);
        for effect in &effects {
            table = table.row([
                div().min_w_0().child(effect.name).into_any_element(),
                div()
                    .child(format::value(effect.value as f64))
                    .into_any_element(),
                div().child(effect.help).into_any_element(),
            ]);
        }
        div()
            .id("monument-justice-delayed")
            .role(Role::Label)
            .aria_label(format!("{} active: {}", names.len(), names.join(", ")))
            .child(table)
            .into_any_element()
    };
    Card::new()
        .header(CardHeader::new("Delayed effects currently active"))
        .child(body)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_case_number_field_is_the_round_plus_one() {
        let mut state = State::default();
        Field::Round.set(&mut state, 3.0);
        assert_eq!(state.round, 2);
        assert_eq!(Field::Round.value(&state), 3.0);
        Field::Round.set(&mut state, 0.0);
        assert_eq!(state.round, 0, "the first case is the fresh story");
    }

    #[test]
    fn the_stat_fields_clamp_negative_input() {
        let mut state = State::default();
        Field::Health.set(&mut state, -4.0);
        Field::Coins.set(&mut state, 12.6);
        Field::Popularity.set(&mut state, -3.0);
        assert_eq!(state.health, 0);
        assert_eq!(state.coins, 13);
        assert_eq!(
            state.popularity, -3,
            "popularity is the one stat that can go negative"
        );
    }
}
