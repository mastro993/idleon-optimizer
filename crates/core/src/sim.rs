//! Research EXP, insight, level curves and forward projection.
//!
//! Formulas are ports of `js/stats/systems/w7/research-math.js` in the upstream
//! calculators, minus the save-specific multipliers this app leaves to the user.

use crate::data::{game, ShapeDef};
use crate::model::{GridState, Mag, MagKind, Observations, SimContext, Simulation, Target};

/// Research EXP per hour, split into its factors for the dashboard.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExpRate {
    /// EXP/hour from observations with regular magnifiers.
    pub obs_base: f64,
    /// The multiplier the grid and account bonuses apply.
    pub multi: f64,
    /// `obs_base * multi`.
    pub total: f64,
}

/// What the level curves say about a run.
#[derive(Clone, Debug, Default)]
pub struct Projection {
    /// Hours until the target is met.
    pub hours: f64,
    /// Research level at the end.
    pub final_level: u32,
    /// Research EXP earned over the run.
    pub total_exp: f64,
    /// Insight levels gained over the run.
    pub insight_gained: u32,
    /// Rate samples, oldest first.
    pub samples: Vec<Sample>,
}

/// One point on the projection's rate curve.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub hours: f64,
    pub level: u32,
    pub rate: f64,
}

/// Grid points earned, spent and free at the current level.
#[derive(Clone, Copy, Debug, Default)]
pub struct GridPoints {
    pub earned: u32,
    pub spent: u32,
    pub free: u32,
}

/// A rate as `1.23M` or `845K`, for step descriptions.
pub fn format_rate(value: f64) -> String {
    if value >= 1e9 {
        format!("{:.2}B", value / 1e9)
    } else if value >= 1e6 {
        format!("{:.2}M", value / 1e6)
    } else if value >= 1e3 {
        format!("{:.1}K", value / 1e3)
    } else {
        format!("{value:.0}")
    }
}

/// The base EXP of one observation with a single magnifier.
pub fn obs_base_exp(obs: usize) -> f64 {
    let t = obs as f64;
    (4.0 + (t / 2.0 + (t / 4.0).floor())) * (1.0 + t.powf(1.0 + t / 15.0 * 0.4) / 10.0)
        + (t.powf(1.5) + 1.5 * t)
}

/// Insight EXP needed to take an observation from `level` to `level + 1`.
pub fn insight_exp_req(obs: usize, level: u32) -> f64 {
    let t = obs as f64;
    (2.0 + 0.7 * t) * (1.75 + t / 200.0).powi(level as i32) * (1.0 + t * t / 100.0) + level as f64
}

/// Research EXP needed to go from `level` to `level + 1`.
pub fn research_exp_req(level: u32, server_var_res_xp: f64) -> f64 {
    let t = level as f64;
    let scale = server_var_res_xp.max(1.0);
    10.0 * (t - 1.0).max(0.0)
        + 10.0
            * (1.0 + t.powf(1.0 + t / 10.0 * 0.4) / 10.0)
            * 1.1_f64.powf(t)
            * scale.powf((level as f64 - 20.0).max(0.0))
}

/// Magnifiers one observation holds at a research level.
pub fn mag_cap(research_level: u32) -> u32 {
    let lv = research_level;
    (1.0 + (lv / 40).min(1) as f64 + (lv / 70).min(1) as f64 + (lv / 120).min(1) as f64)
        .round()
        .min(4.0) as u32
}

/// How many observations exist at a research level.
pub fn observations_to_find(research_level: u32, found: &[bool]) -> usize {
    if research_level < 1 {
        return 0;
    }
    if !found.first().copied().unwrap_or(false) {
        return 1;
    }
    let lv = research_level;
    (5 * ((lv + 10) / 10) - lv / 20 - lv / 30 - lv / 50).min(game().observations.len() as u32)
        as usize
}

/// Is an observation usable: found, and the research level high enough for it.
pub fn observation_usable(obs: usize, research_level: u32, observations: &Observations) -> bool {
    let data = &game().observations[obs];
    observations.found.get(obs).copied().unwrap_or(false) && research_level as f64 >= data.roll_req
}

/// Grid bonus of one cell, with the shapes covering it and the all-bonus multiplier.
pub fn grid_bonus(grid: &GridState, overlay: &[i32], cell: usize, ctx: &SimContext) -> f64 {
    let game = game();
    let Some(node) = game.node_at(cell) else {
        return 0.0;
    };
    let level = grid.level(cell);
    if level == 0 {
        return 0.0;
    }
    let mut value = node.bonus_per_lv * level as f64;
    let shape = overlay.get(cell).copied().unwrap_or(-1);
    if let Some(shape) = usize::try_from(shape).ok().and_then(|s| game.shapes.get(s)) {
        value *= 1.0 + shape.bonus_pct / 100.0;
    }
    value * ctx.all_bonus_multi
}

/// Kaleidoscope neighbours: the observation grid is eight wide.
pub fn kaleidoscope_map(mags: &[Mag], slots: usize) -> Vec<u32> {
    let mut map = vec![0_u32; slots.max(1)];
    for mag in mags {
        if mag.kind != MagKind::Kaleidoscope {
            continue;
        }
        let Some(slot) = mag.slot else { continue };
        let col = slot % 8;
        if col != 7 && slot + 1 < map.len() {
            map[slot + 1] += 1;
        }
        if col != 0 {
            map[slot - 1] += 1;
        }
        if slot > 7 {
            map[slot - 8] += 1;
        }
        // The last row of the observation grid has no slot below it; the map is
        // only as wide as the observations this app models.
        if slot + 8 < map.len() {
            map[slot + 8] += 1;
        }
    }
    map
}

/// The multiplier a kaleidoscope adds, from the grid's own kaleidoscope nodes.
pub fn kalei_base(grid: &GridState, overlay: &[i32], ctx: &SimContext) -> f64 {
    (30.0 + grid_bonus(grid, overlay, 52, ctx) + grid_bonus(grid, overlay, 72, ctx)) / 100.0
}

/// Insight EXP per hour one observation earns from its monocles.
pub fn insight_rate(
    obs: usize,
    sim: &Simulation,
    overlay: &[i32],
    ctx: &SimContext,
    kal_map: &[u32],
    kal_base: f64,
) -> f64 {
    if !observation_usable(obs, sim.research_level, &sim.observations) {
        return 0.0;
    }
    let count = sim
        .mags
        .iter()
        .filter(|mag| mag.kind == MagKind::Monocle && mag.slot == Some(obs))
        .count() as f64;
    if count == 0.0 {
        return 0.0;
    }
    let bonus = grid_bonus(&sim.grid, overlay, 92, ctx) + grid_bonus(&sim.grid, overlay, 91, ctx);
    let extra = kal_map.get(obs).copied().unwrap_or(0) as f64;
    3.0 * count * (1.0 + bonus / 100.0) * (1.0 + extra * kal_base)
}

/// Total research EXP per hour for a full state.
pub fn exp_rate(sim: &Simulation, ctx: &SimContext) -> ExpRate {
    let game = game();
    let overlay = sim.overlay();
    let to_find = observations_to_find(sim.research_level, &sim.observations.found);
    let kal_map = kaleidoscope_map(&sim.mags, game.observations.len());
    let kal_base = kalei_base(&sim.grid, &overlay, ctx);
    let insight_bonus = grid_bonus(&sim.grid, &overlay, 93, ctx);

    let mut per_slot = vec![0_u32; game.observations.len()];
    for mag in &sim.mags {
        if mag.kind == MagKind::Regular
            && let Some(slot) = mag.slot
                && slot < to_find && observation_usable(slot, sim.research_level, &sim.observations) {
                    per_slot[slot] += 1;
                }
    }

    let mut obs_total = 0.0;
    for obs in 0..to_find {
        let count = per_slot[obs];
        if count == 0 {
            continue;
        }
        let level = sim.observations.insight_level.get(obs).copied().unwrap_or(0);
        let kal_multi = 1.0 + kal_map[obs] as f64 * kal_base;
        obs_total += count as f64
            * obs_base_exp(obs)
            * (1.0 + insight_bonus * level as f64 / 100.0)
            * kal_multi;
    }

    let additive_base = grid_bonus(&sim.grid, &overlay, 50, ctx)
        + grid_bonus(&sim.grid, &overlay, 90, ctx)
        + grid_bonus(&sim.grid, &overlay, 110, ctx)
        + grid_bonus(&sim.grid, &overlay, 31, ctx)
        + grid_bonus(&sim.grid, &overlay, 51, ctx)
        + ctx.extra_pct;

    let found_count = sim
        .observations
        .found
        .iter()
        .take(to_find)
        .filter(|found| **found)
        .count() as f64;
    let insight_sum: u32 = sim
        .observations
        .insight_level
        .iter()
        .take(to_find)
        .sum();

    let additive = additive_base
        + grid_bonus(&sim.grid, &overlay, 112, ctx) * found_count
        + grid_bonus(&sim.grid, &overlay, 94, ctx) * insight_sum as f64;
    let takin_notes = grid_bonus(&sim.grid, &overlay, 70, ctx);

    let multi = (1.0 + additive / 100.0) * (1.0 + takin_notes / 100.0) * ctx.extra_multiplier;
    ExpRate {
        obs_base: obs_total,
        multi,
        total: obs_total * multi,
    }
}

/// Research EXP/hour when every cell in `cells` carries a shape of `bonus_pct`.
pub fn rate_with_shape(
    sim: &Simulation,
    ctx: &SimContext,
    overlay: &mut Vec<i32>,
    cells: &[usize],
    bonus_pct: f64,
) -> f64 {
    let mut touched = Vec::with_capacity(cells.len());
    for cell in cells {
        if overlay.get(*cell).copied().unwrap_or(-1) < 0 {
            overlay[*cell] = 0;
            touched.push(*cell);
        }
    }
    // Shapes with a different bonus need their own multiplier; score the 25%
    // baseline here and let the caller scale, as the upstream optimizer does.
    let mut shaped = sim.clone();
    shaped.placements.clear();
    let mask = std::mem::take(overlay);
    let raw = shaped_rate_with_overlay(&shaped, ctx, &mask, bonus_pct);
    *overlay = mask;
    for cell in touched {
        overlay[cell] = -1;
    }
    raw
}

fn shaped_rate_with_overlay(
    sim: &Simulation,
    ctx: &SimContext,
    overlay: &[i32],
    bonus_pct: f64,
) -> f64 {
    let game = game();
    let to_find = observations_to_find(sim.research_level, &sim.observations.found);
    let kal_map = kaleidoscope_map(&sim.mags, game.observations.len());
    let kal_base = kalei_base(&sim.grid, overlay, ctx);
    let insight_bonus = grid_bonus_with(&sim.grid, overlay, 93, ctx, bonus_pct);

    let mut per_slot = vec![0_u32; game.observations.len()];
    for mag in &sim.mags {
        if mag.kind == MagKind::Regular
            && let Some(slot) = mag.slot
                && slot < to_find && observation_usable(slot, sim.research_level, &sim.observations) {
                    per_slot[slot] += 1;
                }
    }
    let mut obs_total = 0.0;
    for obs in 0..to_find {
        if per_slot[obs] == 0 {
            continue;
        }
        let level = sim.observations.insight_level.get(obs).copied().unwrap_or(0);
        obs_total += per_slot[obs] as f64
            * obs_base_exp(obs)
            * (1.0 + insight_bonus * level as f64 / 100.0)
            * (1.0 + kal_map[obs] as f64 * kal_base);
    }

    let additive_base = grid_bonus_with(&sim.grid, overlay, 50, ctx, bonus_pct)
        + grid_bonus_with(&sim.grid, overlay, 90, ctx, bonus_pct)
        + grid_bonus_with(&sim.grid, overlay, 110, ctx, bonus_pct)
        + grid_bonus_with(&sim.grid, overlay, 31, ctx, bonus_pct)
        + grid_bonus_with(&sim.grid, overlay, 51, ctx, bonus_pct)
        + ctx.extra_pct;
    let found_count = sim
        .observations
        .found
        .iter()
        .take(to_find)
        .filter(|found| **found)
        .count() as f64;
    let insight_sum: u32 = sim.observations.insight_level.iter().take(to_find).sum();
    let additive = additive_base
        + grid_bonus_with(&sim.grid, overlay, 112, ctx, bonus_pct) * found_count
        + grid_bonus_with(&sim.grid, overlay, 94, ctx, bonus_pct) * insight_sum as f64;
    let takin = grid_bonus_with(&sim.grid, overlay, 70, ctx, bonus_pct);
    obs_total * (1.0 + additive / 100.0) * (1.0 + takin / 100.0) * ctx.extra_multiplier
}

/// Grid bonus under an explicit overlay and one shape bonus, used while scoring.
fn grid_bonus_with(
    grid: &GridState,
    overlay: &[i32],
    cell: usize,
    ctx: &SimContext,
    bonus_pct: f64,
) -> f64 {
    let Some(node) = game().node_at(cell) else {
        return 0.0;
    };
    let level = grid.level(cell);
    if level == 0 {
        return 0.0;
    }
    let mut value = node.bonus_per_lv * level as f64;
    if overlay.get(cell).copied().unwrap_or(-1) >= 0 {
        value *= 1.0 + bonus_pct / 100.0;
    }
    value * ctx.all_bonus_multi
}

/// EXP/hour added by putting a shape on each cell, at the 25% baseline bonus.
pub fn cell_values(sim: &Simulation, ctx: &SimContext) -> Vec<f64> {
    let game = game();
    let base = exp_rate(sim, ctx).total;
    let mut overlay = sim.overlay();
    let mut values = vec![0.0; game.cells()];
    for cell in 0..game.cells() {
        if game.node_at(cell).is_none() || sim.grid.level(cell) == 0 || overlay[cell] >= 0 {
            continue;
        }
        overlay[cell] = 0;
        let shaped = shaped_rate_with_overlay(sim, ctx, &overlay, 25.0);
        overlay[cell] = -1;
        values[cell] = shaped - base;
    }
    values
}

/// Grid points earned at a research level.
pub fn grid_points_earned(research_level: u32, points_node_level: u32, bonus: u32) -> u32 {
    let lv = research_level;
    lv + bonus
        + (lv / 10) * (1 + (lv / 60).min(1) + points_node_level)
}

/// Grid points earned, spent and free.
pub fn grid_points(sim: &Simulation, ctx: &SimContext) -> GridPoints {
    let earned = grid_points_earned(sim.research_level, sim.grid.level(50), ctx.bonus_grid_points);
    let spent = sim.grid.spent();
    GridPoints {
        earned,
        spent,
        free: earned.saturating_sub(spent),
    }
}

/// Shapes the research level and account grant.
pub fn shapes_owned(research_level: u32, ctx: &SimContext) -> usize {
    let lv = research_level;
    let granted = (1..=10)
        .filter(|step| lv >= step * 20)
        .count() as u32;
    (ctx.extra_shapes + granted).min(game().shapes.len() as u32) as usize
}

/// Magnifiers owned: the grid's grants plus the account's.
pub fn magnifiers_owned(sim: &Simulation, ctx: &SimContext) -> u32 {
    let lv = sim.research_level;
    let level_bonus = (lv / 10).min(1) + (lv / 100).min(1) + (lv / 130).min(1) + (lv / 140).min(1);
    (1 + sim.grid.level(72) + sim.grid.level(91) + ctx.extra_magnifiers + level_bonus).min(80)
}

/// Rotations unlock at research level 90.
pub fn can_rotate(research_level: u32) -> bool {
    research_level >= 90
}

/// Advance a level while the accumulated EXP covers the next requirement.
pub fn advance_level(level: &mut u32, exp: &mut f64, server_var_res_xp: f64) {
    while research_level_affordable(*level, *exp, server_var_res_xp) {
        *exp -= research_exp_req(*level, server_var_res_xp);
        *level += 1;
    }
}

fn research_level_affordable(level: u32, exp: f64, server_var_res_xp: f64) -> bool {
    exp >= research_exp_req(level, server_var_res_xp)
}

/// Project research EXP, research levels and insight forward.
pub fn project(sim: &Simulation, ctx: &SimContext, target: Target) -> Projection {
    let mut state = sim.clone();
    let mut hours = 0.0;
    let mut total_exp = 0.0;
    let mut samples = vec![Sample {
        hours: 0.0,
        level: state.research_level,
        rate: exp_rate(&state, ctx).total,
    }];
    let mut insight_gained = 0;

    let max_hours = match target {
        Target::Level(_) => f64::INFINITY,
        Target::Hours(hours) => hours,
    };
    let target_level = match target {
        Target::Level(level) => Some(level),
        Target::Hours(_) => None,
    };

    for _ in 0..20_000 {
        if hours >= max_hours {
            break;
        }
        if let Some(level) = target_level
            && state.research_level >= level {
                break;
            }
        let rate = exp_rate(&state, ctx).total;
        if rate <= 0.0 {
            break;
        }

        // Time to the next research level, or to the target's horizon.
        let exp_req = research_exp_req(state.research_level, ctx.server_var_res_xp);
        let to_level = ((exp_req - state.research_exp) / rate).max(0.0);

        // Time to the next insight level among monocle targets.
        let overlay = state.overlay();
        let kal_map = kaleidoscope_map(&state.mags, game().observations.len());
        let kal_base = kalei_base(&state.grid, &overlay, ctx);
        let mut to_insight = f64::INFINITY;
        let mut insight_obs = 0;
        for obs in 0..game().observations.len() {
            let rate = insight_rate(obs, &state, &overlay, ctx, &kal_map, kal_base);
            if rate <= 0.0 {
                continue;
            }
            let req = insight_exp_req(obs, state.observations.insight_level[obs]);
            let exp = state.observations.insight_exp[obs];
            let hours_left = ((req - exp) / rate).max(0.0);
            if hours_left < to_insight {
                to_insight = hours_left;
                insight_obs = obs;
            }
        }

        let step = to_level.min(to_insight).min(max_hours - hours);
        let step = if step.is_finite() { step.max(1e-9) } else { 0.0 };
        if step <= 0.0 && to_level > 0.0 && to_insight.is_infinite() {
            break;
        }

        hours += step;
        let gained = rate * step;
        state.research_exp += gained;
        total_exp += gained;
        advance_level(
            &mut state.research_level,
            &mut state.research_exp,
            ctx.server_var_res_xp,
        );

        if step >= to_insight && to_insight.is_finite() {
            let req = insight_exp_req(insight_obs, state.observations.insight_level[insight_obs]);
            let insight_rate_value = insight_rate(
                insight_obs,
                &state,
                &overlay,
                ctx,
                &kal_map,
                kal_base,
            );
            state.observations.insight_exp[insight_obs] += insight_rate_value * step;
            state.observations.insight_exp[insight_obs] -= req;
            state.observations.insight_level[insight_obs] += 1;
            insight_gained += 1;
        }

        samples.push(Sample {
            hours,
            level: state.research_level,
            rate: exp_rate(&state, ctx).total,
        });

        if let Some(level) = target_level
            && state.research_level >= level {
                break;
            }
    }

    Projection {
        hours,
        final_level: state.research_level,
        total_exp,
        insight_gained,
        samples,
    }
}

/// Shape def by index, if the data has one.
pub fn shape_def(shape: usize) -> Option<&'static ShapeDef> {
    game().shapes.get(shape)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Mag, MagKind};

    fn filled() -> (Simulation, SimContext) {
        let mut sim = Simulation {
            research_level: 60,
            grid: GridState::seeded(2),
            observations: Observations::all_at(0),
            ..Simulation::default()
        };
        sim.grid.set(50, 1);
        let ctx = SimContext::default();
        (sim, ctx)
    }

    #[test]
    fn observation_curve_matches_the_upstream_values() {
        assert!((obs_base_exp(0) - 4.0).abs() < 1e-9);
        // Upstream: (4 + (t/2 + floor(t/4))) * (1 + t^(1+t/15*0.4)/10) + t^1.5 + 1.5t
        let t = 10.0_f64;
        let expected = (4.0 + (t / 2.0 + (t / 4.0).floor())) * (1.0 + t.powf(1.0 + t / 15.0 * 0.4) / 10.0)
            + t.powf(1.5)
            + 1.5 * t;
        assert!((obs_base_exp(10) - expected).abs() < 1e-9);
        assert!(obs_base_exp(30) > obs_base_exp(10));
    }

    #[test]
    fn research_curve_grows_per_level() {
        assert!(research_exp_req(1, 1.0) < research_exp_req(2, 1.0));
        assert!(research_exp_req(30, 1.0) > research_exp_req(29, 1.0));
        assert!(research_exp_req(30, 2.0) > research_exp_req(30, 1.0));
    }

    #[test]
    fn a_kaleidoscope_on_the_last_row_has_no_southern_neighbour() {
        let last = game().observations.len() - 1;
        let map = kaleidoscope_map(&[Mag::new(MagKind::Kaleidoscope, Some(last))], last + 1);
        assert_eq!(map.len(), last + 1);
        assert_eq!(map[last], 0, "the bottom row must not boost itself");
        assert_eq!(map[last - 1], 1, "its left neighbour is boosted");
        assert_eq!(map[last - 8], 1, "its northern neighbour is boosted");
        // A slot one past the data must also be safe.
        let wide = kaleidoscope_map(&[Mag::new(MagKind::Kaleidoscope, Some(41))], 43);
        assert_eq!(wide[41], 0);
    }

    #[test]
    fn magnifier_caps_follow_the_level_gates() {
        assert_eq!(mag_cap(1), 1);
        assert_eq!(mag_cap(40), 2);
        assert_eq!(mag_cap(70), 3);
        assert_eq!(mag_cap(120), 4);
    }

    #[test]
    fn exp_rate_needs_magnifiers() {
        let (mut sim, ctx) = filled();
        assert_eq!(exp_rate(&sim, &ctx).total, 0.0);
        sim.mags = vec![Mag::new(MagKind::Regular, Some(3))];
        let rate = exp_rate(&sim, &ctx);
        assert!(rate.obs_base > 0.0);
        assert!(rate.multi >= 1.0);
        assert!(rate.total > 0.0);
    }

    #[test]
    fn shapes_and_bonus_nodes_raise_the_rate() {
        let (mut sim, ctx) = filled();
        sim.mags = vec![Mag::new(MagKind::Regular, Some(3))];
        let plain = exp_rate(&sim, &ctx).total;
        let mut values = cell_values(&sim, &ctx);
        let best = values
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(cell, _)| cell)
            .expect("a cell");
        assert!(values[best] > 0.0, "some cell must be worth shaping");
        values.clear();
        let mut shaped = sim.clone();
        shaped.placements = vec![crate::model::Placement {
            shape: 3,
            x: 0.0,
            y: 0.0,
            rot: 0.0,
        }];
        let with_shape = exp_rate(&shaped, &ctx).total;
        assert!(with_shape >= plain);
    }

    #[test]
    fn projection_reaches_a_level_and_stops() {
        let (mut sim, ctx) = filled();
        sim.mags = vec![Mag::new(MagKind::Regular, Some(3))];
        let projection = project(&sim, &ctx, Target::Level(62));
        assert_eq!(projection.final_level, 62);
        assert!(projection.hours > 0.0);
        assert!(projection.samples.len() >= 2);
    }

    #[test]
    fn projection_to_a_far_level_terminates_with_an_honest_hour_count() {
        let (mut sim, ctx) = filled();
        sim.research_level = 150;
        sim.mags = vec![Mag::new(MagKind::Regular, Some(3))];
        let projection = project(&sim, &ctx, Target::Level(151));
        assert_eq!(projection.final_level, 151);
        assert!(projection.hours > 1e6, "hours {}", projection.hours);
        assert!(projection.samples.len() < 64, "samples {}", projection.samples.len());
    }

    #[test]
    fn projection_honours_an_hour_budget() {
        let (mut sim, ctx) = filled();
        sim.mags = vec![
            Mag::new(MagKind::Regular, Some(3)),
            Mag::new(MagKind::Monocle, Some(3)),
        ];
        let projection = project(&sim, &ctx, Target::Hours(5.0));
        assert!((projection.hours - 5.0).abs() < 1e-6);
        assert!(projection.insight_gained > 0);
    }

    #[test]
    fn grid_points_count_what_the_level_grants() {
        let (sim, ctx) = filled();
        let points = grid_points(&sim, &ctx);
        assert!(points.earned > 0);
        assert_eq!(points.spent, sim.grid.spent());
        assert_eq!(
            points.free,
            points.earned.saturating_sub(points.spent)
        );
    }
}
