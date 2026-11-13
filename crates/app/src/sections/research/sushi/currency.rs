//! The Sushi Station currency pane: the bucks multiplier, the per-slot totals
//! and the two fuel trees, as `_renderCurrency` in `js/ui/sushi-ui.js`.

use crate::state::AppState;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::ActiveTheme as _;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_common::formula::{FormulaNode, ValueFormat};
use idleon_sushi::data::game;
use idleon_sushi::math;
use idleon_sushi::{External, Station};

/// A factor node: a true multiplier.
fn factor(label: impl Into<String>, value: f64, note: impl Into<String>) -> FormulaNode {
    FormulaNode::new(label, value)
        .with_format(ValueFormat::Multiplier)
        .with_note(note)
}

/// A percentage node.
fn percent(label: impl Into<String>, value: f64, note: impl Into<String>) -> FormulaNode {
    FormulaNode::new(label, value)
        .with_format(ValueFormat::Percent)
        .with_note(note)
}

/// The upgrade nodes that add into one percentage pool.
fn upgrade_pool(
    station: &Station,
    upgrades: &[usize],
    labels: &[&'static str],
    note: impl Fn(f64) -> String,
) -> Vec<FormulaNode> {
    upgrades
        .iter()
        .zip(labels)
        .map(|(upg, label)| percent(*label, station.qty(*upg), note(station.level(*upg))))
        .collect()
}

/// The bucks/hr tree.
fn bucks_tree(inputs: &idleon_sushi::Inputs) -> FormulaNode {
    let data = game();
    let station = &inputs.station;
    let ext = &inputs.external;
    let levels = &station.levels;
    let totals = station.knowledge_totals();
    let multi = station.currency_multi(levels, ext);
    let total = station.bucks_per_hour(levels, ext);
    let surcharge = (30..=34).map(|upg| station.qty(upg)).sum::<f64>();
    let knowledge = totals.first().copied().unwrap_or(0.0);
    let overtuned = math::compute_overtuned_multi(station.spa());
    let no_tax = station.qty(40);
    let wage = station.qty(41);
    let vision = station.qty(43);
    let sailing = ext.sailing39;
    let combo = math::combo_multiplier(station.combo_score());
    let labels = ["Sushi 30", "Sushi 31", "Sushi 32", "Sushi 33", "Sushi 34"];
    let surcharges = FormulaNode::new("Customer Surcharges I-V", surcharge)
        .with_format(ValueFormat::Percent)
        .with_children(upgrade_pool(
            station,
            &[30, 31, 32, 33, 34],
            &labels,
            |level| format!("Lv {level:.0}"),
        ));
    let multiplier = FormulaNode::new("Currency Multiplier", multi)
        .with_format(ValueFormat::Multiplier)
        .with_children(vec![
            factor("Arcade 67", 1.0 + ext.arcade67 / 100.0, "Arcade upgrade 67"),
            factor(
                format!("1.1^Unique Sushi ({})", station.unique),
                1.1_f64.powi(station.unique as i32),
                "One factor per discovered tier",
            ),
            factor(
                "Bundle V",
                if ext.bundle_v { 2.0 } else { 1.0 },
                if ext.bundle_v {
                    "BundlesReceived.bon_v owned"
                } else {
                    "BundlesReceived.bon_v not owned"
                },
            ),
            FormulaNode::new("Customer Surcharges", 1.0 + surcharge / 100.0)
                .with_format(ValueFormat::Multiplier)
                .with_children(vec![
                    surcharges,
                    percent("Super Bit 67", ext.super_bit67 * 100.0, "Gaming"),
                ]),
            factor("Knowledge 0", 1.0 + knowledge / 100.0, "Bucks knowledge"),
            factor(
                "Grid 189",
                1.0 + ext.grid189 / 100.0,
                "Research grid square",
            ),
            factor(
                "Sushi 40",
                1.0 + no_tax / 100.0,
                format!("Lv {:.0}", station.level(40)),
            ),
            factor(
                "Minehead Floor 11",
                (1.0 + ext.minehead11 / 100.0).clamp(1.0, 1.25),
                "Capped at 1.25x",
            ),
            factor(
                "Hourly Wage + Tier Vision",
                1.0 + (wage + vision) / 100.0,
                "Sushi 41 and 43",
            ),
            factor(
                "Overtuned Multi",
                1.0 + overtuned / 100.0,
                "Hoarded spa amount",
            ),
            factor("Button 2", 1.0 + ext.button2 / 100.0, "OptLacc 594 presses"),
            factor("Atom 14", 1.0 + ext.atom14 / 100.0, "Atom collider"),
            factor("Artifact 39", 1.0 + sailing / 100.0, "Sailing artifact 39"),
            factor(
                "Combo Meter",
                combo,
                format!("Score {}", idleon_sushi::fmt_num(station.combo_score())),
            ),
        ]);
    let mut slots: Vec<(usize, f64, f64)> = (0..data.max_slots)
        .map(|slot| {
            (
                slot,
                station.tiers.get(slot).copied().unwrap_or(-1.0),
                station.currency_per_slot(slot, multi),
            )
        })
        .filter(|(_, _, bucks)| *bucks > 0.0)
        .collect();
    slots.sort_by(|left, right| right.2.total_cmp(&left.2));
    let mut children = Vec::new();
    for (slot, tier, bucks) in slots.iter().take(15) {
        children.push(
            FormulaNode::new(format!("#{slot} (T{})", tier + 1.0), *bucks)
                .with_format(ValueFormat::Rate)
                .with_note(format!("{:.1}% of total", 100.0 * bucks / total)),
        );
    }
    if slots.len() > 15 {
        let rest: f64 = slots.iter().skip(15).map(|(_, _, bucks)| bucks).sum();
        children.push(
            FormulaNode::new(format!("Other ({} slots)", slots.len() - 15), rest)
                .with_format(ValueFormat::Rate),
        );
    }
    let per_slot = FormulaNode::new(format!("Per-Slot Totals ({} active)", slots.len()), total)
        .with_format(ValueFormat::Rate)
        .with_children(children);
    FormulaNode::new("Bucks/hr", total)
        .with_format(ValueFormat::Rate)
        .with_note("Currency multiplier × every active slot")
        .with_children(vec![multiplier, per_slot])
}

/// The fuel generation tree.
fn fuel_tree(inputs: &idleon_sushi::Inputs) -> FormulaNode {
    let station = &inputs.station;
    let ext = &inputs.external;
    let levels = &station.levels;
    let totals = station.knowledge_totals();
    let fuel_gen = station.fuel_gen(levels, ext);
    let upgrade_fuel = (8..=12).map(|upg| station.qty(upg)).sum::<f64>();
    let unique = (1.0 + station.level(9) / 100.0)
        * (1.0 + station.level(10) / 100.0)
        * (1.0 + station.level(11) / 100.0)
        * (1.0 + station.level(12) / 100.0);
    let labels = ["Sushi 8", "Sushi 9", "Sushi 10", "Sushi 11", "Sushi 12"];
    let fastburn = FormulaNode::new("Fastburn Fuel I-V", 1.0 + upgrade_fuel / 100.0)
        .with_format(ValueFormat::Multiplier)
        .with_children(upgrade_pool(
            station,
            &[8, 9, 10, 11, 12],
            &labels,
            |level| format!("Lv {level}"),
        ));
    let spec = |tier: usize| station.knowledge_bonus(tier);
    FormulaNode::new("Fuel Gen/hr", fuel_gen)
        .with_format(ValueFormat::Rate)
        .with_children(vec![
            FormulaNode::leaf("Base", 50.0, ValueFormat::Raw),
            fastburn,
            factor(
                "Orange Fire Sum",
                1.0 + station.orange_fire_sum() / 100.0,
                format!("+{:.1}% from red fireplaces", station.orange_fire_sum()),
            ),
            factor(
                "Knowledge 4",
                1.0 + totals.get(4).copied().unwrap_or(0.0) / 100.0,
                "Fuel generation knowledge",
            ),
            factor(
                "Knowledge Tier 27",
                1.0 + spec(27) / 100.0,
                "Tier-specific knowledge",
            ),
            factor(
                "Knowledge Tier 36",
                1.0 + spec(36) / 100.0,
                "Tier-specific knowledge",
            ),
            factor(
                "Knowledge Tier 45",
                1.0 + spec(45) / 100.0,
                "Tier-specific knowledge",
            ),
            FormulaNode::new("Fastburn II-V Unique Multi", unique)
                .with_format(ValueFormat::Multiplier)
                .with_children(
                    (9..=12)
                        .map(|upg| {
                            factor(
                                format!("FB {}", ["I", "II", "III", "IV", "V"][upg - 8]),
                                1.0 + station.level(upg) / 100.0,
                                format!("Lv {:.0}", station.level(upg)),
                            )
                        })
                        .collect::<Vec<_>>(),
                ),
            factor(
                "Bundle V",
                if ext.bundle_v { 2.0 } else { 1.0 },
                "BundlesReceived.bon_v",
            ),
        ])
}

/// The fuel capacity tree.
fn capacity_tree(inputs: &idleon_sushi::Inputs) -> FormulaNode {
    let station = &inputs.station;
    let ext = &inputs.external;
    let levels = &station.levels;
    let totals = station.knowledge_totals();
    let capacity = station.fuel_capacity(levels, ext);
    let upgrade_cap = (1..=5).map(|upg| station.qty(upg)).sum::<f64>();
    let unique = (1.0 + station.level(2) / 100.0)
        * (1.0 + station.level(3) / 100.0)
        * (1.0 + station.level(4) / 100.0)
        * (1.0 + station.level(5) / 100.0);
    let labels = ["Sushi 1", "Sushi 2", "Sushi 3", "Sushi 4", "Sushi 5"];
    let capacity_nodes = FormulaNode::new("Fuel Cap I-V", 1.0 + upgrade_cap / 100.0)
        .with_format(ValueFormat::Multiplier)
        .with_children(upgrade_pool(station, &[1, 2, 3, 4, 5], &labels, |level| {
            format!("Lv {level}")
        }));
    FormulaNode::new("Fuel Capacity", capacity).with_children(vec![
        factor(
            "Fuel Cap event purchase",
            1.0 + 9.0 * ext.event_shop66,
            "Event shop item 66",
        ),
        FormulaNode::leaf("Base", 200.0, ValueFormat::Raw),
        percent(
            "Knowledge 3",
            totals.get(3).copied().unwrap_or(0.0),
            "Fuel capacity knowledge",
        ),
        capacity_nodes,
        FormulaNode::new("Fuel Cap II-V Unique Multi", unique)
            .with_format(ValueFormat::Multiplier)
            .with_children(
                (2..=5)
                    .map(|upg| {
                        factor(
                            format!("FC {}", ["I", "II", "III", "IV", "V"][upg - 1]),
                            1.0 + station.level(upg) / 100.0,
                            format!("Lv {:.0}", station.level(upg)),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
        factor(
            "Bundle V",
            if ext.bundle_v { 2.0 } else { 1.0 },
            "BundlesReceived.bon_v",
        ),
    ])
}

pub(super) fn pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(inputs) = state.research.sushi.inputs.clone() else {
        return crate::ui::section::no_save(cx);
    };
    let station = &inputs.station;
    let ext: &External = &inputs.external;
    let levels = &station.levels;
    let fuel_gen = station.fuel_gen(levels, ext);
    let capacity = station.fuel_capacity(levels, ext);
    let total = station.bucks_per_hour(levels, ext);
    let fill = if fuel_gen > 0.0 {
        Some(capacity / fuel_gen)
    } else {
        None
    };
    let muted = cx.theme().colors.fg_muted;

    let bucks = state.sushi_bucks.clone();
    bucks.update(cx, |tree, cx| tree.set_root(bucks_tree(&inputs), cx));
    let fuel = state.sushi_fuel.clone();
    fuel.update(cx, |tree, cx| tree.set_root(fuel_tree(&inputs), cx));
    let cap = state.sushi_capacity.clone();
    cap.update(cx, |tree, cx| tree.set_root(capacity_tree(&inputs), cx));

    let summary = format!(
        "{} bucks/hr · {} fuel/hr · {} capacity",
        idleon_sushi::fmt_num(total),
        idleon_sushi::fmt_num(fuel_gen),
        idleon_sushi::fmt_num(capacity)
    );
    let column = |id: &'static str, title: &'static str, tree: AnyElement| {
        Card::new()
            .header(CardHeader::new(title))
            .child(div().id(id).h(px(430.0)).p_3().child(tree))
    };

    div()
        .id("sushi-currency")
        .flex()
        .flex_col()
        .gap_3()
        .min_h_0()
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    gpui::div()
                        .id("su-currency-summary")
                        .child(summary)
                        .text_color(muted),
                )
                .child(match fill {
                    Some(hours) => div()
                        .text_color(muted)
                        .child(format!("fills in {hours:.1}h")),
                    None => div(),
                }),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .min_h_0()
                .child(div().flex_1().min_w_0().child(column(
                    "sushi-bucks-tree",
                    "Bucks/hr",
                    bucks.into_any_element(),
                )))
                .child(div().flex_1().min_w_0().child(column(
                    "sushi-fuel-tree",
                    "Fuel Generation",
                    fuel.into_any_element(),
                )))
                .child(div().flex_1().min_w_0().child(column(
                    "sushi-capacity-tree",
                    "Fuel Capacity",
                    cap.into_any_element(),
                ))),
        )
        .into_any_element()
}
