//! JR-04: the Enchant Simulator tab.
//!
//! The run goes through the shared [`idleon_ui::runner::TaskRunner`], so
//! progress and cancel match every other long job, and the odds table reads the
//! simulated average. The controls are the kit entities of
//! [`super::Controls`]; render shows the simulator's values in them.

use super::table::{Col, table, text};
use crate::sections::jar::{metric, row};
use crate::state::AppState;
use crate::ui::number_field;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Role, Styled, Window, div, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::Select;
use gpui_kit::component::{Disableable as _, Sizable as _};
use idleon_jar::{Jar, SimResult, collectible_name};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone, show_number};

/// Target fields to a row: a row never wraps (`docs/ui-kit/display.md`).
const TARGETS_PER_ROW: usize = 6;

/// The simulator pane: its summary, the controls, the targets and the result.
pub(crate) fn enchant(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let mut body = div()
        .id("jar-enchant-sim")
        .flex()
        .flex_col()
        .gap_3()
        .child(summary(jar))
        .child(controls(state, jar, window, cx));
    if jar.collectibles_found() == 0 {
        body = body.child(
            Card::new()
                .header(CardHeader::new("No collectible found"))
                .child(Caption::new(
                    "The simulator distributes enchants across the collectibles the save has \
                     found; this cavern has none yet.",
                )),
        );
    } else {
        body = body.child(targets(state, window, cx));
    }
    body.child(result(state, jar, cx)).into_any_element()
}

/// The three figures the page leads the tab with.
fn summary(jar: &Jar) -> AnyElement {
    let found = jar.collectibles_found();
    row("jar-sim-summary")
        .child(metric(
            "jar-sim-found",
            "Collectibles Found",
            format!("{found} / {}", jar.collectible_slots()),
            "Eligible targets".to_owned(),
            Tone::Accent,
        ))
        .child(metric(
            "jar-sim-focus-prob",
            "Focus Prob",
            format!("{:.1}%", jar.targeted_enchant_chance() * 100.0),
            format!("5/({found}+3) per enchant"),
            Tone::Info,
        ))
        .child(metric(
            "jar-sim-current",
            "Current Total",
            format::number(f64::from(jar.enchant_levels())),
            "Your current enchant levels".to_owned(),
            Tone::Neutral,
        ))
        .into_any_element()
}

/// The run's own knobs and the run actions.
fn controls(
    state: &AppState,
    jar: &Jar,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let knobs = &state.jar_state;
    let fields = &state.jar_controls;
    let running = state.jar_sim.read(cx).running();
    show_number(&fields.enchants, f64::from(knobs.enchants), 0, window, cx);
    show_number(&fields.sims, f64::from(knobs.sims), 0, window, cx);
    show_number(&fields.seed, knobs.seed as f64, 0, window, cx);
    fields.strategy.show(
        super::controls::strategy_choices(),
        Some(&knobs.strategy),
        window,
        cx,
    );
    fields.focus.show(
        state.jar_controls.figures().focus.clone(),
        Some(&knobs.focus),
        window,
        cx,
    );
    let run_entity = cx.entity();
    let run = Button::new("jar-sim-run")
        .label(if running {
            "Running…"
        } else {
            "Run Simulation"
        })
        .primary()
        .icon(IconName::Play)
        .disabled(running || jar.collectibles_found() == 0)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_jar_simulation(cx));
        });
    let cancel_entity = cx.entity();
    let cancel = Button::new("jar-sim-cancel")
        .label("Cancel")
        .icon(IconName::X)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_jar_simulation(cx));
        });
    let strategy = field(
        "Strategy",
        div().flex_none().w(px(180.0)).child(
            Select::new(fields.strategy.state())
                .id("jar-sim-strategy")
                .accessibility_label("Strategy")
                .small(),
        ),
    );
    let focus = field(
        "Focus on",
        div().flex_none().w(px(200.0)).child(
            Select::new(fields.focus.state())
                .id("jar-sim-focus")
                .accessibility_label("Focus on")
                .small(),
        ),
    );
    Card::new()
        .header(
            CardHeader::new("Enchant Simulator")
                .description("All simulations start from the found collectibles at one level."),
        )
        .child(
            div()
                .id("jar-sim-controls")
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_end()
                        .gap_4()
                        .child(field(
                            "Enchantments",
                            number_field("jar-sim-enchants", &fields.enchants, "Enchantments"),
                        ))
                        .child(field(
                            "Sims",
                            number_field("jar-sim-sims", &fields.sims, "Sims"),
                        ))
                        .child(strategy)
                        .child(focus)
                        .child(field(
                            "Seed",
                            number_field("jar-sim-seed", &fields.seed, "Seed"),
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(run)
                        .child(cancel)
                        .child(Caption::new(
                            "A seeded run repeats exactly; the result names its seed.",
                        )),
                ),
        )
        .into_any_element()
}

/// One labelled control, so the number inputs and selects line up.
fn field(label: &'static str, control: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label))
        .child(control)
        .into_any_element()
}

/// The target level of every found collectible.
fn targets(state: &AppState, window: &mut Window, cx: &mut App) -> AnyElement {
    let found = &state.jar_controls.figures().found;
    let fields = &state.jar_controls.targets;
    let mut grid = div().id("jar-sim-targets").flex().flex_col().gap_2();
    for chunk in found.chunks(TARGETS_PER_ROW) {
        let mut row = div().flex().gap_2();
        for index in chunk {
            let Some(field) = fields.get(*index) else {
                continue;
            };
            let target = state.jar_state.targets.get(index).copied().unwrap_or(0);
            show_number(field, f64::from(target), 0, window, cx);
            let name = collectible_name(*index);
            row = row.child(
                div()
                    .w(px(150.0))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Caption::new(name))
                    .child(number_field(format!("jar-sim-target-{index}"), field, name)),
            );
        }
        grid = grid.child(row);
    }
    Card::new()
        .header(
            CardHeader::new("Target Levels")
                .description("Set a target to see the share of simulations that reach it."),
        )
        .child(grid)
        .into_any_element()
}

/// The stored spread: its summary, every collectible and the odds table.
fn result(state: &AppState, jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let runner = state.jar_sim.read(cx);
    let running = runner.running();
    let cancelled = runner.cancelled();
    let progress = runner.progress().cloned();
    let stored = runner.result().is_some();
    let mut body = div().id("jar-sim-result").flex().flex_col().gap_3();
    if running && let Some(progress) = progress {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "jar-sim",
            "Simulating enchants",
            "Runs off the UI thread; cancelling keeps the spread this run replaced.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_jar_simulation(cx));
            },
        ));
    }
    if cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "jar-sim-cancelled",
            stored,
        ));
    }
    let stale = state.jar_simulation_stale(cx);
    let Some(spread) = state.jar_sim.read(cx).result() else {
        return body
            .child(Caption::new("Configure and run to see results."))
            .into_any_element();
    };
    let mut header = CardHeader::new(format!(
        "{} enchants × {} sims — {}",
        spread.enchants,
        spread.sims,
        spread.strategy_label()
    ))
    .description(format!("Seed {}", spread.seed));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("jar-sim"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("jar-sim"));
    }
    if let Some(all_hit) = spread.all_hit {
        card = card.child(all_targets_hit(all_hit));
    }
    body.child(
        card.child(result_line(spread))
            .child(spread_table(spread, cx))
            .child(odds(jar, spread, cx)),
    )
    .into_any_element()
}

/// The run's headline as one value: the seed and the average spread, which a
/// seeded rerun reproduces exactly.
fn result_line(result: &SimResult) -> AnyElement {
    let summary = format!(
        "{} enchants × {} sims · {} · seed {} · average {:.1} levels",
        result.enchants,
        result.sims,
        result.strategy_label(),
        result.seed,
        result.average_total()
    );
    div()
        .id("jar-sim-result-line")
        .role(Role::Label)
        .aria_label(summary.clone())
        .child(Caption::new(summary))
        .into_any_element()
}

/// The `All targets hit` figure the page highlights first.
fn all_targets_hit(share: f64) -> AnyElement {
    let tone = if share >= 0.9 {
        Tone::Success
    } else if share >= 0.5 {
        Tone::Warning
    } else {
        Tone::Danger
    };
    metric(
        "jar-sim-all-hit",
        "All targets hit",
        format!("{:.1}%", share * 100.0),
        "The share of simulations that reached every target".to_owned(),
        tone,
    )
}

/// Every found collectible's average, range and hit share.
fn spread_table(result: &SimResult, cx: &App) -> AnyElement {
    let cols = [
        Col::Text("Collectible"),
        Col::Number("Average"),
        Col::Number("Min"),
        Col::Number("Max"),
        Col::Number("Target"),
        Col::Number("Hit"),
    ];
    let rows = result.spread.iter().map(|entry| {
        vec![
            div()
                .id(format!("jar-sim-spread-{}", entry.index))
                .child(entry.name)
                .into_any_element(),
            text(format!("{:.1}", entry.average)),
            text(entry.min.to_string()),
            text(entry.max.to_string()),
            text(match entry.target {
                0 => "—".to_string(),
                target => target.to_string(),
            }),
            text(match entry.hit {
                Some(hit) => format!("{:.0}%", hit * 100.0),
                None => "—".to_string(),
            }),
        ]
    });
    table("jar-sim-spread", &cols, rows, cx).into_any_element()
}

/// The jar heads of the odds table, one per bought jar type.
const JAR_HEADS: [&str; 10] = [
    "Jar 1", "Jar 2", "Jar 3", "Jar 4", "Jar 5", "Jar 6", "Jar 7", "Jar 8", "Jar 9", "Jar 10",
];

/// `Enchant Odds by Jar Tier`: the hours one enchant takes at the simulated
/// average, on every bought jar type, for the first six tiers.
fn odds(jar: &Jar, result: &SimResult, cx: &App) -> AnyElement {
    let average = result.average_total().round().max(0.0) as u32;
    let odds = jar.enchant_tier_hours(average);
    let current = jar.enchant_chance();
    let simulated = Jar::enchant_chance_at(average, jar.enchant_multi());
    let jars = odds.first().map_or(0, Vec::len).min(JAR_HEADS.len());
    let mut cols = vec![Col::Text("Tier")];
    cols.extend(JAR_HEADS[..jars].iter().copied().map(Col::Number));
    let rows = odds.iter().enumerate().map(|(index, row)| {
        std::iter::once(text(format!("T{}", index + 1)))
            .chain(row.iter().take(jars).map(|hours| text(hours_cell(*hours))))
            .collect()
    });
    div()
        .id("jar-sim-odds")
        .flex()
        .flex_col()
        .gap_2()
        .role(Role::Label)
        .aria_label(format!(
            "Enchant odds by jar tier at {average} average enchant levels: current chance {:.4}%, after the simulation {:.4}%",
            current * 100.0,
            simulated * 100.0
        ))
        .child(div().child("Enchant Odds by Jar Tier"))
        .child(Caption::new(format!(
            "Current chance: {:.4}% · After sim avg ({average} enchant lvs): {:.4}% · Multi: {:.2}x",
            current * 100.0,
            simulated * 100.0,
            jar.enchant_multi()
        )))
        .child(table("jar-sim-odds-table", &cols, rows, cx))
        .child(Caption::new(
            "Values show the time per enchant from one slot's production at the simulated \
             average: under an hour reads per hour, then hours, days and years.",
        ))
        .into_any_element()
}

/// One odds cell, in hr/d/y units.
fn hours_cell(hours: f64) -> String {
    if !hours.is_finite() {
        return "—".to_owned();
    }
    if hours < 1.0 {
        format!("{:.1}/hr", 1.0 / hours.max(f64::MIN_POSITIVE))
    } else if hours < 24.0 {
        format!("{hours:.1}h")
    } else if hours < 8_760.0 {
        format!("{:.1}d", hours / 24.0)
    } else {
        format!("{:.1}y", hours / 8_760.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_odds_cells_follow_the_pages_units() {
        assert_eq!(hours_cell(0.5), "2.0/hr");
        assert_eq!(hours_cell(12.0), "12.0h");
        assert_eq!(hours_cell(48.0), "2.0d");
        assert_eq!(hours_cell(17_520.0), "2.0y");
        assert_eq!(hours_cell(f64::INFINITY), "—");
    }
}
