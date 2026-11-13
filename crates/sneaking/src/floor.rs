//! SN-04: the floor × mastery table, the active door and the next captive.
//!
//! Ported from the floor table in `sneaking-calc.html` (`_cellData`, `_thresholds`,
//! `doorStateLabel`), the saved-placement rates in `_savedFloorsHtml` and the
//! decision cards in `_recommendationHtml`, with the formulas of
//! `js/stats/systems/w6/sneaking-math.js` (`floorDetectionBase`, `floorJadeBase`,
//! `floorExpMult`, `doorHP`, `doorState`, `captiveTargetForFloor`, `untieReq`,
//! `funeralStealthBonus`) and `computeDoorClearEstimate` in `sneaking-jade.js`.
//!
//! Each row is one floor at one mastery level. The save's own mastery carries the
//! saved placement and its flowers; a projected mastery is upstream's what-if:
//! the flowers reset to zero and every door reads fresh at its full HP, because
//! the game relocks the floors when the mastery advances. Detection, Jade, EXP,
//! door damage and untie progress all come from the loadout's own runtime loop,
//! so a charm or weapon edit moves every row that depends on it.

use super::math::{DoorRate, TeamMember, team_door_rates};
use super::{DoorState, FLOOR_COUNT, Job, Loadout, detection_base};

/// One floor at one mastery level: the floor's own tables plus what the twins
/// standing on it produce at that level.
#[derive(Clone, Debug, PartialEq)]
pub struct FloorMastery {
    pub floor: usize,
    pub mastery: u32,
    /// Whether this row is the save's own mastery.
    pub current: bool,
    /// The saved Funeral Flowers on the floor, zero on a projected mastery.
    pub flowers: f64,
    /// `funeralStealthBonus`: the flowers times Ninja Knowledge 21, in percent.
    pub funeral_percent: f64,
    pub detection_base: f64,
    pub jade_base: f64,
    pub exp_base: f64,
    pub door_hp: f64,
    /// The door's state phrase: `No door`, `Unavailable`, `Cleared`, `Active`,
    /// `Locked by F4` or `Fresh` on a projected mastery.
    pub door_state: String,
    /// The saved twins standing on the floor, in roster order.
    pub twins: Vec<usize>,
    /// The action each of those twins takes at this mastery, aligned with
    /// `twins`: a projected door turns a Nunchaku into a door attack.
    pub jobs: Vec<Job>,
    /// The best (lowest) detection chance among them; `None` on an empty floor.
    pub best_detection: Option<f64>,
    pub jade_per_hour: f64,
    pub exp_per_hour: f64,
    /// The floor's summed door damage per hour, zero when nobody on it holds a
    /// Nunchaku or the door is not active.
    pub door_damage_per_hour: f64,
    /// The hours of successful door hits the remaining HP needs; `0` when the
    /// door is finished and `None` when nothing on the floor can land a hit.
    pub door_eta: Option<f64>,
    /// The captive waiting on this floor, when one does.
    pub captive: Option<usize>,
    pub untie_remaining: f64,
    pub untie_eta: Option<f64>,
}

/// The one unfinished door and the saved placement's time to clear it.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveDoor {
    pub floor: usize,
    pub max_hp: f64,
    pub progress: f64,
    pub remaining_hp: f64,
    pub damage_per_hour: f64,
    pub hits_per_hour: f64,
    pub eta: Option<f64>,
}

/// The first captive the save still holds and the saved placement's time to
/// free it.
#[derive(Clone, Debug, PartialEq)]
pub struct NextCaptive {
    pub twin: usize,
    pub floor: usize,
    pub remaining: f64,
    pub progress_per_hour: f64,
    pub eta: Option<f64>,
}

/// One twin's inputs and outputs on its saved floor, resolved once per mastery
/// level so the floor table reads them instead of rescanning the loadout.
struct TwinMetric {
    twin: usize,
    floor: usize,
    job: Job,
    detection: f64,
    action_speed: f64,
    ko_time: f64,
    dodge: f64,
    taunting: bool,
    jade: f64,
    exp: f64,
    door_damage: f64,
    untie: f64,
}

impl Loadout {
    /// The saved mastery and its neighbours: upstream's `M-1`, `M (cur)`, `M+1`.
    pub fn mastery_levels(&self) -> Vec<u32> {
        let mastery = self.mastery;
        let mut levels = vec![mastery.saturating_sub(1), mastery, mastery + 1];
        levels.dedup();
        levels
    }

    /// The floor × mastery table: every floor from eleven down to zero, each at
    /// every [`Self::mastery_levels`] level.
    pub fn floor_mastery(&self) -> Vec<FloorMastery> {
        let levels: Vec<(u32, Loadout, Vec<TwinMetric>)> = self
            .mastery_levels()
            .into_iter()
            .map(|level| {
                let view = self.at_mastery(level);
                let metrics = view.metrics();
                (level, view, metrics)
            })
            .collect();
        let mut rows = Vec::with_capacity(FLOOR_COUNT * levels.len());
        for floor in (0..FLOOR_COUNT).rev() {
            for (level, view, metrics) in &levels {
                rows.push(view.floor_row(floor, *level, metrics));
            }
        }
        rows
    }

    /// The one unfinished door, or `None` when the current mastery has none.
    pub fn active_door(&self) -> Option<ActiveDoor> {
        let metrics = self.metrics();
        (1..FLOOR_COUNT).find_map(|floor| {
            let state = self.door_state(floor);
            state.active.then(|| {
                let (damage_per_hour, hits_per_hour) = self.door_rates(&metrics, floor);
                ActiveDoor {
                    floor,
                    max_hp: state.max_hp,
                    progress: state.progress,
                    remaining_hp: state.remaining_hp,
                    damage_per_hour,
                    hits_per_hour,
                    eta: door_eta(state.remaining_hp, damage_per_hour, hits_per_hour),
                }
            })
        })
    }

    /// The first captive the save holds, or `None` when every twin is free.
    pub fn next_captive(&self) -> Option<NextCaptive> {
        let twin = self
            .twins
            .iter()
            .take(self.players.min(self.twins.len()))
            .find(|twin| !twin.untied)?;
        let floor = self
            .captive_floor(twin.index)
            .filter(|floor| *floor > 0)
            .unwrap_or(twin.floor);
        let remaining = (twin.untie_required - twin.untie_progress).max(0.0);
        let metrics = self.metrics();
        let progress_per_hour = self
            .floor_group_rates(&metrics, floor)
            .into_iter()
            .filter(|(metric, _)| metric.job == Job::Untie)
            .map(|(metric, rate)| metric.untie * rate.hits_per_hour)
            .sum();
        Some(NextCaptive {
            twin: twin.index,
            floor,
            remaining,
            progress_per_hour,
            eta: (remaining > 0.0 && progress_per_hour > 0.0)
                .then(|| remaining / progress_per_hour),
        })
    }

    /// The loadout at a what-if mastery: the flowers reset and every door reads
    /// fresh, the game's own relock when the mastery advances.
    fn at_mastery(&self, mastery: u32) -> Loadout {
        let projected = mastery != self.mastery;
        let mut view = self.clone();
        view.mastery = mastery;
        view.projected = projected;
        if projected {
            view.flowers = vec![0.0; self.flowers.len()];
        }
        view
    }

    /// Every active twin's per-action values at this view's mastery.
    fn metrics(&self) -> Vec<TwinMetric> {
        let gold = self.gold_bonuses();
        self.twins
            .iter()
            .take(self.players.min(self.twins.len()))
            .map(|twin| {
                let bonuses = self.bonuses_with(twin.index, &gold);
                TwinMetric {
                    twin: twin.index,
                    floor: twin.floor,
                    job: self.job(twin.index),
                    detection: self.detection_with(twin.index, &gold),
                    action_speed: self.action_speed_with(twin.index, &gold),
                    ko_time: self.ko_seconds_with(twin.index, &gold),
                    dodge: bonuses.get(0),
                    taunting: bonuses.get(1) > 0.0,
                    jade: self.jade_coin_with(twin.index, &gold).coin,
                    exp: self.exp_action_with(twin.index, &gold).exp,
                    door_damage: self.door_damage_with(twin.index, &gold).damage,
                    untie: self.untie_progress(twin.index).progress,
                }
            })
            .collect()
    }

    /// The team rates of the twins standing on one floor.
    fn floor_group_rates<'a>(
        &self,
        metrics: &'a [TwinMetric],
        floor: usize,
    ) -> Vec<(&'a TwinMetric, DoorRate)> {
        let group: Vec<&TwinMetric> = metrics
            .iter()
            .filter(|metric| metric.floor == floor)
            .collect();
        let team: Vec<TeamMember> = group
            .iter()
            .map(|metric| TeamMember {
                action_speed: metric.action_speed,
                detection: metric.detection,
                ko_time: metric.ko_time,
                dodge_pct: metric.dodge,
                taunting: metric.taunting,
                flower_gain: 1.0 + self.ninja_bonus(30) / 100.0,
            })
            .collect();
        group.into_iter().zip(team_door_rates(&team)).collect()
    }

    /// The summed door damage and successful hits per hour on a floor.
    fn door_rates(&self, metrics: &[TwinMetric], floor: usize) -> (f64, f64) {
        self.floor_group_rates(metrics, floor)
            .into_iter()
            .filter(|(metric, _)| metric.job == Job::Door)
            .fold((0.0, 0.0), |(damage, hits), (metric, rate)| {
                (
                    damage + metric.door_damage * rate.hits_per_hour,
                    hits + rate.hits_per_hour,
                )
            })
    }

    /// One floor's row at a mastery level.
    fn floor_row(&self, floor: usize, level: u32, metrics: &[TwinMetric]) -> FloorMastery {
        let state = self.door_state(floor);
        let mut twins = Vec::new();
        let mut jobs = Vec::new();
        let mut best_detection: Option<f64> = None;
        let mut jade_per_hour = 0.0;
        let mut exp_per_hour = 0.0;
        let mut door_damage = 0.0;
        let mut door_hits = 0.0;
        let mut untie_per_hour = 0.0;
        for (metric, rate) in self.floor_group_rates(metrics, floor) {
            twins.push(metric.twin);
            jobs.push(metric.job);
            best_detection = Some(
                best_detection.map_or(metric.detection, |best: f64| best.min(metric.detection)),
            );
            if metric.job == Job::Jade {
                jade_per_hour += metric.jade * rate.hits_per_hour;
            }
            if matches!(metric.job, Job::Jade | Job::Training) {
                exp_per_hour += metric.exp * rate.hits_per_hour;
            }
            if metric.job == Job::Door {
                door_damage += metric.door_damage * rate.hits_per_hour;
                door_hits += rate.hits_per_hour;
            }
            if metric.job == Job::Untie {
                untie_per_hour += metric.untie * rate.hits_per_hour;
            }
        }
        let captive = self.captive_target(floor);
        let untie_remaining = captive
            .and_then(|index| self.twin(index))
            .map_or(0.0, |twin| {
                (twin.untie_required - twin.untie_progress).max(0.0)
            });
        FloorMastery {
            floor,
            mastery: level,
            current: !self.projected,
            flowers: self.flowers.get(floor).copied().unwrap_or(0.0),
            funeral_percent: self.funeral_percent(floor),
            detection_base: detection_base(floor, level),
            jade_base: super::math::jade_base(floor, level),
            exp_base: super::math::exp_base(floor, level),
            door_hp: super::door_hp(floor, level),
            door_state: self.door_label(floor, &state),
            twins,
            jobs,
            best_detection,
            jade_per_hour,
            exp_per_hour,
            door_damage_per_hour: door_damage,
            door_eta: door_eta(state.remaining_hp, door_damage, door_hits),
            captive,
            untie_remaining,
            untie_eta: (untie_remaining > 0.0 && untie_per_hour > 0.0)
                .then(|| untie_remaining / untie_per_hour),
        }
    }

    /// The word upstream prints in the Door column.
    fn door_label(&self, floor: usize, state: &DoorState) -> String {
        if floor == 0 {
            return "No door".into();
        }
        if self.projected {
            return if state.max_hp > 0.0 {
                "Fresh".into()
            } else {
                "Unavailable".into()
            };
        }
        if self.mastery < self.mastery_max || state.max_hp <= 0.0 {
            return "Unavailable".into();
        }
        if state.progress >= state.max_hp {
            return "Cleared".into();
        }
        if state.active {
            return "Active".into();
        }
        match state.active_floor {
            Some(active) => format!("Locked by F{active}"),
            None => "Inactive".into(),
        }
    }
}

/// `computeDoorClearEstimate`: the hours of successful hits the remaining HP
/// needs. A finished door is `0`; `None` when the door still needs hits but
/// nothing on the floor can land them, which reads as the door's state instead
/// of an endless ETA.
pub fn door_eta(remaining_hp: f64, damage_per_hour: f64, hits_per_hour: f64) -> Option<f64> {
    if remaining_hp.max(0.0) <= 0.0 {
        return Some(0.0);
    }
    if damage_per_hour <= 0.0
        || hits_per_hour <= 0.0
        || damage_per_hour.is_nan()
        || hits_per_hour.is_nan()
    {
        return None;
    }
    let average_damage = damage_per_hour / hits_per_hour;
    if average_damage <= 0.0 || average_damage.is_nan() {
        return None;
    }
    let required_hits = (remaining_hp / average_damage).ceil().max(1.0);
    let hours = required_hits / hits_per_hour;
    hours.is_finite().then_some(hours)
}
