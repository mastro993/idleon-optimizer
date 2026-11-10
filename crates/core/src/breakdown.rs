//! Research EXP breakdown shared by the Rates pane and the formula tests.
//!
//! The tree mirrors the factors in [`crate::sim::exp_rate`]: the observation
//! base, the additive grid/account bonuses, Takin' Notes and the account
//! multiplier. Its root result is the same rate the KPI cards show, and every
//! additive group sums to the value the section reports.

use crate::data::game;
use crate::formula::{FormulaNode, ValueFormat};
use crate::model::{SimContext, Simulation, coord};
use crate::sim;

/// The full research EXP/hour formula tree.
pub fn exp(sim: &Simulation, ctx: &SimContext) -> FormulaNode {
    let rate = sim::exp_rate(sim, ctx);
    let overlay = sim.overlay();
    let game_data = game();
    let to_find = sim::observations_to_find(sim.research_level, &sim.observations.found);
    let kal_map = sim::kaleidoscope_map(&sim.mags, game_data.observations.len());
    let kal_base = sim::kalei_base(&sim.grid, &overlay, ctx);
    let insight_bonus = sim::grid_bonus(&sim.grid, &overlay, 93, ctx);

    let observations = observation_group(sim, to_find, &kal_map, kal_base, insight_bonus);
    let additive = additive_group(sim, ctx, &overlay, to_find);
    let takin = takin_notes(sim, ctx, &overlay);
    let account = FormulaNode::leaf(
        "Account multiplier",
        ctx.extra_multiplier,
        ValueFormat::Multiplier,
    )
    .with_input("Supplements · extra account ×")
    .with_source("Supplements");

    FormulaNode::new("Research EXP/hr", rate.total)
        .with_format(ValueFormat::Rate)
        .with_input(format!(
            "{} × total multiplier",
            ValueFormat::Rate.format(rate.obs_base)
        ))
        .with_multiplier(rate.multi)
        .with_source("research-math.js · total research EXP")
        .with_note("Observation base × additive bonus × Takin' Notes × account multiplier.")
        .with_children([observations, additive, takin, account])
}

fn observation_group(
    sim: &Simulation,
    to_find: usize,
    kal_map: &[u32],
    kal_base: f64,
    insight_bonus: f64,
) -> FormulaNode {
    let game_data = game();
    let mut per_slot = vec![0_u32; game_data.observations.len()];
    for mag in &sim.mags {
        if mag.kind == crate::model::MagKind::Regular
            && let Some(slot) = mag.slot
            && slot < to_find
            && sim::observation_usable(slot, sim.research_level, &sim.observations)
        {
            per_slot[slot] += 1;
        }
    }

    let mut children = Vec::new();
    for (obs, &count) in per_slot.iter().enumerate().take(to_find) {
        if count == 0 {
            continue;
        }
        let level = sim
            .observations
            .insight_level
            .get(obs)
            .copied()
            .unwrap_or(0);
        let base = sim::obs_base_exp(obs);
        let insight_multi = 1.0 + insight_bonus * level as f64 / 100.0;
        let neighbours = kal_map.get(obs).copied().unwrap_or(0);
        let kaleido_multi = 1.0 + neighbours as f64 * kal_base;
        let value = count as f64 * base * insight_multi * kaleido_multi;
        let name = game_data
            .observations
            .get(obs)
            .map(|observation| observation.name.replace('_', " "))
            .unwrap_or_else(|| format!("Observation {}", obs + 1));
        children.push(
            FormulaNode::new(format!("{} · {name}", obs + 1), value)
                .with_format(ValueFormat::Rate)
                .with_input(format!(
                    "{count} × {} × {insight_multi:.4} × {kaleido_multi:.4}",
                    ValueFormat::Rate.format(base)
                ))
                .with_multiplier(insight_multi * kaleido_multi)
                .with_source("Magnifying glasses on this observation")
                .with_children([
                    FormulaNode::leaf("Base EXP", base, ValueFormat::Rate)
                        .with_source("Observation base EXP"),
                    FormulaNode::leaf("Glasses", count as f64, ValueFormat::Count)
                        .with_source("Regular magnifiers placed here"),
                    FormulaNode::leaf("Insight", insight_multi, ValueFormat::Multiplier)
                        .with_input(format!("Level {level} × grid insight {insight_bonus:.2}%"))
                        .with_source("Game Design 101"),
                    FormulaNode::leaf("Kaleidoscope", kaleido_multi, ValueFormat::Multiplier)
                        .with_input(format!("{neighbours} neighbours × {kal_base:.4}"))
                        .with_source("Neighbouring kaleidoscopes"),
                ]),
        );
    }

    let glasses: u32 = per_slot.iter().take(to_find).sum();
    FormulaNode::group("Observation base", ValueFormat::Rate, children)
        .with_input(format!("{glasses} regular magnifying glasses"))
        .with_source("Observations and their insight")
}

fn additive_group(
    sim: &Simulation,
    ctx: &SimContext,
    overlay: &[i32],
    to_find: usize,
) -> FormulaNode {
    let mut children = Vec::new();
    for cell in [50, 90, 110, 31, 51] {
        children.push(grid_term(sim, ctx, overlay, cell));
    }

    let found_count = sim
        .observations
        .found
        .iter()
        .take(to_find)
        .filter(|found| **found)
        .count() as f64;
    let insight_sum: u32 = sim.observations.insight_level.iter().take(to_find).sum();
    children.push(scaled_grid_term(
        sim,
        ctx,
        overlay,
        112,
        found_count,
        "Insights found",
        "Found observations",
    ));
    children.push(scaled_grid_term(
        sim,
        ctx,
        overlay,
        94,
        insight_sum as f64,
        "Total insight levels",
        "Insight levels across found observations",
    ));
    children.push(
        FormulaNode::leaf("Extra account %", ctx.extra_pct, ValueFormat::Percent)
            .with_input("Supplements · extra account percentage")
            .with_source("Supplements"),
    );

    let group = FormulaNode::group("Additive bonus", ValueFormat::Percent, children)
        .with_input("Grid and account percentages, added")
        .with_source("Grid bonuses and supplements");
    let multiplier = 1.0 + group.value / 100.0;
    group.with_multiplier(multiplier)
}

fn takin_notes(sim: &Simulation, ctx: &SimContext, overlay: &[i32]) -> FormulaNode {
    let bonus = sim::grid_bonus(&sim.grid, overlay, 70, ctx);
    let grid = grid_term(sim, ctx, overlay, 70);
    FormulaNode::new("Takin' Notes", 1.0 + bonus / 100.0)
        .with_format(ValueFormat::Multiplier)
        .with_input(format!("1 + {bonus:.2}%"))
        .with_multiplier(1.0 + bonus / 100.0)
        .with_source(grid.source.clone().unwrap_or_else(|| "Grid".into()))
        .with_note("A separate multiplicative bonus.")
        .with_children([grid])
}

fn scaled_grid_term(
    sim: &Simulation,
    ctx: &SimContext,
    overlay: &[i32],
    cell: usize,
    count: f64,
    count_label: &str,
    count_source: &str,
) -> FormulaNode {
    let per = grid_term(sim, ctx, overlay, cell);
    let value = per.value * count;
    let label = per.label.clone();
    let source = per.source.clone();
    FormulaNode::new(format!("{label} × {count_label}"), value)
        .with_format(ValueFormat::Percent)
        .with_input(format!("{} × {count}", per.format.format(per.value)))
        .with_source(source.unwrap_or_else(|| "Grid".into()))
        .with_children([
            per,
            FormulaNode::leaf(count_label, count, ValueFormat::Count).with_source(count_source),
        ])
}

fn grid_term(sim: &Simulation, ctx: &SimContext, overlay: &[i32], cell: usize) -> FormulaNode {
    let level = sim.grid.level(cell);
    let node = game().node_at(cell);
    let label = node
        .map(|node| node.name.replace('_', " "))
        .unwrap_or_else(|| format!("Grid {}", coord(cell)));
    let source = format!("Grid {} · {label}", coord(cell));
    let base = node.map(|node| node.bonus_per_lv).unwrap_or(0.0);
    let value = sim::grid_bonus(&sim.grid, overlay, cell, ctx);

    let shape = overlay
        .get(cell)
        .copied()
        .and_then(|shape| usize::try_from(shape).ok())
        .and_then(|shape| game().shapes.get(shape));
    let shape_multi = shape
        .map(|shape| 1.0 + shape.bonus_pct / 100.0)
        .unwrap_or(1.0);
    let term = FormulaNode::new(label, value)
        .with_format(ValueFormat::Percent)
        .with_input(format!("{base}% × Lv {level}"))
        .with_multiplier(shape_multi * ctx.all_bonus_multi)
        .with_source(source);
    match shape {
        Some(shape) if level > 0 => term.with_note(format!(
            "Shape: {} (+{}%) · all-bonus ×{:.4}.",
            shape.name, shape.bonus_pct, ctx.all_bonus_multi
        )),
        Some(shape) => term.with_note(format!("Covered by shape {} but not leveled.", shape.name)),
        None if level == 0 => term.with_note("Square not leveled."),
        None => term,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Mag, MagKind, Observations, Placement, SimContext, Simulation};

    fn filled() -> (Simulation, SimContext) {
        let ctx = SimContext::default();
        let mut sim = Simulation {
            research_level: 100,
            grid: crate::model::GridState::seeded(2),
            observations: Observations::all_at(2),
            ..Simulation::default()
        };
        sim.grid.set(112, 2);
        sim.grid.set(94, 2);
        sim.grid.set(70, 2);
        sim.placements = vec![Placement {
            shape: 0,
            x: 0.0,
            y: 0.0,
            rot: 0.0,
        }];
        sim.mags = (0..8)
            .map(|slot| Mag::new(MagKind::Regular, Some(slot)))
            .collect();
        sim.observations.found.fill(true);
        (sim, ctx)
    }

    #[test]
    fn the_root_matches_the_rate_the_section_shows() {
        let (sim, ctx) = filled();
        let tree = exp(&sim, &ctx);
        let rate = sim::exp_rate(&sim, &ctx);
        assert!((tree.value - rate.total).abs() < 1e-6);
        assert_eq!(tree.multiplier, Some(rate.multi));
        assert_eq!(tree.format, ValueFormat::Rate);
    }

    #[test]
    fn observation_children_sum_to_the_observation_base() {
        let (sim, ctx) = filled();
        let tree = exp(&sim, &ctx);
        let rate = sim::exp_rate(&sim, &ctx);
        let observations = &tree.children[0];
        assert_eq!(observations.label, "Observation base");
        assert!((observations.value - rate.obs_base).abs() < 1e-6);
        assert!((observations.sum_children() - rate.obs_base).abs() < 1e-6);
        assert!(!observations.children.is_empty());
    }

    #[test]
    fn additive_children_sum_to_the_additive_group() {
        let (sim, ctx) = filled();
        let tree = exp(&sim, &ctx);
        let additive = &tree.children[1];
        assert_eq!(additive.label, "Additive bonus");
        assert!(additive.value > 0.0);
        assert!((additive.sum_children() - additive.value).abs() < 1e-6);
    }

    #[test]
    fn observation_products_reproduce_their_node_values() {
        let (sim, ctx) = filled();
        let tree = exp(&sim, &ctx);
        for observation in &tree.children[0].children {
            let product: f64 = observation
                .children
                .iter()
                .map(|child| child.value)
                .product();
            assert!(
                (product - observation.value).abs() < 1e-6,
                "{}: {product} != {}",
                observation.label,
                observation.value
            );
        }
    }

    #[test]
    fn search_finds_grid_names_and_source_lines() {
        let (sim, ctx) = filled();
        let tree = exp(&sim, &ctx);
        assert!(tree.match_count("Takin' Notes") >= 1);
        assert!(tree.match_count("grid") > 0);
        assert!(tree.match_count("research-math") >= 1);
        assert_eq!(tree.match_count("note that cannot exist"), 0);
    }
}
