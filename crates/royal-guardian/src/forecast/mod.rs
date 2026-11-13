//! The forecast: an event projection over the native collect pass.
//!
//! Ported from `js/stats/systems/w7/royal-forecast.js`. State changes only at
//! the collects the player places, the menu-open windows that collect
//! continuously, and the daily reset; between them time banks and "ghost"
//! markers show what a collect at that moment would commit. Layout, points and
//! currency are never spent.
//!
//! The engine walks the horizon boundary by boundary. A collect commits one
//! native pass over the banked time; a reset refills the drained nodes when
//! Resource Replenish is owned and grades them when Resource Grades is; a
//! menu-open window steps from fill to fill, rank to rank and clear to clear so
//! no event inside it is missed.

use super::*;
use crate::collect_exp::{RankUp, row_mut};
use std::collections::BTreeMap;

/// How far apart the daily resets sit.
const RESET_INTERVAL: f64 = 24.0;

/// How many future ranks one segment ghosts.
const RANK_GHOSTS: f64 = 3.0;

/// How close the rank search stops to the exact threshold hour.
const GHOST_TOLERANCE: f64 = 1.0 / 240.0;

/// The most steps one menu-open window may take.
const MAX_WINDOW_STEPS: usize = 4000;

const EPSILON: f64 = 1e-9;

/// The horizon bounds: minimum, maximum, default.
pub const HORIZON: (f64, f64, f64) = (1.0, 336.0, 72.0);

/// What the forecast projects: the horizon, the collects the player places and
/// the menu-open windows.
#[derive(Clone, Debug, PartialEq)]
pub struct ForecastOptions {
    /// Hours from the save snapshot; clamped to [`HORIZON`].
    pub horizon_hours: f64,
    /// The hours a collect happens at.
    pub collects: Vec<f64>,
    /// The menu-open windows, as `(start, end)` hours.
    pub windows: Vec<(f64, f64)>,
}

impl Default for ForecastOptions {
    /// The upstream defaults: a three-day horizon, no collects and no windows.
    fn default() -> Self {
        Self {
            horizon_hours: HORIZON.2,
            collects: Vec::new(),
            windows: Vec::new(),
        }
    }
}

/// One Armory shelf a collect would make affordable: upstream
/// `_armoryCandidates` entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Affordable {
    pub index: usize,
    pub order: usize,
    pub name: &'static str,
    pub level: f64,
    pub cost: f64,
    pub currency_slot: i64,
    pub at: f64,
}

/// What one collect committed: upstream `_commit`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Commit {
    pub at: f64,
    pub kind: &'static str,
    pub banked: f64,
    pub currency: BTreeMap<i64, f64>,
    pub drained: Vec<usize>,
    pub rank_ups: Vec<RankUp>,
    pub points: BTreeMap<usize, f64>,
    pub secured: Vec<usize>,
    pub affordable: Vec<Affordable>,
}

/// What happened across one menu-open window: upstream `_active` summary.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowSummary {
    pub currency: BTreeMap<i64, f64>,
    pub drained: Vec<usize>,
    pub rank_ups: Vec<RankUp>,
    pub points: BTreeMap<usize, f64>,
    pub secured: Vec<usize>,
    pub affordable: Vec<Affordable>,
    pub resets: Vec<ResetEvent>,
}

/// One menu-open window: upstream `_active`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Window {
    pub start: f64,
    pub end: f64,
    pub entry: Option<Commit>,
    pub summary: WindowSummary,
    pub steps: usize,
    pub truncated: bool,
}

/// A node the reset refilled too late to collect.
#[derive(Clone, Debug, PartialEq)]
pub struct Missed {
    pub resource: usize,
    pub ready_at: f64,
    pub maps: Vec<usize>,
}

/// One daily reset: upstream `_reset`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetEvent {
    pub at: f64,
    /// Whether Resource Replenish refills drained nodes.
    pub refills: bool,
    pub refilled: Vec<usize>,
    pub grade_gains: f64,
    pub missed: Vec<Missed>,
}

/// A node that fills at a known hour inside a segment.
#[derive(Clone, Debug, PartialEq)]
pub struct FillGhost {
    pub resource: usize,
    pub maps: Vec<usize>,
    pub at: f64,
}

/// A rank a collect would commit.
#[derive(Clone, Debug, PartialEq)]
pub struct RankGhost {
    pub map: usize,
    pub bar: usize,
    pub rank: f64,
    pub from: f64,
    pub at: f64,
    /// Whether the rank is already reached and only needs a collect.
    pub ready: bool,
}

/// An outpost clear that would finish.
#[derive(Clone, Debug, PartialEq)]
pub struct ClearGhost {
    pub map: usize,
    pub at: f64,
}

/// The events a collect would commit at each segment: upstream `_segment` ghosts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ghosts {
    pub fills: Vec<FillGhost>,
    pub ranks: Vec<RankGhost>,
    pub affordable: Vec<Affordable>,
    pub clears: Vec<ClearGhost>,
}

/// One node inside a segment: its saved amount and its projection over time.
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentNode {
    pub resource: usize,
    pub maps: Vec<usize>,
    pub capacity: f64,
    pub grade: f64,
    pub quantity: f64,
    pub drained: bool,
    /// What the save has now, as a fraction of capacity.
    pub committed: f64,
    /// `(hour, fraction)` points across the segment.
    pub projected: Vec<(f64, f64)>,
    pub fill_at: Option<f64>,
}

/// One segment between two boundaries, with what a collect there would commit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub banked_at_start: f64,
    pub banked_at_end: f64,
    pub ghosts: Ghosts,
    pub nodes: Vec<SegmentNode>,
    /// The currency balance over the segment, per slot.
    pub currency: BTreeMap<i64, Vec<(f64, f64)>>,
}

/// The forecast's own totals: upstream `_summary`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub collects: usize,
    pub windows: usize,
    pub currency: BTreeMap<i64, f64>,
    pub rank_ups: usize,
    pub points: f64,
    pub secured: Vec<usize>,
    pub refilled: usize,
    pub missed: usize,
    pub grade_gains: f64,
    pub banked_at_horizon: f64,
}

/// The projection: upstream `forecastRoyal` result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Forecast {
    pub horizon_hours: f64,
    /// Whether time banks while the menu is closed.
    pub banks: bool,
    pub banked_start_hours: f64,
    pub end_banked_hours: f64,
    pub resets: Vec<f64>,
    pub collects: Vec<Commit>,
    /// The collects placed inside a menu-open window, which the window covers.
    pub ignored_collects: Vec<f64>,
    pub windows: Vec<Window>,
    pub reset_events: Vec<ResetEvent>,
    pub segments: Vec<Segment>,
    pub truncated: bool,
    pub summary: Summary,
}

mod engine;
mod model;

pub(crate) use engine::{Boundary, BoundaryKind, active, commit, reset, segment};
pub(crate) use model::{armory_candidates, balances, fill_hours, gains, model_of, rank_ghosts};

impl RoyalGuardian {
    /// The forecast over the current state: upstream `forecastRoyal`.
    pub fn forecast(&self, options: &ForecastOptions) -> Forecast {
        let horizon = clamp_horizon(options.horizon_hours);
        let mut state = self.clone();
        let banks = state.banks_time();
        let timing = state.reset_timing();
        let mut resets = Vec::new();
        if let Some(hours) = timing.hours_remaining {
            let mut at = hours;
            while at <= horizon + EPSILON {
                resets.push(at);
                at += RESET_INTERVAL;
            }
        }
        let windows = merge_windows(&options.windows, horizon);
        let requested = round_times(&options.collects, horizon);
        let collects: Vec<f64> = requested
            .iter()
            .copied()
            .filter(|hour| !inside(*hour, &windows))
            .collect();
        let ignored_collects: Vec<f64> = requested
            .iter()
            .copied()
            .filter(|hour| inside(*hour, &windows))
            .collect();
        let saved_counter = state.daily_purchase_counter();
        let counter_at = |hours: f64| {
            if !resets.is_empty() && hours >= resets[0] - EPSILON {
                0.0
            } else {
                saved_counter
            }
        };
        let mut boundaries: Vec<Boundary> = Vec::new();
        for hour in &collects {
            boundaries.push(Boundary {
                at: *hour,
                order: 1,
                kind: BoundaryKind::Collect,
            });
        }
        for reset in &resets {
            if !strictly_inside(*reset, &windows) {
                boundaries.push(Boundary {
                    at: *reset,
                    order: 0,
                    kind: BoundaryKind::Reset,
                });
            }
        }
        for (start, end) in &windows {
            boundaries.push(Boundary {
                at: *start,
                order: 2,
                kind: BoundaryKind::Window(*start, *end),
            });
        }
        boundaries.push(Boundary {
            at: horizon,
            order: 3,
            kind: BoundaryKind::Horizon,
        });
        boundaries.sort_by(|left, right| {
            left.at
                .partial_cmp(&right.at)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(left.order.cmp(&right.order))
        });
        let mut forecast = Forecast {
            horizon_hours: horizon,
            banks,
            banked_start_hours: state.banked_hours(),
            resets: resets.clone(),
            ignored_collects,
            ..Forecast::default()
        };
        let mut at = 0.0;
        let mut banked = if banks { state.banked_hours() } else { 0.0 };
        let mut model = model_of(&state);
        for boundary in boundaries {
            if boundary.at < at - EPSILON {
                continue;
            }
            let segment = (boundary.at > at + EPSILON).then(|| {
                segment(
                    &state,
                    &model,
                    at,
                    boundary.at,
                    banked,
                    banks,
                    counter_at(at),
                )
            });
            if let Some(segment) = &segment {
                forecast.segments.push(segment.clone());
            }
            if banks {
                banked += boundary.at - at;
            }
            at = boundary.at;
            match boundary.kind {
                BoundaryKind::Collect => {
                    forecast.collects.push(commit(
                        &mut state,
                        banked,
                        counter_at(at),
                        at,
                        "collect",
                    ));
                    banked = 0.0;
                }
                BoundaryKind::Reset => {
                    let event = reset(&mut state, &model, segment.as_ref(), at);
                    forecast.reset_events.push(event);
                }
                BoundaryKind::Window(start, end) => {
                    let entry = if banked > 0.0 {
                        Some(commit(
                            &mut state,
                            banked,
                            counter_at(at),
                            at,
                            "window-entry",
                        ))
                    } else {
                        None
                    };
                    banked = 0.0;
                    let mut window =
                        active(&mut state, (start, end), &forecast.resets, &counter_at);
                    window.entry = entry;
                    if window.truncated {
                        forecast.truncated = true;
                    }
                    forecast
                        .reset_events
                        .extend(window.summary.resets.iter().cloned());
                    forecast.windows.push(window);
                    at = end;
                }
                BoundaryKind::Horizon => break,
            }
            model = model_of(&state);
            if matches!(boundary.kind, BoundaryKind::Horizon) {
                break;
            }
        }
        forecast.end_banked_hours = banked;
        forecast.summary = summary(&forecast);
        forecast
    }
}

/// The forecast's totals: upstream `_summary`.
fn summary(forecast: &Forecast) -> Summary {
    let mut summary = Summary {
        collects: forecast.collects.len(),
        windows: forecast.windows.len(),
        ..Summary::default()
    };
    let mut commits: Vec<&Commit> = forecast.collects.iter().collect();
    for window in &forecast.windows {
        if let Some(entry) = &window.entry {
            commits.push(entry);
        }
    }
    for commit in &commits {
        for (slot, value) in &commit.currency {
            *summary.currency.entry(*slot).or_insert(0.0) += value;
        }
        summary.rank_ups += commit.rank_ups.len();
        for points in commit.points.values() {
            summary.points += points;
        }
        for map in &commit.secured {
            if !summary.secured.contains(map) {
                summary.secured.push(*map);
            }
        }
    }
    for window in &forecast.windows {
        for (slot, value) in &window.summary.currency {
            *summary.currency.entry(*slot).or_insert(0.0) += value;
        }
        summary.rank_ups += window.summary.rank_ups.len();
        for points in window.summary.points.values() {
            summary.points += points;
        }
        for map in &window.summary.secured {
            if !summary.secured.contains(map) {
                summary.secured.push(*map);
            }
        }
    }
    for event in &forecast.reset_events {
        summary.refilled += event.refilled.len();
        summary.missed += event.missed.len();
        summary.grade_gains += event.grade_gains;
    }
    summary.banked_at_horizon = forecast.end_banked_hours;
    summary
}

/// Clamp the horizon the way upstream `_clamp` does.
fn clamp_horizon(value: f64) -> f64 {
    let (min, max, fallback) = HORIZON;
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

/// The requested collect hours, rounded to a microsecond and deduplicated:
/// upstream `_times`.
fn round_times(list: &[f64], horizon: f64) -> Vec<f64> {
    let mut hours: Vec<f64> = list
        .iter()
        .copied()
        .filter(|hour| hour.is_finite() && *hour >= 0.0 && *hour <= horizon)
        .map(|hour| (hour * 1e6).round() / 1e6)
        .collect();
    hours.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    hours.dedup();
    hours
}

/// Clamp and merge the menu-open windows: upstream `_windows`.
fn merge_windows(list: &[(f64, f64)], horizon: f64) -> Vec<(f64, f64)> {
    let mut items: Vec<(f64, f64)> = list
        .iter()
        .map(|(start, end)| (start.max(0.0), end.min(horizon)))
        .filter(|(start, end)| start.is_finite() && end.is_finite() && end - start > EPSILON)
        .collect();
    items.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (start, end) in items {
        match merged.last_mut() {
            Some(last) if start <= last.1 + EPSILON => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// Whether an hour sits inside a window, edges included.
fn inside(hour: f64, windows: &[(f64, f64)]) -> bool {
    windows
        .iter()
        .any(|(start, end)| hour > start - EPSILON && hour < end + EPSILON)
}

/// Whether an hour sits strictly inside a window.
fn strictly_inside(hour: f64, windows: &[(f64, f64)]) -> bool {
    windows
        .iter()
        .any(|(start, end)| hour > start + EPSILON && hour < end - EPSILON)
}

impl RoyalGuardian {
    /// Writes one node quantity, for the forecast's reset: `RoyalG[4][index]`.
    fn set_quantity(&mut self, index: usize, value: f64) {
        let cells = row_mut(&mut self.g, row::RESOURCES);
        while cells.len() <= index {
            cells.push(Value::from(0.0));
        }
        cells[index] = Value::from(value);
    }

    /// Writes one node grade, for the forecast's reset: `RoyalG[5][index]`.
    fn set_grade(&mut self, index: usize, value: f64) {
        let cells = row_mut(&mut self.g, row::GRADES);
        while cells.len() <= index {
            cells.push(Value::from(0.0));
        }
        cells[index] = Value::from(value);
    }
}
