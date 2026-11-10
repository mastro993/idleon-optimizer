//! The Overview tab: the jar summary, the rupie balances and the cavern's state.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{Badge, KpiCard, Statistic, Tone};
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Role, SharedString, Styled, div,
    px,
};
use idleon_core::Jar;
use idleon_core::jar::{COLLECTIBLE_NAMES, RUPIE_COLORS, RUPIE_NAMES, TYPE_NAMES};

pub fn overview(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let mut pane = div()
        .id("jar-overview")
        .flex()
        .flex_col()
        .gap_3()
        .child(summary(jar));
    if jar.missing_types().next().is_some() {
        pane = pane.child(missing(jar, cx));
    }
    pane.child(balances(jar, cx))
        .child(state_cards(jar, cx))
        .into_any_element()
}

/// The four figures the upstream overview leads with.
fn summary(jar: &Jar) -> AnyElement {
    let label = format!(
        "{} / {} jar types · {} production slots · {} opals",
        jar.types_available(),
        TYPE_NAMES.len(),
        jar.production_slots(),
        format::whole(jar.opals())
    );
    div()
        .id("jar-summary-line")
        .role(Role::Label)
        .aria_label(label)
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new("jar-kpi-types", "Jar types", jar.types_available() as f64)
                    .suffix(format!(" / {}", TYPE_NAMES.len())),
            )
            .caption(next_type(jar)),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "jar-kpi-slots",
                    "Production slots",
                    jar.production_slots() as f64,
                )
                .suffix(" / 3"),
            )
            .caption("Every slot shares one production rate"),
        )
        .child(
            KpiCard::new(
                Statistic::new("jar-kpi-opals", "Opals from jars", jar.opals()).decimals(0),
            )
            .caption("Banked by Opal jar breaks"),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "jar-kpi-collectibles",
                    "Collectibles",
                    jar.collectibles_found() as f64,
                )
                .suffix(format!(" / {}", jar.collectible_slots())),
            )
            .caption(format!("{} enchant levels", jar.enchant_levels())),
        )
        .into_any_element()
}

/// What the next jar type costs, and whether the rupie it takes can pay for it.
fn next_type(jar: &Jar) -> SharedString {
    if jar.missing_types().next().is_none() {
        return format!("All {} types owned", TYPE_NAMES.len()).into();
    }
    let cost = format::whole(jar.next_type_cost());
    let rupie = RUPIE_NAMES[jar.types_purchased()];
    match jar.can_afford_next_type() {
        true => format!("Next: {cost} {rupie} — affordable").into(),
        false => format!("Next: {cost} {rupie}").into(),
    }
}

/// The jar types the account has not bought: they cannot break, so they are
/// named here instead of shown as empty balances.
fn missing(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let names: Vec<&str> = jar.missing_types().map(|index| TYPE_NAMES[index]).collect();
    let count = names.len();
    let cost = format::whole(jar.next_type_cost());
    let rupie = RUPIE_NAMES[jar.types_purchased()];
    let muted = cx.theme().colors.fg_muted;
    div()
        .id("jar-missing")
        .child(
            Callout::new(Severity::Warning)
                .title(format!("{count} jar types not owned"))
                .child(
                    Caption::new(format!(
                        "{} cannot break until the type is bought; the next one costs {cost} {rupie}.",
                        names.join(", ")
                    ))
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// The twelve rupie balances, as the upstream pills.
fn balances(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let mut pills = div().id("jar-rupies").flex().flex_wrap().gap_2();
    let available = jar.types_available();
    for index in 0..RUPIE_NAMES.len() {
        let balance = jar.rupies(index);
        // Upstream hides a standard rupie the account cannot earn yet.
        if index < TYPE_NAMES.len() && index >= available && balance <= 0.0 {
            continue;
        }
        pills = pills.child(pill(index, balance, cx));
    }
    Card::new()
        .header(CardHeader::new("Rupie balances").description(
            "Standard rupies bank in the cavern; white and dark have their own storage",
        ))
        .child(pills)
        .into_any_element()
}

/// One rupie's balance, wearing its own colour.
fn pill(index: usize, balance: f64, cx: &mut Context<AppState>) -> AnyElement {
    let colors = &cx.theme().colors;
    let name = RUPIE_NAMES[index];
    let shown = format::fmt_val(balance);
    div()
        .id(("jar-rupie", index))
        .role(Role::Label)
        .aria_label(format!("{name}: {shown}"))
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .rounded_full()
        .border_1()
        .border_color(colors.border)
        .bg(colors.hover)
        .child(
            div()
                .flex_none()
                .size(px(10.0))
                .rounded_full()
                .bg(format::hex(RUPIE_COLORS[index])),
        )
        .child(Caption::new(name).text_color(colors.fg_muted))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(shown),
        )
        .into_any_element()
}

/// The gilded bonus, Break All and the targeted enchant, as the upstream cards.
fn state_cards(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let doubler = jar.doubler_bonus();
    let gilded = match doubler > 0.0 {
        true => (
            format!("+{}%", format::whole(doubler)),
            Tone::Success,
            "Applied to the next rupie break".to_owned(),
        ),
        false => (
            "None".into(),
            Tone::Neutral,
            "Break a Gilded jar to bank a bonus".to_owned(),
        ),
    };
    let break_all = match jar.break_all_unlocked() {
        true => (
            "Unlocked".to_owned(),
            Tone::Success,
            "The Break All schematic is owned".to_owned(),
        ),
        false => (
            "Locked".to_owned(),
            Tone::Neutral,
            "Buy the Break All schematic".to_owned(),
        ),
    };
    let target = match jar.targeted_enchant() {
        Some(index) => (
            COLLECTIBLE_NAMES[index].to_owned(),
            Tone::Accent,
            format!(
                "{} of enchants land on it",
                ely_gpui_component::typography::format::percent(
                    jar.targeted_enchant_chance(),
                    2,
                    false
                )
            ),
        ),
        None => (
            "None".into(),
            Tone::Neutral,
            "Pick a collectible in game to target it".into(),
        ),
    };
    div()
        .id("jar-state")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(card("jar-gilded", "Gilded bonus", gilded, cx))
        .child(card("jar-break-all", "Break All", break_all, cx))
        .child(card("jar-target", "Targeted enchant", target, cx))
        .into_any_element()
}

/// One state card: its name, its value as a badge, and a line of context.
fn card(
    id: &'static str,
    label: &'static str,
    (value, tone, caption): (String, Tone, String),
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    div()
        .id(id)
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(240.0))
        .child(
            Card::new()
                .header(CardHeader::new(label).action(Badge::new(value).tone(tone)))
                .child(Caption::new(caption).text_color(muted)),
        )
        .into_any_element()
}
