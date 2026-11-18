//! The Minehead currency pane: every factor the currency/hr formula reads, as
//! one breakdown tree.

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use gpui_kit::component::ActiveTheme as _;
use idleon_common::formula::{FormulaNode, ValueFormat};
use idleon_minehead::{Inputs, game};
use idleon_ui::format;
use idleon_ui::widgets::{Card, CardHeader, TEXT_CAPTION};

/// Every factor of the currency/hr formula, as one tree.
fn currency_tree(inputs: &Inputs) -> FormulaNode {
    let total = inputs.currency_per_hour();
    let data = game();
    let levels = &inputs.qty_levels;
    let qty = |t: usize| {
        idleon_minehead::math::upgrade_qty(data, t, levels.get(t).copied().unwrap_or(0.0), 1.0)
    };
    let factor = |label: &str, value: f64, note: &str| {
        FormulaNode::new(label, value)
            .with_format(ValueFormat::Multiplier)
            .with_note(note)
    };
    let button = if inputs.button1 > 0.0 {
        factor(
            "Button 1",
            1.0 + inputs.button1 / 100.0,
            "OPT Lacc 594 presses × rate 3",
        )
    } else {
        factor("Button 1", 1.0, "No button presses in the save")
    };
    let children = vec![
        FormulaNode::leaf("Grid 129", inputs.grid[0], ValueFormat::Raw)
            .with_note("Base Minehead Currency/hr"),
        factor("Grid 148", 1.0 + inputs.grid[1] / 100.0, "×(1 + Grid/100)"),
        factor(
            "RoG 12",
            1.0 + inputs.rog12 / 100.0,
            "Ring of Glory, unique sushi",
        ),
        factor(
            "Companion 143",
            inputs.comp_143.clamp(1.0, 2.0) + inputs.comp_143_level2,
            "max(1, min(2, bonus)) + level 2",
        ),
        factor(
            "Royal Guardian focus",
            inputs.royal_currency_multi,
            "Outpost RoG bonus 3",
        ),
        factor(
            "Jelly 8",
            1.0 + inputs.jelly_currency8 / 100.0,
            "Jelly Operator reward 8",
        ),
        factor(
            "Jelly bundle",
            if inputs.bundle_j { 2.0 } else { 1.0 },
            "BundlesReceived.ban_j",
        ),
        factor(
            "Floor 6 reward",
            (1.0 + data.floor_bonus(6, inputs.floor) / 100.0).min(3.0),
            "BonusQTY(6), capped at 3×",
        ),
        factor(
            "Farm upgrades",
            1.0 + (qty(5)
                + qty(22)
                + qty(28) * idleon_common::curve::log(inputs.highest_dmg)
                + inputs.dancing_coral5)
                / 100.0,
            "Upgrades 5, 22 and 28 × LOG(best hit) + Dancing Coral",
        ),
        button,
        factor(
            "Arcade 62",
            1.0 + inputs.arcade62 / 100.0,
            "Arcade upgrade 62",
        ),
        factor("Atom 13", 1.0 + inputs.atom13 / 100.0, "Atom collider 13"),
        factor(
            "Legend 40",
            1.0 + inputs.legend40 / 100.0,
            "Spelunking Legend talent 40",
        ),
        factor(
            "Grid 147 + 166 + meals",
            1.0 + (inputs.grid[2] + inputs.grid[3] + inputs.meal_mine_curr) / 100.0,
            "Shared passive pool",
        ),
        factor(
            "Event shop 44",
            1.0 + inputs.event_shop_44,
            "Event shop item 44",
        ),
        factor(
            "W7 currency task",
            1.0 + 10.0 * inputs.task_currency_level / 100.0,
            "TaskZZ2[6][4]",
        ),
    ];
    FormulaNode::new("Minehead Currency/hr", total)
        .with_format(ValueFormat::Rate)
        .with_note("Grid 129 × every account factor")
        .with_children(children)
}

pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let minehead = &mut state.research.minehead;
    let Some(inputs) = minehead.inputs.as_ref() else {
        return crate::ui::section::no_save(cx);
    };
    let tree = state.minehead_breakdown.clone();
    // The tree is rebuilt only when the inputs it reads changed.
    let total = match minehead.currency_total {
        Some(total) => total,
        None => {
            let root = currency_tree(inputs);
            let total = root.value;
            tree.update(cx, |tree, cx| tree.set_root(root, cx));
            minehead.currency_total = Some(total);
            total
        }
    };

    let muted = cx.theme().muted_foreground;
    div()
        .id("minehead-currency")
        .flex()
        .flex_col()
        .gap_3()
        .min_h_0()
        .child(
            Card::new()
                .header(
                    CardHeader::new("Currency/hr")
                        .description(format!("/hr · {}", format::value(total))),
                )
                .child(
                    div()
                        .id("minehead-currency-tree")
                        .h(gpui::px(460.0))
                        .child(tree),
                )
                .child(
                    div()
                        .text_size(TEXT_CAPTION)
                        .text_color(muted)
                        .child(
                            "Every factor comes from the loaded save; a missing field stays at ×1.",
                        ),
                ),
        )
        .into_any_element()
}
