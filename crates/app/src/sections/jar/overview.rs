//! The Overview tab: the jar summary, the rupie balances and the cavern's state.

use crate::sections::jar::{metric, row};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, FontWeight, IntoElement, ParentElement, Role, SharedString, Styled,
    div, px,
};
use gpui_kit::component::ActiveTheme as _;
use idleon_jar::Jar;
use idleon_jar::{RUPIE_COLORS, RUPIE_NAMES, TYPE_NAMES, collectible_name};
use idleon_ui::format;
use idleon_ui::widgets::{
    Callout, Caption, Card, CardHeader, KpiStrip, Statistic, TEXT_MONO, Tone,
};

pub(crate) fn overview(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
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
        pane = pane.child(missing(jar));
    }
    pane.child(balances(jar, cx))
        .child(state_cards(jar))
        .into_any_element()
}

/// The four figures the overview leads with.
fn summary(jar: &Jar) -> AnyElement {
    let label = format!(
        "{} / {} jar types · {} production slots · {} opals",
        jar.types_available(),
        TYPE_NAMES.len(),
        jar.production_slots(),
        format::number(jar.opals())
    );
    div()
        .id("jar-summary-line")
        .role(Role::Label)
        .aria_label(label)
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new(
                        "jar-kpi-types",
                        "Jar types",
                        jar.types_available().to_string(),
                    )
                    .suffix(format!("/ {}", TYPE_NAMES.len()))
                    .caption(next_type(jar)),
                )
                .child(
                    Statistic::new(
                        "jar-kpi-slots",
                        "Production slots",
                        jar.production_slots().to_string(),
                    )
                    .suffix("/ 3")
                    .caption("Every slot shares one production rate"),
                )
                .child(
                    Statistic::new(
                        "jar-kpi-opals",
                        "Opals from jars",
                        format::count(jar.opals().max(0.0).round() as u64),
                    )
                    .caption("Banked by Opal jar breaks"),
                )
                .child(
                    Statistic::new(
                        "jar-kpi-collectibles",
                        "Collectibles",
                        jar.collectibles_found().to_string(),
                    )
                    .suffix(format!("/ {}", jar.collectible_slots()))
                    .caption(format!("{} enchant levels", jar.enchant_levels())),
                ),
        )
        .into_any_element()
}

/// What the next jar type costs, and whether the rupie it takes can pay for it.
fn next_type(jar: &Jar) -> SharedString {
    if jar.missing_types().next().is_none() {
        return format!("All {} types owned", TYPE_NAMES.len()).into();
    }
    let cost = format::number(jar.next_type_cost());
    let rupie = RUPIE_NAMES[jar.types_purchased()];
    match jar.can_afford_next_type() {
        true => format!("Next: {cost} {rupie} — affordable").into(),
        false => format!("Next: {cost} {rupie}").into(),
    }
}

/// The jar types the account has not bought: they cannot break, so they are
/// named here instead of shown as empty balances.
fn missing(jar: &Jar) -> AnyElement {
    let names: Vec<&str> = jar.missing_types().map(|index| TYPE_NAMES[index]).collect();
    let count = names.len();
    let cost = format::number(jar.next_type_cost());
    let rupie = RUPIE_NAMES[jar.types_purchased()];
    div()
        .id("jar-missing")
        .child(
            Callout::new(Tone::Warning, format!("{count} jar types not owned")).child(
                Caption::new(format!(
                    "{} cannot break until the type is bought; the next one costs {cost} {rupie}.",
                    names.join(", ")
                )),
            ),
        )
        .into_any_element()
}

/// The twelve rupie balances, as pills, six to a row so a row never wraps.
fn balances(jar: &Jar, cx: &App) -> AnyElement {
    let available = jar.types_available();
    let shown: Vec<usize> = (0..RUPIE_NAMES.len())
        .filter(|index| {
            // A standard rupie the account cannot earn yet stays hidden.
            *index >= TYPE_NAMES.len() || *index < available || jar.rupies(*index) > 0.0
        })
        .collect();
    let pills = div()
        .id("jar-rupies")
        .flex()
        .flex_col()
        .gap_2()
        .children(shown.chunks(6).map(|chunk| {
            div().flex().gap_2().children(
                chunk
                    .iter()
                    .map(|index| pill(*index, jar.rupies(*index), cx)),
            )
        }));
    Card::new()
        .header(CardHeader::new("Rupie balances").description(
            "Standard rupies bank in the cavern; white and dark have their own storage",
        ))
        .child(pills)
        .into_any_element()
}

/// One rupie's balance, wearing its own colour.
fn pill(index: usize, balance: f64, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let name = RUPIE_NAMES[index];
    let shown = format::value(balance);
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
        .border_color(theme.border)
        .bg(theme.accent)
        .child(
            div()
                .flex_none()
                .size(px(10.0))
                .rounded_full()
                .bg(format::hex(RUPIE_COLORS[index])),
        )
        .child(Caption::new(name))
        .child(
            div()
                .font_family(theme.mono_font_family.clone())
                .text_size(TEXT_MONO)
                .font_weight(FontWeight::SEMIBOLD)
                .child(shown),
        )
        .into_any_element()
}

/// The gilded bonus, Break All and the targeted enchant, as panels.
fn state_cards(jar: &Jar) -> AnyElement {
    let doubler = jar.doubler_bonus();
    let gilded = match doubler > 0.0 {
        true => (
            format!("+{}%", format::number(doubler)),
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
            collectible_name(index).to_owned(),
            Tone::Accent,
            format!(
                "{:.2}% of enchants land on it",
                jar.targeted_enchant_chance() * 100.0
            ),
        ),
        None => (
            "None".into(),
            Tone::Neutral,
            "Pick a collectible in game to target it".into(),
        ),
    };
    let cards = [
        ("jar-gilded", "Gilded bonus", gilded),
        ("jar-break-all", "Break All", break_all),
        ("jar-target", "Targeted enchant", target),
    ];
    let mut row = row("jar-state");
    for (id, label, (value, tone, caption)) in cards {
        row = row.child(metric(id, label, value, caption, tone));
    }
    row.into_any_element()
}
