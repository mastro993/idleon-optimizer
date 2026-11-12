//! The seeded attempt itself: one board against one obstruction.
//!
//! Ports `simulateJellyOperation` and its helpers (`roidActivationTime`,
//! `reviveTarget`) from `js/stats/systems/w7/jelly-operator.js`. The port keeps
//! the frame-by-frame loop and the upstream fast path's one behavioural
//! difference: the displayed DPS, and therefore the Best DPS the Bloodcell
//! multiplier reads, only updates on the one-second callback ticks.

use super::rng::{Geometry, Rng, geometry, hit_frame, random_int};
use super::{Contribution, Death, OperationOptions, RevivePolicy, RoidTiming, Trial};
use crate::board::Layout;
use crate::data::data;
use crate::metrics::PROGRESS_PER_FRAME;
use crate::{
    FRAME_SECONDS, Jelly, MetricOptions, ROID_DECREMENT_PER_FRAME, ROID_TIME_TOTAL,
    boss_attack_cooldown, boss_hp, boss_time,
};

/// One placed cell's runtime constants.
#[derive(Clone, Debug)]
struct RuntimeCell {
    anchor: usize,
    cell_type: usize,
    slots: Vec<usize>,
    cooldown: f64,
    level_factor: f64,
    normal_progress: f64,
    roid_progress: f64,
    proximity: f64,
    dps: f64,
    geometry: Geometry,
}

/// `roidActivationTime`.
fn roid_activation_time(timing: RoidTiming, obstruction: usize) -> f64 {
    match timing {
        RoidTiming::Never => f64::INFINITY,
        RoidTiming::Immediate => 0.0,
        RoidTiming::FiveSeconds => 5.0,
        RoidTiming::Critical => boss_time(obstruction),
        RoidTiming::At(seconds) => seconds.max(0.0),
    }
}

/// `simulateJellyOperation`: one seeded attempt.
pub fn simulate(jelly: &Jelly, layout: &Layout, options: OperationOptions) -> Trial {
    let obstruction = options.obstruction.unwrap_or(jelly.progress.obstruction);
    let fever = options.fever.unwrap_or(jelly.progress.fever);
    let hp = boss_hp(obstruction);
    let standard_time = boss_time(obstruction);
    let mut trial = Trial {
        obstruction,
        fever,
        hp,
        standard_time,
        seed: options.seed,
        tries_before: jelly.progress.tries,
        obstruction_after: obstruction,
        ..Trial::default()
    };
    let metrics = jelly.layout_metrics(
        layout,
        MetricOptions {
            fever: Some(fever),
            ..MetricOptions::default()
        },
    );
    if !metrics.valid {
        trial.reason = metrics.reason;
        return trial;
    }
    trial.valid = true;

    let total_cell_level = jelly.total_cell_level();
    let exp_multiplier = jelly.cell_exp_multiplier(Some(fever));
    let cell_level_percent = 1.0 + jelly.upgrade_quantity(17);
    let currency = jelly.currency_factors(Some(fever), total_cell_level);
    let cell_damage_base = jelly.cell_damage_multiplier(MetricOptions {
        fever: Some(fever),
        total_cell_level: Some(total_cell_level),
        ..MetricOptions::default()
    });
    let can_level = jelly.upgrade_quantity(10) >= 1.0;
    let critical_unlocked = jelly.upgrade_quantity(36) == 1.0;
    let amoeba_unlocked = jelly.upgrade_quantity(28) >= 1.0;
    let dps_unlocked = jelly.upgrade_quantity(12) >= 1.0;
    let roid_level = jelly.upgrade_quantity(29);
    let roid_start = if roid_level >= 1.0 {
        roid_activation_time(options.roid_timing, obstruction)
    } else {
        f64::INFINITY
    };
    let roid_speed = 1.0 + (50.0 + roid_level) / 100.0;
    let cells: Vec<RuntimeCell> = metrics
        .cells
        .iter()
        .map(|cell| RuntimeCell {
            anchor: cell.anchor,
            cell_type: cell.cell_type,
            slots: cell.slots.clone(),
            cooldown: cell.cooldown_progress,
            level_factor: 1.0
                + f64::from(jelly.cell_level(cell.cell_type)) * cell_level_percent / 100.0,
            normal_progress: PROGRESS_PER_FRAME * cell.organelle * cell.proximity,
            roid_progress: PROGRESS_PER_FRAME * roid_speed * cell.organelle * cell.proximity,
            proximity: cell.proximity,
            dps: cell.dps,
            geometry: geometry(cell.anchor, cell.cell_type),
        })
        .collect();
    let all_slots: Vec<usize> = cells.iter().flat_map(|cell| cell.slots.clone()).collect();
    let immunoid_order: Vec<usize> = cells
        .iter()
        .filter(|cell| cell.cell_type == 4)
        .flat_map(|cell| cell.slots.clone())
        .collect();
    let revives_total = jelly.upgrade_quantity(35).floor().max(0.0);

    let mut rng = Rng::new(options.seed);
    let mut timer_rng = Rng::new(options.seed ^ 0x51ed_270b);
    let mut alive = vec![false; data().size()];
    for slot in &all_slots {
        alive[*slot] = true;
    }
    let mut live_immunoid: Vec<usize> = immunoid_order.clone();
    let mut live_slots: Vec<usize> = all_slots.clone();

    // The opening roll walks every board anchor, like the upstream loop: one
    // drawn progress per placed cell and one skipped value per anchor.
    let mut cell_progress = vec![0.0_f64; cells.len()];
    let mut opening_progress = Vec::with_capacity(cells.len());
    let by_anchor: Vec<Option<usize>> = {
        let mut map = vec![None; data().size()];
        for (index, cell) in cells.iter().enumerate() {
            map[cell.anchor] = Some(index);
        }
        map
    };
    for index in &by_anchor {
        if let Some(index) = *index {
            let cell = &cells[index];
            let maximum = (cell.cooldown - 1.0_f64).max(5.0).floor();
            let sampled = random_int(&mut rng, 0.0, maximum) as f64;
            cell_progress[index] = sampled;
            opening_progress.push((cell.anchor, sampled));
        }
        rng.advance(1);
    }
    // The player's own anchors are not part of the estimate's opening roll.

    let mut contribution: Vec<Contribution> = (0..9)
        .map(|cell_type| Contribution {
            cell_type,
            name: data().cell_name(cell_type).to_string(),
            ..Contribution::default()
        })
        .collect();
    let mut exp_by_type = [0.0_f64; 9];
    let mut damage = 0.0;
    let mut remaining_hp = hp;
    let mut bloodcells = 0.0;
    let mut shots = 0.0;
    let mut hits = 0.0;
    let mut amoeba_stacks = 0.0;
    let mut amoeba_at_critical: Option<f64> = None;
    let mut damage_at_critical: Option<f64> = None;
    let mut fever_seconds = 0.0;
    let mut best_dps = jelly.progress.best_dps;
    let mut currency_best_dps = best_dps;
    let mut currency_multiplier = currency.multiplier(best_dps);
    let mut displayed_dps: f64 = 0.0;
    let mut peak_displayed_dps: f64 = 0.0;
    let mut buckets = vec![0.0_f64];
    let mut next_second_tick = timer_rng.next();
    let mut roid_remaining = 0.0;
    let mut roid_used = false;
    let mut boss_counter = 0.0;
    let mut time = 0.0;
    let mut frame_index: u64 = 0;
    let mut deaths = 0.0;
    let mut revives_used = 0.0;
    let mut revives_remaining = revives_total;
    let mut boss_attacks = 0.0;
    let mut immunoid_hits = 0.0;
    let mut immunoid_focus_time = 0.0;
    let mut timer_remaining = standard_time;
    let mut ended_by = "running";
    let mut death_log: Vec<Death> = Vec::new();
    let boss_cooldown = boss_attack_cooldown(obstruction);
    let cold_unlocked = fever == 0 && jelly.upgrade_quantity(16) >= 1.0;
    let source_critical_limit =
        (3.0 * boss_cooldown * (all_slots.len() as f64 + revives_total + 1.0) + 1.0) / 60.0;
    let max_critical_seconds = options
        .max_critical_seconds
        .map(|seconds| seconds.max(0.0))
        .unwrap_or(source_critical_limit);
    let max_time = standard_time + max_critical_seconds;
    let revive_policy = options.revive_policy;
    let mut projectiles: Vec<(u64, f64, usize)> = Vec::new();

    while time < max_time && ended_by == "running" {
        let frame_end = time + FRAME_SECONDS;
        let mut dps_changed = false;
        while dps_unlocked && next_second_tick < frame_end {
            buckets.insert(0, 0.0);
            if buckets.len() >= 6 {
                buckets.remove(4);
            }
            fever_seconds += 1.0;
            next_second_tick += 1.0;
            dps_changed = true;
        }
        if !roid_used && time >= roid_start {
            roid_used = true;
            roid_remaining = ROID_TIME_TOTAL;
        }
        roid_remaining -= ROID_DECREMENT_PER_FRAME;
        let roid_active = roid_remaining > 0.0;
        let cold_factor = if cold_unlocked {
            1.0 + fever_seconds / 100.0
        } else {
            1.0
        };

        for (index, cell) in cells.iter().enumerate() {
            if !alive[cell.anchor] {
                continue;
            }
            cell_progress[index] += if roid_active {
                cell.roid_progress
            } else {
                cell.normal_progress
            };
            if cell_progress[index] < cell.cooldown {
                continue;
            }
            cell_progress[index] = 0.0;
            let arrival = frame_index + u64::from(hit_frame(cell.geometry, &mut rng));
            let damage_base = data().cells.base_damage[cell.cell_type]
                * (cell_damage_base * cold_factor)
                * metrics.damage_passive
                * cell.level_factor;
            let shot_damage = damage_base
                * (1.0 + amoeba_stacks / 100.0)
                * metrics.virus_multiplier
                * cell.proximity;
            projectiles.push((arrival.max(frame_index), shot_damage, cell.cell_type));
            shots += 1.0;
            contribution[cell.cell_type].shots += 1.0;
        }

        let mut index = 0;
        while index < projectiles.len() {
            let (arrival, hit_damage, cell_type) = projectiles[index];
            if arrival > frame_index {
                index += 1;
                continue;
            }
            projectiles.remove(index);
            damage += hit_damage;
            remaining_hp -= hit_damage;
            buckets[0] += hit_damage;
            if currency_best_dps != best_dps {
                currency_best_dps = best_dps;
                currency_multiplier = currency.multiplier(best_dps);
            }
            let gained = hit_damage * currency_multiplier;
            bloodcells += gained;
            hits += 1.0;
            contribution[cell_type].hits += 1.0;
            contribution[cell_type].damage += hit_damage;
            contribution[cell_type].bloodcells += gained;
            if amoeba_unlocked && cell_type == 0 {
                amoeba_stacks += 1.0;
            }
            if can_level {
                exp_by_type[cell_type] += exp_multiplier;
                contribution[cell_type].exp += exp_multiplier;
            }
        }

        if dps_changed && buckets.len() > 3 {
            displayed_dps = (buckets[1] + buckets[2] + buckets[3]) / 3.0;
            peak_displayed_dps = peak_displayed_dps.max(displayed_dps);
            best_dps = best_dps.max(displayed_dps);
        }

        timer_remaining -= FRAME_SECONDS;
        time = frame_end;
        if amoeba_at_critical.is_none() && timer_remaining < 0.0 {
            amoeba_at_critical = Some(amoeba_stacks);
            damage_at_critical = Some(damage);
        }
        if remaining_hp <= 0.0 {
            ended_by = "clear";
            break;
        }

        if timer_remaining < 0.0 {
            if !live_immunoid.is_empty() {
                immunoid_focus_time += FRAME_SECONDS;
            }
            if !critical_unlocked {
                ended_by = "timer";
                break;
            }
            boss_counter += 1.0;
            if boss_counter >= boss_cooldown {
                live_slots.retain(|slot| alive[*slot]);
                if live_slots.is_empty() {
                    ended_by = "all-slots-dead";
                    break;
                }
                let focused = !live_immunoid.is_empty();
                let pool = if focused {
                    live_immunoid.clone()
                } else {
                    live_slots.clone()
                };
                let target = pool[random_int(&mut rng, 0.0, pool.len() as f64 - 1.0)];
                alive[target] = false;
                deaths += 1.0;
                boss_attacks += 1.0;
                let owner = cells.iter().find(|cell| cell.slots.contains(&target));
                death_log.push(Death {
                    time,
                    slot: target,
                    focused,
                    disabled_anchor: owner.is_some_and(|cell| cell.anchor == target),
                    revived: false,
                });
                if focused {
                    immunoid_hits += 1.0;
                    boss_counter = -2.0 * boss_cooldown;
                } else {
                    boss_counter = 0.0;
                }
                if revives_remaining > 0.0 && revive_policy != RevivePolicy::None {
                    let dead: Vec<usize> = all_slots
                        .iter()
                        .copied()
                        .filter(|slot| !alive[*slot])
                        .collect();
                    if let Some(revived) = revive_target(revive_policy, &dead, &cells) {
                        alive[revived] = true;
                        revives_remaining -= 1.0;
                        revives_used += 1.0;
                        if let Some(death) = death_log
                            .iter_mut()
                            .rev()
                            .find(|death| death.slot == revived && !death.revived)
                        {
                            death.revived = true;
                        }
                    }
                }
                live_immunoid.retain(|slot| alive[*slot]);
                live_slots = all_slots
                    .iter()
                    .copied()
                    .filter(|slot| alive[*slot])
                    .collect();
            }
        }
        if ended_by == "running" && timer_remaining < 0.0 && critical_unlocked {
            rng.next();
            rng.next();
        }
        frame_index += 1;
    }
    if ended_by == "running" {
        ended_by = "simulation-cap";
    }

    let live_slots_count = all_slots.iter().filter(|slot| alive[**slot]).count();
    trial.success = ended_by == "clear";
    trial.ended_by = ended_by.to_string();
    trial.time = time;
    trial.normal_time = time.min(standard_time);
    trial.critical_time = (time - standard_time).max(0.0);
    trial.critical_unlocked = critical_unlocked;
    trial.damage = damage;
    trial.damage_pct = if hp > 0.0 { damage / hp } else { 0.0 };
    trial.shots = shots;
    trial.hits = hits;
    trial.projectiles_remaining = projectiles.len();
    trial.bloodcells = bloodcells;
    trial.cell_exp = exp_by_type.iter().sum();
    trial.exp_by_type = exp_by_type;
    trial.amoeba_stacks = amoeba_stacks;
    trial.amoeba_stacks_at_critical_start = amoeba_at_critical.unwrap_or(amoeba_stacks);
    trial.damage_at_critical_start = damage_at_critical.unwrap_or(damage);
    trial.fever_seconds = fever_seconds;
    trial.displayed_dps = displayed_dps;
    trial.peak_dps = peak_displayed_dps;
    trial.best_dps_before = jelly.progress.best_dps;
    trial.best_dps_after = best_dps;
    trial.deaths = deaths;
    trial.boss_attacks = boss_attacks;
    trial.immunoid_hits = immunoid_hits;
    trial.immunoid_focus_time = immunoid_focus_time;
    trial.live_slots = live_slots_count as f64;
    trial.revives_used = revives_used;
    trial.revives_remaining = revives_remaining;
    trial.roid_used = roid_used;
    trial.roid_start = roid_start.is_finite().then_some(roid_start);
    trial.opening_progress = opening_progress;
    trial.contribution = contribution;
    trial.death_log = death_log;
    trial.attempt_delta =
        (if trial.success { 1.0 } else { 0.0 }) - (if obstruction > 1 { 1.0 } else { 0.0 });
    trial.obstruction_after = if trial.success {
        (obstruction + 1).min(data().rewards.len().saturating_sub(1))
    } else {
        obstruction
    };
    trial
}

/// `reviveTarget`: which dead slot the policy brings back first.
fn revive_target(
    policy: RevivePolicy,
    dead_slots: &[usize],
    cells: &[RuntimeCell],
) -> Option<usize> {
    if policy == RevivePolicy::None || dead_slots.is_empty() {
        return None;
    }
    let mut details: Vec<(usize, bool, bool, f64)> = dead_slots
        .iter()
        .map(|slot| {
            let cell = cells.iter().find(|cell| cell.slots.contains(slot));
            (
                *slot,
                cell.is_some_and(|cell| cell.cell_type == 4),
                cell.is_some_and(|cell| cell.anchor == *slot),
                cell.map_or(0.0, |cell| cell.dps),
            )
        })
        .collect();
    let rank = |a: &(usize, bool, bool, f64), b: &(usize, bool, bool, f64)| match policy {
        RevivePolicy::ImmunoidFirst => {
            b.1.cmp(&a.1)
                .then(b.2.cmp(&a.2))
                .then(b.3.total_cmp(&a.3))
                .then(a.0.cmp(&b.0))
        }
        RevivePolicy::AnchorFirst => b.2.cmp(&a.2).then(b.1.cmp(&a.1)).then(a.0.cmp(&b.0)),
        RevivePolicy::DpsFirst => {
            b.2.cmp(&a.2)
                .then(b.3.total_cmp(&a.3))
                .then(b.1.cmp(&a.1))
                .then(a.0.cmp(&b.0))
        }
        RevivePolicy::SoakFirst => b.1.cmp(&a.1).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0)),
        RevivePolicy::None => std::cmp::Ordering::Equal,
    };
    details.sort_by(rank);
    details.first().map(|detail| detail.0)
}
