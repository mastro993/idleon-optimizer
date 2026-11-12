//! The Cooking EXP tab: one balloon at a time, with the overflow carried.

use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Checkbox, Choice, Select, Stepper};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use idleon_ladle::{
    self as ladle, EXP_BALLOONS, Ladle, Split, balloons_to_level, levels_from_balloons,
    split_balloons,
};
use idleon_ui::format;

/// The whole Cooking EXP tab.
pub(crate) fn pane(
    state: &mut AppState,
    ladle: &Ladle,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let panel = state.ladle_state.panel;
    let claim_hours = state.ladle_state.claim_hours;
    let stats = super::summary::all(ladle, panel, claim_hours);
    let balloon = state.ladle_state.balloon_key();
    let owned = ladle
        .balloons
        .get(state.ladle_state.balloon)
        .map(|count| count.value)
        .unwrap_or(0.0);
    let target = state.ladle_state.target_level;
    let split = state.ladle_state.split.then(|| {
        let entries: Vec<Option<(f64, f64)>> =
            stats.iter().map(|row| Some((row.level, row.exp))).collect();
        split_balloons(&entries, owned, panel.exp_multi, balloon.item_id)
    });
    let chosen = state
        .ladle_state
        .breakdown_character
        .min(stats.len().saturating_sub(1));
    if let Some(row) = stats.get(chosen) {
        let root = row.balloons_tree(
            balloon,
            owned,
            target_choice(target, row.level),
            split.as_ref(),
        );
        state
            .ladle_state
            .balloons_tree
            .update(cx, |tree, cx| tree.set_root(root, cx));
    }
    let tree = state.ladle_state.balloons_tree.clone();
    div()
        .id("ladle-exp-pane")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            ScrollArea::new("ladle-exp-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(controls(state, window, cx))
                        .child(table(state, ladle, &stats, split.as_ref(), cx))
                        .child(super::breakdowns::panel(
                            "ladle-breakdown-balloons",
                            "Cooking EXP balloons",
                            "Item ID × (Lv + 1) × EXP multi, one at a time with overflow",
                            &tree,
                        )),
                ),
        )
        .into_any_element()
}

/// The balloon, the target level and the split toggle.
fn controls(state: &mut AppState, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let owned = state.ladle.clone().unwrap_or_default().balloons;
    let choices: Vec<Choice> = EXP_BALLOONS
        .iter()
        .enumerate()
        .map(|(index, balloon)| {
            let count = owned.get(index).map(|count| count.value).unwrap_or(0.0);
            Choice::new(
                index.to_string(),
                format!("{} ({})", balloon.label, format::number(count)),
            )
        })
        .collect();
    let balloon_pick = entity.clone();
    let target_entity = entity.clone();
    let split_entity = entity.clone();
    div()
        .id("ladle-exp-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(
            column("EXP balloon", muted).child(
                Select::new("ladle-balloon", choices)
                    .label("EXP balloon")
                    .selected(state.ladle_state.balloon.to_string())
                    .on_change(move |value, _window, cx| {
                        if let Ok(index) = value.parse::<usize>() {
                            balloon_pick.update(cx, |state, cx| {
                                state.ladle_state.balloon = index.min(EXP_BALLOONS.len() - 1);
                                cx.notify();
                            });
                        }
                    }),
            ),
        )
        .child(
            column("Target Cooking level (0 = next)", muted).child(
                Stepper::new(
                    "ladle-target-level",
                    f64::from(state.ladle_state.target_level),
                )
                .range(0.0, 1000.0)
                .step(1.0)
                .precision(0)
                .on_change(move |value, _window, cx| {
                    target_entity.update(cx, |state, cx| {
                        state.ladle_state.target_level = value.max(0.0).round() as u32;
                        cx.notify();
                    });
                }),
            ),
        )
        .child(
            Checkbox::new("ladle-split", state.ladle_state.split)
                .label("Share the owned balloons to keep levels even")
                .on_change(move |on, _window, cx| {
                    split_entity.update(cx, |state, cx| {
                        state.ladle_state.split = on;
                        cx.notify();
                    });
                }),
        )
        .into_any_element()
}

/// The per-character balloon table.
fn table(
    state: &AppState,
    ladle: &Ladle,
    stats: &[ladle::Stats],
    split: Option<&Split>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let balloon = state.ladle_state.balloon_key();
    let owned = ladle
        .balloons
        .get(state.ladle_state.balloon)
        .map(|count| count.value)
        .unwrap_or(0.0);
    let target_level = state.ladle_state.target_level;
    let mut table = Table::new([
        "Character",
        "Cook Lv",
        "EXP progress",
        "EXP multi",
        "EXP / hr",
        "EXP / balloon",
        "Balloons → next Lv",
        "Balloons → target",
        "AFK time → target",
        if split.is_some() {
            "Lv with split share"
        } else {
            "Lv if given all owned"
        },
    ]);
    let mut next_total = 0.0;
    let mut target_total = 0.0;
    for row in stats {
        let target = target_choice(target_level, row.level);
        let next = row.next_level_balloons;
        let to_target = match target {
            Some(target) => {
                balloons_to_level(row.level, row.exp, target, row.exp_multi, balloon.item_id)
                    .balloons
            }
            None => next,
        };
        let spend = match split {
            Some(split) => match split.characters.get(row.index) {
                Some(Some(share)) => (share.level, share.used),
                _ => (row.level, 0.0),
            },
            None => {
                let all =
                    levels_from_balloons(row.level, row.exp, owned, row.exp_multi, balloon.item_id);
                (all.level, all.used)
            }
        };
        // AFK time to the target: the banked EXP pays some of it first.
        let needed = match target {
            Some(target) => exp_to_level(row.level, row.exp, target),
            None => (row.exp_req - row.exp).max(0.0),
        };
        let afk_hours = match row.exp_per_hour > 0.0 {
            true => needed / row.exp_per_hour,
            false => f64::INFINITY,
        };
        next_total += next;
        target_total += to_target;
        let fraction = match row.exp_req > 0.0 {
            true => (row.exp / row.exp_req).clamp(0.0, 1.0),
            false => 0.0,
        };
        let mut name = div().flex().items_center().gap_2().child(row.name.clone());
        if row.cooking {
            name = name.child(Badge::new("cooking").tone(Tone::Success));
        }
        let mut level_cell = div().flex().flex_col().child(
            div()
                .child(match spend.0 > row.level {
                    true => format!(
                        "{} → {}",
                        format::number(row.level),
                        format::number(spend.0)
                    ),
                    false => format::number(row.level),
                })
                .into_any_element(),
        );
        if split.is_some() {
            level_cell = level_cell.child(
                Caption::new(format!("{} balloons", format::number(spend.1)))
                    .text_color(cx.theme().colors.fg_muted),
            );
        }
        table = table.row([
            name.into_any_element(),
            div().child(format::number(row.level)).into_any_element(),
            div()
                .child(format!(
                    "{}% · {} / {}",
                    (fraction * 100.0).round(),
                    format::rate(row.exp),
                    format::rate(row.exp_req)
                ))
                .into_any_element(),
            div()
                .child(format!("×{:.2}", row.exp_multi))
                .into_any_element(),
            div()
                .child(format::rate(row.exp_per_hour))
                .into_any_element(),
            div()
                .child(format::number(ladle::balloon_gain(
                    balloon.item_id,
                    row.level,
                    row.exp_multi,
                )))
                .into_any_element(),
            div().child(format::number(next)).into_any_element(),
            div()
                .child(match target {
                    Some(target) => format!("Lv {target}: {}", format::number(to_target)),
                    None => format!(
                        "Lv {}: {}",
                        format::number(row.level + 1.0),
                        format::number(to_target)
                    ),
                })
                .into_any_element(),
            div()
                .child(format::projection(afk_hours))
                .into_any_element(),
            level_cell.into_any_element(),
        ]);
    }
    table = table.row([
        div().child("All characters").into_any_element(),
        div().into_any_element(),
        div().into_any_element(),
        div().into_any_element(),
        div().into_any_element(),
        div().into_any_element(),
        div().child(format::number(next_total)).into_any_element(),
        div().child(format::number(target_total)).into_any_element(),
        div().into_any_element(),
        div()
            .child(format!(
                "{}{} owned",
                match split {
                    Some(split) => format!("{} of ", format::number(split.used)),
                    None => String::new(),
                },
                format::number(owned)
            ))
            .into_any_element(),
    ]);
    let partial = ladle
        .balloons
        .get(state.ladle_state.balloon)
        .filter(|count| count.partial)
        .map(|count| count.missing.join(", "));
    let mut card = Card::new()
        .header(CardHeader::new("Cooking EXP").description(
            "Balloons add item ID × (Cooking Lv + 1) × EXP multi, one at a time with overflow",
        ))
        .child(
            div()
                .id("ladle-exp-table")
                .role(Role::Label)
                .aria_label(format!(
                    "{} characters, {} balloons owned",
                    stats.len(),
                    format::number(owned)
                ))
                .child(table),
        )
        .child(
            div().px_3().pb_3().child(
                Caption::new(
                    "Each balloon is read at the current level, so the overflow lands on the next one; a \
                     character with banked EXP may need none at all.",
                )
                .text_color(cx.theme().colors.fg_muted),
            ),
        );
    if let Some(missing) = partial {
        card = card.child(
            div().px_3().pb_3().child(
                Caption::new(format!("The count is a floor: {missing} could not be read"))
                    .text_color(cx.theme().colors.fg_muted),
            ),
        );
    }
    card.into_any_element()
}

/// The target the balloon tables plan for: the entered level, or the next one.
pub(crate) fn target_choice(target_level: u32, level: f64) -> Option<f64> {
    match f64::from(target_level) > level {
        true => Some(f64::from(target_level)),
        false => None,
    }
}

/// `expToLevel`: the EXP between the current level and `target`.
pub(crate) fn exp_to_level(level: f64, exp: f64, target: f64) -> f64 {
    let mut need = -exp;
    let mut each = level;
    while each < target {
        need += ladle::cooking_exp_req(each);
        each += 1.0;
    }
    need.max(0.0)
}

fn column(label: &'static str, muted: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
}
