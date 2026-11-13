//! The comparison (CS-04): run character, loadout and map setups side by side.
//!
//! Ported from `js/stats/systems/common/combat-sim-setup-compare.js`: a row is
//! one character, one loadout and one map (`characterMapComparePlan`), the
//! encounter run gives it its outcome, and every metric carries its paired
//! delta against the first row (`setupCompareCell`). Upstream compares runs
//! over several seeds because they roll random numbers; this build's run is
//! deterministic, so a row holds one run and the standard error stays `None`.
//!
//! The outcome metrics are the ones the run reports; the setup metrics are the
//! resolved gear totals the loadout editor shows. The run itself still does not
//! read the loadout (the auto-mode player is not ported), which the UI states
//! instead of hiding: two loadouts on one map share their outcome and differ in
//! their gear totals.
//!
//! `fixtures/combat-compare.json` is written by `script/combat-compare-oracle.mjs`
//! from the upstream module; the Rust test compares [`cell`] with its output.

use crate::encounter::{self, Outcome, Player, STEP_MS};
use crate::loadout::Totals;
use idleon_common::{Control, Progress};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// One compared setup before its run: who, with what and where.
#[derive(Clone, Debug, PartialEq)]
pub struct RowSpec {
    /// The save character index.
    pub character: usize,
    pub character_name: String,
    /// The loadout's label, such as `Imported save` or a preset name.
    pub loadout: String,
    /// The map index the run reads.
    pub map: usize,
    /// The setup's resolved gear totals, the loadout half of the comparison.
    pub totals: Totals,
}

/// One player and duration over every selected setup.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub player: Player,
    pub duration_ms: u64,
    pub rows: Vec<RowSpec>,
}

impl Plan {
    /// Why this plan cannot be compared, when it cannot. A comparison needs at
    /// least two setups; every map and the player must be runnable.
    pub fn validate(&self) -> Result<(), String> {
        if self.rows.len() < 2 {
            return Err("Pick at least two character, loadout and map setups.".into());
        }
        if self.duration_ms < STEP_MS {
            return Err("A run lasts at least one 10 ms step.".into());
        }
        for row in &self.rows {
            encounter::validate(&encounter::Options {
                map: row.map,
                player: self.player,
                duration_ms: self.duration_ms,
            })?;
        }
        Ok(())
    }
}

/// A finished row: its setup, the map's readable name and the run.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub spec: RowSpec,
    pub map_name: String,
    pub outcome: Outcome,
}

/// A finished comparison: the plan it ran and one row per setup.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub plan: Plan,
    pub rows: Vec<Row>,
}

/// A metric the comparison shows, and whether more is better.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    KillsPerHour,
    DamagePerSecond,
    KillIntervalSeconds,
    WeaponPower,
    Defence,
    MainStat,
    TalentPoints,
}

impl Metric {
    /// Every column, outcomes first.
    pub const ALL: [Metric; 7] = [
        Metric::KillsPerHour,
        Metric::DamagePerSecond,
        Metric::KillIntervalSeconds,
        Metric::WeaponPower,
        Metric::Defence,
        Metric::MainStat,
        Metric::TalentPoints,
    ];

    /// The export's stable key.
    pub fn key(self) -> &'static str {
        match self {
            Metric::KillsPerHour => "killsPerHour",
            Metric::DamagePerSecond => "damagePerSecond",
            Metric::KillIntervalSeconds => "killIntervalSeconds",
            Metric::WeaponPower => "weaponPower",
            Metric::Defence => "defence",
            Metric::MainStat => "mainStat",
            Metric::TalentPoints => "talentPoints",
        }
    }

    /// A short key for UI element ids.
    pub fn short_key(self) -> &'static str {
        match self {
            Metric::KillsPerHour => "kills",
            Metric::DamagePerSecond => "damage",
            Metric::KillIntervalSeconds => "interval",
            Metric::WeaponPower => "weapon",
            Metric::Defence => "defence",
            Metric::MainStat => "main",
            Metric::TalentPoints => "talents",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Metric::KillsPerHour => "Kills / hr",
            Metric::DamagePerSecond => "Damage / s",
            Metric::KillIntervalSeconds => "Kill interval (s)",
            Metric::WeaponPower => "Weapon power",
            Metric::Defence => "Defence",
            Metric::MainStat => "Main stat",
            Metric::TalentPoints => "Talents",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Metric::KillsPerHour => "perHour",
            Metric::DamagePerSecond => "perSecond",
            Metric::KillIntervalSeconds => "seconds",
            _ => "value",
        }
    }

    /// The kill interval is the one metric where a smaller value is better.
    pub fn higher_is_better(self) -> bool {
        self != Metric::KillIntervalSeconds
    }

    /// The row's value for this metric; `None` when the run has no ratio (no
    /// kill to average for the interval).
    pub fn value(self, row: &Row) -> Option<f64> {
        Some(match self {
            Metric::KillsPerHour => row.outcome.kills_per_hour(),
            Metric::DamagePerSecond => row.outcome.damage_per_second(),
            Metric::KillIntervalSeconds => row.outcome.kill_interval_ms()? / 1000.0,
            Metric::WeaponPower => row.spec.totals.weapon_power,
            Metric::Defence => row.spec.totals.defence,
            Metric::MainStat => row.spec.totals.main_value,
            Metric::TalentPoints => row.spec.totals.talents as f64,
        })
    }
}

/// A metric's mean and its paired delta against the baseline.
///
/// The fields mirror upstream's `setupCompareCell`: `n` is the number of runs
/// that reported the metric, `delta` the mean paired difference, `delta_pct`
/// that difference over the baseline mean, and `se` its standard error when at
/// least two pairs exist.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cell {
    pub mean: Option<f64>,
    pub n: usize,
    pub delta: Option<f64>,
    pub delta_pct: Option<f64>,
    pub se: Option<f64>,
    pub base_mean: Option<f64>,
}

/// The mean of `values` plus its paired difference against `baseline`, ported
/// from `setupCompareCell`. Pairs are taken by position, which is the seed in
/// upstream and the single run here; a non-finite value is a missing run.
pub fn cell(values: &[f64], baseline: &[f64]) -> Cell {
    let present: Vec<f64> = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect();
    let mean = (!present.is_empty()).then(|| present.iter().sum::<f64>() / present.len() as f64);
    let mut cell = Cell {
        mean,
        n: present.len(),
        ..Cell::default()
    };
    let pairs: Vec<(f64, f64)> = values
        .iter()
        .zip(baseline)
        .filter(|(value, base)| value.is_finite() && base.is_finite())
        .map(|(value, base)| (*value, *base))
        .collect();
    if pairs.is_empty() {
        return cell;
    }
    let differences: Vec<f64> = pairs.iter().map(|(value, base)| value - base).collect();
    let delta = differences.iter().sum::<f64>() / differences.len() as f64;
    let base_mean = pairs.iter().map(|(_, base)| base).sum::<f64>() / pairs.len() as f64;
    cell.delta = Some(delta);
    cell.base_mean = Some(base_mean);
    cell.delta_pct = if base_mean != 0.0 {
        Some(delta / base_mean)
    } else if delta == 0.0 {
        Some(0.0)
    } else {
        None
    };
    if differences.len() >= 2 {
        let variance = differences
            .iter()
            .map(|difference| (difference - delta) * (difference - delta))
            .sum::<f64>()
            / (differences.len() - 1) as f64;
        cell.se = Some((variance / differences.len() as f64).sqrt());
    }
    cell
}

impl Comparison {
    /// One row's cell for a metric: its value and its delta against the first
    /// row, which is the comparison's baseline.
    pub fn cell(&self, index: usize, metric: Metric) -> Cell {
        let Some(row) = self.rows.get(index) else {
            return Cell::default();
        };
        let values: Vec<f64> = metric.value(row).into_iter().collect();
        if index == 0 {
            return cell(&values, &[]);
        }
        let baseline: Vec<f64> = self
            .rows
            .first()
            .and_then(|row| metric.value(row))
            .into_iter()
            .collect();
        cell(&values, &baseline)
    }
}

/// Run every row, reporting progress and stopping when the reporter says so.
///
/// Rows that share a map reuse its outcome: the modelled run reads the map, the
/// player and the duration only, so a second run on the same map would repeat
/// the same steps. Returns `None` when the run was stopped.
pub fn run(
    plan: &Plan,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Result<Option<Comparison>, String> {
    plan.validate()?;
    let total = plan.rows.len();
    let mut outcomes: BTreeMap<usize, Outcome> = BTreeMap::new();
    let mut rows = Vec::with_capacity(total);
    for (index, spec) in plan.rows.iter().enumerate() {
        let outcome = match outcomes.get(&spec.map) {
            Some(outcome) => outcome.clone(),
            None => {
                let options = encounter::Options {
                    map: spec.map,
                    player: plan.player,
                    duration_ms: plan.duration_ms,
                };
                let offset = index as f32 / total as f32;
                let span = 1.0 / total as f32;
                let result = encounter::run(options, &mut |inner| {
                    report(Progress {
                        fraction: offset + inner.fraction * span,
                        message: format!(
                            "Row {} of {total} · map {} · {}",
                            index + 1,
                            spec.map,
                            inner.message
                        ),
                    })
                })?;
                let Some(outcome) = result else {
                    return Ok(None);
                };
                outcomes.insert(spec.map, outcome.clone());
                outcome
            }
        };
        let map_name = encounter::map(spec.map)
            .map_or_else(|| format!("Map {}", spec.map), encounter::map_name);
        rows.push(Row {
            spec: spec.clone(),
            map_name,
            outcome,
        });
        if report(Progress {
            fraction: (index + 1) as f32 / total as f32,
            message: format!("Row {} of {total} compared", index + 1),
        }) == Control::Stop
        {
            return Ok(None);
        }
    }
    Ok(Some(Comparison {
        plan: plan.clone(),
        rows,
    }))
}

/// The comparison as the section's JSON export: the run, every setup, its
/// outcome, its gear totals and every metric's value and delta.
pub fn document(comparison: &Comparison) -> Value {
    let plan = &comparison.plan;
    let rows: Vec<Value> = comparison
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let outcome = &row.outcome;
            let totals = &row.spec.totals;
            let mut metrics = Map::new();
            for metric in Metric::ALL {
                let cell = comparison.cell(index, metric);
                metrics.insert(
                    metric.key().to_string(),
                    json!({
                        "value": cell.mean,
                        "baseline": cell.base_mean,
                        "delta": cell.delta,
                        "deltaPct": cell.delta_pct,
                    }),
                );
            }
            json!({
                "character": row.spec.character,
                "characterName": row.spec.character_name,
                "loadout": row.spec.loadout,
                "map": row.spec.map,
                "mapName": row.map_name,
                "outcome": {
                    "kills": outcome.kills,
                    "damage": outcome.damage,
                    "killsPerHour": outcome.kills_per_hour(),
                    "damagePerSecond": outcome.damage_per_second(),
                    "killIntervalSeconds": outcome.kill_interval_ms().map(|ms| ms / 1000.0),
                    "killsByType": outcome.kills_by_type,
                    "simulatedMs": outcome.simulated_ms,
                    "steps": outcome.steps,
                    "frames": outcome.frames,
                },
                "totals": {
                    "weaponPower": totals.weapon_power,
                    "defence": totals.defence,
                    "mainStat": totals.main_stat,
                    "mainValue": totals.main_value,
                    "talents": totals.talents,
                    "equipped": totals.equipped,
                    "slots": totals.slots,
                },
                "metrics": Value::Object(metrics),
            })
        })
        .collect();
    json!({
        "section": "combat-sim",
        "kind": "comparison",
        "player": player_document(plan.player),
        "durationMs": plan.duration_ms,
        "baseline": 0,
        "metrics": Metric::ALL.iter().map(|metric| json!({
            "key": metric.key(),
            "label": metric.label(),
            "unit": metric.unit(),
            "higherIsBetter": metric.higher_is_better(),
        })).collect::<Vec<_>>(),
        "rows": rows,
        "unsupported": encounter::unsupported(),
    })
}

/// [`document`] as the pretty-printed text the clipboard receives.
pub fn document_json(comparison: &Comparison) -> String {
    serde_json::to_string_pretty(&document(comparison)).expect("a comparison document serializes")
}

fn player_document(player: Player) -> Value {
    match player {
        Player::StandIn { kill_every_ms } => json!({
            "id": player.id(),
            "label": player.label(),
            "killEveryMs": kill_every_ms,
        }),
        Player::Idle => json!({
            "id": player.id(),
            "label": player.label(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kills: u64, simulated_ms: u64) -> Row {
        let mut outcome = Outcome {
            map: 1,
            scene: String::new(),
            player: "standIn",
            duration_ms: simulated_ms,
            simulated_ms,
            steps: 0,
            frames: 0,
            kills,
            kills_by_type: BTreeMap::new(),
            damage: 0.0,
            idle_ticks: 0,
            respawn: Vec::new(),
        };
        outcome.damage = kills as f64;
        Row {
            spec: RowSpec {
                character: 0,
                character_name: "Ana".into(),
                loadout: "Imported save".into(),
                map: 1,
                totals: Totals::default(),
            },
            map_name: String::new(),
            outcome,
        }
    }

    #[test]
    fn a_cell_is_the_mean_and_its_paired_delta() {
        let cell = cell(&[10.0, 20.0, 30.0], &[8.0, 8.0, 8.0]);
        assert_eq!(cell.mean, Some(20.0));
        assert_eq!(cell.n, 3);
        assert_eq!(cell.delta, Some(12.0));
        assert!((cell.delta_pct.expect("a percentage") - 1.5).abs() < 1e-9);
        assert!(cell.se.expect("three pairs") > 0.0);
        assert_eq!(cell.base_mean, Some(8.0));
    }

    #[test]
    fn a_missing_run_is_left_out_of_the_mean_and_the_pairing() {
        let cell = cell(&[10.0, f64::NAN], &[5.0, 9.0]);
        assert_eq!(cell.mean, Some(10.0));
        assert_eq!(cell.n, 1);
        assert_eq!(cell.delta, Some(5.0));
        assert_eq!(cell.se, None);
    }

    #[test]
    fn a_zero_baseline_reports_no_percentage() {
        let missing = cell(&[3.0], &[0.0]);
        assert_eq!(missing.delta_pct, None);
        let flat = cell(&[0.0], &[0.0]);
        assert_eq!(flat.delta_pct, Some(0.0));
        assert_eq!(flat.se, None);
    }

    #[test]
    fn the_comparison_delta_is_against_the_first_row() {
        let first = row(10, 10_000);
        let second = row(20, 10_000);
        let comparison = Comparison {
            plan: Plan {
                player: Player::Idle,
                duration_ms: 10_000,
                rows: vec![first.spec.clone(), second.spec.clone()],
            },
            rows: vec![first, second],
        };
        let baseline = comparison.cell(0, Metric::KillsPerHour);
        assert_eq!(baseline.delta, None);
        assert_eq!(baseline.mean, Some(3_600.0));
        let second = comparison.cell(1, Metric::KillsPerHour);
        assert_eq!(second.delta, Some(3_600.0));
        assert_eq!(second.delta_pct, Some(1.0));
        let interval = comparison.cell(1, Metric::KillIntervalSeconds);
        assert_eq!(interval.delta, Some(-0.5));
    }
}
