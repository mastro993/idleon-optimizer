//! The Overview's bonus cards: missing sources, lucky coins and ducks, the ground
//! stacks, the cross-system bonuses and the shared multiplier tree.

use super::overview::{num, pct, time};
use crate::state::AppState;
use crate::ui::layout::figure;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::motion::ProgressBar;
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, Styled, div, px};
use idleon_fountain as fountain;
use idleon_fountain::Fountain;
use idleon_ui::formula::FormulaTree;

/// The cross-system bonuses the save cannot supply, named instead of reading zero.
pub(crate) fn missing(fountain: &Fountain, cx: &mut Context<AppState>) -> AnyElement {
    let missing = fountain.missing_bonuses();
    let muted = cx.theme().colors.fg_muted;
    div()
        .id("fountain-missing")
        .child(
            Callout::new(Severity::Warning)
                .title(format!(
                    "{} bonus source{} not in this save",
                    missing.len(),
                    if missing.len() == 1 { "" } else { "s" }
                ))
                .child(
                    Caption::new(format!(
                        "{} read as neutral until a save with those rows is imported.",
                        missing.join(", ")
                    ))
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// Lucky coins and rubber ducks, when either system is unlocked.
pub(crate) fn lucky_and_ducks(
    fountain: &Fountain,
    cx: &mut Context<AppState>,
) -> Option<AnyElement> {
    let any_lucky =
        (0..fountain::CURRENCIES).any(|currency| fountain.lucky_coin_count(currency) > 0.0);
    if !fountain.duck_bar_unlocked() && !any_lucky {
        return None;
    }
    let muted = cx.theme().colors.fg_muted;
    let mut table = Table::new(["Currency", "Coins", "Value Multi", "Next Coin Chance"]);
    for currency in 0..fountain::CURRENCIES {
        if !fountain.currency_unlocked(currency) {
            continue;
        }
        let chance = fountain.lucky_coin_chance(currency);
        table = table.row([
            div()
                .child(fountain::currency_name(currency).to_owned())
                .into_any_element(),
            div()
                .child(num(fountain.lucky_coin_count(currency)))
                .into_any_element(),
            div()
                .child(format!("{:.2}×", fountain.lucky_coin_value(currency)))
                .into_any_element(),
            div()
                .child(if chance > 0.0 {
                    format!("1 in {}", num((1.0 / chance).round()))
                } else {
                    "-".to_owned()
                })
                .into_any_element(),
        ]);
    }
    let mut card = Card::new()
        .header(CardHeader::new("Lucky Coins & Ducks").description(
            "Each lucky coin makes the next one four times rarer; each duck makes the next \
                 five times rarer",
        ))
        .child(
            div()
                .id("fountain-lucky")
                .flex()
                .flex_col()
                .gap_2()
                .child(table)
                .child(
                    Caption::new(format!(
                        "Lucky coins are worth +{}% each; the base chance is 1 in {}.",
                        num(fountain.lucky_coin_value_per()),
                        if fountain.lucky_coin_base_chance() > 0.0 {
                            num((1.0 / fountain.lucky_coin_base_chance()).round())
                        } else {
                            "-".to_owned()
                        }
                    ))
                    .text_color(muted),
                ),
        );
    if fountain.duck_bar_unlocked() {
        let count = fountain.duck_count();
        let chance = fountain.duck_chance();
        let (ratio, percent) = fountain.duck_odds();
        let fill = Fountain::duck_bar_fill_time();
        let progress = fountain.duck_bar_progress();
        let active = fill / fountain.active_speed();
        let fraction = (progress / fill).clamp(0.0, 1.0);
        let remaining = (fill - progress).max(0.0);
        card = card.child(
            div()
                .id("fountain-ducks")
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_3()
                        .child(figure(
                            "fountain-kpi-ducks",
                            "Ducks",
                            num(count),
                            format!("Currency multi: {:.2}×", fountain.duck_multi()),
                            cx,
                        ))
                        .child(figure(
                            "fountain-kpi-duck-chance",
                            "Next Duck Chance",
                            if chance > 0.0 {
                                format!("1 in {ratio}")
                            } else {
                                "-".to_owned()
                            },
                            format!("{percent}% per duck bar fill"),
                            cx,
                        ))
                        .child(figure(
                            "fountain-kpi-duck-bar",
                            "Duck Bar",
                            pct(fraction),
                            format!(
                                "{} active · {} AFK",
                                time(remaining / fountain.active_speed()),
                                time(remaining)
                            ),
                            cx,
                        ))
                        .child(figure(
                            "fountain-kpi-duck-fill",
                            "Bar Fill Time",
                            time(active),
                            format!(
                                "AFK: {} · {:.2}× active",
                                time(fill),
                                fountain.active_speed()
                            ),
                            cx,
                        )),
                )
                .child(
                    div()
                        .w_full()
                        .max_w(px(400.0))
                        .child(ProgressBar::new("fountain-duck-progress", fraction as f32)),
                ),
        );
    }
    Some(card.into_any_element())
}

/// The coin stacks lying on the fountain floor.
pub(crate) fn stacks(fountain: &Fountain, _cx: &mut Context<AppState>) -> Option<AnyElement> {
    let stacks = fountain.stacks();
    if stacks.is_empty() {
        return None;
    }
    let mut row = div().id("fountain-stacks").flex().flex_wrap().gap_3();
    for stack in &stacks {
        let mut counts = [0usize; fountain::CURRENCIES];
        for currency in &stack.coins {
            if let Some(count) = counts.get_mut(*currency) {
                *count += 1;
            }
        }
        let types = (0..fountain::CURRENCIES)
            .filter(|currency| counts[*currency] > 0)
            .map(|currency| {
                format!(
                    "{}×{}",
                    counts[currency],
                    fountain::currency_label(currency)
                )
            })
            .collect::<Vec<String>>()
            .join(", ");
        row = row.child(
            div()
                .id(("fountain-stack", stack.slot))
                .flex()
                .flex_col()
                .gap_1()
                .min_w(px(150.0))
                .px_3()
                .py_2()
                .rounded_md()
                .when(stack.royal, |cell| {
                    cell.child(Badge::new("♔ royal").tone(Tone::Accent))
                })
                .child(Caption::new(format!("#{}", stack.slot + 1)))
                .child(
                    div()
                        .text_size(px(18.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(num(stack.count() as f64)),
                )
                .child(Caption::new(types))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(num(fountain.stack_redeem_value(stack))),
                ),
        );
    }
    Some(
        Card::new()
            .header(
                CardHeader::new("Coin Stacks")
                    .description("What the coins on the ground redeem for at the current values"),
            )
            .child(row)
            .into_any_element(),
    )
}

/// The eighteen upgrades that boost other systems.
pub(crate) fn cross_system(fountain: &Fountain, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut grid = div().id("fountain-cross").flex().flex_wrap().gap_3();
    for bonus in fountain.cross_bonuses() {
        let value = if bonus.percent > 0.0 {
            format!("{:.2}×", 1.0 + bonus.percent / 100.0)
        } else {
            "-".to_owned()
        };
        grid = grid.child(
            div()
                .id(format!("fountain-bonus-{}", bonus.label))
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(170.0))
                .gap_1()
                .child(Caption::new(bonus.label))
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(value),
                )
                .child(Caption::new(format!("From {}", bonus.source)).text_color(muted)),
        );
    }
    Card::new()
        .header(
            CardHeader::new("Cross-System Bonuses").description(
                "What the fountain upgrades give the other systems at their saved levels",
            ),
        )
        .child(grid)
        .into_any_element()
}

/// The shared formula tree over the currency value multiplier.
pub(crate) fn formula(
    state: &AppState,
    fountain: &Fountain,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let tree: Entity<FormulaTree> = state.fountain_breakdown.clone();
    tree.update(cx, |tree, cx| tree.set_root(fountain.multi_tree(), cx));
    Card::new()
        .header(
            CardHeader::new("Currency Multiplier")
                .description("Search every factor and copy the full tree"),
        )
        .child(
            div()
                .id("fountain-formula")
                .h(px(220.0))
                .min_h_0()
                .child(tree),
        )
        .into_any_element()
}
