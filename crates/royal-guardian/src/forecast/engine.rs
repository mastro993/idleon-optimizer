//! The forecast walk: the segments between boundaries, the commits, the resets
//! and the menu-open pass.

use super::*;
use crate::forecast::model::Model;

/// One horizon boundary, in the order the engine visits it.
pub(crate) struct Boundary {
    pub(crate) at: f64,
    pub(crate) order: usize,
    pub(crate) kind: BoundaryKind,
}

/// What happens at a boundary.
pub(crate) enum BoundaryKind {
    Collect,
    Reset,
    Window(f64, f64),
    Horizon,
}

/// The segment between two boundaries: upstream `_segment`.
pub(crate) fn segment(
    royal: &RoyalGuardian,
    model: &Model,
    start: f64,
    end: f64,
    banked_start: f64,
    banks: bool,
    counter: f64,
) -> Segment {
    let banked_end = if banks {
        banked_start + (end - start)
    } else {
        banked_start
    };
    let at = |banked: f64| start + (banked - banked_start);
    let mut segment = Segment {
        start,
        end,
        banked_at_start: banked_start,
        banked_at_end: banked_end,
        ..Segment::default()
    };
    let mut breaks: Vec<f64> = vec![banked_start, banked_end];
    for node in &model.nodes {
        let fill = fill_hours(node);
        let fill_at = fill.map(|banked| at(banked).max(start));
        let fraction = if node.quantity <= -1.0 {
            1.0
        } else {
            node.quantity.max(0.0) / node.capacity
        };
        let projected = if node.quantity <= -1.0 || node.drain_rate <= 0.0 {
            vec![(start, fraction), (end, fraction)]
        } else if let Some(fill_at) = fill_at.filter(|fill| *fill <= end) {
            vec![
                (
                    start,
                    (1.0_f64).min(
                        (node.quantity.max(0.0) + node.drain_rate * banked_start) / node.capacity,
                    ),
                ),
                (fill_at, 1.0),
                (end, 1.0),
            ]
        } else {
            vec![
                (
                    start,
                    (1.0_f64).min(
                        (node.quantity.max(0.0) + node.drain_rate * banked_start) / node.capacity,
                    ),
                ),
                (
                    end,
                    (1.0_f64).min(
                        (node.quantity.max(0.0) + node.drain_rate * banked_end) / node.capacity,
                    ),
                ),
            ]
        };
        segment.nodes.push(SegmentNode {
            resource: node.resource,
            maps: node.maps.clone(),
            capacity: node.capacity,
            grade: node.grade,
            quantity: node.quantity,
            drained: node.quantity <= -1.0,
            committed: fraction,
            projected,
            fill_at: fill_at.filter(|fill| *fill <= end + EPSILON),
        });
        if banks && let Some(fill_at) = fill_at {
            if fill_at <= end + EPSILON {
                segment.ghosts.fills.push(FillGhost {
                    resource: node.resource,
                    maps: node.maps.clone(),
                    at: fill_at,
                });
            }
            if let Some(banked) =
                fill.filter(|banked| *banked > banked_start && *banked < banked_end)
            {
                breaks.push(banked);
            }
        }
    }
    if !banks {
        return segment;
    }
    breaks.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    breaks.dedup();
    let saved = balances(royal);
    let curves: Vec<(f64, BTreeMap<i64, f64>)> = breaks
        .iter()
        .map(|banked| (at(*banked), gains(royal, model, *banked)))
        .collect();
    let slots: Vec<i64> = {
        let (streams, _) = royal.resource_streams();
        let mut slots: Vec<i64> = streams
            .iter()
            .map(|stream| stream.currency_slot)
            .filter(|slot| *slot >= 0)
            .collect();
        slots.sort_unstable();
        slots.dedup();
        slots
    };
    for slot in slots {
        let points = curves
            .iter()
            .map(|(hour, gain)| {
                let balance = saved.get(slot as usize).copied().unwrap_or(0.0);
                (*hour, balance + gain.get(&slot).copied().unwrap_or(0.0))
            })
            .collect();
        segment.currency.insert(slot, points);
    }
    for candidate in armory_candidates(royal, counter) {
        let Some(curve) = segment.currency.get(&candidate.currency_slot) else {
            continue;
        };
        let balance = saved
            .get(candidate.currency_slot as usize)
            .copied()
            .unwrap_or(0.0);
        if balance >= candidate.cost {
            continue;
        }
        for index in 1..curve.len() {
            let (previous_hour, previous_value) = curve[index - 1];
            let (hour, value) = curve[index];
            if value >= candidate.cost {
                let reached = if value == previous_value {
                    hour
                } else {
                    previous_hour
                        + (candidate.cost - previous_value) / (value - previous_value)
                            * (hour - previous_hour)
                };
                segment.ghosts.affordable.push(Affordable {
                    at: reached.max(start),
                    ..candidate
                });
                break;
            }
        }
    }
    for outpost in &model.models {
        segment
            .ghosts
            .ranks
            .extend(rank_ghosts(outpost, start, banked_start, banked_end));
    }
    for clear in &model.clears {
        let remaining = clear.requirement - clear.progress;
        if remaining <= 0.0 || clear.rate <= 0.0 {
            continue;
        }
        let hour = at(remaining / clear.rate);
        if hour <= end + EPSILON {
            segment.ghosts.clears.push(ClearGhost {
                map: clear.map,
                at: hour.max(start),
            });
        }
    }
    segment
}

/// One native pass over `banked` hours, and what it made affordable:
/// upstream `_commit`.
pub(crate) fn commit(
    royal: &mut RoyalGuardian,
    banked: f64,
    counter: f64,
    at: f64,
    kind: &'static str,
) -> Commit {
    let before = balances(royal);
    let candidates = armory_candidates(royal, counter);
    let mut pass = royal.collect_all(banked);
    let after = balances(royal);
    let mut affordable = Vec::new();
    for candidate in candidates {
        let slot = candidate.currency_slot;
        let old = before.get(slot as usize).copied().unwrap_or(0.0);
        let new = after.get(slot as usize).copied().unwrap_or(0.0);
        if old < candidate.cost && new >= candidate.cost {
            affordable.push(Affordable { at, ..candidate });
        }
    }
    let mut secured = Vec::new();
    for clear in &pass.clears {
        let progress = royal.map_cell(clear.map, 0);
        let requirement = royal.kill_requirement(clear.map);
        if progress - clear.gain < requirement && progress >= requirement {
            secured.push(clear.map);
        }
    }
    Commit {
        at,
        kind,
        banked,
        currency: std::mem::take(&mut pass.currency),
        drained: std::mem::take(&mut pass.drained),
        rank_ups: std::mem::take(&mut pass.rank_ups),
        points: std::mem::take(&mut pass.points),
        secured,
        affordable,
    }
}

/// One daily reset: upstream `_reset`.
pub(crate) fn reset(
    royal: &mut RoyalGuardian,
    model: &Model,
    segment: Option<&Segment>,
    at: f64,
) -> ResetEvent {
    let refills = royal.armory_bonus(70) >= 1.0;
    let grade = royal.armory_bonus(0) >= 1.0;
    let mut event = ResetEvent {
        at,
        refills,
        ..ResetEvent::default()
    };
    let fills: BTreeMap<usize, f64> = segment
        .map(|segment| {
            segment
                .ghosts
                .fills
                .iter()
                .map(|fill| (fill.resource, fill.at))
                .collect()
        })
        .unwrap_or_default();
    for node in &model.nodes {
        let quantity = royal.resource_raw_progress(node.resource);
        if quantity != -1.0
            && let Some(ready_at) = fills.get(&node.resource)
            && *ready_at <= at + EPSILON
        {
            event.missed.push(Missed {
                resource: node.resource,
                ready_at: *ready_at,
                maps: node.maps.clone(),
            });
        }
    }
    for index in 0..data::resource_count() {
        if resource_released(index) && royal.resource_raw_progress(index) == -1.0 && refills {
            royal.set_quantity(index, 0.0);
            if grade {
                royal.set_grade(index, royal.resource_grade(index) + 1.0);
            }
            event.refilled.push(index);
        }
    }
    event.grade_gains = if grade {
        event.refilled.len() as f64
    } else {
        0.0
    };
    event
}

/// Whether the game releases a node at all: upstream `_released`.
pub(crate) fn resource_released(index: usize) -> bool {
    data::resource(index).is_some_and(|row| row.base_capacity > 0.0 && row.currency_slot >= 0)
}

/// A menu-open window: upstream `_active`. Steps exactly to each fill, rank,
/// clear or reset boundary.
pub(crate) fn active(
    royal: &mut RoyalGuardian,
    window: (f64, f64),
    resets: &[f64],
    counter_at: &impl Fn(f64) -> f64,
) -> Window {
    let (start, end) = window;
    let mut window = Window {
        start,
        end,
        ..Window::default()
    };
    let mut at = start;
    while at < end - EPSILON {
        window.steps += 1;
        if window.steps > MAX_WINDOW_STEPS {
            window.truncated = true;
            break;
        }
        let model = model_of(royal);
        let mut step = end - at;
        let reset_hour = resets
            .iter()
            .copied()
            .find(|reset| *reset > at + EPSILON && *reset < end - EPSILON);
        if let Some(reset) = reset_hour {
            step = step.min(reset - at);
        }
        for node in &model.nodes {
            if let Some(fill) = fill_hours(node) {
                step = step.min(fill);
            }
        }
        for outpost in &model.models {
            let rates = outpost.rates();
            for (bar, rate) in rates.iter().enumerate() {
                if *rate > 0.0 {
                    let need =
                        RoyalGuardian::exp_formula(outpost.ranks[bar], bar) - outpost.exp[bar];
                    step = step.min(need / *rate * (1.0 + 1e-9));
                }
            }
        }
        for clear in &model.clears {
            if clear.progress < clear.requirement && clear.rate > 0.0 {
                step = step.min((clear.requirement - clear.progress) / clear.rate);
            }
        }
        step = step.max(1e-7);
        let committed = commit(royal, step, counter_at(at), at + step, "active");
        at += step;
        for (slot, value) in committed.currency {
            *window.summary.currency.entry(slot).or_insert(0.0) += value;
        }
        window.summary.drained.extend(committed.drained);
        window.summary.rank_ups.extend(committed.rank_ups);
        for (map, points) in committed.points {
            *window.summary.points.entry(map).or_insert(0.0) += points;
        }
        window.summary.secured.extend(committed.secured);
        window.summary.affordable.extend(committed.affordable);
        if let Some(reset_hour) = reset_hour
            && (at - reset_hour).abs() <= 1e-6
        {
            let event = reset(royal, &model, None, reset_hour);
            window.summary.resets.push(event);
        }
    }
    window
}
