//! The Ladles tab: the panel inputs, the summary figures, the character rates and
//! the breakdowns.

use super::controls::PANEL;
use super::table::{Col, blank, table, text};
use crate::state::AppState;
use crate::ui::number_field;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use idleon_ladle::{self as ladle, Ladle, Panel, Stats};
use idleon_ui::format;
use idleon_ui::widgets::{
    Callout, Caption, Card, CardHeader, KpiStrip, Statistic, Tone, show_number,
};

/// The header's one-line summary.
pub(crate) fn line(ladle: &Ladle, stats: &[Stats], assigned: &[usize]) -> String {
    let guaranteed = total(stats, assigned, |row| row.min_per_day);
    let expected = total(stats, assigned, |row| row.per_day);
    let reset = match ladle.reset_hours {
        Some(hours) => format!("{} to reset", format::hours(hours)),
        None => "reset countdown missing".into(),
    };
    format!(
        "{} ladles owned · {} guaranteed / day · {} expected · {reset}",
        format::number(ladle.ladles.value),
        format::number(guaranteed),
        format::number(expected),
    )
}

/// The whole Ladles tab.
pub(crate) fn pane(
    state: &AppState,
    ladle: &Ladle,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let stats = &state.ladle_state.derived.stats;
    div()
        .id("ladle-ladles-pane")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .gap_3()
                .child(panel_card(state, window, cx))
                .child(cards(state, ladle, stats))
                .child(characters(state, ladle, stats, cx))
                .child(super::breakdowns::ladles(state, stats))
                .overflow_y_scrollbar()
                .id("ladle-ladles-scroll"),
        )
        .into_any_element()
}

/// The stat-engine values the section asks for until the shared engine lands.
fn panel_card(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let mut panel = state.ladle_state.panel;
    let fields = &state.ladle_state.controls.panel;
    let mut controls = div().id("ladle-panel").flex().gap_3();
    for (index, spec) in PANEL.iter().enumerate() {
        let value = *super::controls::panel_value(&mut panel, index);
        show_number(&fields[index], value, spec.precision, window, cx);
        controls = controls.child(super::field(spec.label).child(number_field(
            format!("ladle-panel-{}", spec.id),
            &fields[index],
            spec.label,
        )));
    }
    let note = Callout::new(Tone::Info, "AFK panel values").child(Caption::new(
        "Cooking efficiency, the AFK gain rate, the EXP multiplier, skill prowess and Talent 148 come \
         from the game's Cooking panel: the shared account stats engine is not built yet, so they are \
         entered here and apply to every character. The values you read in game give the exact \
         rates; the neutral defaults are a lower bound.",
    ));
    div()
        .id("ladle-panel-card")
        .flex()
        .flex_col()
        .gap_3()
        .child(controls)
        .child(note)
        .into_any_element()
}

/// The summary figures.
fn cards(state: &AppState, ladle: &Ladle, stats: &[Stats]) -> AnyElement {
    let ladle_state = &state.ladle_state;
    let selected = &ladle_state.assigned;
    let guaranteed = total(stats, selected, |row| row.min_per_day);
    let expected = total(stats, selected, |row| row.per_day);
    let checked = selected.len();
    let panel: Panel = ladle_state.panel;
    let owned = match ladle.ladles.partial {
        true => format!("{} · partial", format::number(ladle.ladles.value)),
        false => format::number(ladle.ladles.value),
    };
    let reset_hours = ladle.reset_hours.unwrap_or(24.0);
    let reset_caption = match ladle.reset_hours {
        Some(_) => "Claim the ladles before it; the reset applies NMLB",
        None => "The save has no reset countdown; assuming a day",
    };
    let strip = KpiStrip::new()
        .child(
            Statistic::new("ladle-kpi-owned", "Ladles owned", count(ladle.ladles.value))
                .caption(owned),
        )
        .child(
            Statistic::new(
                "ladle-kpi-guaranteed",
                "Guaranteed ladles / day",
                count(guaranteed),
            )
            .caption(format!(
                "{checked} checked characters · claims may give ~{} with the 19c roll",
                format::number(expected)
            )),
        )
        .child(
            Statistic::new("ladle-kpi-reset", "Daily reset", figure(reset_hours, 1))
                .suffix("h")
                .caption(reset_caption),
        )
        .child(
            Statistic::new(
                "ladle-kpi-hours",
                "Ladle hours",
                figure(panel.ladle_hours(), 3),
            )
            .suffix("×")
            .caption(format!(
                "Talent 148 Overflowing Ladle {}%",
                format::number(panel.talent_148)
            )),
        )
        .child(
            Statistic::new(
                "ladle-kpi-prowess",
                "Skill prowess",
                figure(panel.prowess, 3),
            )
            .caption("Account-wide · ladle exponent 0.25 + min(0.1, prowess)"),
        );
    let mut row = div()
        .id("ladle-cards")
        .flex()
        .flex_col()
        .gap_2()
        .child(strip);
    if ladle.reset_hours.is_none() {
        row = row.child(
            div()
                .id("ladle-reset-note")
                .child(tag("assumed", Tone::Warning)),
        );
    }
    row.into_any_element()
}

/// The character table: who counts, and what each row's rates are.
fn characters(
    state: &AppState,
    ladle: &Ladle,
    stats: &[Stats],
    cx: &mut Context<AppState>,
) -> AnyElement {
    let ladle_state = &state.ladle_state;
    let cols = [
        Col::Text("Use"),
        Col::Text("Character"),
        Col::Number("Cook Lv"),
        Col::Number("Efficiency"),
        Col::Number("AFK rate"),
        Col::Number("Ladle multi"),
        Col::Number("Panel / day"),
        Col::Number("Guaranteed / day"),
        Col::Number("Expected / day"),
        Col::Number("/ hr"),
        Col::Text("Next gain"),
    ];
    let mut rows: Vec<Vec<AnyElement>> = stats
        .iter()
        .map(|row| {
            let index = row.index;
            let entity = cx.entity();
            let use_cell = Checkbox::new(("ladle-use", index))
                // The name sits in the next column; the label is for the bridge
                // and screen readers.
                .accessibility_label(format!("Count {}", row.name))
                .checked(ladle_state.assigned.contains(&index))
                .on_click(move |_, _window, cx| {
                    entity.update(cx, |state, cx| {
                        state.ladle_state.toggle_assigned(index);
                        cx.notify();
                    });
                });
            let mut name = div().flex().items_center().gap_2().child(row.name.clone());
            if row.cooking {
                name = name.child(tag("cooking", Tone::Success));
            }
            let mut guaranteed = div().flex().flex_col().items_end().child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(format::number(row.min_per_day))
                    .when(row.claim_capped, |cell| {
                        cell.child(tag("10h cap", Tone::Danger))
                    }),
            );
            if row.pending_known {
                guaranteed = guaranteed.child(Caption::new(format!(
                    "{} pending",
                    format::hours(row.pending_hours)
                )));
            }
            vec![
                use_cell.into_any_element(),
                name.into_any_element(),
                text(format::number(row.level)),
                text(format::number(row.efficiency)),
                text(format!("×{:.2}", row.afk)),
                text(format!("×{}", format::number(row.ladle_multi))),
                text(format::number(row.panel_per_day)),
                guaranteed.into_any_element(),
                text(format::number(row.per_day)),
                text(format::number(row.per_hour)),
                text(row.outlook_label()),
            ]
        })
        .collect();
    let selected = &ladle_state.assigned;
    let guaranteed = total(stats, selected, |row| row.min_per_day);
    let expected = total(stats, selected, |row| row.per_day);
    rows.push(vec![
        blank(),
        text(format!("Checked total ({} characters)", selected.len())),
        blank(),
        blank(),
        blank(),
        blank(),
        blank(),
        text(format::number(guaranteed)),
        text(format::number(expected)),
        text(format::number(expected / 24.0)),
        blank(),
    ]);
    let note = match ladle.reset_hours {
        Some(hours) => format!(
            "Each claim may give more (Post Office 19c doubling), so expect extra ladles roughly half the time. \
             Next reset in {}.",
            format::hours(hours),
        ),
        None => "Each claim may give more (Post Office 19c doubling), so expect extra ladles roughly half the \
                 time. The save carries no reset countdown."
            .to_owned(),
    };
    Card::new()
        .header(CardHeader::new("Characters").description(
            "Check the characters AFK-ing Cooking to count their ladles · rates assume Cooking as the AFK target",
        ))
        .child(
            div()
                .id("ladle-characters")
                .role(Role::Label)
                .aria_label(format!(
                    "{} characters, {} checked, {} guaranteed ladles a day",
                    stats.len(),
                    selected.len(),
                    format::number(guaranteed)
                ))
                .child(table("ladle-characters-table", &cols, rows, cx)),
        )
        .child(Caption::new(note))
        .into_any_element()
}

/// A figure with no decimals, grouped by thousands.
pub(super) fn count(value: f64) -> String {
    figure(value, 0)
}

/// A figure with `decimals` decimals, its whole part grouped by thousands;
/// past `u64` it falls back to the short form.
pub(super) fn figure(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return "—".into();
    }
    let text = format!("{:.decimals$}", value.abs());
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let Ok(whole) = whole.parse::<u64>() else {
        return format::number(value);
    };
    let sign = if value < 0.0 && text.chars().any(|digit| ('1'..='9').contains(&digit)) {
        "-"
    } else {
        ""
    };
    match fraction.is_empty() {
        true => format!("{sign}{}", format::count(whole)),
        false => format!("{sign}{}.{fraction}", format::count(whole)),
    }
}

/// A small outlined tag.
pub(super) fn tag(label: impl Into<gpui::SharedString>, tone: Tone) -> Tag {
    Tag::new()
        .with_variant(tone.into())
        .outline()
        .small()
        .child(label.into())
}

/// Every character's rates for the panel the section holds.
pub(crate) fn all(ladle: &Ladle, panel: Panel, claim_hours: f64) -> Vec<Stats> {
    ladle
        .characters
        .iter()
        .map(|character| ladle::stats(character, panel, claim_hours))
        .collect()
}

/// Sum one column over the checked characters.
pub(crate) fn total(stats: &[Stats], selected: &[usize], column: impl Fn(&Stats) -> f64) -> f64 {
    stats
        .iter()
        .filter(|row| selected.contains(&row.index))
        .map(column)
        .sum()
}
