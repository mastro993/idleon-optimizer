//! JR-04: the Enchant Simulator tab.
//!
//! Ported from `_renderEnchantSim` and `_renderSimResults` in `jar-calc.html`.
//! The controls are the page's `esim-*` inputs; the run goes through the shared
//! [`idleon_ui::runner::TaskRunner`], so progress and cancel match every other
//! long job, and the odds table reads the same simulated average the page does.

use crate::sections::jar::metric;
use crate::sections::jar::state::{ENCHANTS, SIMS, TARGET_MAX};
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::Tone;
use ely_gpui_component::forms::{Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, div, px};
use idleon_jar::{COLLECTIBLES, Jar, SimResult, Strategy, collectible_name};
use idleon_ui::format;

/// The simulator pane: its summary, the controls, the targets and the result.
pub(crate) fn enchant(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let mut body = div()
        .id("jar-enchant-sim")
        .flex()
        .flex_col()
        .gap_3()
        .child(summary(jar, cx))
        .child(controls(state, jar, cx));
    if jar.collectibles_found() == 0 {
        let none = "The simulator distributes enchants across the collectibles the save has \
                    found; this cavern has none yet.";
        body = body.child(
            Card::new()
                .header(CardHeader::new("No collectible found"))
                .child(div().p_3().child(Caption::new(none))),
        );
    } else {
        body = body.child(targets(state, jar, cx));
    }
    body = body.child(result(state, jar, cx));
    body.into_any_element()
}

/// The three figures the page leads the tab with.
fn summary(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let found = jar.collectibles_found();
    div()
        .id("jar-sim-summary")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(metric(
            "jar-sim-found",
            "Collectibles Found",
            format!("{found} / {}", jar.collectible_slots()),
            "Eligible targets".to_owned(),
            Tone::Accent,
            cx,
        ))
        .child(metric(
            "jar-sim-focus-prob",
            "Focus Prob",
            format!("{:.1}%", jar.targeted_enchant_chance() * 100.0),
            format!("5/({found}+3) per enchant"),
            Tone::Info,
            cx,
        ))
        .child(metric(
            "jar-sim-current",
            "Current Total",
            format::number(f64::from(jar.enchant_levels())),
            "Your current enchant levels".to_owned(),
            Tone::Neutral,
            cx,
        ))
        .into_any_element()
}

/// The run's own knobs and the run actions.
fn controls(state: &AppState, jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let knobs = &state.jar_state;
    let running = state.jar_sim.read(cx).running();
    let entity = cx.entity();

    let enchants_entity = entity.clone();
    let enchants = NumberInput::new("jar-sim-enchants", f64::from(knobs.enchants))
        .range(f64::from(ENCHANTS.0), f64::from(ENCHANTS.1))
        .step(1.0)
        .precision(0)
        .label("Enchantments")
        .on_change(move |value, _window, cx| {
            enchants_entity.update(cx, |state, cx| {
                state.edit_jar_state(cx, |state| state.enchants = value.max(1.0) as u32);
            });
        });
    let sims_entity = entity.clone();
    let sims = NumberInput::new("jar-sim-sims", f64::from(knobs.sims))
        .range(f64::from(SIMS.0), f64::from(SIMS.1))
        .step(100.0)
        .precision(0)
        .label("Sims")
        .on_change(move |value, _window, cx| {
            sims_entity.update(cx, |state, cx| {
                state.edit_jar_state(cx, |state| state.sims = value.max(1.0) as u32);
            });
        });
    let strategy_entity = entity.clone();
    let strategy = Select::new(
        "jar-sim-strategy",
        Strategy::ALL.map(|strategy| Choice::new(strategy.id(), strategy.label())),
    )
    .label("Strategy")
    .selected(knobs.strategy.id())
    .on_change(move |value, _window, cx| {
        if let Some(strategy) = Strategy::from_id(value.as_ref()) {
            strategy_entity.update(cx, |state, cx| {
                state.edit_jar_state(cx, |state| state.strategy = strategy);
            });
        }
    });
    let focus_entity = entity.clone();
    let mut focus_choices = vec![Choice::new("none", "—")];
    focus_choices.extend(
        (0..COLLECTIBLES)
            .filter(|index| jar.collectible(*index).is_some_and(|item| item.level >= 1))
            .map(|index| Choice::new(index.to_string(), collectible_name(index))),
    );
    let focus = Select::new("jar-sim-focus", focus_choices)
        .label("Focus on")
        .selected(
            knobs
                .focus
                .map_or_else(|| "none".to_string(), |index| index.to_string()),
        )
        .on_change(move |value, _window, cx| {
            let focus = value.parse::<usize>().ok();
            focus_entity.update(cx, |state, cx| {
                state.edit_jar_state(cx, |state| state.focus = focus);
            });
        });
    let seed_entity = entity.clone();
    let seed = NumberInput::new("jar-sim-seed", knobs.seed as f64)
        .range(1.0, 4_294_967_295.0)
        .step(1.0)
        .precision(0)
        .label("Seed")
        .on_change(move |value, _window, cx| {
            seed_entity.update(cx, |state, cx| {
                state.edit_jar_state(cx, |state| state.seed = value.max(1.0) as u64);
            });
        });
    let run_entity = entity.clone();
    let run = Button::new(
        "jar-sim-run",
        if running {
            "Running…"
        } else {
            "Run Simulation"
        },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(running || jar.collectibles_found() == 0)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_jar_simulation(cx));
    });
    let cancel_entity = entity;
    let cancel = Button::new("jar-sim-cancel", "Cancel")
        .icon(IconName::X)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_jar_simulation(cx));
        });

    Card::new()
        .header(CardHeader::new("Enchant Simulator").description(
            "All simulations start from the found collectibles at one level, as the page's worker does.",
        ))
        .child(
            div()
                .id("jar-sim-controls")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_4()
                .p_3()
                .child(field("Enchantments", enchants.into_any_element(), muted))
                .child(field("Sims", sims.into_any_element(), muted))
                .child(field("Strategy", strategy.into_any_element(), muted))
                .child(field("Focus on", focus.into_any_element(), muted))
                .child(field("Seed", seed.into_any_element(), muted))
                .child(div().flex().items_center().gap_2().child(run).child(cancel))
                .child(
                    Caption::new("A seeded run repeats exactly; the result names its seed.")
                        .text_color(muted),
                ),
        )
        .into_any_element()
}

/// One labelled control, so the number inputs and selects line up.
fn field(label: &'static str, control: AnyElement, muted: gpui::Hsla) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_size(px(11.0)).text_color(muted).child(label))
        .child(control)
        .into_any_element()
}

/// The target level of every found collectible.
fn targets(state: &AppState, jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut grid = div().id("jar-sim-targets").flex().flex_wrap().gap_2().p_3();
    for index in 0..COLLECTIBLES {
        let Some(item) = jar.collectible(index) else {
            continue;
        };
        if item.level < 1 {
            continue;
        }
        let target = state.jar_state.targets.get(&index).copied().unwrap_or(0);
        let entity = cx.entity();
        let input = NumberInput::new(format!("jar-sim-target-{index}"), f64::from(target))
            .range(0.0, f64::from(TARGET_MAX))
            .step(1.0)
            .precision(0)
            .label(item.name)
            .on_change(move |value, _window, cx| {
                let target = value.max(0.0) as u32;
                entity.update(cx, |state, cx| {
                    state.edit_jar_state(cx, |state| {
                        if target == 0 {
                            state.targets.remove(&index);
                        } else {
                            state.targets.insert(index, target);
                        }
                    });
                });
            });
        grid = grid.child(
            div()
                .w(px(150.0))
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_size(px(11.0)).text_color(muted).child(item.name))
                .child(input),
        );
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
    let muted = cx.theme().colors.fg_muted;
    let task = state.jar_simulation(cx);
    let mut body = div().id("jar-sim-result").flex().flex_col().gap_3();
    if task.running
        && let Some(progress) = task.progress.clone()
    {
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
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "jar-sim-cancelled",
            task.result.is_some(),
        ));
    }
    let Some(spread) = task.result.clone() else {
        return body
            .child(
                div()
                    .p_3()
                    .child(Caption::new("Configure and run to see results.").text_color(muted)),
            )
            .into_any_element();
    };
    let mut header = CardHeader::new(format!(
        "{} enchants × {} sims — {}",
        spread.enchants,
        spread.sims,
        spread.strategy_label()
    ))
    .description(format!("Seed {}", spread.seed));
    if task.stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("jar-sim"));
    }
    let mut card = Card::new().header(header);
    if task.stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("jar-sim"));
    }
    if let Some(all_hit) = spread.all_hit {
        card = card.child(all_targets_hit(all_hit, cx));
    }
    card.child(result_line(&spread))
        .child(spread_table(&spread))
        .child(odds(jar, &spread, muted))
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
        .px_3()
        .child(Caption::new(summary))
        .into_any_element()
}

/// The `All targets hit` figure the page highlights first.
fn all_targets_hit(share: f64, cx: &mut Context<AppState>) -> AnyElement {
    let tone = if share >= 0.9 {
        Tone::Success
    } else if share >= 0.5 {
        Tone::Warning
    } else {
        Tone::Danger
    };
    div()
        .px_3()
        .child(metric(
            "jar-sim-all-hit",
            "All targets hit",
            format!("{:.1}%", share * 100.0),
            "The share of simulations that reached every target".to_owned(),
            tone,
            cx,
        ))
        .into_any_element()
}

/// Every found collectible's average, range and hit share.
fn spread_table(result: &SimResult) -> AnyElement {
    let mut table = Table::new(["Collectible", "Average", "Min", "Max", "Target", "Hit"]);
    for entry in &result.spread {
        table = table.row([
            div()
                .id(format!("jar-sim-spread-{}", entry.index))
                .child(entry.name)
                .into_any_element(),
            div()
                .child(format!("{:.1}", entry.average))
                .into_any_element(),
            div().child(entry.min.to_string()).into_any_element(),
            div().child(entry.max.to_string()).into_any_element(),
            div()
                .child(match entry.target {
                    0 => "—".to_string(),
                    target => target.to_string(),
                })
                .into_any_element(),
            div()
                .child(match entry.hit {
                    Some(hit) => format!("{:.0}%", hit * 100.0),
                    None => "—".to_string(),
                })
                .into_any_element(),
        ]);
    }
    div()
        .id("jar-sim-spread")
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .child(table)
        .into_any_element()
}

/// `Enchant Odds by Jar Tier`: the hours one enchant takes at the simulated
/// average, on every bought jar type, for the first six tiers.
fn odds(jar: &Jar, result: &SimResult, muted: gpui::Hsla) -> AnyElement {
    let average = result.average_total().round().max(0.0) as u32;
    let odds = jar.enchant_tier_hours(average);
    let current = jar.enchant_chance();
    let simulated = Jar::enchant_chance_at(average, jar.enchant_multi());
    let mut headers: Vec<String> = vec!["Tier".to_string()];
    for jar_type in 0..odds.first().map_or(0, Vec::len) {
        headers.push(format!("Jar {}", jar_type + 1));
    }
    let mut table = Table::new(headers);
    for (index, row) in odds.iter().enumerate() {
        let mut cells = vec![div().child(format!("T{}", index + 1)).into_any_element()];
        for hours in row {
            cells.push(div().child(hours_cell(*hours)).into_any_element());
        }
        table = table.row(cells);
    }
    div()
        .id("jar-sim-odds")
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .role(Role::Label)
        .aria_label(format!(
            "Enchant odds by jar tier at {average} average enchant levels: current chance {:.4}%, after the simulation {:.4}%",
            current * 100.0,
            simulated * 100.0
        ))
        .child(div().text_size(px(14.0)).child("Enchant Odds by Jar Tier"))
        .child(
            Caption::new(format!(
                "Current chance: {:.4}% · After sim avg ({average} enchant lvs): {:.4}% · Multi: {:.2}x",
                current * 100.0,
                simulated * 100.0,
                jar.enchant_multi()
            ))
            .text_color(muted),
        )
        .child(table)
        .child(
            Caption::new(
                "Values show the time per enchant from one slot's production at the simulated \
                 average: under an hour reads per hour, then hours, days and years.",
            )
            .text_color(muted),
        )
        .into_any_element()
}

/// One odds cell, in the page's hr/d/y units.
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
