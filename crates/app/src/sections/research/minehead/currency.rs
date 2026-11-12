//! The Minehead currency pane: every factor the currency/hr formula reads, as
//! one breakdown tree (`renderCurrencyTab`, `js/ui/minehead-dashboard.js`).

use crate::state::AppState;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::ActiveTheme as _;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_common::formula::{FormulaNode, ValueFormat};
use idleon_minehead::game;
use idleon_ui::format;

pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(inputs) = state.research.minehead.inputs.clone() else {
        return crate::ui::section::no_save(cx);
    };
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
    let root = FormulaNode::new("Minehead Currency/hr", total)
        .with_format(ValueFormat::Rate)
        .with_note("Grid 129 × every account factor")
        .with_children(children);
    let tree = state.minehead_breakdown.clone();
    tree.update(cx, |tree, cx| tree.set_root(root, cx));

    let muted = cx.theme().colors.fg_muted;
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
                        .px_3()
                        .pb_3()
                        .text_size(gpui::px(11.0))
                        .text_color(muted)
                        .child(
                            "Every factor comes from the loaded save; a missing field stays at ×1.",
                        ),
                ),
        )
        .into_any_element()
}
