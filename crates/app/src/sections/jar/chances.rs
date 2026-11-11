//! The Chances tab: the opal, collectible and enchant chances, how a tier
//! scales them and how long the next event takes.

use crate::sections::jar::metric;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::Tone;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_core::jar::{COLLECTIBLE_NAMES, INCOME_TIERS};
use idleon_core::{Jar, jar};

pub fn chances(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let mut pane = div()
        .id("jar-chances")
        .flex()
        .flex_col()
        .gap_3()
        .child(headline(jar, cx))
        .child(chance_by_tier(jar))
        .child(expected_events(jar));
    if let Some(target) = jar.targeted_enchant() {
        pane = pane.child(targeted(jar, target, cx));
    }
    pane.into_any_element()
}

/// The three chances a break rolls, with the counts that shape them.
fn headline(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let mut row = div().id("jar-chances-summary").flex().flex_wrap().gap_3();
    for event in jar.chance_events() {
        let caption = match event.name {
            "Opal" => format!(
                "{} opals found; each one cuts the chance by 57%",
                format::number(jar.opals())
            ),
            "Collectible" => format!(
                "{} of {} found; page two gets harder with every find",
                jar.collectibles_found(),
                jar.collectible_slots()
            ),
            _ => format!(
                "{} enchant levels; more levels make the next one rarer",
                jar.enchant_levels()
            ),
        };
        let tone = match event.name {
            "Opal" => Tone::Info,
            "Collectible" => Tone::Accent,
            _ => Tone::Warning,
        };
        row = row.child(metric(
            chance_id(event.name),
            event.name,
            format::chance(event.chance),
            caption,
            tone,
            cx,
        ));
    }
    row.into_any_element()
}

/// The element id one chance card wears.
fn chance_id(name: &str) -> &'static str {
    match name {
        "Opal" => "jar-chance-opal",
        "Collectible" => "jar-chance-collectible",
        _ => "jar-chance-enchant",
    }
}

/// `Chance × Tier`: each tier multiplies the chance by ten, and the last column
/// names the tier where the event is certain.
fn chance_by_tier(jar: &Jar) -> AnyElement {
    let mut headers: Vec<String> = vec!["Chance jar".into(), "Base".into()];
    for tier in 0..INCOME_TIERS {
        headers.push(format!("Tier {}", tier + 1));
    }
    headers.push("Certain at".into());
    let mut table = Table::new(headers);
    for event in jar.chance_events() {
        let mut cells: Vec<AnyElement> = vec![
            div()
                .child(format!(
                    "{} (type {})",
                    event.name,
                    jar::TYPE_NAMES[event.jar_type]
                ))
                .into_any_element(),
            div().child(format::chance(event.chance)).into_any_element(),
        ];
        for tier in 0..INCOME_TIERS {
            let chance = Jar::chance_at_tier(event.chance, tier);
            cells.push(
                div()
                    .child(match chance >= 1.0 {
                        true => "100%+".to_owned(),
                        false => format::chance(chance),
                    })
                    .into_any_element(),
            );
        }
        cells.push(
            div()
                .child(match Jar::tier_for_certainty(event.chance) {
                    Some(tier) => format!("Tier {tier}"),
                    None => "Never".into(),
                })
                .into_any_element(),
        );
        table = table.row(cells);
    }
    Card::new()
        .header(CardHeader::new("Chance by tier").description(
            "A chance jar multiplies its chance by ten every tier; past 100% the event is \
                 certain",
        ))
        .child(table)
        .into_any_element()
}

/// `expectedHoursToEvent`: how many breaks and how many hours the next event
/// takes at tier one, on one slot.
fn expected_events(jar: &Jar) -> AnyElement {
    let mut table = Table::new(["Event", "Chance", "Average breaks", "Estimated time"]);
    for event in jar.chance_events() {
        let breaks = Jar::expected_breaks(event.chance);
        table = table.row([
            div().child(event.name).into_any_element(),
            div().child(format::chance(event.chance)).into_any_element(),
            div()
                .child(match breaks.is_finite() {
                    true => format::number(breaks),
                    false => "∞".into(),
                })
                .into_any_element(),
            div()
                .child(format::projection(
                    jar.expected_hours(event.jar_type, event.chance),
                ))
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Expected time per event")
                .description("Tier one jars at the current production rate, one slot at a time"),
        )
        .child(table)
        .into_any_element()
}

/// The targeted enchant: what an enchant lands on and how often.
fn targeted(jar: &Jar, target: usize, cx: &mut Context<AppState>) -> AnyElement {
    let chance = jar.targeted_enchant_chance();
    let row = div()
        .id("jar-chances-target")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(metric(
            "jar-target-name",
            "Target",
            COLLECTIBLE_NAMES[target].to_owned(),
            format!("Collectible #{target}"),
            Tone::Accent,
            cx,
        ))
        .child(metric(
            "jar-target-chance",
            "Target chance",
            format::chance(chance),
            format!("5 / ({} found + 3)", jar.collectibles_found()),
            Tone::Info,
            cx,
        ))
        .child(metric(
            "jar-target-combined",
            "Combined",
            format::chance(jar.enchant_chance() * chance),
            "An enchant that lands on the target".to_owned(),
            Tone::Warning,
            cx,
        ));
    Card::new()
        .header(
            CardHeader::new("Targeted enchant")
                .description("An enchant lands on the target this share of the time"),
        )
        .child(row)
        .into_any_element()
}
