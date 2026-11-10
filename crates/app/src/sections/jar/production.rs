//! The Production tab: the shared rate, what multiplies it, every slot and the
//! requirement of each jar type.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{Badge, KpiCard, Meter, Statistic, Tone};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_core::Jar;
use idleon_core::jar::{BASE_PRODUCTION_PER_HOUR, TYPE_CATEGORIES, TYPE_NAMES, TypeCategory};

/// The tier columns the slot table shows.
const TIERS: u32 = 6;

pub fn production(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    div()
        .id("jar-production")
        .flex()
        .flex_col()
        .gap_3()
        .child(rates(jar))
        .child(multipliers(jar))
        .child(slots(jar))
        .child(requirements(jar, cx))
        .into_any_element()
}

/// The shared rate, the slots that spend it and the base it starts from.
fn rates(jar: &Jar) -> AnyElement {
    div()
        .id("jar-rates")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new(
                    "jar-kpi-production",
                    "Production / hr",
                    jar.production_per_hour(),
                )
                .decimals(0),
            )
            .caption("Progress every slot shares"),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "jar-kpi-active-slots",
                    "Active slots",
                    jar.production_slots() as f64,
                )
                .suffix(" / 3"),
            )
            .caption("Big Jar Mach III and VI add slots"),
        )
        .child(
            KpiCard::new(
                Statistic::new("jar-kpi-base", "Base", BASE_PRODUCTION_PER_HOUR).decimals(0),
            )
            .caption("/hr before any bonus"),
        )
        .into_any_element()
}

/// `productionPerHRBreakdown`: what multiplies the base rate.
fn multipliers(jar: &Jar) -> AnyElement {
    let mut table = Table::new(["Source", "Value"]).row([
        div().child("Base").into_any_element(),
        div()
            .child(format!("{}/hr", format::whole(BASE_PRODUCTION_PER_HOUR)))
            .into_any_element(),
    ]);
    for term in jar.production_terms() {
        table = table.row([
            div().child(term.label).into_any_element(),
            div()
                .child(format!("+{}%", format::whole(term.percent)))
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Production multipliers").description(
                "The collectibles, the measurement and Light Speed scale the shared rate",
            ),
        )
        .child(table)
        .into_any_element()
}

/// The per-slot details: its jar type, its progress, and when it reaches each tier.
fn slots(jar: &Jar) -> AnyElement {
    let mut headers = vec!["#", "Jar type", "Requirement", "Progress", "Fill"];
    headers.extend(["T1", "T2", "T3", "T4", "T5", "T6"]);
    let mut table = Table::new(headers);
    let available = jar.types_available();
    for slot in 0..jar.production_slots() {
        let jar_type = jar.slot_type(slot);
        let requirement = jar.production_requirement(jar_type);
        let progress = jar.slot_progress(slot);
        let share = (progress / requirement).clamp(0.0, 1.0) as f32;
        let mut cells: Vec<AnyElement> = vec![
            div().child(format!("{}", slot + 1)).into_any_element(),
            jar_type_cell(jar_type, jar_type >= available),
            div().child(format::whole(requirement)).into_any_element(),
            div().child(format::whole(progress)).into_any_element(),
            div()
                .child(
                    // A full slot is the good end, so the meter never turns amber.
                    Meter::new(("jar-fill", slot), "", share)
                        .thresholds(1.0, 1.0)
                        .detail(ely_gpui_component::typography::format::percent(
                            share as f64,
                            1,
                            false,
                        )),
                )
                .into_any_element(),
        ];
        for tier in 0..TIERS {
            cells.push(
                div()
                    .child(format::projection(jar.time_to_tier(jar_type, tier)))
                    .into_any_element(),
            );
        }
        table = table.row(cells);
    }
    Card::new()
        .header(
            CardHeader::new("Slot details").description(
                "Progress is banked toward the next jar; the tiers count from the first",
            ),
        )
        .child(table)
        .into_any_element()
}

/// A jar type with its category, flagged when the account no longer owns it.
fn jar_type_cell(jar_type: usize, unowned: bool) -> AnyElement {
    let category = TYPE_CATEGORIES[jar_type];
    let tone = match category {
        TypeCategory::Rupie => Tone::Warning,
        TypeCategory::Chance => Tone::Info,
        TypeCategory::Gilded => Tone::Accent,
    };
    let mut cell = div()
        .flex()
        .items_center()
        .gap_2()
        .child(TYPE_NAMES[jar_type])
        .child(Badge::new(category.label()).tone(tone));
    if unowned {
        cell = cell.child(Badge::new("type not owned").tone(Tone::Danger));
    }
    cell.into_any_element()
}

/// Every owned jar type's requirement and the two reducers that shape it.
fn requirements(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut table = Table::new([
        "Jar type",
        "Base requirement",
        "Effective requirement",
        "Broken",
        "Previous type produced",
        "Production line",
    ]);
    for (jar_type, name) in TYPE_NAMES.iter().enumerate().take(jar.types_available()) {
        let base = 1000.0 + 2000.0 * jar_type as f64;
        let (_, line) = jar.requirement_reduction(jar_type);
        let previous = jar.jars_produced(jar_type.saturating_sub(1));
        let line_cell = match line > 0.0 {
            true => format!("-{}%", format::whole(line)),
            false => "—".into(),
        };
        table = table.row([
            div().child(*name).into_any_element(),
            div().child(format::whole(base)).into_any_element(),
            div()
                .child(format::whole(jar.production_requirement(jar_type)))
                .into_any_element(),
            div()
                .child(format::whole(jar.jars_produced(jar_type)))
                .into_any_element(),
            div().child(format::whole(previous)).into_any_element(),
            div().child(line_cell).into_any_element(),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Requirements by jar type").description(
                "Big Jar Mach VII and the production line both divide the requirement",
            ),
        )
        .child(table)
        .child(
            Caption::new("The production line reads the jars broken of the type below.")
                .text_color(muted),
        )
        .into_any_element()
}
