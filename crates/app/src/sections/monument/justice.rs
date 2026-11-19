//! The Justice tab: the saved start, the undoable worksheet, the case search and
//! the response advisor.
//!
//! The advisor is a local worksheet, not a live-game connection. The player
//! records the case they actually received and the outcome that happened; the
//! tab never predicts random visitors and never changes the save.

use super::bravery::labelled;
use super::table::{Col, table, text};
use super::{
    Metric, MilestoneFirst, bonuses_card, clean_text, empty_card, error_line, metrics,
    milestones_card, notes_card, scroll_pane, unavailable_card,
};
use crate::state::AppState;
use crate::ui::number_field;
use gpui::Focusable as _;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div, px};
use gpui_kit::component::button::Button;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_monument::JusticeSnapshot;
pub(crate) use idleon_monument::justice::{Action, Effect};
use idleon_monument::justice::{Response, State};
use idleon_ui::format;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, TEXT_CAPTION, TEXT_MONO, search_input, show_number,
};

/// One of the worksheet's six stats, in worksheet order.
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

    /// The stat's label, as the worksheet names it.
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
    /// the round plus one, as the worksheet shows it.
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
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let body = match &state.monument_controls.derived.justice {
        Some(snapshot) if snapshot.available => loaded(state, snapshot, window, cx),
        Some(snapshot) => unavailable_card("Justice", &snapshot.notes, cx),
        None => empty_card(
            "Justice",
            "Import a decoded save to read the Justice start, milestones, rewards and the response advisor.",
            cx,
        ),
    };
    scroll_pane(
        "monument-pane-justice",
        "monument-justice-scroll",
        v_flex().id("monument-justice-body").gap_3().child(body),
    )
}

fn loaded(
    state: &AppState,
    snapshot: &JusticeSnapshot,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let stats = &snapshot.stats;
    let cards = metrics(
        "monument-justice-stats",
        vec![
            Metric::new(
                "monument-justice-hours",
                "Saved training",
                format!("{} hours", format::value(stats.hours)),
                "Paying Respect; the story start reads it",
            ),
            Metric::new(
                "monument-justice-milestones",
                "Unlocked milestones",
                format!("{} / 8", stats.milestones),
                "Each adds coins, health, popularity or dismissals",
            ),
            Metric::new(
                "monument-justice-opal",
                "Next opal offer chance",
                format::percent(stats.opal_chance),
                "Uses saved opals; Justice study raises it",
            ),
            Metric::new(
                "monument-justice-start",
                "Fresh-story start",
                format!("{} coins", format::value(stats.coins as f64)),
                format!(
                    "{} health · {} popularity · {} dismissals. A normal story; practice mode has no Justice",
                    format::value(stats.health as f64),
                    format::value(stats.popularity as f64),
                    format::value(stats.dismissals as f64)
                ),
            ),
        ],
    );
    let mut column = v_flex().gap_3().child(cards);
    if !snapshot.notes.is_empty() {
        column = column.child(notes_card("monument-justice-notes", &snapshot.notes, cx));
    }
    column
        .child(worksheet_card(state, window, cx))
        .child(effects_card(state, window, cx))
        .child(advisor_card(state, window, cx))
        .child(delayed_card(state, cx))
        .child(milestones_card(
            &snapshot.milestones,
            MilestoneFirst::Coins(stats.coins as f64),
            cx,
        ))
        .child(bonuses_card(&snapshot.bonuses, cx))
        .into_any_element()
}

/// The six stats and the worksheet's reset, undo and next-case buttons.
fn worksheet_card(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let worksheet = &state.monument.worksheet;
    let can_undo = worksheet.can_undo();
    let awaiting = worksheet.awaiting && worksheet.state.health > 0;
    let inputs = &state.monument_controls.fields;
    for (field, input) in Field::ALL.into_iter().zip(inputs) {
        show_number(input, field.value(&worksheet.state), 0, window, cx);
    }
    // Two rows of three that never wrap (`docs/ui-kit/display.md`).
    let fields = v_flex()
        .id("monument-justice-fields")
        .gap_3()
        .children(Field::ALL.chunks(3).map(|row| {
            h_flex().gap_3().children(row.iter().map(|field| {
                let index = Field::ALL
                    .iter()
                    .position(|each| each == field)
                    .unwrap_or(0);
                labelled(
                    field.label(),
                    number_field(
                        format!("monument-justice-{}", field.id()),
                        &inputs[index],
                        field.label(),
                    )
                    .w(px(160.0)),
                )
            }))
        }));

    let reset = cx.entity();
    let undo = cx.entity();
    let next = cx.entity();
    let mut buttons = h_flex()
        .gap_2()
        .child(
            Button::new("monument-justice-reset")
                .label("Fresh story from save")
                .small()
                .on_click(move |_event, _window, cx| {
                    reset.update(cx, |state, cx| state.reset_monument_justice(cx));
                }),
        )
        .child(
            Button::new("monument-justice-undo")
                .label("Undo worksheet step")
                .small()
                .disabled(!can_undo)
                .on_click(move |_event, _window, cx| {
                    undo.update(cx, |state, cx| state.undo_monument_justice(cx));
                }),
        );
    if awaiting {
        buttons = buttons.child(
            Button::new("monument-justice-next")
                .label("Open selected next case")
                .small()
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
            v_flex()
                .id("monument-justice-controls")
                .gap_3()
                .child(buttons)
                .child(fields)
                .child(Caption::new(
                    "Recording a response, opening the next case and every stat edit are each one undoable step. A typed stat applies on Enter or when the field loses focus.",
                )),
        )
        .into_any_element()
}

/// The deals, promises and investments the worksheet carries, four to a row.
fn effects_card(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let effects = &state.monument.worksheet.state.effects;
    let inputs = &state.monument_controls.effects;
    let mut rows = v_flex().id("monument-justice-effects").gap_3();
    for (chunk, row_inputs) in Effect::ALL.chunks(4).zip(inputs.chunks(4)) {
        let mut row = h_flex().items_start().gap_3();
        for (effect, input) in chunk.iter().copied().zip(row_inputs) {
            let entity = cx.entity();
            let active = effects.get(&effect).copied();
            let mut cell = v_flex()
                .w(px(240.0))
                .gap_1()
                .child(
                    Checkbox::new(format!("monument-justice-effect-{}", effect.key()))
                        .label(effect.name())
                        .checked(active.is_some())
                        .on_click(move |on, _window, cx| {
                            let on = *on;
                            entity.update(cx, |state, cx| {
                                state.toggle_monument_justice_effect(effect, on, cx);
                            });
                        }),
                )
                .child(Caption::new(effect.help()).text_color(muted));
            if let Some(value) = active
                && !effect.flag()
            {
                show_number(input, value as f64, 0, window, cx);
                cell = cell.child(
                    number_field(
                        format!("monument-justice-effect-value-{}", effect.key()),
                        input,
                        effect.name(),
                    )
                    .w(px(160.0)),
                );
            }
            row = row.child(cell);
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
fn advisor_card(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let search_focus = state.monument_search.focus_handle(cx);
    let worksheet = &state.monument.worksheet;
    let derived = &state.monument_controls.derived;
    let select = &state.monument_controls.case;
    select.show(derived.cases.clone(), Some(&worksheet.case_id), window, cx);

    let mut body = v_flex().id("monument-justice-advisor").gap_3().child(
        h_flex()
            .items_end()
            .gap_3()
            .child(labelled(
                "Find case",
                div()
                    .id("monument-justice-search")
                    .role(Role::Search)
                    .aria_label("Find a Justice case")
                    .w(px(320.0))
                    .on_click(move |_event, window, cx| {
                        window.focus(&search_focus, cx);
                    })
                    .child(
                        search_input(&state.monument_search)
                            .aria_label("Find a Justice case")
                            .small(),
                    ),
            ))
            .child(labelled(
                "Observed case",
                div().flex_none().w(px(420.0)).child(
                    Select::new(select.state())
                        .id("monument-justice-case")
                        .accessibility_label("Observed case")
                        .small(),
                ),
            ))
            .child(
                div()
                    .id("monument-justice-match-count")
                    .role(Role::Label)
                    .aria_label(derived.case_count.clone())
                    .text_size(TEXT_MONO)
                    .text_color(muted)
                    .child(derived.case_count.clone()),
            ),
    );

    let notice = (!state.monument.notice.is_empty()).then(|| notice_line(state, cx));
    match &derived.report {
        Some(Ok(report)) => {
            body = body.child(
                div()
                    .id("monument-justice-request")
                    .role(Role::Label)
                    .aria_label(format!("{}: {}", report.row.name, report.row.text))
                    .child(format!(
                        "{}: {}",
                        report.row.name,
                        clean_text(&report.row.text)
                    )),
            );
            body = body.children(notice);
            let responses = h_flex()
                .id("monument-justice-responses")
                .items_start()
                .gap_3()
                .children(report.responses.iter().map(|response| {
                    response_card(&worksheet.state, response, report.recommended, cx)
                }));
            let heuristic = format!(
                "{}{}",
                report.heuristic,
                if report.ambiguous {
                    " Grumblo uses identical wording for hidden gifts. These outcomes use their equally likely native case variants; record only the actual result."
                } else {
                    ""
                }
            );
            body = body.child(responses).child(
                div()
                    .id("monument-justice-heuristic")
                    .role(Role::Label)
                    .aria_label(heuristic.clone())
                    .text_size(TEXT_MONO)
                    .text_color(muted)
                    .child(heuristic),
            );
        }
        Some(Err(error)) => {
            body = body.children(notice).child(error_line(
                "monument-justice-error",
                error.to_string(),
                cx,
            ));
        }
        None => {}
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
    let muted = cx.theme().muted_foreground;
    div()
        .id("monument-justice-notice")
        .role(Role::Label)
        .aria_label(state.monument.notice.clone())
        .text_size(TEXT_MONO)
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
    let theme = cx.theme();
    let (border, accent, danger, muted) = (
        theme.border,
        theme.primary,
        theme.danger,
        theme.muted_foreground,
    );
    let radius = theme.radius;
    let suggested = recommended == Some(response.action);
    let mut card = v_flex()
        .id(format!(
            "monument-justice-response-{}",
            response.action.id()
        ))
        .flex_1()
        .min_w(px(260.0))
        .gap_1()
        .p_2()
        .rounded(radius)
        .border_1()
        .border_color(if suggested { accent } else { border })
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
                .text_size(TEXT_CAPTION)
                .text_color(accent)
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
                    .text_color(danger)
                    .child(reason.clone()),
            );
        }
        None => {
            let survival = format!(
                "Survive this response: {}",
                format::percent(response.survival)
            );
            card = card.child(
                div()
                    .id(format!(
                        "monument-justice-survival-{}",
                        response.action.id()
                    ))
                    .role(Role::Label)
                    .aria_label(survival.clone())
                    .text_size(TEXT_MONO)
                    .text_color(muted)
                    .child(survival),
            );
            let mut outcomes = v_flex().gap_2();
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
                let label = if response.outcomes.len() > 1 {
                    format!("Record {} - {}", action.label(), outcome.label)
                } else {
                    format!("Record {}", action.label())
                };
                outcomes = outcomes.child(
                    v_flex()
                        .gap_1()
                        .pt_2()
                        .border_t_1()
                        .border_color(border)
                        .text_size(TEXT_MONO)
                        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(format!(
                            "{} - {}",
                            outcome.label,
                            format::percent(outcome.probability)
                        )))
                        .child(div().child(changes.join(" / ")))
                        .when(outcome.state.health <= 0, |card| {
                            card.child(div().text_color(danger).child(
                                "Story ends; existing retirement chests remain available to open.",
                            ))
                        })
                        .child(
                            div()
                                .text_color(muted)
                                .child(effect_changes(current, &outcome.state)),
                        )
                        .child(
                            h_flex().child(
                                Button::new(format!(
                                    "monument-justice-record-{}-{index}",
                                    action.id()
                                ))
                                .label(label)
                                .small()
                                .on_click(
                                    move |_event, _window, cx| {
                                        entity.update(cx, |state, cx| {
                                            state.record_monument_justice(action, index, cx);
                                        });
                                    },
                                ),
                            ),
                        ),
                );
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
fn delayed_card(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let effects = &state.monument_controls.derived.delayed;
    let body = if effects.is_empty() {
        div()
            .id("monument-justice-delayed")
            .role(Role::Label)
            .aria_label("No active deals or investments recorded.")
            .child(Caption::new("No active deals or investments recorded."))
            .into_any_element()
    } else {
        let names: Vec<&str> = effects.iter().map(|effect| effect.name).collect();
        let cols = [
            Col::Text("Effect"),
            Col::Number("Value"),
            Col::Text("When / consequence"),
        ];
        let cells = effects.iter().map(|effect| {
            vec![
                text(effect.name),
                text(format::value(effect.value as f64)),
                text(effect.help),
            ]
        });
        div()
            .id("monument-justice-delayed")
            .role(Role::Label)
            .aria_label(format!("{} active: {}", names.len(), names.join(", ")))
            .child(table("monument-delayed", &cols, cells, cx))
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
