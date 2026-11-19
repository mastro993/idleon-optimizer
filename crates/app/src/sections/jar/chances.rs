//! The Chances tab: the opal, collectible and enchant chances, how a tier
//! scales them and how long the next event takes.

use super::rupies::TIER_HEADS;
use super::table::{Col, table, text};
use crate::sections::jar::{metric, row};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, div};
use idleon_jar as jar;
use idleon_jar::Jar;
use idleon_jar::{INCOME_TIERS, collectible_name};
use idleon_ui::format;
use idleon_ui::widgets::{Card, CardHeader, Tone};

pub(crate) fn chances(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let mut pane = div()
        .id("jar-chances")
        .flex()
        .flex_col()
        .gap_3()
        .child(headline(jar))
        .child(chance_by_tier(jar, cx))
        .child(expected_events(jar, cx));
    if let Some(target) = jar.targeted_enchant() {
        pane = pane.child(targeted(jar, target));
    }
    pane.into_any_element()
}

/// The three chances a break rolls, with the counts that shape them.
fn headline(jar: &Jar) -> AnyElement {
    let mut row = row("jar-chances-summary");
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
fn chance_by_tier(jar: &Jar, cx: &App) -> AnyElement {
    let mut cols = vec![Col::Text("Chance jar"), Col::Number("Base")];
    cols.extend(
        TIER_HEADS[..INCOME_TIERS as usize]
            .iter()
            .copied()
            .map(Col::Number),
    );
    cols.push(Col::Text("Certain at"));
    let rows = jar.chance_events().into_iter().map(|event| {
        let mut cells = vec![
            text(format!(
                "{} (type {})",
                event.name,
                jar::TYPE_NAMES[event.jar_type]
            )),
            text(format::chance(event.chance)),
        ];
        cells.extend((0..INCOME_TIERS).map(|tier| {
            let chance = Jar::chance_at_tier(event.chance, tier);
            text(match chance >= 1.0 {
                true => "100%+".to_owned(),
                false => format::chance(chance),
            })
        }));
        cells.push(text(match Jar::tier_for_certainty(event.chance) {
            Some(tier) => format!("Tier {tier}"),
            None => "Never".into(),
        }));
        cells
    });
    Card::new()
        .header(CardHeader::new("Chance by tier").description(
            "A chance jar multiplies its chance by ten every tier; past 100% the event is certain",
        ))
        .child(table("jar-chance-tiers", &cols, rows, cx))
        .into_any_element()
}

/// How many breaks and how many hours the next event
/// takes at tier one, on one slot.
fn expected_events(jar: &Jar, cx: &App) -> AnyElement {
    let rows = jar.chance_events().into_iter().map(|event| {
        let breaks = Jar::expected_breaks(event.chance);
        vec![
            text(event.name),
            text(format::chance(event.chance)),
            text(match breaks.is_finite() {
                true => format::number(breaks),
                false => "∞".into(),
            }),
            text(format::projection(
                jar.expected_hours(event.jar_type, event.chance),
            )),
        ]
    });
    Card::new()
        .header(
            CardHeader::new("Expected time per event")
                .description("Tier one jars at the current production rate, one slot at a time"),
        )
        .child(table(
            "jar-expected-events",
            &[
                Col::Text("Event"),
                Col::Number("Chance"),
                Col::Number("Average breaks"),
                Col::Number("Estimated time"),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}

/// The targeted enchant: what an enchant lands on and how often.
fn targeted(jar: &Jar, target: usize) -> AnyElement {
    let chance = jar.targeted_enchant_chance();
    let row = row("jar-chances-target")
        .child(metric(
            "jar-target-name",
            "Target",
            collectible_name(target).to_owned(),
            format!("Collectible #{target}"),
            Tone::Accent,
        ))
        .child(metric(
            "jar-target-chance",
            "Target chance",
            format::chance(chance),
            format!("5 / ({} found + 3)", jar.collectibles_found()),
            Tone::Info,
        ))
        .child(metric(
            "jar-target-combined",
            "Combined",
            format::chance(jar.enchant_chance() * chance),
            "An enchant that lands on the target".to_owned(),
            Tone::Warning,
        ));
    Card::new()
        .header(
            CardHeader::new("Targeted enchant")
                .description("An enchant lands on the target this share of the time"),
        )
        .child(row)
        .into_any_element()
}
