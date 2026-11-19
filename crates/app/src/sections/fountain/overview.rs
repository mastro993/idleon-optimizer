//! The Overview tab: the fountain summary, fill and income rates, and the level plan.

use super::bonuses;
use super::controls::{Overview, cached};
use super::table::{number, table, tag, text};
use crate::state::AppState;
use crate::ui::layout::figure;
use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div};
use gpui_kit::component::table::TableRow;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use idleon_fountain as fountain;
use idleon_fountain::{Fountain, Income, LevelStep};
use idleon_ui::widgets::{
    Caption, Card, CardHeader, KpiStrip, PANEL_GAP, Statistic, TEXT_CAPTION, Tone,
};

/// The overview pane, in order: summary, income, bonuses, stacks.
pub(crate) fn overview(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let fountain = state
        .fountain
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let lists = cached(
        &mut state.fountain_controls.overview,
        state.save_revision,
        || Overview {
            income: fountain.income(),
            levels: fountain.level_plan(15),
            stacks: fountain.stacks(),
            cross: fountain.cross_bonuses(),
        },
    );
    let mut pane = v_flex().id("fountain-overview").gap(PANEL_GAP);
    if !fountain.missing_bonuses().is_empty() {
        pane = pane.child(bonuses::missing(fountain, cx));
    }
    pane = pane
        .child(summary(fountain, cx))
        .child(fill_summary(fountain, cx))
        .child(income(fountain, &lists.income, cx));
    if let Some(lucky) = bonuses::lucky_and_ducks(fountain, cx) {
        pane = pane.child(lucky);
    }
    if let Some(stacks) = bonuses::stacks(fountain, &lists.stacks) {
        pane = pane.child(stacks);
    }
    pane.child(fill_times(fountain, cx))
        .child(bonuses::cross_system(&lists.cross, cx))
        .child(bonuses::formula(state))
        .child(level_plan(&lists.levels, cx))
        .into_any_element()
}

/// The first card row: the fountain itself and what it holds.
fn summary(fountain: &Fountain, cx: &mut Context<AppState>) -> AnyElement {
    let level = fountain.level();
    let currency = fountain::level_currency(level);
    let cost = Fountain::opal_cost(level);
    let marble = if fountain.marble_bar_unlocked() {
        format!("Per fill: {}", num(fountain.marble_per_fill()))
    } else {
        "Marble Filling is locked".to_owned()
    };
    div()
        .id("fountain-summary")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .id("fountain-summary-row")
                .flex()
                .flex_wrap()
                .gap_3()
                .child(figure(
                    "fountain-kpi-level",
                    "Fountain Level",
                    num(level),
                    format!("Next: {} {}", num(cost), fountain::currency_label(currency)),
                    cx,
                ))
                .child(figure(
                    "fountain-kpi-waters",
                    "Waters Unlocked",
                    format!("{} / {}", fountain.waters_owned(), fountain::WATERS),
                    "Blue, Yellow and Green",
                    cx,
                ))
                .child(figure(
                    "fountain-kpi-coins",
                    "Max Coins",
                    num(fountain.max_coins()),
                    format!(
                        "{} spaces × {} stack",
                        fountain.spaces(),
                        num(fountain.max_stack())
                    ),
                    cx,
                ))
                .child(figure(
                    "fountain-kpi-marble",
                    "Marble",
                    num(fountain.marble()),
                    marble,
                    cx,
                )),
        )
        .into_any_element()
}

/// The second card row: the fill rates and the multipliers behind them.
fn fill_summary(fountain: &Fountain, cx: &mut Context<AppState>) -> AnyElement {
    let active = fountain.active_speed();
    let display = fountain.active_speed_display();
    let coin_fill = time(fountain.active_fill_time());
    let coin_caption = format!(
        "AFK {}. {active:.2}× active ({display:.2}× + AFK)",
        time(fountain.bar_fill_time())
    );
    let marble = if fountain.marble_bar_unlocked() {
        figure(
            "fountain-kpi-marble-fill",
            "Marble Fill",
            time(Fountain::marble_bar_fill_time() / active),
            format!(
                "AFK {} · {} /fill",
                time(Fountain::marble_bar_fill_time()),
                num(fountain.marble_per_fill())
            ),
            cx,
        )
    } else {
        figure(
            "fountain-kpi-marble-fill",
            "Marble Fill",
            "Locked".to_owned(),
            "Buy Marble Filling",
            cx,
        )
    };
    let royal = fountain.royal_chance();
    let royal_card = if royal > 0.0 {
        figure(
            "fountain-kpi-royal",
            "Royal Chance",
            odds(royal),
            format!("{} per fill", pct(royal)),
            cx,
        )
    } else {
        figure(
            "fountain-kpi-royal",
            "Royal Chance",
            "Locked".to_owned(),
            "Buy Royal Stacks",
            cx,
        )
    };
    let stacks_per_hr = fountain.optimal_stacks_per_hr();
    let (royal_stack, fills) = if royal > 0.0 && stacks_per_hr > 0.0 {
        (
            figure(
                "fountain-kpi-royal-stack",
                "Est. Royal Stack",
                time(3600.0 / stacks_per_hr),
                format!(
                    "Avg {:.1} fills",
                    (3600.0 / fountain.active_fill_time()) / stacks_per_hr
                ),
                cx,
            ),
            figure(
                "fountain-kpi-flush",
                "Est. Board Flush",
                time(fountain.flush_time()),
                format!("{} stacks at estimated throughput", fountain.spaces()),
                cx,
            ),
        )
    } else {
        (
            figure(
                "fountain-kpi-royal-stack",
                "Est. Royal Stack",
                "Locked".to_owned(),
                "Buy Royal Stacks",
                cx,
            ),
            figure(
                "fountain-kpi-flush",
                "Est. Board Flush",
                "Locked".to_owned(),
                "Buy Royal Stacks",
                cx,
            ),
        )
    };
    let redeem = fountain.redeem_by_currency();
    let redeem_caption = (0..fountain::CURRENCIES)
        .filter(|currency| redeem[*currency] > 0.0)
        .map(|currency| {
            format!(
                "{}: {}",
                fountain::currency_label(currency),
                num(redeem[currency])
            )
        })
        .collect::<Vec<String>>()
        .join(" · ");
    div()
        .id("fountain-fill-row")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(figure(
            "fountain-kpi-fill",
            "Coin Fill",
            coin_fill,
            coin_caption,
            cx,
        ))
        .child(marble)
        .child(figure(
            "fountain-kpi-multi",
            "Total Multi",
            format!("{}×", num(fountain.currency_all_multi())),
            "Currency value multiplier",
            cx,
        ))
        .child(royal_card)
        .child(royal_stack)
        .child(fills)
        .child(figure(
            "fountain-kpi-redeem",
            "Redeem Value",
            num(fountain.total_redeem_value()),
            if redeem_caption.is_empty() {
                "No coins on ground".to_owned()
            } else {
                redeem_caption
            },
            cx,
        ))
        .into_any_element()
}

/// The Income Rates card: three summary figures, the per-currency table and notes.
fn income(fountain: &Fountain, income: &Income, cx: &mut Context<AppState>) -> AnyElement {
    let active_speed = fountain.active_speed();
    let marble_afk = fountain.marble_per_hr() / active_speed;
    let fills = 3600.0 / fountain.active_fill_time();
    let fills_afk = 3600.0 / fountain.bar_fill_time();
    let keep = fountain.currency_keep();
    let muted = cx.theme().muted_foreground;
    let rows = income.rows.iter().map(|row| {
        TableRow::new()
            .id(("fountain-income-row", row.currency))
            .child(text(currency_cell(fountain, row.currency)))
            .child(number(num(row.value)))
            .child(number(rate_cell(row.single, row.single_afk, muted)))
            .child(number(rate_cell(row.full, row.full_afk, muted)))
            .child(number(rate_cell(row.optimal, row.optimal_afk, muted)))
    });
    let total = TableRow::new()
        .id("fountain-income-total")
        .font_weight(FontWeight::SEMIBOLD)
        .child(text("Total"))
        .child(number(""))
        .child(number(rate_cell(income.single, income.single_afk, muted)))
        .child(number(rate_cell(income.full, income.full_afk, muted)))
        .child(number(rate_cell(income.optimal, income.optimal_afk, muted)));
    let table = table(
        &[
            ("Currency", false),
            ("Coin value", true),
            ("Single /hr (active)", true),
            ("Full stack /hr (active)", true),
            ("Est. optimal /hr", true),
        ],
        rows.chain(std::iter::once(total)),
    );
    let size = fountain.size_matter_bonus();
    let mut notes = v_flex()
        .id("fountain-income-notes")
        .gap_1()
        .text_size(TEXT_CAPTION)
        .text_color(muted);
    if size > 0.0 {
        notes = notes.child(format!(
            "Size Does Matter: +{}% per coin in a stack · a full stack is {} coins (×{:.2})",
            num(size),
            num(fountain.max_stack()),
            1.0 + fountain.max_stack() * size / 100.0
        ));
    }
    notes = if fountain.royal_chance() > 0.0 {
        notes.child(format!(
            "Estimated optimal /hr only collects full royal stacks (×{:.2}). The steady-state \
             crown wait is an estimate; the game crowns the first non-royal space on the board.",
            fountain.effective_royal_multi()
        ))
    } else {
        notes.child(
            "Estimated optimal /hr uses full desired stacks without a royal multiplier until \
             Royal Stacks is unlocked.",
        )
    };
    Card::new()
        .header(CardHeader::new("Income Rates").description(
            "Coins per hour at the current upgrades and save; every type shares the fills",
        ))
        .child(
            div().id("fountain-income-kpis").child(
                KpiStrip::new()
                    .child(
                        Statistic::new(
                            "fountain-kpi-marble-rate",
                            "Marble /hr (Active)",
                            num(fountain.marble_per_hr()),
                        )
                        .caption(format!("AFK: {}/hr", num(marble_afk))),
                    )
                    .child(
                        Statistic::new(
                            "fountain-kpi-fills",
                            "Fills /hr (Active)",
                            format!("{fills:.1}"),
                        )
                        .caption(format!("AFK: {fills_afk:.1}/hr")),
                    )
                    .child(
                        Statistic::new("fountain-kpi-keep", "Money Back Guarantee", pct(keep))
                            .caption("Full-board coins auto-collect at this value"),
                    ),
            ),
        )
        .child(div().id("fountain-income").child(table))
        .child(notes)
        .into_any_element()
}

/// The stack fill times, active and AFK.
fn fill_times(fountain: &Fountain, cx: &mut Context<AppState>) -> AnyElement {
    let stack = fountain.max_stack();
    let spaces = fountain.spaces();
    div()
        .id("fountain-fill-times")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(figure(
            "fountain-kpi-stack-fill",
            "Stack Fill",
            time(fountain.stack_fill_time()),
            format!(
                "{} coins × {}/fill",
                num(stack),
                time(fountain.active_fill_time())
            ),
            cx,
        ))
        .child(figure(
            "fountain-kpi-all-fill",
            "All Stacks Fill",
            time(fountain.all_stacks_fill_time()),
            format!(
                "{spaces} spaces × {}/stack",
                time(fountain.stack_fill_time())
            ),
            cx,
        ))
        .child(figure(
            "fountain-kpi-afk-stack-fill",
            "AFK Stack Fill",
            time(fountain.afk_stack_fill_time()),
            format!(
                "{} coins × {}/fill",
                num(stack),
                time(fountain.bar_fill_time())
            ),
            cx,
        ))
        .child(figure(
            "fountain-kpi-afk-all-fill",
            "AFK All Fill",
            time(fountain.afk_all_stacks_fill_time()),
            format!(
                "{spaces} spaces × {}/stack",
                time(fountain.afk_stack_fill_time())
            ),
            cx,
        ))
        .into_any_element()
}

/// The fountain level planner: the next fifteen levels and their opal spend.
fn level_plan(levels: &[LevelStep], cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let rows = levels.iter().map(|step| {
        let cumulative = h_flex()
            .justify_end()
            .gap_2()
            .when(step.affordable, |cell| cell.child(tag("✓", Tone::Success)))
            .child(format!(
                "{} {}",
                num(step.cumulative),
                fountain::currency_label(step.currency)
            ));
        TableRow::new()
            .id(("fountain-level-row", step.level))
            .child(text(format!("{} → {}", step.level, step.level + 1)))
            .child(number(format!(
                "{} {}",
                num(step.cost),
                fountain::currency_label(step.currency)
            )))
            .child(text(fountain::currency_name(step.currency)))
            .child(number(cumulative))
    });
    Card::new()
        .header(
            CardHeader::new("Fountain Level Planner").description(
                "The next fifteen levels; the opal currency changes every five levels",
            ),
        )
        .child(
            v_flex()
                .id("fountain-levels")
                .gap_2()
                .child(table(
                    &[
                        ("Level", false),
                        ("Cost (Opals)", true),
                        ("Currency type", false),
                        ("Cumulative", true),
                    ],
                    rows,
                ))
                .child(
                    Caption::new(
                        "A row dims when the saved opal balance cannot cover its currency's spend.",
                    )
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

// ----- shared formatting and cards -----

/// A number: whole numbers under 1000 stay whole, the rest scale.
pub(super) fn num(value: f64) -> String {
    fountain::short_number(value)
}

/// A duration: hours, minutes, seconds and milliseconds.
pub(super) fn time(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "0s".into();
    }
    let whole = seconds as u64;
    let (hours, minutes, secs) = (whole / 3600, whole % 3600 / 60, whole % 60);
    let millis = (seconds.fract() * 1000.0).round() as u64;
    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if secs > 0 || (parts.is_empty() && millis == 0) {
        parts.push(format!("{secs}s"));
    }
    if millis > 0 {
        parts.push(format!("{millis}ms"));
    }
    parts.join(" ")
}

/// A percentage: one decimal place.
pub(super) fn pct(ratio: f64) -> String {
    format!("{:.1}%", 100.0 * ratio)
}

/// Odds: Locked, Guaranteed or `1 in X`.
pub(super) fn odds(chance: f64) -> String {
    if chance <= 0.0 {
        return "Locked".into();
    }
    if chance >= 1.0 {
        return "Guaranteed".into();
    }
    format!("1 in {:.2}", 1.0 / chance)
}

/// A currency's name, marked when it is the desired one.
fn currency_cell(fountain: &Fountain, currency: usize) -> impl IntoElement {
    let desired = currency == fountain.desired_currency();
    h_flex()
        .gap_1()
        .child(fountain::currency_label(currency))
        .when(desired, |cell| cell.child(tag("★", Tone::Accent)))
}

/// A rate with its AFK rate under it.
fn rate_cell(active: f64, afk: f64, muted: gpui::Hsla) -> impl IntoElement {
    v_flex()
        .items_end()
        .child(num(active))
        .child(Caption::new(format!("AFK {}", num(afk))).text_color(muted))
}
