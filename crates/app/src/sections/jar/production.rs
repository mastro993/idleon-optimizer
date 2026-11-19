//! The Production tab: the shared rate, what multiplies it, every slot and the
//! requirement of each jar type.

use super::table::{Col, table, text};
use super::tag;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_kit::component::progress::Progress;
use idleon_jar::Jar;
use idleon_jar::{BASE_PRODUCTION_PER_HOUR, TYPE_CATEGORIES, TYPE_NAMES, TypeCategory};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, Tone};

/// The tier columns the slot table shows.
const TIERS: u32 = 6;

pub(crate) fn production(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
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
        .child(multipliers(jar, cx))
        .child(slots(jar, cx))
        .child(requirements(jar, cx))
        .into_any_element()
}

/// The shared rate, the slots that spend it and the base it starts from.
fn rates(jar: &Jar) -> AnyElement {
    div()
        .id("jar-rates")
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new(
                        "jar-kpi-production",
                        "Production / hr",
                        format::count(jar.production_per_hour().max(0.0).round() as u64),
                    )
                    .caption("Progress every slot shares"),
                )
                .child(
                    Statistic::new(
                        "jar-kpi-active-slots",
                        "Active slots",
                        jar.production_slots().to_string(),
                    )
                    .suffix("/ 3")
                    .caption("Big Jar Mach III and VI add slots"),
                )
                .child(
                    Statistic::new(
                        "jar-kpi-base",
                        "Base",
                        format::count(BASE_PRODUCTION_PER_HOUR.round() as u64),
                    )
                    .caption("/hr before any bonus"),
                ),
        )
        .into_any_element()
}

/// What multiplies the base rate.
fn multipliers(jar: &Jar, cx: &App) -> AnyElement {
    let rows = std::iter::once(vec![
        text("Base"),
        text(format!("{}/hr", format::number(BASE_PRODUCTION_PER_HOUR))),
    ])
    .chain(jar.production_terms().into_iter().map(|term| {
        vec![
            text(term.label),
            text(format!("+{}%", format::number(term.percent))),
        ]
    }));
    Card::new()
        .header(
            CardHeader::new("Production multipliers").description(
                "The collectibles, the measurement and Light Speed scale the shared rate",
            ),
        )
        .child(table(
            "jar-multipliers",
            &[Col::Text("Source"), Col::Number("Value")],
            rows,
            cx,
        ))
        .into_any_element()
}

/// The per-slot details: its jar type, its progress, and when it reaches each tier.
fn slots(jar: &Jar, cx: &App) -> AnyElement {
    let mut cols = vec![
        Col::Number("#"),
        Col::Text("Jar type"),
        Col::Number("Requirement"),
        Col::Number("Progress"),
        Col::Text("Fill"),
    ];
    cols.extend(["T1", "T2", "T3", "T4", "T5", "T6"].map(Col::Number));
    let available = jar.types_available();
    let rows = (0..jar.production_slots()).map(|slot| {
        let jar_type = jar.slot_type(slot);
        let requirement = jar.production_requirement(jar_type);
        let progress = jar.slot_progress(slot);
        let share = (progress / requirement).clamp(0.0, 1.0) as f32;
        let mut cells = vec![
            text(format!("{}", slot + 1)),
            jar_type_cell(jar_type, jar_type >= available),
            text(format::number(requirement)),
            text(format::number(progress)),
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(96.0))
                        .child(Progress::new(("jar-fill", slot)).value(share * 100.0)),
                )
                .child(Caption::new(format!("{:.1}%", share * 100.0)))
                .into_any_element(),
        ];
        cells.extend(
            (0..TIERS).map(|tier| text(format::projection(jar.time_to_tier(jar_type, tier)))),
        );
        cells
    });
    Card::new()
        .header(
            CardHeader::new("Slot details").description(
                "Progress is banked toward the next jar; the tiers count from the first",
            ),
        )
        .child(table("jar-slots", &cols, rows, cx))
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
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(TYPE_NAMES[jar_type])
        .child(tag(category.label(), tone))
        .when(unowned, |cell| {
            cell.child(tag("type not owned", Tone::Danger))
        })
        .into_any_element()
}

/// Every owned jar type's requirement and the two reducers that shape it.
fn requirements(jar: &Jar, cx: &App) -> AnyElement {
    let cols = [
        Col::Text("Jar type"),
        Col::Number("Base requirement"),
        Col::Number("Effective requirement"),
        Col::Number("Broken"),
        Col::Number("Previous type produced"),
        Col::Number("Production line"),
    ];
    let rows = TYPE_NAMES
        .iter()
        .enumerate()
        .take(jar.types_available())
        .map(|(jar_type, name)| {
            let base = 1000.0 + 2000.0 * jar_type as f64;
            let (_, line) = jar.requirement_reduction(jar_type);
            let previous = jar.jars_produced(jar_type.saturating_sub(1));
            vec![
                text(*name),
                text(format::number(base)),
                text(format::number(jar.production_requirement(jar_type))),
                text(format::number(jar.jars_produced(jar_type))),
                text(format::number(previous)),
                text(match line > 0.0 {
                    true => format!("-{}%", format::number(line)),
                    false => "—".into(),
                }),
            ]
        });
    Card::new()
        .header(
            CardHeader::new("Requirements by jar type").description(
                "Big Jar Mach VII and the production line both divide the requirement",
            ),
        )
        .child(table("jar-requirements", &cols, rows, cx))
        .child(Caption::new(
            "The production line reads the jars broken of the type below.",
        ))
        .into_any_element()
}
