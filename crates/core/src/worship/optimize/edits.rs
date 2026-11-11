//! The plan edits: random construction, mutations over towers, groups, types and
//! phases, and the beam's direct relocations and recipe retries.
//!
//! Ported from `randomPlan`, `_mutateRaw`, `_tdPlanNeighbors` and their helpers in
//! `js/stats/systems/w3/worship-td.js`.

use super::Candidate;
use super::plans::{
    Ctx, Stage, all_towers, earlier_towers, global_start, group_of, insert_towers, legalize_plan,
    reindex, remove_towers, sold_slots, stage_len, stage_list,
};
use crate::worship::sim::Rng;
use crate::worship::{
    Inputs, PhaseSell, PhaseWhen, Plan, PlanPhase, PlanTower, SkipPolicy, Spot, TOWER_COUNT, Trial,
    validate_placement,
};
use std::collections::HashMap;

/// Level caps the search tries, with `None` for unlimited upgrades.
const LEVEL_CAPS: [Option<u32>; 8] = [
    None,
    Some(5),
    Some(10),
    Some(15),
    Some(20),
    Some(25),
    Some(30),
    Some(40),
];

/// Mobs-alive triggers a phase may wait for.
const ALIVE_TRIGGERS: [u32; 6] = [1, 3, 7, 14, 28, 60];

/// Boss HP fractions a phase may wait for.
const BOSS_FRACS: [f64; 3] = [0.4, 0.6, 0.8];

/// The low base-damage types the beam relocates first.
const WEAK_TYPES: [usize; 5] = [0, 1, 2, 4, 7];

/// Trait recipes the plan neighbors try on whole tower types.
const RECIPES: [&str; 4] = ["AAAAAA", "BBBBBB", "ABABBB", "BAAAAA"];

/// The offsets a group of towers shifts by.
const SHIFTS: [f64; 6] = [-124.0, -62.0, -31.0, 31.0, 62.0, 124.0];

/// The unlocked tower types the search may place.
pub(super) fn unlocked_types(inputs: &Inputs) -> Vec<usize> {
    (0..TOWER_COUNT)
        .filter(|index| inputs.unlocked(*index))
        .collect()
}

/// A random index below `count`, weighted by `weight`.
fn weighted_pick(rng: &mut Rng, count: usize, weight: impl Fn(usize) -> f64) -> usize {
    let total: f64 = (0..count).map(&weight).sum();
    let mut roll = rng.next() * total;
    for index in 0..count {
        roll -= weight(index);
        if roll <= 0.0 {
            return index;
        }
    }
    count.saturating_sub(1)
}

/// A random item from a slice.
pub(super) fn pick<'a, T>(rng: &mut Rng, items: &'a [T]) -> &'a T {
    &items[((rng.next() * items.len() as f64) as usize) % items.len()]
}

/// A 12-character trait recipe from the templates the game rewards.
fn trait_recipe(rng: &mut Rng) -> String {
    let threshold = 1 + (rng.next() * 6.0) as usize;
    let kind = (rng.next() * 7.0) as usize;
    (0..12)
        .map(|index| {
            let even = index % 2 == 0;
            match kind {
                0 => 'A',
                1 => 'B',
                2 => {
                    if even {
                        'A'
                    } else {
                        'B'
                    }
                }
                3 => {
                    if even {
                        'B'
                    } else {
                        'A'
                    }
                }
                4 => {
                    if index < threshold {
                        'B'
                    } else {
                        'A'
                    }
                }
                5 => {
                    if index < threshold {
                        'A'
                    } else {
                        'B'
                    }
                }
                _ => {
                    if rng.next() < 0.5 {
                        'A'
                    } else {
                        'B'
                    }
                }
            }
        })
        .collect()
}

/// A random level cap, half of the time unlimited.
fn random_cap(rng: &mut Rng) -> Option<u32> {
    if rng.next() < 0.5 {
        None
    } else {
        *pick(rng, &LEVEL_CAPS)
    }
}

/// A random legal slot, weighted toward coverage.
fn random_slot(rng: &mut Rng, ctx: &Ctx) -> Spot {
    let index = weighted_pick(rng, ctx.slots.len(), |index| 1.0 + ctx.coverage[index]);
    ctx.slots[index]
}

/// A random tower on `slot`.
fn new_tower(rng: &mut Rng, ctx: &Ctx, slot: Spot, type_index: Option<usize>) -> PlanTower {
    PlanTower {
        type_index: type_index.unwrap_or_else(|| *pick(rng, &ctx.types)),
        slot,
        weight: 0.5 + rng.next() * 2.0,
        traits: trait_recipe(rng),
        max_level: random_cap(rng),
    }
}

/// `count` same-type towers in 62 px slots on one platform, sharing traits, cap
/// and weight.
fn cluster(rng: &mut Rng, ctx: &Ctx, max_count: usize) -> Vec<PlanTower> {
    let count = (2 + (rng.next() * 4.0) as usize).min(max_count.max(1));
    let slot = random_slot(rng, ctx);
    let first = new_tower(rng, ctx, slot, None);
    let direction = if rng.next() < 0.5 { 1.0 } else { -1.0 };
    (0..count)
        .map(|index| PlanTower {
            slot: Spot {
                node: slot.node,
                x: slot.x + direction * 62.0 * index as f64,
            },
            ..first.clone()
        })
        .collect()
}

/// The conditions a new phase waits for.
fn random_when(rng: &mut Rng, ctx: &Ctx) -> PhaseWhen {
    let lo = (f64::from(ctx.wave_lo.max(10)) / 10.0).ceil() as u32;
    let hi = lo.max(ctx.wave_hi / 10);
    PhaseWhen {
        min_wave: Some(10 * (lo + (rng.next() * f64::from(hi - lo + 1)) as u32)),
        boss_hold: if rng.next() < 0.5 { Some(true) } else { None },
        alive_max: if rng.next() < 0.35 {
            Some(*pick(rng, &ALIVE_TRIGGERS))
        } else {
            None
        },
        boss_hp_frac_max: if rng.next() < 0.15 {
            Some(*pick(rng, &BOSS_FRACS))
        } else {
            None
        },
        min_time_sec: None,
    }
}

/// The towers a phase sells.
fn random_sell(rng: &mut Rng, plan: &Plan, stage: Stage) -> PhaseSell {
    let mut sell = PhaseSell::default();
    let previous = earlier_towers(plan, stage);
    if previous.is_empty() {
        return sell;
    }
    let mut present: Vec<usize> = Vec::new();
    for (_, tower) in &previous {
        if !present.contains(&tower.type_index) {
            present.push(tower.type_index);
        }
    }
    let roll = rng.next();
    if roll < 0.3 {
        sell.types.push(*pick(rng, &present));
        if present.len() > 1 && rng.next() < 0.3 {
            let second = *pick(rng, &present);
            if !sell.types.contains(&second) {
                sell.types.push(second);
            }
        }
    } else if roll < 0.75 {
        let anchor = pick(rng, &previous).1.clone();
        let same_type = rng.next() < 0.5;
        for (index, tower) in &previous {
            if tower.slot.node == anchor.slot.node
                && (!same_type || tower.type_index == anchor.type_index)
            {
                sell.plan_idx.push(*index);
            }
        }
    } else if roll < 0.9 {
        let count = 1 + (rng.next() * 4.0) as usize;
        for _ in 0..count {
            let index = pick(rng, &previous).0;
            if !sell.plan_idx.contains(&index) {
                sell.plan_idx.push(index);
            }
        }
    } else {
        sell.all = true;
        sell.keep_types.push(*pick(rng, &present));
        if present.len() > 1 && rng.next() < 0.5 {
            let second = *pick(rng, &present);
            if !sell.keep_types.contains(&second) {
                sell.keep_types.push(second);
            }
        }
    }
    sell
}

/// A phase's new towers: the slots its sell freed, rebuilt, or new clusters
/// elsewhere.
fn phase_towers(
    rng: &mut Rng,
    ctx: &Ctx,
    plan: &Plan,
    stage: Stage,
    sell: &PhaseSell,
) -> Vec<PlanTower> {
    let sold = sold_slots(plan, stage, sell);
    if !sold.is_empty() && rng.next() < 0.6 {
        let first = new_tower(rng, ctx, sold[0], None);
        let mixed = rng.next() < 0.3;
        return sold
            .into_iter()
            .take(ctx.max_towers)
            .map(|slot| {
                let mut tower = if mixed {
                    new_tower(rng, ctx, slot, None)
                } else {
                    first.clone()
                };
                tower.slot = slot;
                tower
            })
            .collect();
    }
    let mut out = Vec::new();
    for _ in 0..1 + (rng.next() * 3.0) as usize {
        if rng.next() < 0.6 {
            out.extend(cluster(rng, ctx, 5));
        } else {
            let slot = random_slot(rng, ctx);
            out.push(new_tower(rng, ctx, slot, None));
        }
    }
    out
}

/// Insert a new phase, then build its towers.
fn add_phase(plan: &mut Plan, rng: &mut Rng, ctx: &Ctx) {
    let at = if rng.next() < 0.7 {
        plan.phases.len()
    } else {
        (rng.next() * (plan.phases.len() + 1) as f64) as usize
    };
    plan.phases.insert(
        at,
        PlanPhase {
            label: format!("Stage {}", plan.phases.len() + 1),
            when: random_when(rng, ctx),
            sell: PhaseSell::default(),
            towers: Vec::new(),
            skip_remaining: true,
        },
    );
    let sell = random_sell(rng, plan, Stage::Phase(at));
    plan.phases[at].sell = sell.clone();
    let towers = phase_towers(rng, ctx, plan, Stage::Phase(at), &sell);
    insert_towers(plan, Stage::Phase(at), 0, towers);
}

/// Drop a phase together with its towers.
fn remove_phase(plan: &mut Plan, index: usize) {
    let count = plan.phases[index].towers.len();
    remove_towers(plan, Stage::Phase(index), (0..count).collect());
    plan.phases.remove(index);
}

fn mutate_when(when: &mut PhaseWhen, rng: &mut Rng, ctx: &Ctx) {
    let roll = rng.next();
    if roll < 0.4 {
        let current = when.min_wave.unwrap_or(ctx.wave_lo);
        let step = if rng.next() < 0.5 { -10 } else { 10 } * (1 + (rng.next() * 2.0) as i64);
        let next = ((f64::from(current) + step as f64) / 10.0).round();
        when.min_wave = Some(10 * (next as u32).max(1));
    } else if roll < 0.6 {
        when.boss_hold = if when.boss_hold.is_some() {
            None
        } else {
            Some(true)
        };
    } else if roll < 0.85 {
        when.alive_max = if rng.next() < 0.3 {
            None
        } else {
            Some(*pick(rng, &ALIVE_TRIGGERS))
        };
    } else {
        when.boss_hp_frac_max = if rng.next() < 0.4 {
            None
        } else {
            Some(*pick(rng, &BOSS_FRACS))
        };
    }
}

/// Rebuild the phase's towers from scratch for its current sell set.
fn rebuild_phase(plan: &mut Plan, index: usize, rng: &mut Rng, ctx: &Ctx) {
    let count = plan.phases[index].towers.len();
    remove_towers(plan, Stage::Phase(index), (0..count).collect());
    let sell = plan.phases[index].sell.clone();
    let towers = phase_towers(rng, ctx, plan, Stage::Phase(index), &sell);
    insert_towers(plan, Stage::Phase(index), 0, towers);
}

/// Add, drop, retime or rebuild one phase.
fn mutate_phase(plan: &mut Plan, rng: &mut Rng, ctx: &Ctx) {
    let roll = rng.next();
    if plan.phases.is_empty() || (roll < 0.25 && plan.phases.len() < 3) {
        add_phase(plan, rng, ctx);
        return;
    }
    let index = (rng.next() * plan.phases.len() as f64) as usize;
    if roll < 0.37 {
        remove_phase(plan, index);
        return;
    }
    if roll < 0.65 {
        let mut when = plan.phases[index].when.clone();
        mutate_when(&mut when, rng, ctx);
        plan.phases[index].when = when;
        return;
    }
    if roll < 0.85 {
        if rng.next() < 0.5 {
            plan.phases[index].sell = random_sell(rng, plan, Stage::Phase(index));
            return;
        }
        let previous = earlier_towers(plan, Stage::Phase(index));
        if previous.is_empty() {
            return;
        }
        if plan.phases[index].sell.all {
            plan.phases[index].sell.keep_types = if rng.next() < 0.5 {
                Vec::new()
            } else {
                vec![pick(rng, &previous).1.clone().type_index]
            };
            return;
        }
        let global = pick(rng, &previous).0;
        let sell = &mut plan.phases[index].sell;
        if let Some(at) = sell.plan_idx.iter().position(|index| *index == global) {
            sell.plan_idx.remove(at);
        } else {
            sell.plan_idx.push(global);
        }
        return;
    }
    rebuild_phase(plan, index, rng, ctx);
}

/// One edit to a plan: a tower, a group, a type or a phase.
fn mutate_raw(plan: &Plan, rng: &mut Rng, ctx: &Ctx) -> Plan {
    let mut plan = plan.clone();
    if rng.next() < 0.15 {
        mutate_phase(&mut plan, rng, ctx);
        return plan;
    }
    let stage = if !plan.phases.is_empty() && rng.next() < 0.3 {
        Stage::Phase((rng.next() * plan.phases.len() as f64) as usize)
    } else {
        Stage::Initial
    };
    let length = stage_len(&plan, stage);
    let room = ctx.max_towers.saturating_sub(length);
    let roll = rng.next();
    if length == 0 || roll < 0.1 {
        if room > 0 {
            let at = (rng.next() * (length + 1) as f64) as usize;
            let slot = random_slot(rng, ctx);
            let tower = new_tower(rng, ctx, slot, None);
            insert_towers(&mut plan, stage, at, vec![tower]);
        }
        return plan;
    }
    if roll < 0.2 {
        if room > 1 {
            let at = (rng.next() * (length + 1) as f64) as usize;
            let towers = cluster(rng, ctx, room.min(5));
            insert_towers(&mut plan, stage, at, towers);
        }
        return plan;
    }
    let index = (rng.next() * length as f64) as usize;
    let tower = stage_list(&mut plan, stage)[index].clone();
    if roll < 0.26 {
        if room > 0 {
            let sibling = PlanTower {
                slot: Spot {
                    node: tower.slot.node,
                    x: tower.slot.x + if rng.next() < 0.5 { 62.0 } else { -62.0 },
                },
                ..tower.clone()
            };
            insert_towers(&mut plan, stage, index + 1, vec![sibling]);
        }
        return plan;
    }
    if roll < 0.34 {
        if length > 1 || !matches!(stage, Stage::Initial) {
            remove_towers(&mut plan, stage, vec![index]);
        }
        return plan;
    }
    if roll < 0.38 {
        let group = group_of(stage_list(&mut plan, stage), &tower);
        if group.len() < length || !matches!(stage, Stage::Initial) {
            remove_towers(&mut plan, stage, group);
        }
        return plan;
    }
    if roll < 0.48 {
        let near: Vec<Spot> = ctx
            .slots
            .iter()
            .copied()
            .filter(|slot| slot.node == tower.slot.node && (slot.x - tower.slot.x).abs() <= 60.0)
            .collect();
        let pool = if rng.next() < 0.6 && !near.is_empty() {
            near
        } else {
            ctx.slots.clone()
        };
        let slot = *pick(rng, &pool);
        stage_list(&mut plan, stage)[index].slot = slot;
        return plan;
    }
    if roll < 0.52 {
        let group = group_of(stage_list(&mut plan, stage), &tower);
        if rng.next() < 0.5 {
            let shift = *pick(rng, &SHIFTS);
            for member in group {
                if let Some(tower) = stage_list(&mut plan, stage).get_mut(member) {
                    tower.slot.x += shift;
                }
            }
        } else {
            let destination = random_slot(rng, ctx);
            let origin = tower.slot.x;
            for member in group {
                if let Some(tower) = stage_list(&mut plan, stage).get_mut(member) {
                    tower.slot = Spot {
                        node: destination.node,
                        x: destination.x + (tower.slot.x - origin),
                    };
                }
            }
        }
        return plan;
    }
    if roll < 0.58 {
        stage_list(&mut plan, stage)[index].type_index = *pick(rng, &ctx.types);
        return plan;
    }
    if roll < 0.62 {
        let group = group_of(stage_list(&mut plan, stage), &tower);
        let type_index = *pick(rng, &ctx.types);
        for member in group {
            stage_list(&mut plan, stage)[member].type_index = type_index;
        }
        return plan;
    }
    if roll < 0.70 {
        let factor = ((rng.next() - 0.5) * 1.6).exp();
        let tower = &mut stage_list(&mut plan, stage)[index];
        tower.weight = (tower.weight * factor).max(0.05);
        return plan;
    }
    if roll < 0.74 {
        let factor = ((rng.next() - 0.5) * 1.6).exp();
        let type_index = tower.type_index;
        for other in all_towers(&mut plan) {
            if other.type_index == type_index {
                other.weight = (other.weight * factor).max(0.05);
            }
        }
        return plan;
    }
    if roll < 0.80 {
        let mut sides: Vec<char> = tower.traits.chars().collect();
        sides.resize(12, 'A');
        let at = (rng.next() * 8.0) as usize;
        sides[at] = if sides[at] == 'A' { 'B' } else { 'A' };
        stage_list(&mut plan, stage)[index].traits = sides.into_iter().collect();
        return plan;
    }
    if roll < 0.86 {
        let recipe = trait_recipe(rng);
        if rng.next() < 0.5 {
            stage_list(&mut plan, stage)[index].traits = recipe;
        } else {
            let type_index = tower.type_index;
            for other in all_towers(&mut plan) {
                if other.type_index == type_index {
                    other.traits = recipe.clone();
                }
            }
        }
        return plan;
    }
    if roll < 0.93 {
        let cap = random_cap(rng);
        if rng.next() < 0.5 {
            stage_list(&mut plan, stage)[index].max_level = cap;
        } else {
            let type_index = tower.type_index;
            for other in all_towers(&mut plan) {
                if other.type_index == type_index {
                    other.max_level = cap;
                }
            }
        }
        return plan;
    }
    if length > 1 {
        let other = (rng.next() * length as f64) as usize;
        let start = global_start(&plan, stage);
        let (first, second) = (start + index, start + other);
        stage_list(&mut plan, stage).swap(index, other);
        reindex(&mut plan, |global| {
            if global == first {
                Some(second)
            } else if global == second {
                Some(first)
            } else {
                Some(global)
            }
        });
    }
    plan
}

/// One mutated plan, legalized so every tower sits on a legal slot.
pub(super) fn mutate(plan: &Plan, trial: &Trial, rng: &mut Rng, ctx: &Ctx) -> Plan {
    legalize_plan(trial, &mutate_raw(plan, rng, ctx), &ctx.slots)
}

/// A fresh random plan: a few towers, sometimes a cluster, sometimes a phase.
pub(super) fn random_plan(trial: &Trial, rng: &mut Rng, ctx: &Ctx) -> Plan {
    let max = ctx.max_towers;
    let mut towers = Vec::new();
    let count = 3 + (rng.next() * (max.max(3) - 2) as f64) as usize;
    while towers.len() < count {
        if rng.next() < 0.5 {
            let add = cluster(rng, ctx, count - towers.len());
            towers.extend(add);
        } else {
            let slot = random_slot(rng, ctx);
            towers.push(new_tower(rng, ctx, slot, None));
        }
    }
    towers.truncate(count);
    let mut plan = Plan {
        towers,
        skip_policy: SkipPolicy::Never,
        ..Plan::default()
    };
    if rng.next() < 0.35 {
        add_phase(&mut plan, rng, ctx);
    }
    legalize_plan(trial, &plan, &ctx.slots)
}

/// The direct edits the beam tries on a screened plan: relocate the weakest
/// damage towers to their best legal platform, and retry whole-type recipes.
pub(super) fn plan_neighbors(entry: &Candidate, trial: &Trial, ctx: &Ctx) -> Vec<Plan> {
    let plan = &entry.plan;
    let mut out = Vec::new();
    let mut damage: HashMap<usize, f64> = HashMap::new();
    for run in &entry.agg.runs {
        for tower in &run.towers {
            if let Some(index) = tower.plan_index {
                *damage.entry(index).or_default() += tower.damage;
            }
        }
    }
    let mut weak: Vec<usize> = (0..plan.towers.len())
        .filter(|index| WEAK_TYPES.contains(&plan.towers[*index].type_index))
        .collect();
    weak.sort_by(|a, b| {
        damage
            .get(a)
            .copied()
            .unwrap_or(0.0)
            .total_cmp(&damage.get(b).copied().unwrap_or(0.0))
            .then(a.cmp(b))
    });
    weak.truncate(2);
    for index in weak {
        let standing: Vec<Spot> = plan
            .towers
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(_, tower)| tower.slot)
            .collect();
        let mut choices: Vec<(Spot, f64)> = ctx
            .slots
            .iter()
            .enumerate()
            .map(|(slot_index, slot)| (*slot, ctx.coverage[slot_index]))
            .filter(|(slot, _)| {
                let check = validate_placement(trial, &standing, slot.node, slot.x, 0.0);
                check.ok && check.x == slot.x
            })
            .collect();
        choices.sort_by(|a, b| {
            b.1.total_cmp(&a.1)
                .then(a.0.node.cmp(&b.0.node))
                .then(a.0.x.total_cmp(&b.0.x))
        });
        let mut nodes: Vec<usize> = Vec::new();
        for (slot, _) in choices {
            if nodes.contains(&slot.node) {
                continue;
            }
            nodes.push(slot.node);
            let mut moved = plan.clone();
            moved.towers[index].slot = slot;
            out.push(moved);
        }
    }
    let mut types: Vec<usize> = Vec::new();
    for tower in &plan.towers {
        if !types.contains(&tower.type_index) {
            types.push(tower.type_index);
        }
    }
    for type_index in types {
        for recipe in RECIPES {
            let mut edited = plan.clone();
            for tower in all_towers(&mut edited) {
                if tower.type_index == type_index {
                    tower.traits = recipe.into();
                }
            }
            out.push(edited);
        }
    }
    out
}
