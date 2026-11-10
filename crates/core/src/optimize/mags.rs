//! Magnifier placement: kaleidoscopes, monocles, then regular magnifiers.

use crate::data::game;
use crate::model::{Mag, MagKind, Simulation, SimContext};
use crate::sim;

/// A magnifier pool plus what the optimizer did to it.
#[derive(Clone, Debug, Default)]
pub struct MagsOutcome {
    pub mags: Vec<Mag>,
    pub rate: f64,
    pub steps: Vec<String>,
}

/// Assign the owned magnifiers to observations.
pub fn optimize(sim: &Simulation, ctx: &SimContext) -> MagsOutcome {
    let game = game();
    let owned = sim::magnifiers_owned(sim, ctx) as usize;
    let cap = sim::mag_cap(sim.research_level) as usize;
    let usable: Vec<usize> = (0..game.observations.len())
        .filter(|obs| sim::observation_usable(*obs, sim.research_level, &sim.observations))
        .collect();
    let mut outcome = MagsOutcome::default();
    if usable.is_empty() || owned == 0 || cap == 0 {
        outcome
            .steps
            .push("No observation can hold a magnifier at this level yet.".into());
        return outcome;
    }

    let counts = count_kinds(&sim.mags);
    let mut room: Vec<usize> = vec![cap; game.observations.len()];
    for (obs, slots) in room.iter_mut().enumerate() {
        if !usable.contains(&obs) {
            *slots = 0;
        }
    }

    let kaleidoscopes = counts.kaleidoscopes.min(owned);
    let remaining = owned - kaleidoscopes;
    let monocles = counts.monocles.min(remaining);
    let regulars = remaining - monocles;

    // Kaleidoscopes first: they multiply whatever ends up beside them.
    let mut kaleido_slots = Vec::new();
    for _ in 0..kaleidoscopes {
        let best = usable
            .iter()
            .copied()
            .filter(|obs| room[*obs] > 0)
            .max_by(|a, b| neighbour_value(*a, &room).total_cmp(&neighbour_value(*b, &room)));
        if let Some(obs) = best {
            room[obs] -= 1;
            kaleido_slots.push(obs);
        }
    }

    let kal_map = sim::kaleidoscope_map(&kaleido_slots.iter().map(|slot| Mag::new(MagKind::Kaleidoscope, Some(*slot))).collect::<Vec<_>>(), game.observations.len());
    let multipliers = |obs: usize, map: &[u32]| -> f64 {
        let base = sim::kalei_base(&sim.grid, &sim.overlay(), ctx);
        1.0 + map[obs] as f64 * base
    };

    // Monocles go where insight arrives soonest.
    let mut monocle_slots = Vec::new();
    for _ in 0..monocles {
        let best = usable
            .iter()
            .copied()
            .filter(|obs| room[*obs] > 0)
            .max_by(|a, b| {
                insight_roi(sim, *a, multipliers(*a, &kal_map))
                    .total_cmp(&insight_roi(sim, *b, multipliers(*b, &kal_map)))
            });
        if let Some(obs) = best {
            room[obs] -= 1;
            monocle_slots.push(obs);
        }
    }

    // Regular magnifiers take the richest observations left.
    let insight_node = sim.grid.level(93) as f64 * game
        .node_at(93)
        .map(|node| node.bonus_per_lv)
        .unwrap_or(0.0);
    let mut regular_slots = Vec::new();
    for _ in 0..regulars {
        let best = usable
            .iter()
            .copied()
            .filter(|obs| room[*obs] > 0)
            .max_by(|a, b| {
                let value = |obs: usize| {
                    sim::obs_base_exp(obs)
                        * (1.0 + insight_node * sim.observations.insight_level[obs] as f64 / 100.0)
                        * multipliers(obs, &kal_map)
                };
                value(*a).total_cmp(&value(*b))
            });
        if let Some(obs) = best {
            room[obs] -= 1;
            regular_slots.push(obs);
        }
    }

    outcome.mags = kaleido_slots
        .into_iter()
        .map(|slot| Mag::new(MagKind::Kaleidoscope, Some(slot)))
        .chain(
            monocle_slots
                .into_iter()
                .map(|slot| Mag::new(MagKind::Monocle, Some(slot))),
        )
        .chain(
            regular_slots
                .into_iter()
                .map(|slot| Mag::new(MagKind::Regular, Some(slot))),
        )
        .collect();

    let mut trial = sim.clone();
    trial.mags = outcome.mags.clone();
    outcome.rate = sim::exp_rate(&trial, ctx).total;
    if !outcome.mags.is_empty() {
        outcome.steps.push(format!(
            "Placed {} magnifiers across {} observations.",
            outcome.mags.len(),
            usable.len()
        ));
    }
    outcome
}

fn count_kinds(mags: &[Mag]) -> KindCounts {
    let mut counts = KindCounts::default();
    for mag in mags {
        match mag.kind {
            MagKind::Regular => counts.regular += 1,
            MagKind::Monocle => counts.monocles += 1,
            MagKind::Kaleidoscope => counts.kaleidoscopes += 1,
        }
    }
    counts
}

#[derive(Default)]
struct KindCounts {
    regular: usize,
    monocles: usize,
    kaleidoscopes: usize,
}

/// Base EXP of an observation's neighbours, weighted by spare room.
fn neighbour_value(obs: usize, room: &[usize]) -> f64 {
    neighbour_slots(obs)
        .into_iter()
        .filter(|slot| room.get(*slot).copied().unwrap_or(0) > 0)
        .map(sim::obs_base_exp)
        .sum()
}

fn neighbour_slots(obs: usize) -> Vec<usize> {
    let mut slots = Vec::with_capacity(4);
    let col = obs % 8;
    if col != 7 {
        slots.push(obs + 1);
    }
    if col != 0 {
        slots.push(obs.saturating_sub(1));
    }
    if obs > 7 {
        slots.push(obs - 8);
    }
    if obs < 72 {
        slots.push(obs + 8);
    }
    slots
}

/// Insight gain per hour against the EXP still missing for the next level.
fn insight_roi(sim: &Simulation, obs: usize, kal_multi: f64) -> f64 {
    let level = sim.observations.insight_level[obs];
    let missing = (sim::insight_exp_req(obs, level) - sim.observations.insight_exp[obs]).max(1.0);
    let bonus = sim.grid.level(91) as f64 * 0.05 + sim.grid.level(92) as f64 * 0.05;
    (3.0 * (1.0 + bonus) * kal_multi) / missing
}
