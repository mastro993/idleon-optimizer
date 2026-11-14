//! Monocle targeting: where the optical monocles go, ported from upstream
//! `js/optimizers/monos.js`.
//!
//! A monocle earns insight on its observation, and insight only pays EXP where
//! the grid reads it (squares 93 and 94), so the choice of observation is a
//! trade between levelling insight fastest and levelling the insight the rate
//! already multiplies. Upstream scores each usable observation by that trade and
//! greedily fills the rooms the other magnifiers left, which is what the
//! decision tree's Auto Insight does at every insight level-up it skips through.

use crate::data::game;
use crate::model::{Mag, MagKind, SimContext, Simulation};
use crate::sim;
use std::collections::BTreeSet;

/// Re-assign the monocles in `state.mags`; every other magnifier keeps its slot.
pub fn choose_mono_targets(state: &Simulation, ctx: &SimContext) -> Vec<Mag> {
    let game = game();
    let mono_count = state
        .mags
        .iter()
        .filter(|mag| mag.kind == MagKind::Monocle)
        .count();
    if mono_count == 0 {
        return state.mags.clone();
    }
    let overlay = state.overlay();
    let kal_base = sim::kalei_base(&state.grid, &overlay, ctx);
    let gd101 = sim::grid_bonus(&state.grid, &overlay, 93, ctx);
    let gd94 = sim::grid_bonus(&state.grid, &overlay, 94, ctx);
    let cap = sim::mag_cap(state.research_level) as usize;

    let others: Vec<Mag> = state
        .mags
        .iter()
        .copied()
        .filter(|mag| mag.kind != MagKind::Monocle)
        .collect();
    let mut other_slots = vec![0usize; game.observations.len()];
    let mut mag_slots = BTreeSet::new();
    for mag in &others {
        let Some(slot) = mag.slot else { continue };
        if let Some(count) = other_slots.get_mut(slot) {
            *count += 1;
            if mag.kind == MagKind::Regular {
                mag_slots.insert(slot);
            }
        }
    }

    // Every usable observation, whether or not the found count has reached it;
    // upstream scans the ones past the count in a second pass of the same list.
    let mut candidates = Vec::new();
    let mut trial = state.clone();
    for obs in 0..game.observations.len() {
        if !sim::observation_usable(obs, state.research_level, &state.observations) {
            continue;
        }
        let room = cap.saturating_sub(other_slots.get(obs).copied().unwrap_or(0));
        if room == 0 {
            continue;
        }
        trial.mags = others.clone();
        trial.mags.push(Mag::new(MagKind::Monocle, Some(obs)));
        let rate = sim::insight_rate(
            obs,
            &trial,
            &overlay,
            ctx,
            &sim::kaleidoscope_map(&trial.mags, game.observations.len()),
            kal_base,
        );
        let required = sim::insight_exp_required(obs, state.observations.insight(obs));
        let remaining = (required - state.observations.insight_exp[obs]).max(0.0);
        let hours = if rate > 0.0 {
            remaining / rate
        } else {
            f64::INFINITY
        };
        candidates.push(Candidate { obs, hours, room });
    }
    if candidates.is_empty() {
        return state.mags.clone();
    }

    // What one more insight level on an observation is worth in EXP/hour. With
    // square 93 the regular magnifiers are re-fitted first: an insight level
    // changes what a glass is worth on that observation.
    let insight_helps = gd101 > 0.0 || gd94 > 0.0;
    let base = if insight_helps {
        sim::exp_rate(state, ctx).total
    } else {
        0.0
    };
    let mut gains = vec![0.0; game.observations.len()];
    if insight_helps {
        let mut projected = state.clone();
        for candidate in &candidates {
            let obs = candidate.obs;
            let level = projected.observations.insight_level[obs];
            projected.observations.insight_level[obs] = level + 1;
            let rate = if gd101 > 0.0 {
                let mut trial = projected.clone();
                trial.mags = reopt_regular_mags(&trial, ctx);
                sim::exp_rate(&trial, ctx).total
            } else {
                sim::exp_rate(&projected, ctx).total
            };
            projected.observations.insight_level[obs] = level;
            gains[obs] = rate - base;
        }
    }

    spread(state, &candidates, mono_count, insight_helps, &gains)
}

/// One observation a monocle could go on.
struct Candidate {
    obs: usize,
    /// Hours until the observation's next insight level with one monocle here.
    hours: f64,
    /// Monocles the slot still takes beside its other magnifiers.
    room: usize,
}

/// Strip the monocles and fill the rooms best score first, as `_spreadMonos`.
fn spread(
    state: &Simulation,
    candidates: &[Candidate],
    mono_count: usize,
    insight_helps: bool,
    gains: &[f64],
) -> Vec<Mag> {
    let mut mags = state.mags.clone();
    for mag in &mut mags {
        if mag.kind == MagKind::Monocle {
            mag.slot = None;
        }
    }
    let indices: Vec<usize> = mags
        .iter()
        .enumerate()
        .filter(|(_, mag)| mag.kind == MagKind::Monocle)
        .map(|(index, _)| index)
        .collect();
    let mut assigned = vec![0usize; game().observations.len()];
    for index in indices.iter().take(mono_count) {
        let mut best: Option<(f64, usize)> = None;
        for candidate in candidates {
            if assigned[candidate.obs] >= candidate.room {
                continue;
            }
            let hours = candidate.hours.max(0.01);
            let score = if insight_helps {
                let gain = gains.get(candidate.obs).copied().unwrap_or(0.0);
                if gain > 0.0 {
                    gain / hours
                } else {
                    1.0 / hours
                }
            } else {
                1.0 / hours
            };
            if best.is_none_or(|(current, _)| score > current) {
                best = Some((score, candidate.obs));
            }
        }
        if let Some((_, obs)) = best {
            mags[*index].slot = Some(obs);
            assigned[obs] += 1;
        }
    }
    // A monocle the scoring left over goes anywhere with room, as upstream.
    for index in &indices {
        if mags[*index].slot.is_some() {
            continue;
        }
        for candidate in candidates {
            if assigned[candidate.obs] < candidate.room {
                mags[*index].slot = Some(candidate.obs);
                assigned[candidate.obs] += 1;
                break;
            }
        }
    }
    mags
}

/// Re-place the regular magnifiers for the current insight levels, keeping the
/// kaleidoscopes and monocles where they are (upstream `reoptRegularMags`).
fn reopt_regular_mags(state: &Simulation, ctx: &SimContext) -> Vec<Mag> {
    let game = game();
    let overlay = state.overlay();
    let kal_base = sim::kalei_base(&state.grid, &overlay, ctx);
    let kal_map = sim::kaleidoscope_map(&state.mags, game.observations.len());
    let gd101 = sim::grid_bonus(&state.grid, &overlay, 93, ctx);
    let cap = sim::mag_cap(state.research_level) as usize;
    let regulars = state
        .mags
        .iter()
        .filter(|mag| mag.kind == MagKind::Regular)
        .count();

    let mut kept = Vec::with_capacity(state.mags.len());
    let mut used = vec![0usize; game.observations.len()];
    for mag in &state.mags {
        match mag.kind {
            MagKind::Regular => {}
            MagKind::Monocle | MagKind::Kaleidoscope => {
                kept.push(*mag);
                if let Some(slot) = mag.slot
                    && let Some(count) = used.get_mut(slot)
                {
                    *count += 1;
                }
            }
        }
    }

    let mut slots = Vec::new();
    for obs in 0..game.observations.len() {
        if !sim::observation_usable(obs, state.research_level, &state.observations) {
            continue;
        }
        let room = cap.saturating_sub(used[obs]);
        if room == 0 {
            continue;
        }
        let value = sim::obs_base_exp(obs)
            * (1.0 + gd101 * f64::from(state.observations.insight(obs)) / 100.0)
            * (1.0 + f64::from(kal_map[obs]) * kal_base);
        for _ in 0..room {
            slots.push((value, obs));
        }
    }
    // Best value first; equal values keep the observation order, as the
    // stable sort upstream relies on does.
    slots.sort_by(|left, right| right.0.total_cmp(&left.0));
    let reclaimed: Vec<Mag> = slots
        .into_iter()
        .take(regulars)
        .map(|(_, obs)| Mag::new(MagKind::Regular, Some(obs)))
        .collect();
    kept.extend(reclaimed);
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GridState, Observations};

    fn state() -> (Simulation, SimContext) {
        let mut simulation = Simulation {
            research_level: 60,
            grid: GridState::empty(),
            observations: Observations::all_at(1),
            ..Simulation::default()
        };
        simulation.grid.set(93, 1);
        simulation.mags = vec![
            Mag::new(MagKind::Regular, Some(0)),
            Mag::new(MagKind::Regular, Some(1)),
            Mag::new(MagKind::Monocle, Some(4)),
            Mag::new(MagKind::Monocle, Some(5)),
        ];
        (simulation, SimContext::default())
    }

    #[test]
    fn every_monocle_lands_on_a_usable_observation() {
        let (state, ctx) = state();
        let mags = choose_mono_targets(&state, &ctx);
        let placed: Vec<usize> = mags
            .iter()
            .filter(|mag| mag.kind == MagKind::Monocle)
            .filter_map(|mag| mag.slot)
            .collect();
        assert_eq!(placed.len(), 2, "both monocles keep a slot");
        for obs in placed {
            assert!(sim::observation_usable(
                obs,
                state.research_level,
                &state.observations
            ));
        }
    }

    #[test]
    fn monocles_fill_the_room_the_other_magnifiers_left() {
        let (mut state, ctx) = state();
        // Up to level 39 an observation holds one magnifier, so the two
        // monocles must spread rather than stack on one observation.
        state.research_level = 30;
        assert_eq!(sim::mag_cap(30), 1);
        state.mags.retain(|mag| mag.kind == MagKind::Monocle);
        let mags = choose_mono_targets(&state, &ctx);
        let slots: Vec<usize> = mags
            .iter()
            .filter(|mag| mag.kind == MagKind::Monocle)
            .filter_map(|mag| mag.slot)
            .collect();
        assert_eq!(slots.len(), 2);
        assert_ne!(slots[0], slots[1], "a slot takes one magnifier at level 30");
    }

    #[test]
    fn a_full_board_still_receives_the_monocle() {
        let (mut state, ctx) = state();
        // Square 93 makes insight pay EXP, so a monocle is worth placing even
        // when every usable observation already holds a glass.
        state.observations = Observations::all_at(1);
        state.mags = vec![
            Mag::new(MagKind::Regular, Some(0)),
            Mag::new(MagKind::Regular, Some(1)),
            Mag::new(MagKind::Monocle, None),
        ];
        let mags = choose_mono_targets(&state, &ctx);
        assert!(
            mags.iter()
                .any(|mag| mag.kind == MagKind::Monocle && mag.slot.is_some()),
            "the unplaced monocle found no room: {mags:?}"
        );
    }

    #[test]
    fn regulars_keep_the_monocle_slots_and_fill_the_rest() {
        let (state, ctx) = state();
        let mut laid_out = state.clone();
        laid_out.mags = vec![
            Mag::new(MagKind::Kaleidoscope, Some(2)),
            Mag::new(MagKind::Monocle, Some(7)),
            Mag::new(MagKind::Regular, Some(0)),
        ];
        let mags = reopt_regular_mags(&laid_out, &ctx);
        assert_eq!(
            mags.iter()
                .filter(|mag| mag.kind == MagKind::Regular)
                .count(),
            1
        );
        assert!(
            mags.iter()
                .any(|mag| mag.kind == MagKind::Kaleidoscope && mag.slot == Some(2))
        );
        assert!(
            mags.iter()
                .any(|mag| mag.kind == MagKind::Monocle && mag.slot == Some(7))
        );
    }
}
