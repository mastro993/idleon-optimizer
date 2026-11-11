//! The plan edits the search builds from: legal slots, plan index math and the
//! legalization that puts every request where the game would place it.

use crate::worship::plan::validate_planning_placement;
use crate::worship::{PhaseSell, Plan, PlanTower, Spot, Trial};

/// The search's constants and the slot pool every edit draws from.
pub(super) struct Ctx {
    pub(super) slots: Vec<Spot>,
    pub(super) coverage: Vec<f64>,
    pub(super) types: Vec<usize>,
    pub(super) max_towers: usize,
    pub(super) wave_lo: u32,
    pub(super) wave_hi: u32,
}

/// A tower standing after a stage's placements.
pub(super) struct Standing {
    spot: Spot,
    type_index: usize,
    plan_index: usize,
}

/// Move `tower` onto a slot the game accepts while `placed` stands.
pub(super) fn fit(
    trial: &Trial,
    slots: &[Spot],
    placed: &[Standing],
    tower: &mut PlanTower,
) -> bool {
    let spots: Vec<Spot> = placed.iter().map(|standing| standing.spot).collect();
    let check = validate_planning_placement(trial, &spots, tower.slot.node, tower.slot.x);
    if check.ok {
        tower.slot.x = check.x;
        return true;
    }
    let mut best: Option<(Spot, f64)> = None;
    for slot in slots {
        let distance = if slot.node == tower.slot.node {
            0.0
        } else {
            1.0e6
        } + (slot.x - tower.slot.x).abs();
        if best.is_some_and(|(_, best_distance)| distance >= best_distance) {
            continue;
        }
        let attempt = validate_planning_placement(trial, &spots, slot.node, slot.x);
        if attempt.ok && attempt.x == slot.x {
            best = Some((*slot, distance));
        }
    }
    let Some((slot, _)) = best else {
        return false;
    };
    tower.slot = slot;
    true
}

/// Lay one stage's towers out and record their global plan indices.
pub(super) fn layout(
    trial: &Trial,
    slots: &[Spot],
    placed: &mut Vec<Standing>,
    map: &mut Vec<Option<usize>>,
    kept: &mut usize,
    towers: Vec<PlanTower>,
) -> Vec<PlanTower> {
    let mut out = Vec::with_capacity(towers.len());
    for mut tower in towers {
        if fit(trial, slots, placed, &mut tower) {
            placed.push(Standing {
                spot: tower.slot,
                type_index: tower.type_index,
                plan_index: *kept,
            });
            map.push(Some(*kept));
            *kept += 1;
            out.push(tower);
        } else {
            map.push(None);
        }
    }
    out
}

/// Rewrite every stage's slots so each request sits exactly where the game
/// would place it. A request that does not fit moves to the nearest free legal
/// slot on its platform (or anywhere, failing that); towers with no legal slot
/// are dropped and every phase's sell indices are renumbered.
pub(super) fn legalize_plan(trial: &Trial, plan: &Plan, slots: &[Spot]) -> Plan {
    let mut plan = plan.normalized();
    let mut placed: Vec<Standing> = Vec::new();
    let mut map: Vec<Option<usize>> = Vec::new();
    let mut kept = 0;
    let initial = std::mem::take(&mut plan.towers);
    plan.towers = layout(trial, slots, &mut placed, &mut map, &mut kept, initial);

    for index in 0..plan.phases.len() {
        let mut remapped = Vec::new();
        for plan_index in &plan.phases[index].sell.plan_idx {
            // A phase sells before queueing its own towers, so only earlier
            // global indices are meaningful.
            if let Some(Some(mapped)) = map.get(*plan_index)
                && !remapped.contains(mapped)
            {
                remapped.push(*mapped);
            }
        }
        plan.phases[index].sell.plan_idx = remapped;
        let sell = plan.phases[index].sell.clone();
        placed.retain(|standing| {
            if sell.all {
                return sell.keep_types.contains(&standing.type_index);
            }
            !(sell.types.contains(&standing.type_index)
                || sell.plan_idx.contains(&standing.plan_index))
        });
        let towers = std::mem::take(&mut plan.phases[index].towers);
        plan.phases[index].towers = layout(trial, slots, &mut placed, &mut map, &mut kept, towers);
    }
    plan
}

/// The stage a plan edit acts on: the opening build or one phase.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    Initial,
    Phase(usize),
}

pub(super) fn stage_len(plan: &Plan, stage: Stage) -> usize {
    match stage {
        Stage::Initial => plan.towers.len(),
        Stage::Phase(index) => plan.phases.get(index).map_or(0, |phase| phase.towers.len()),
    }
}

pub(super) fn stage_list(plan: &mut Plan, stage: Stage) -> &mut Vec<PlanTower> {
    match stage {
        Stage::Initial => &mut plan.towers,
        Stage::Phase(index) => &mut plan.phases[index].towers,
    }
}

/// The first global plan index of a stage.
pub(super) fn global_start(plan: &Plan, stage: Stage) -> usize {
    match stage {
        Stage::Initial => 0,
        Stage::Phase(index) => {
            plan.towers.len()
                + plan.phases[..index]
                    .iter()
                    .map(|phase| phase.towers.len())
                    .sum::<usize>()
        }
    }
}

/// Rewrite every phase's sell indices through `map`.
pub(super) fn reindex(plan: &mut Plan, map: impl Fn(usize) -> Option<usize>) {
    for phase in &mut plan.phases {
        let mut out: Vec<usize> = Vec::with_capacity(phase.sell.plan_idx.len());
        for index in &phase.sell.plan_idx {
            if let Some(mapped) = map(*index)
                && !out.contains(&mapped)
            {
                out.push(mapped);
            }
        }
        phase.sell.plan_idx = out;
    }
}

pub(super) fn insert_towers(plan: &mut Plan, stage: Stage, at: usize, list: Vec<PlanTower>) {
    if list.is_empty() {
        return;
    }
    let start = global_start(plan, stage) + at;
    let added = list.len();
    reindex(plan, |index| {
        Some(if index >= start { index + added } else { index })
    });
    let towers = stage_list(plan, stage);
    let at = at.min(towers.len());
    towers.splice(at..at, list);
}

pub(super) fn remove_towers(plan: &mut Plan, stage: Stage, locals: Vec<usize>) {
    let mut locals = locals;
    locals.sort_unstable();
    locals.dedup();
    let start = global_start(plan, stage);
    for local in locals.into_iter().rev() {
        let global = start + local;
        reindex(plan, |index| {
            if index == global {
                None
            } else if index > global {
                Some(index - 1)
            } else {
                Some(index)
            }
        });
        let towers = stage_list(plan, stage);
        if local < towers.len() {
            towers.remove(local);
        }
    }
}

/// The indices of same-type towers on the same platform.
pub(super) fn group_of(towers: &[PlanTower], tower: &PlanTower) -> Vec<usize> {
    towers
        .iter()
        .enumerate()
        .filter(|(_, other)| {
            other.type_index == tower.type_index && other.slot.node == tower.slot.node
        })
        .map(|(index, _)| index)
        .collect()
}

/// Every tower of a plan, stage by stage.
pub(super) fn all_towers(plan: &mut Plan) -> impl Iterator<Item = &mut PlanTower> {
    std::iter::once(&mut plan.towers)
        .chain(plan.phases.iter_mut().map(|phase| &mut phase.towers))
        .flatten()
}

/// The towers standing before `stage` sells, with their global plan indices.
pub(super) fn earlier_towers(plan: &Plan, stage: Stage) -> Vec<(usize, PlanTower)> {
    let mut out = Vec::new();
    let mut global = 0;
    for tower in &plan.towers {
        out.push((global, tower.clone()));
        global += 1;
    }
    let phases = match stage {
        Stage::Initial => 0,
        Stage::Phase(index) => index,
    };
    for phase in plan.phases.iter().take(phases) {
        for tower in &phase.towers {
            out.push((global, tower.clone()));
            global += 1;
        }
    }
    out
}

/// The slots a phase's sell rules free.
pub(super) fn sold_slots(plan: &Plan, stage: Stage, sell: &PhaseSell) -> Vec<Spot> {
    earlier_towers(plan, stage)
        .into_iter()
        .filter(|(index, tower)| {
            if sell.all {
                !sell.keep_types.contains(&tower.type_index)
            } else {
                sell.types.contains(&tower.type_index) || sell.plan_idx.contains(index)
            }
        })
        .map(|(_, tower)| tower.slot)
        .collect()
}
