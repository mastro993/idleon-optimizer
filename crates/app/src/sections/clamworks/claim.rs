//! The Clamworks AFK claim: one trip's pearls and the `+1` comparison.
//!
//! The AFK claim tab: the character and pearl-table pickers, the hours away,
//! the expected pearl currency and the two drop rows. The `+1` comparison is
//! beside it in [`super::compare`].
//!
//! A full claim would also run in combat and compare a reroll policy on top of
//! it. Neither is here: the combat simulator supplies the kills and the drop
//! rate as entries until `CS-02`/`CS-03` land, and the reroll policy needs the
//! talent and guild bonuses the stats context resolves. What is left is the
//! claim itself: the first-claim payout.

use super::compare::compare_card;
use super::controls::phase_choices;
use super::pane;
use super::table::{Col, note, table, text};
use crate::state::AppState;
use crate::ui;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Role, SharedString, Styled, Window, div,
    px,
};
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::Input;
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_clamworks::afk::{Claim, ClaimError, Compare};
use idleon_ui::format;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, Choice, KpiStrip, PANEL_GAP, Statistic, Tone, show_number,
};

pub(crate) fn claim_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    // The claim reads the levels the section shows, so an applied plan's
    // projection previews the claim it would buy.
    let clamworks = state.clamworks_shown();
    let multikill = state.clamworks_multikill;
    let outcome = state.clamworks_claim.compare(&clamworks, multikill);
    let body = v_flex()
        .id("clamworks-claim")
        .gap(PANEL_GAP)
        .child(claim_card(state, outcome.as_ref(), window, cx))
        .child(compare_card(
            outcome.as_ref(),
            state.clamworks_claim.phase,
            cx,
        ));
    pane("clamworks-claim-scroll", body)
}

/// A control under its caption.
fn labelled(label: &'static str, control: impl IntoElement, cx: &App) -> AnyElement {
    v_flex()
        .gap_1()
        .child(Caption::new(label).text_color(cx.theme().muted_foreground))
        .child(control)
        .into_any_element()
}

/// The claim itself: the controls, the refused entry, the figures and the rows.
fn claim_card(
    state: &mut AppState,
    outcome: Result<&Compare, &ClaimError>,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let character = state.clamworks_claim.character;
    let characters: Vec<Choice<usize>> = state
        .save
        .as_ref()
        .map(|save| {
            (0..save.character_count())
                .map(|index| Choice::new(index, save.character_name(index)))
                .collect()
        })
        .unwrap_or_default();
    let character_name = state.save.as_ref().map_or_else(
        || "Character".to_string(),
        |save| save.character_name(character),
    );
    let promotion = state.clamworks.promotion_level;
    let stand_in = state.clamworks_claim.stand_in(character);
    let has_characters = !characters.is_empty();
    state.clamworks_claim.show_seed(window, cx);
    let hours_input = state.clamworks_claim.hours_field.clone();
    let controls = &state.clamworks_controls;
    // Values flow section state -> control; edits come back through the
    // subscriptions in `Controls::new`.
    controls
        .character
        .show(characters, Some(&character), window, cx);
    controls.phase.show(
        phase_choices(),
        Some(&state.clamworks_claim.phase),
        window,
        cx,
    );
    show_number(
        &controls.claim_kills,
        stand_in.kills_per_hour,
        0,
        window,
        cx,
    );
    show_number(&controls.drop_rate, stand_in.drop_rate, 2, window, cx);
    let calculate = cx.entity();
    let focus = hours_input.clone();

    let controls = h_flex()
        .id("clamworks-claim-controls")
        .items_end()
        .gap_3()
        .child(labelled(
            "Character",
            div().flex_none().w(px(180.0)).child(
                Select::new(controls.character.state())
                    .id("clamworks-claim-character")
                    .accessibility_label("Character")
                    .placeholder("No characters")
                    .disabled(!has_characters)
                    .small(),
            ),
            cx,
        ))
        .child(labelled(
            "Pearl table",
            div().flex_none().w(px(160.0)).child(
                Select::new(controls.phase.state())
                    .id("clamworks-claim-phase")
                    .accessibility_label("Pearl table")
                    .small(),
            ),
            cx,
        ))
        .child(labelled(
            "Kills / hour",
            ui::number_field(
                "clamworks-claim-kills",
                &controls.claim_kills,
                "Kills per hour",
            ),
            cx,
        ))
        .child(labelled(
            "Drop rate ×",
            ui::number_field(
                "clamworks-claim-drop-rate",
                &controls.drop_rate,
                "Drop rate multiplier",
            ),
            cx,
        ))
        .child(labelled(
            "Hours away",
            div()
                .id("clamworks-claim-hours")
                .role(Role::Label)
                .aria_label("Hours away")
                .w(px(140.0))
                .on_click(move |_event, window, cx| {
                    focus.update(cx, |input, cx| input.focus(window, cx));
                })
                .child(Input::new(&hours_input).aria_label("Hours away").small()),
            cx,
        ))
        .child(
            Button::new("clamworks-claim-calculate")
                .label("Calculate claim")
                .small()
                .on_click(move |_event, _window, cx| {
                    calculate.update(cx, |state, cx| state.calculate_clamworks_claim(cx));
                }),
        );

    let mut card = Card::new()
        .header(CardHeader::new("AFK claim").description(
            "One ordinary trip on Clamworks: the pearls its credited kills roll, Lucky Day included.",
        ))
        .child(controls);
    match outcome {
        Ok(compare) => {
            let claim = &compare.rows[0].claim;
            card = card
                .child(claim_figures(claim, &character_name, promotion, cx))
                .child(drops_table(claim, cx))
                .child(
                    v_flex()
                        .id("clamworks-claim-notes")
                        .gap_1()
                        .child(note(
                            "Kills per hour and the drop rate are entries until the combat simulator derives them; \
                             the pearl table is the one the game starts the claim from.",
                            cx,
                        ))
                        .child(note(
                            "One claim, before rerolls, claim doubling or inventory losses. Time Candy cannot be \
                             used on this map.",
                            cx,
                        ))
                        .children(claim.warnings.iter().enumerate().map(|(index, warning)| {
                            div()
                                .id(("clamworks-claim-warning", index))
                                .role(Role::Label)
                                .aria_label(warning.clone())
                                .child(note(warning.clone(), cx))
                        })),
                );
        }
        Err(error) => {
            card = card.child(
                div()
                    .id("clamworks-claim-error")
                    .role(Role::Label)
                    .aria_label(error.message())
                    .child(
                        Alert::new("clamworks-claim-error-banner", error.message())
                            .with_variant(Tone::Danger.into())
                            .banner(),
                    ),
            );
        }
    }
    card.into_any_element()
}

/// The claim's four figures and the line that says where they came from.
fn claim_figures(claim: &Claim, character_name: &str, promotion: u32, cx: &App) -> AnyElement {
    let (pearls, pearl_suffix) = format::scale(claim.expected_pearls);
    let (rolls, roll_suffix) = format::scale(claim.rolls as f64);
    v_flex()
        .id("clamworks-claim-figures")
        .gap_2()
        .child(note(
            format!(
                "{character_name} · promotion {promotion} · {} × {} kills/hour = {} kills, {} drop rolls",
                format::hours(claim.inputs.hours),
                quantity(claim.inputs.kills_per_hour),
                quantity(claim.kills),
                format::count(claim.rolls),
            ),
            cx,
        ))
        .child(
            KpiStrip::new()
                .child(kpi(
                    "clamworks-claim-pearls",
                    "Expected pearls",
                    format!("{} pearls", quantity(claim.expected_pearls)),
                    Statistic::new(
                        "clamworks-claim-pearls-value",
                        "Expected pearls",
                        format!("{pearls:.2}"),
                    )
                    .suffix(pearl_suffix),
                    "Both pearl types, Lucky Day included",
                ))
                .child(kpi(
                    "clamworks-claim-pure",
                    "Pure pearl chance",
                    format::chance(claim.pure.chance),
                    Statistic::new(
                        "clamworks-claim-pure-value",
                        "Pure pearl chance",
                        format!("{:.2}", claim.pure.chance * 100.0),
                    )
                    .suffix("%"),
                    if claim.pure.probability > 0.0 {
                        "At least one Pure Pearl this claim"
                    } else {
                        "PURE PEARLS is not purchased"
                    },
                ))
                .child(kpi(
                    "clamworks-claim-rolls",
                    "Drop rolls",
                    format!("{} kills", format::count(claim.rolls)),
                    Statistic::new(
                        "clamworks-claim-rolls-value",
                        "Drop rolls",
                        format!("{rolls:.0}"),
                    )
                    .suffix(roll_suffix),
                    "One pearl roll per kill",
                ))
                .child(kpi(
                    "clamworks-claim-value",
                    "Pearl value",
                    format!("×{}", quantity(claim.value)),
                    Statistic::new(
                        "clamworks-claim-value-value",
                        "Pearl value",
                        format!("{:.2}", claim.value),
                    )
                    .prefix("×"),
                    &format!("Lucky Day: {} for 10x", format::chance(claim.lucky_chance)),
                )),
        )
        .into_any_element()
}

/// The drop table: both pearl types over the claim.
fn drops_table(claim: &Claim, cx: &App) -> AnyElement {
    let rows = [
        ("clamworks-claim-drops-regular", "Pearls", &claim.regular),
        ("clamworks-claim-drops-pure", "Pure Pearls", &claim.pure),
    ]
    .map(|(id, name, drop)| {
        [
            div()
                .id(id)
                .role(Role::Label)
                .aria_label(format!("{name}: {}", quantity(drop.expected)))
                .child(SharedString::from(name))
                .into_any_element(),
            text(quantity(drop.expected)),
            text(format!(
                "{}{}",
                format::chance(drop.chance),
                if drop.guaranteed { " (Guaranteed)" } else { "" }
            )),
            text(quantity(drop.minimum)),
            text(quantity(drop.pearls)),
        ]
    });
    div()
        .id("clamworks-claim-drops")
        .child(table(
            "clamworks-claim-drops",
            &[
                Col::Text("Drop"),
                Col::Number("Expected quantity"),
                Col::Number("Chance of at least one"),
                Col::Number("Guaranteed quantity"),
                Col::Number("Expected pearl currency"),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}

/// A figure in a strip, labelled so its value reads as one figure.
fn kpi(
    id: &'static str,
    title: &'static str,
    display: String,
    statistic: Statistic,
    caption: &str,
) -> AnyElement {
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(format!("{title}: {display}"))
        .child(statistic.caption(caption.to_string()))
        .into_any_element()
}

/// A quantity as the claim prints it: a
/// fraction below one percent in scientific notation, everything else scaled.
pub(super) fn quantity(value: f64) -> String {
    if value > 0.0 && value < 0.01 {
        format!("{value:.2e}")
    } else {
        format::fmt_val(value)
    }
}
