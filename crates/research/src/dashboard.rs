//! The research dashboard's numbers: the summary, the observation board, the
//! grid-point split and the insight multiplier.
//!
//! Ported from `js/ui/dashboard.js` and `js/ui/dash-breakdowns.js`; every value
//! comes from the same [`crate::sim`] functions the Optimizer tab reads, so the
//! dashboard and a run can never disagree.

use crate::data::game;
use crate::model::{SimContext, Simulation};
use crate::sim::{self, GridPoints};

/// The dashboard summary: rates, level progress and what the account owns.
#[derive(Clone, Copy, Debug)]
pub struct Summary {
    pub level: u32,
    /// Research EXP/hour with the game open.
    pub active_rate: f64,
    /// Research EXP/hour while away: [`Self::active_rate`] times the AFK rate.
    pub offline_rate: f64,
    /// The AFK gains rate, 0 to 1.
    pub afk_rate: f64,
    /// EXP banked at this level.
    pub exp_current: f64,
    /// EXP the next level needs.
    pub exp_required: f64,
    /// EXP still missing for the next level, never negative.
    pub exp_remaining: f64,
    /// Hours to the next level with the game open.
    pub hours_active: f64,
    /// Hours to the next level while away.
    pub hours_offline: f64,
    /// Progress to the next level, 0 to 1.
    pub progress: f32,
    pub magnifiers: u32,
    /// Magnifiers one observation holds at this level.
    pub mag_cap: u32,
    pub grid_points: GridPoints,
}

/// One observation on the dashboard board.
#[derive(Clone, Debug)]
pub struct ObservationCard {
    pub obs: usize,
    pub name: String,
    pub found: bool,
    pub insight_level: u32,
    /// Magnifying glasses on the observation.
    pub regular: u32,
    /// Optical monocles on the observation.
    pub monocles: u32,
    /// Kaleidoscopes on the observation.
    pub kaleidoscopes: u32,
    /// Neighbouring kaleidoscopes the observation gains from.
    pub neighbours: u32,
    /// Research EXP/hour from its regular magnifiers, after every multiplier.
    pub exp: f64,
    /// Insight EXP/hour from its monocles.
    pub insight: f64,
}

/// The grid points a level earns and where they come from.
#[derive(Clone, Copy, Debug)]
pub struct GridPointBreakdown {
    pub level: u32,
    /// The points the "every ten levels" term grants, including square 50.
    pub every_ten: u32,
    /// Flat points from companions, artifacts, tasks, jelly and bundles.
    pub account_sources: u32,
    pub points: GridPoints,
}

/// The insight bonus one monocle applies.
#[derive(Clone, Copy, Debug)]
pub struct InsightMultiplier {
    pub grid_91: f64,
    pub grid_92: f64,
    /// Emperor 46's insight percentage.
    pub emperor_pct: f64,
    /// Jelly reward 7's insight percentage.
    pub jelly_pct: f64,
    /// Grid 91 plus grid 92, the additive part.
    pub additive_pct: f64,
    /// Every factor multiplied together.
    pub multi: f64,
    /// Insight EXP/hour one monocle earns before kaleidoscope neighbours.
    pub per_monocle: f64,
}

/// The dashboard summary for a run.
pub fn summary(sim: &Simulation, ctx: &SimContext) -> Summary {
    let rate = sim::exp_rate(sim, ctx);
    let afk_rate = sim::afk_rate(sim, ctx).rate.clamp(0.0, 1.0);
    let offline_rate = rate.total * afk_rate;
    let exp_required = sim::research_exp_req(sim.research_level, ctx.server_var_res_xp);
    let exp_current = sim.research_exp.max(0.0);
    let exp_remaining = (exp_required - exp_current).max(0.0);
    Summary {
        level: sim.research_level,
        active_rate: rate.total,
        offline_rate,
        afk_rate,
        exp_current,
        exp_required,
        exp_remaining,
        hours_active: hours(exp_remaining, rate.total),
        hours_offline: hours(exp_remaining, offline_rate),
        progress: if exp_required > 0.0 {
            (exp_current / exp_required).clamp(0.0, 1.0) as f32
        } else {
            0.0
        },
        magnifiers: sim::magnifiers_owned(sim, ctx),
        mag_cap: sim::mag_cap(sim.research_level),
        grid_points: sim::grid_points(sim, ctx),
    }
}

fn hours(remaining: f64, rate: f64) -> f64 {
    if rate > 0.0 {
        remaining / rate
    } else {
        f64::INFINITY
    }
}

/// Every observation up to the ones this level has found, as the board lists them.
///
/// A port of the dashboard's observation loop: the EXP column includes the
/// account multiplier, the insight column is the monocles' rate.
pub fn observations(sim: &Simulation, ctx: &SimContext) -> Vec<ObservationCard> {
    let game_data = game();
    let overlay = sim.overlay();
    let to_find = sim::observations_to_find(sim.research_level, &sim.observations.found);
    let kal_map = sim::kaleidoscope_map(&sim.mags, game_data.observations.len());
    let kal_base = sim::kalei_base(&sim.grid, &overlay, ctx);
    let insight_bonus = sim::grid_bonus(&sim.grid, &overlay, 93, ctx);
    let multi = sim::exp_rate(sim, ctx).multi;

    let mut cards = Vec::with_capacity(to_find);
    for (obs, observation) in game_data.observations.iter().enumerate().take(to_find) {
        let mut regular = 0;
        let mut monocles = 0;
        let mut kaleidoscopes = 0;
        for mag in &sim.mags {
            if mag.slot != Some(obs) {
                continue;
            }
            match mag.kind {
                crate::model::MagKind::Regular => regular += 1,
                crate::model::MagKind::Monocle => monocles += 1,
                crate::model::MagKind::Kaleidoscope => kaleidoscopes += 1,
            }
        }
        let level = sim.observations.insight(obs);
        let neighbours = kal_map.get(obs).copied().unwrap_or(0);
        let exp = if regular == 0 {
            0.0
        } else {
            regular as f64
                * sim::obs_base_exp(obs)
                * (1.0 + insight_bonus * level as f64 / 100.0)
                * (1.0 + neighbours as f64 * kal_base)
                * multi
        };
        cards.push(ObservationCard {
            obs,
            name: observation.name.replace('_', " "),
            found: sim.observations.found.get(obs).copied().unwrap_or(false),
            insight_level: level,
            regular,
            monocles,
            kaleidoscopes,
            neighbours,
            exp,
            insight: sim::insight_rate(obs, sim, &overlay, ctx, &kal_map, kal_base),
        });
    }
    cards
}

/// The grid points a run earns, split into the terms the dashboard tree lists.
pub fn grid_points(sim: &Simulation, ctx: &SimContext) -> GridPointBreakdown {
    let level = sim.research_level;
    let lv = level as f64;
    let square_50 = sim.grid.level(50) as f64;
    let every_ten = ((lv / 10.0).floor() * (1.0 + (lv / 60.0).floor().min(1.0) + square_50).round())
        .floor() as u32;
    GridPointBreakdown {
        level,
        every_ten,
        account_sources: ctx.bonus_grid_points,
        points: sim::grid_points(sim, ctx),
    }
}

/// The insight multiplier one monocle applies, from `buildInsightBreakdownTree`.
pub fn insight_multiplier(sim: &Simulation, ctx: &SimContext) -> InsightMultiplier {
    let overlay = sim.overlay();
    let grid_91 = sim::grid_bonus(&sim.grid, &overlay, 91, ctx);
    let grid_92 = sim::grid_bonus(&sim.grid, &overlay, 92, ctx);
    let emperor_pct = 35.0 * ctx.emporium(46);
    let jelly_pct = ctx.jelly_insight_pct;
    let additive_pct = grid_91 + grid_92;
    let multi =
        (1.0 + additive_pct / 100.0) * (1.0 + emperor_pct / 100.0) * (1.0 + jelly_pct / 100.0);
    InsightMultiplier {
        grid_91,
        grid_92,
        emperor_pct,
        jelly_pct,
        additive_pct,
        multi,
        per_monocle: 3.0 * multi,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Mag, MagKind, Observations};

    fn filled() -> (Simulation, SimContext) {
        let mut sim = Simulation {
            research_level: 100,
            research_exp: 500.0,
            grid: crate::model::GridState::seeded(2),
            observations: Observations::all_at(2),
            ..Simulation::default()
        };
        sim.grid.set(93, 2);
        sim.mags = (0..4)
            .map(|slot| Mag::new(MagKind::Regular, Some(slot)))
            .collect();
        sim.mags.push(Mag::new(MagKind::Monocle, Some(0)));
        (sim, SimContext::default())
    }

    #[test]
    fn the_summary_uses_the_same_rate_the_optimizer_shows() {
        let (sim, ctx) = filled();
        let summary = summary(&sim, &ctx);
        let rate = sim::exp_rate(&sim, &ctx);
        assert_eq!(summary.level, sim.research_level);
        assert!((summary.active_rate - rate.total).abs() < 1e-6);
        assert!((summary.afk_rate - sim::afk_rate(&sim, &ctx).rate).abs() < 1e-12);
        assert!((summary.offline_rate - rate.total * summary.afk_rate).abs() < 1e-6);
        assert!((summary.exp_required - sim::research_exp_req(100, 1.01)).abs() < 1e-6);
        assert!((summary.exp_remaining - (summary.exp_required - 500.0)).abs() < 1e-6);
        assert!(
            (summary.hours_active - summary.exp_remaining / rate.total).abs() < 1e-6,
            "{}",
            summary.hours_active
        );
        assert!(summary.progress > 0.0 && summary.progress < 1.0);
    }

    #[test]
    fn a_level_without_a_rate_has_endless_hours() {
        let (mut sim, ctx) = filled();
        sim.mags.clear();
        let summary = summary(&sim, &ctx);
        assert_eq!(summary.active_rate, 0.0);
        assert!(summary.hours_active.is_infinite());
        assert!(summary.hours_offline.is_infinite());
    }

    #[test]
    fn observation_rows_sum_to_the_rate_the_summary_shows() {
        let (sim, ctx) = filled();
        let cards = observations(&sim, &ctx);
        let rate = sim::exp_rate(&sim, &ctx);
        assert!(!cards.is_empty());
        let exp: f64 = cards.iter().map(|card| card.exp).sum();
        assert!((exp - rate.total).abs() < 1e-6, "{exp} != {}", rate.total);
        assert_eq!(cards[0].regular, 1);
        assert_eq!(cards[0].monocles, 1);
        assert!(cards[0].insight > 0.0);
        assert!(cards.iter().all(|card| card.found));
    }

    #[test]
    fn the_insight_multiplier_matches_a_lone_monocles_rate() {
        let (mut sim, ctx) = filled();
        sim.mags = vec![Mag::new(MagKind::Monocle, Some(0))];
        let insight = insight_multiplier(&sim, &ctx);
        let overlay = sim.overlay();
        let kal_map = sim::kaleidoscope_map(&sim.mags, game().observations.len());
        let kal_base = sim::kalei_base(&sim.grid, &overlay, &ctx);
        let rate = sim::insight_rate(0, &sim, &overlay, &ctx, &kal_map, kal_base);
        assert!((insight.per_monocle - rate).abs() < 1e-9);
        assert!(insight.multi > 1.0);
    }

    #[test]
    fn grid_point_terms_add_up_to_the_earned_total() {
        let (sim, ctx) = filled();
        let breakdown = grid_points(&sim, &ctx);
        let points = breakdown.points;
        assert_eq!(breakdown.level, 100);
        assert_eq!(
            points.earned,
            (breakdown.level as f64 + breakdown.account_sources as f64 + breakdown.every_ten as f64)
                .floor() as u32
        );
        assert_eq!(points.free, points.earned.saturating_sub(points.spent));
    }
}
