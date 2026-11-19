//! The Currency tab: the value table, the per-water multipliers, the desired
//! currency simulator and the next unlock tracker.
//!
//! The table starts from the save's desired currency and can simulate another
//! choice through the select and the simulator's rows; the save is never
//! written.

use super::controls::{Currency, cached};
use super::overview::{num, pct};
use super::table::{number, table, tag, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_kit::component::select::Select;
use gpui_kit::component::table::TableRow;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_fountain as fountain;
use idleon_fountain::{CurrencyRow, DesiredRow, Fountain, UnlockRow, WaterMultiplier};
use idleon_ui::widgets::{Caption, Card, CardHeader, Choice, PANEL_GAP, Tone};

/// The Currency pane: values, per-water multipliers, desired simulator, unlocks.
pub(super) fn currency_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let desired = state.fountain_desired();
    let Some(fountain) = state.fountain.as_ref() else {
        return div().into_any_element();
    };
    let tables = cached(
        &mut state.fountain_controls.currency,
        (state.save_revision, desired),
        || Currency {
            values: fountain.currency_rows(desired),
            waters: fountain.water_multipliers(),
            desired: fountain.desired_rows(desired),
            unlocks: fountain.next_unlocks(),
        },
    );
    let select = desired_select(state, fountain, desired, window, cx);
    v_flex()
        .id("fountain-currency")
        .gap(PANEL_GAP)
        .child(values(fountain, &tables.values, desired, select, cx))
        .child(water_multipliers(&tables.waters, cx))
        .child(desired_simulator(&tables.desired, cx))
        .child(unlocks(&tables.unlocks, cx))
        .into_any_element()
}

/// The desired currency the section simulates, or the save's own choice.
fn desired_select(
    state: &AppState,
    fountain: &Fountain,
    desired: usize,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let choices = std::iter::once(Choice::new(None, "Save's choice"))
        .chain(
            (0..fountain::CURRENCIES)
                .filter(|currency| fountain.currency_unlocked(*currency))
                .map(|currency| Choice::new(Some(currency), fountain::currency_label(currency))),
        )
        .collect();
    let saved = fountain.desired_currency();
    let selected = (desired != saved).then_some(desired);
    let select = &state.fountain_controls.currency_desired;
    select.show(choices, Some(&selected), window, cx);
    h_flex()
        .gap_2()
        .child(Caption::new("Desired currency"))
        .child(
            div().w(px(160.0)).child(
                Select::new(select.state())
                    .id("fountain-currency-desired")
                    .accessibility_label("Desired currency")
                    .small(),
            ),
        )
        .into_any_element()
}

/// The Currency Values table and the multipliers behind its totals.
fn values(
    fountain: &Fountain,
    rows: &[CurrencyRow],
    desired: usize,
    select: AnyElement,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let table = table(
        &[
            ("Currency", false),
            ("Base", true),
            ("Lucky Multi", true),
            ("Total", true),
            ("Full Stack", true),
            ("Royal Stack", true),
            ("Status", false),
            ("Create %", true),
        ],
        rows.iter().map(|row| {
            TableRow::new()
                .id(("fountain-currency-row", row.currency))
                .child(text(
                    h_flex()
                        .gap_1()
                        .child(fountain::currency_name(row.currency))
                        .when(row.currency == desired, |cell| {
                            cell.child(tag("Desired", Tone::Accent))
                        }),
                ))
                .child(number(num(row.base)))
                .child(number(if row.lucky > 1.0 {
                    format!("{:.2}×", row.lucky)
                } else {
                    "1.00×".to_owned()
                }))
                .child(number(bold(num(row.total))))
                .child(number(num(row.full_stack)))
                .child(number(num(row.royal_stack)))
                .child(text(status(row)))
                .child(number(if row.unlocked {
                    pct(row.create)
                } else {
                    "—".to_owned()
                }))
        }),
    );
    let saved = fountain.desired_currency();
    let caption = format!(
        "All multi {:.2}× · Royal {:.2}× · Desire {:.2}×{}",
        fountain.currency_all_multi(),
        fountain.effective_royal_multi(),
        fountain.desire_multi(),
        if desired == saved {
            " (the save's choice)".to_owned()
        } else {
            format!(" (simulating {})", fountain::currency_name(desired))
        }
    );
    Card::new()
        .header(
            CardHeader::new("Currency Values")
                .description(
                    "One coin at the simulated desired currency; every type shares the fills",
                )
                .action(select),
        )
        .child(
            v_flex()
                .id("fountain-currency-values")
                .gap_2()
                .child(table)
                .child(Caption::new(caption).text_color(muted))
                .child(
                    Caption::new(
                        "Locked currencies cannot appear yet; ignored ones are hidden with Turn \
                         and Push. Create % is the share of fills that makes each active type.",
                    )
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// The per-water POW-10 log table.
fn water_multipliers(rows: &[WaterMultiplier], cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let names = ["Mythril Transience", "Sharp Surrogate", "Rupie Monie"];
    let table = table(
        &[
            ("Water", false),
            ("Shared All Multi", true),
            ("Mythril Transience", true),
            ("Sharp Surrogate", true),
            ("Rupie Monie", true),
        ],
        rows.iter().map(|row| {
            let own = |water: usize| {
                number(if row.water == water {
                    format!("{:.4}×", row.own)
                } else {
                    "—".to_owned()
                })
            };
            TableRow::new()
                .id(("fountain-water-row", row.water))
                .child(text(fountain::water_names()[row.water].clone()))
                .child(number(format!("{}×", num(row.shared))))
                .child(own(0))
                .child(own(1))
                .child(own(2))
        }),
    );
    Card::new()
        .header(CardHeader::new("Per-Water Multipliers").description(
            "The shared currency multiplier, and the POW 10 log each water's currencies add",
        ))
        .child(
            v_flex()
                .id("fountain-water-multipliers")
                .gap_2()
                .child(table)
                .child(
                    Caption::new(format!(
                        "{} scale with POW 10 Mythril, Sharp Notes and Red Rupie at the \
                         Transience upgrades' saved levels.",
                        names.join(", ")
                    ))
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// The Optimal Desired Currency simulator; a row switches the simulation.
fn desired_simulator(rows: &[DesiredRow], cx: &mut Context<AppState>) -> AnyElement {
    let theme = cx.theme();
    let (muted, accent, hover) = (theme.muted_foreground, theme.primary, theme.list_hover);
    let table = table(
        &[
            ("Currency", false),
            ("Avg Coin Value", true),
            ("Royal-Weighted", true),
            ("Full Ground Value", true),
            ("Status", false),
        ],
        rows.iter().map(|row| {
            let entity = cx.entity();
            let currency = row.currency;
            let pick = div()
                .id(("fountain-desired", currency))
                .role(gpui::Role::Button)
                .aria_label(format!(
                    "Simulate {} as the desired currency",
                    fountain::currency_name(currency)
                ))
                .aria_selected(row.current)
                .rounded(cx.theme().radius)
                .px_1()
                .cursor_pointer()
                .when(row.current, |cell| cell.text_color(accent))
                .hover(move |style| style.bg(hover))
                .on_click(move |_event, _window, cx| {
                    entity.update(cx, |state, cx| {
                        state.set_fountain_desired(Some(currency), cx);
                    });
                })
                .child(fountain::currency_name(currency));
            TableRow::new()
                .id(("fountain-desired-row", currency))
                .child(text(pick))
                .child(number(num(row.average)))
                .child(number(num(row.weighted)))
                .child(number(bold(num(row.ground))))
                .child(text(
                    h_flex()
                        .gap_1()
                        .when(row.current, |cell| cell.child(tag("Current", Tone::Accent)))
                        .when(row.best, |cell| cell.child(tag("Best", Tone::Success))),
                ))
        }),
    );
    Card::new()
        .header(
            CardHeader::new("Optimal Desired Currency")
                .description("What a full ground redeems for at each unlocked desired currency"),
        )
        .child(
            v_flex().id("fountain-desired").gap_2().child(table).child(
                Caption::new(
                    "Pick a row to simulate it. Desired is a save setting; this section never \
                         writes one.",
                )
                .text_color(muted),
            ),
        )
        .into_any_element()
}

/// The currencies the save has not unlocked, and the upgrades that grant them.
fn unlocks(rows: &[UnlockRow], cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let card = Card::new().header(
        CardHeader::new("Next Unlock Tracker")
            .description("Every currency still locked, with the upgrade that unlocks it"),
    );
    if rows.is_empty() {
        return card
            .child(
                div()
                    .id("fountain-unlocks")
                    .child(Caption::new("All currencies unlocked.").text_color(muted)),
            )
            .into_any_element();
    }
    let table = table(
        &[
            ("Currency", false),
            ("Unlock Upgrade", false),
            ("Water", false),
            ("Current Lv", true),
            ("Status", false),
        ],
        rows.iter().map(|row| {
            TableRow::new()
                .id(("fountain-unlock-row", row.currency))
                .child(text(fountain::currency_name(row.currency)))
                .child(text(row.upgrade))
                .child(text(fountain::water_names()[row.water].clone()))
                .child(number(num(row.level)))
                .child(text(if row.prerequisite_unlocked {
                    tag("Available", Tone::Warning)
                } else {
                    tag("Prerequisite locked", Tone::Danger)
                }))
        }),
    );
    card.child(
        v_flex().id("fountain-unlocks").gap_2().child(table).child(
            Caption::new("An Available row only needs its own levels; its prerequisite is met.")
                .text_color(muted),
        ),
    )
    .into_any_element()
}

// ----- shared cells -----

/// An emphasised figure.
pub(super) fn bold(text: String) -> impl IntoElement {
    div().font_weight(FontWeight::SEMIBOLD).child(text)
}

/// The Locked, Active or Ignored tag.
fn status(row: &CurrencyRow) -> impl IntoElement {
    if !row.unlocked {
        tag("Locked", Tone::Danger)
    } else if row.active {
        tag("Active", Tone::Success)
    } else {
        tag("Ignored", Tone::Neutral)
    }
}
