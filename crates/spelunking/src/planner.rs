//! The expedition optimizer: the search scope, the exhaustive configuration
//! search and the bounded Amber phase search, ported from
//! `js/stats/systems/w7/spelunk-planner.js`.
//!
//! Upstream drives both searches as generators: it yields expedition requests
//! and receives the runs the workers simulated. This port runs the same search
//! synchronously with [`simulate`], reporting the upstream progress fractions,
//! and returns `None` when the reporter asks to stop. The seeds, the candidate
//! order and the summary arithmetic are the upstream ones, so the same inputs,
//! scope and seed reproduce the recommendation and every row behind it.

use super::sim::{RunOptions, RunResult};
use super::{Config, ConfigError, Inputs, Policy, Requested, ToolChoice, normalize, simulate};
use idleon_common::{Control, Progress};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The objectives the optimizer compares, in the upstream menu order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Objective {
    #[default]
    Depth,
    Amber,
    AmberPerStamina,
    SustainableAmber,
    Grand,
}

impl Objective {
    pub const ALL: [Objective; 5] = [
        Objective::Depth,
        Objective::Amber,
        Objective::AmberPerStamina,
        Objective::SustainableAmber,
        Objective::Grand,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Objective::Depth => "depth",
            Objective::Amber => "amber",
            Objective::AmberPerStamina => "amber-per-stamina",
            Objective::SustainableAmber => "sustainable-amber",
            Objective::Grand => "grand",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Objective::Depth => "Depth",
            Objective::Amber => "Collected Amber",
            Objective::AmberPerStamina => "Amber per stamina",
            Objective::SustainableAmber => "Sustainable Amber per hour",
            Objective::Grand => "Grand Discoveries",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "depth" => Some(Objective::Depth),
            "amber" => Some(Objective::Amber),
            "amber-per-stamina" => Some(Objective::AmberPerStamina),
            "sustainable-amber" => Some(Objective::SustainableAmber),
            "grand" => Some(Objective::Grand),
            _ => None,
        }
    }

    /// The three objectives the Amber search accepts.
    pub fn amber(self) -> bool {
        matches!(
            self,
            Objective::Amber | Objective::AmberPerStamina | Objective::SustainableAmber
        )
    }
}

/// The play style a candidate runs with.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Auto,
    Manual,
}

/// The descent choice: first exit, the whole floor, or every fifth depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Clear {
    First,
    Full,
    Fifth,
}

impl serde::Serialize for Clear {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Clear::First => serializer.serialize_bool(false),
            Clear::Full => serializer.serialize_bool(true),
            Clear::Fifth => serializer.serialize_str("fifth"),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Clear {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = Clear;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("`false`, `true` or `fifth`")
            }
            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Clear, E> {
                Ok(if value { Clear::Full } else { Clear::First })
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Clear, E> {
                if value == "fifth" {
                    Ok(Clear::Fifth)
                } else {
                    Err(E::custom("`false`, `true` or `fifth`"))
                }
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

/// Which search dimensions may change; an unsearched dimension stays at the
/// captured controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Enabled {
    pub party: bool,
    pub modes: bool,
    pub clears: bool,
    pub speeds: bool,
    pub elixirs: bool,
    pub memorized: bool,
    pub tools: bool,
    pub entry: bool,
}

impl Default for Enabled {
    fn default() -> Self {
        Self {
            party: true,
            modes: true,
            clears: true,
            speeds: true,
            elixirs: true,
            memorized: true,
            tools: true,
            entry: true,
        }
    }
}

impl Enabled {
    fn get(&self, key: &str) -> bool {
        match key {
            "party" => self.party,
            "modes" => self.modes,
            "clears" => self.clears,
            "speeds" => self.speeds,
            "elixirs" => self.elixirs,
            "memorized" => self.memorized,
            "tools" => self.tools,
            "entry" => self.entry,
            _ => true,
        }
    }
}

/// A requested search scope; every field falls back to the full upstream
/// default for the captured configuration.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScopeRequest {
    pub party_min: Option<usize>,
    pub party_max: Option<usize>,
    pub characters: Option<Vec<usize>>,
    pub ignore_order: Option<bool>,
    pub modes: Option<Vec<Mode>>,
    pub clears: Option<Vec<Clear>>,
    pub speeds: Option<Vec<u32>>,
    pub elixirs: Option<Vec<usize>>,
    pub memorized: Option<Vec<usize>>,
    pub enabled: Option<Enabled>,
}

/// The search scope the upstream normaliser accepts.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchScope {
    pub party_min: usize,
    pub party_max: usize,
    pub characters: Vec<usize>,
    pub ignore_order: bool,
    pub modes: Vec<Mode>,
    pub clears: Vec<Clear>,
    pub speeds: Vec<u32>,
    pub elixirs: Vec<usize>,
    pub memorized: Vec<usize>,
    pub enabled: Enabled,
}

impl SearchScope {
    fn enabled(&self, key: &str) -> bool {
        self.enabled.get(key)
    }
}

/// The upstream `normalizeSpelunkSearchScope`.
pub fn normalize_scope(
    input: &Inputs,
    baseline: &Config,
    requested: &ScopeRequest,
) -> Result<SearchScope, ConfigError> {
    let capacity = input.caves[baseline.cave].capacity;
    let enabled = requested.enabled.unwrap_or_default();
    let select = |key: &str, available: &[usize], empty: bool| -> Result<Vec<usize>, ConfigError> {
        let values = match key {
            "characters" => requested.characters.clone(),
            "elixirs" => requested.elixirs.clone(),
            "memorized" => requested.memorized.clone(),
            _ => None,
        }
        .unwrap_or_else(|| available.to_vec());
        if (!empty && values.is_empty()) || values.iter().any(|value| !available.contains(value)) {
            return Err(ConfigError(format!("Invalid search {key} selection.")));
        }
        Ok(unique(values))
    };
    let modes = if enabled.modes {
        let values = requested.modes.clone().unwrap_or_else(|| {
            if input.auto {
                vec![Mode::Auto, Mode::Manual]
            } else {
                vec![Mode::Manual]
            }
        });
        if values.is_empty() {
            return Err(ConfigError("Invalid search modes selection.".into()));
        }
        unique(values)
    } else {
        vec![if baseline.policy == Policy::Auto {
            Mode::Auto
        } else {
            Mode::Manual
        }]
    };
    let clears = if enabled.clears {
        let values = requested
            .clears
            .clone()
            .unwrap_or_else(|| vec![Clear::First, Clear::Full, Clear::Fifth]);
        if values.is_empty() {
            return Err(ConfigError("Invalid search clears selection.".into()));
        }
        unique(values)
    } else {
        vec![clear_mode(baseline)]
    };
    if !modes.contains(&Mode::Manual) && !clears.iter().any(|clear| *clear != Clear::Fifth) {
        return Err(ConfigError(
            "Every-fifth-depth clearing requires manual play. Allow Manual or another descent choice.".into(),
        ));
    }
    let speeds = if enabled.speeds {
        let values = requested
            .speeds
            .clone()
            .unwrap_or_else(|| (0..=input.speed_max).collect());
        if values.is_empty() && modes.contains(&Mode::Auto) {
            return Err(ConfigError("Invalid search speeds selection.".into()));
        }
        if values.iter().any(|speed| *speed > input.speed_max) {
            return Err(ConfigError("Invalid search speeds selection.".into()));
        }
        unique(values)
    } else {
        vec![baseline.speed]
    };
    let ids: Vec<usize> = (0..input.elixir_owned).collect();
    let party_min = if enabled.party {
        count(requested.party_min, 1, 1, capacity, "minimum party size")?
    } else {
        baseline.party.len()
    };
    let party_max = if enabled.party {
        count(
            requested.party_max,
            capacity,
            party_min,
            capacity,
            "maximum party size",
        )?
    } else {
        baseline.party.len()
    };
    let characters = if enabled.party {
        let mut characters = select(
            "characters",
            &input.characters.iter().map(|c| c.id).collect::<Vec<_>>(),
            true,
        )?;
        characters.sort_unstable();
        characters
    } else {
        baseline.party.clone()
    };
    if enabled.party
        && requested.ignore_order.is_some()
        && requested.ignore_order != Some(true)
        && requested.ignore_order != Some(false)
    {
        return Err(ConfigError("Invalid party order search option.".into()));
    }
    let ignore_order = enabled.party && requested.ignore_order == Some(true);
    if eligible(input, baseline, &characters, ignore_order).len() < party_min {
        return Err(ConfigError(
            "Not enough selected characters have the required starting stamina for this party range.".into(),
        ));
    }
    Ok(SearchScope {
        party_min,
        party_max,
        characters,
        ignore_order,
        modes,
        clears,
        speeds,
        elixirs: if enabled.elixirs {
            select("elixirs", &ids, true)?
        } else {
            unique(
                baseline
                    .elixirs
                    .iter()
                    .filter(|id| **id >= 0)
                    .map(|id| *id as usize),
            )
        },
        memorized: if enabled.memorized {
            select("memorized", &ids, true)?
        } else {
            baseline.memorized.clone()
        },
        enabled,
    })
}

fn unique<T: PartialEq + Copy>(values: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut out = Vec::new();
    for value in values {
        if !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

fn count(
    value: Option<usize>,
    fallback: usize,
    lo: usize,
    hi: usize,
    name: &str,
) -> Result<usize, ConfigError> {
    let value = value.unwrap_or(fallback);
    if !(lo..=hi).contains(&value) {
        return Err(ConfigError(format!("Invalid {name}.")));
    }
    Ok(value)
}

/// The clear mode a configuration represents.
fn clear_mode(config: &Config) -> Clear {
    match config.policy {
        Policy::Auto => {
            if config.full_clear {
                Clear::Full
            } else {
                Clear::First
            }
        }
        Policy::ClearFifth => Clear::Fifth,
        Policy::Clear | Policy::AmberFarm => Clear::Full,
        Policy::Dive | Policy::Efficient => Clear::First,
    }
}

fn auto_clears(scope: &SearchScope) -> Vec<Clear> {
    scope
        .clears
        .iter()
        .copied()
        .filter(|clear| *clear != Clear::Fifth)
        .collect()
}

fn mean(values: impl IntoIterator<Item = f64>) -> f64 {
    let values: Vec<f64> = values.into_iter().collect();
    values.iter().sum::<f64>() / values.len() as f64
}

/// The upstream seed derivation.
fn seed_of(seed: u32, index: usize, final_trials: bool) -> u32 {
    seed.wrapping_add((index as u32 + 1).wrapping_mul(0x9e37_79b9))
        .wrapping_add(if final_trials { 0x4f1b_bcdc } else { 0 })
}

fn eligible(input: &Inputs, base: &Config, characters: &[usize], ignore_order: bool) -> Vec<usize> {
    let mut ids: Vec<usize> = input
        .characters
        .iter()
        .filter(|character| {
            (base.stamina == super::StaminaMode::Full
                || character.stamina >= character.max_stamina / 2.0)
                && characters.contains(&character.id)
        })
        .map(|character| character.id)
        .collect();
    if ignore_order {
        ids.sort_unstable();
    }
    ids
}

fn policies(scope: &SearchScope, base: &Config) -> Vec<Policy> {
    if !scope.enabled("modes") {
        if base.policy == Policy::Auto {
            return vec![Policy::Auto];
        }
        return scope
            .clears
            .iter()
            .map(|clear| match clear {
                Clear::Fifth => Policy::ClearFifth,
                Clear::Full => {
                    if base.policy == Policy::AmberFarm {
                        Policy::AmberFarm
                    } else {
                        Policy::Clear
                    }
                }
                Clear::First => {
                    if matches!(
                        base.policy,
                        Policy::Clear | Policy::ClearFifth | Policy::AmberFarm
                    ) {
                        Policy::Efficient
                    } else {
                        base.policy
                    }
                }
            })
            .collect();
    }
    Policy::ALL
        .into_iter()
        .filter(|policy| *policy != Policy::AmberFarm)
        .filter(|policy| {
            if *policy == Policy::Auto {
                scope.enabled("modes")
                    && scope.modes.contains(&Mode::Auto)
                    && !auto_clears(scope).is_empty()
            } else {
                scope.enabled("modes")
                    && scope.modes.contains(&Mode::Manual)
                    && scope.clears.contains(&clear_mode(&Config {
                        policy: *policy,
                        ..base.clone()
                    }))
            }
        })
        .collect()
}

fn belt_occupancy(input: &Inputs, scope: &SearchScope) -> usize {
    input.slots.min(scope.elixirs.len() * input.dupes)
}

fn in_scope(config: &Config, scope: &SearchScope, occupancy: usize, base: &Config) -> bool {
    config.party.len() >= scope.party_min
        && config.party.len() <= scope.party_max
        && (scope.enabled("party") || config.party == base.party)
        && config.party.iter().all(|id| scope.characters.contains(id))
        && (!scope.ignore_order || config.party.windows(2).all(|pair| pair[0] < pair[1]))
        && policies(scope, base).contains(&config.policy)
        && scope.clears.contains(&clear_mode(config))
        && (config.policy != Policy::Auto || scope.speeds.contains(&config.speed))
        && (scope.enabled("speeds") || config.speed == base.speed)
        && (scope.enabled("tools") || config.tool == base.tool)
        && (scope.enabled("entry") || config.entry == base.entry)
        && config
            .elixirs
            .iter()
            .all(|id| *id == -1 || scope.elixirs.contains(&(*id as usize)))
        && (if scope.enabled("elixirs") {
            config.elixirs.iter().filter(|id| **id >= 0).count() == occupancy
        } else {
            config.elixirs == base.elixirs
        })
        && (if scope.enabled("memorized") {
            config
                .memorized
                .iter()
                .all(|id| scope.memorized.contains(id))
        } else {
            config.memorized == base.memorized
        })
}

/// One run's score under an objective.
pub fn run_metric(run: &RunResult, input: &Inputs, objective: Objective) -> f64 {
    match objective {
        Objective::Depth => run.depth as f64,
        Objective::Grand => run.grands,
        _ if !run.settled => 0.0,
        Objective::Amber => run.rewards[0],
        Objective::AmberPerStamina => run.rewards[0] / run.spent.max(1.0),
        Objective::SustainableAmber => {
            let recovery = run
                .stamina
                .iter()
                .map(|member| ((member.start - member.end) / input.regen).max(0.0))
                .fold(0.0, f64::max);
            run.rewards[0] / (run.seconds / 3600.0 + recovery).max(1.0 / 3600.0)
        }
    }
}

/// A sample count and its denominator.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rate {
    pub count: usize,
    pub total: usize,
}

/// One member's averaged contribution.
#[derive(Clone, Debug, PartialEq)]
pub struct MemberSummary {
    pub id: usize,
    pub digs: f64,
    pub hits: f64,
    pub misses: f64,
    pub spent: f64,
    pub resisted_stamina: f64,
    pub damage: f64,
    pub broken: f64,
    pub descents: f64,
    pub amber: f64,
    pub unused: usize,
    pub no_advance: usize,
    pub no_further_floor: usize,
    pub resist_stall: usize,
    pub total: usize,
}

/// The averaged diagnostics of a candidate's trials.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticsSummary {
    pub resist_stall: Rate,
    pub blocked: Rate,
    pub unused: f64,
    pub no_advance: f64,
    pub resisted_stamina: f64,
    pub members: Vec<MemberSummary>,
}

/// One candidate's summary: its scored trials and the means behind them.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub config: Config,
    pub runs: Vec<RunResult>,
    pub score: f64,
    pub se: f64,
    pub depth: f64,
    pub start_depth: f64,
    pub amber: f64,
    pub seconds: f64,
    pub settled: usize,
    pub total: usize,
    pub rope: usize,
    pub toll: usize,
    pub blocked: usize,
    pub recovery_hours: f64,
    pub diagnostics: Option<DiagnosticsSummary>,
}

fn summarize(
    config: Config,
    runs: Vec<RunResult>,
    input: &Inputs,
    objective: Objective,
) -> Summary {
    let metrics: Vec<f64> = runs
        .iter()
        .map(|run| run_metric(run, input, objective))
        .collect();
    let score = mean(metrics.iter().copied());
    let sd = if metrics.len() > 1 {
        (metrics
            .iter()
            .map(|value| (value - score).powi(2))
            .sum::<f64>()
            / (metrics.len() - 1) as f64)
            .sqrt()
    } else {
        0.0
    };
    let recovery_hours = mean(runs.iter().map(|run| {
        run.stamina
            .iter()
            .map(|member| ((member.start - member.end) / input.regen).max(0.0))
            .fold(0.0, f64::max)
    }));
    Summary {
        se: sd / (metrics.len() as f64).sqrt(),
        depth: mean(runs.iter().map(|run| run.depth as f64)),
        start_depth: mean(runs.iter().map(|run| run.start_depth as f64)),
        amber: mean(
            runs.iter()
                .map(|run| if run.settled { run.rewards[0] } else { 0.0 }),
        ),
        seconds: mean(runs.iter().map(|run| run.seconds)),
        settled: runs.iter().filter(|run| run.settled).count(),
        total: runs.len(),
        rope: runs
            .iter()
            .filter(|run| {
                run.exit
                    .as_ref()
                    .is_some_and(|exit| exit.kind.id() == "rope")
            })
            .count(),
        toll: runs
            .iter()
            .filter(|run| {
                run.exit
                    .as_ref()
                    .is_some_and(|exit| exit.kind.id() == "toll")
            })
            .count(),
        blocked: runs.iter().filter(|run| !run.settled).count(),
        recovery_hours,
        diagnostics: summarize_diagnostics(&config, &runs),
        score,
        config,
        runs,
    }
}

fn summarize_diagnostics(config: &Config, runs: &[RunResult]) -> Option<DiagnosticsSummary> {
    if !runs.iter().all(|run| {
        config.party.iter().all(|id| {
            run.diagnostics
                .members
                .iter()
                .any(|member| member.id == *id)
        })
    }) {
        return None;
    }
    let rate = |predicate: fn(&RunResult) -> bool| Rate {
        count: runs.iter().filter(|run| predicate(run)).count(),
        total: runs.len(),
    };
    Some(DiagnosticsSummary {
        resist_stall: rate(|run| run.diagnostics.resist_stall),
        blocked: rate(|run| run.diagnostics.blocked),
        unused: mean(runs.iter().map(|run| run.diagnostics.unused as f64)),
        no_advance: mean(runs.iter().map(|run| run.diagnostics.no_advance as f64)),
        resisted_stamina: mean(runs.iter().map(|run| run.diagnostics.resisted_stamina)),
        members: config
            .party
            .iter()
            .map(|id| {
                let members: Vec<_> = runs
                    .iter()
                    .filter_map(|run| {
                        run.diagnostics
                            .members
                            .iter()
                            .find(|member| member.id == *id)
                    })
                    .collect();
                let average = |value: fn(&super::Contribution) -> f64| {
                    mean(members.iter().map(|member| value(member)))
                };
                MemberSummary {
                    id: *id,
                    digs: average(|member| member.digs),
                    hits: average(|member| member.hits),
                    misses: average(|member| member.misses),
                    spent: average(|member| member.spent),
                    resisted_stamina: average(|member| member.resisted_stamina),
                    damage: average(|member| member.damage),
                    broken: average(|member| member.broken),
                    descents: average(|member| member.descents),
                    amber: mean(members.iter().map(|member| member.rewards[0])),
                    unused: members.iter().filter(|member| member.unused).count(),
                    no_advance: members.iter().filter(|member| member.no_advance).count(),
                    no_further_floor: members
                        .iter()
                        .filter(|member| member.no_further_floor)
                        .count(),
                    resist_stall: members.iter().filter(|member| member.resist_stall).count(),
                    total: members.len(),
                }
            })
            .collect(),
    })
}

/// The paired change one party size makes over the size below it.
#[derive(Clone, Debug, PartialEq)]
pub struct Paired {
    pub objective: f64,
    pub interval: Option<[f64; 2]>,
    pub depth: f64,
    pub amber: f64,
    pub grands: f64,
    pub extra_depth: usize,
    pub total: usize,
    pub rewards: [f64; 12],
}

fn paired_party(larger: &Summary, smaller: &Summary) -> Paired {
    Paired {
        objective: 0.0,
        interval: None,
        depth: larger.depth - smaller.depth,
        amber: larger.amber - smaller.amber,
        grands: mean(
            larger
                .runs
                .iter()
                .zip(&smaller.runs)
                .map(|(run, other)| run.grands - other.grands),
        ),
        extra_depth: larger
            .runs
            .iter()
            .zip(&smaller.runs)
            .filter(|(run, other)| run.depth > other.depth)
            .count(),
        total: larger.runs.len(),
        rewards: std::array::from_fn(|key| {
            mean(larger.runs.iter().zip(&smaller.runs).map(|(run, other)| {
                let left = if run.settled { run.rewards[key] } else { 0.0 };
                let right = if other.settled {
                    other.rewards[key]
                } else {
                    0.0
                };
                left - right
            }))
        }),
    }
}

fn paired_metric(
    larger: &Summary,
    smaller: &Summary,
    metric_larger: &[f64],
    metric_smaller: &[f64],
) -> Paired {
    let differences: Vec<f64> = metric_larger
        .iter()
        .zip(metric_smaller)
        .map(|(a, b)| a - b)
        .collect();
    let delta = mean(differences.iter().copied());
    let n = differences.len();
    let interval = if n > 1 {
        let se = (differences
            .iter()
            .map(|value| (value - delta).powi(2))
            .sum::<f64>()
            / (n - 1) as f64
            / n as f64)
            .sqrt();
        Some([delta - 1.96 * se, delta + 1.96 * se])
    } else {
        None
    };
    let mut paired = paired_party(larger, smaller);
    paired.objective = delta;
    paired.interval = interval;
    paired
}

/// The party-size comparison behind one finalist.
#[derive(Clone, Debug, PartialEq)]
pub struct PartyAnalysis {
    pub objective: Objective,
    pub retention: f64,
    pub recommended_size: Option<usize>,
    pub trials: usize,
    pub rows: Vec<PartyRow>,
}

/// One measured party prefix.
#[derive(Clone, Debug, PartialEq)]
pub struct PartyRow {
    pub config: Config,
    pub score: f64,
    pub depth: f64,
    pub amber: f64,
    pub settled: usize,
    pub diagnostics: Option<DiagnosticsSummary>,
    pub allowed: bool,
    pub marginal: Option<Paired>,
}

fn party_summary(
    candidates: &[Summary],
    objective: Objective,
    input: &Inputs,
    allowed: &dyn Fn(&Config) -> bool,
) -> PartyAnalysis {
    let eligible: Vec<&Summary> = candidates
        .iter()
        .filter(|row| allowed(&row.config))
        .collect();
    let value = |row: &Summary| {
        if objective == Objective::Depth {
            row.score - row.start_depth
        } else {
            row.score
        }
    };
    let best = eligible
        .iter()
        .map(|row| value(row))
        .fold(f64::NEG_INFINITY, f64::max);
    let recommended = eligible
        .iter()
        .find(|row| value(row) >= best - 0.01 * best.abs())
        .map(|row| row.config.party.len());
    let metrics: Vec<Vec<f64>> = candidates
        .iter()
        .map(|row| {
            row.runs
                .iter()
                .map(|run| run_metric(run, input, objective))
                .collect()
        })
        .collect();
    PartyAnalysis {
        objective,
        retention: 0.99,
        recommended_size: recommended,
        trials: candidates.first().map_or(0, |row| row.total),
        rows: candidates
            .iter()
            .enumerate()
            .map(|(index, row)| PartyRow {
                config: row.config.clone(),
                score: row.score,
                depth: row.depth,
                amber: row.amber,
                settled: row.settled,
                diagnostics: row.diagnostics.clone(),
                allowed: allowed(&row.config),
                marginal: if index == 0 {
                    None
                } else {
                    Some(paired_metric(
                        row,
                        &candidates[index - 1],
                        &metrics[index],
                        &metrics[index - 1],
                    ))
                },
            })
            .collect(),
    }
}

/// One row of the optimizer result.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub label: String,
    pub reference_only: bool,
    pub summary: Summary,
    pub party_analysis: Option<PartyAnalysis>,
    pub risk_eligible: bool,
    pub matching_strategies: usize,
}

/// The upstream search details the result explains.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AmberSearchInfo {
    pub from: usize,
    pub to: usize,
    pub effective_from: usize,
    pub depths: Vec<usize>,
    pub min_rope_pct: usize,
    pub screening: usize,
    pub trials: usize,
    pub initial_trials: usize,
    pub max_trials: usize,
    pub coarse: usize,
    pub refined: usize,
    pub racing_trials: usize,
    pub adaptive_rounds: usize,
    pub observed_behaviors: usize,
    pub finalist_strategies: usize,
    pub unresolved: bool,
    pub risk_uncertain: bool,
}

/// What the optimizer found.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub kind: String,
    pub objective: Objective,
    /// The seed every finalist trial used.
    pub seed: u32,
    pub rows: Vec<Row>,
    pub winner: Option<Row>,
    pub baseline: Row,
    pub screened: u128,
    pub configurations: u128,
    pub exhaustive: bool,
    pub search_scope: Option<SearchScope>,
    pub delta: Option<f64>,
    pub interval: Option<[f64; 2]>,
    pub claim: String,
    pub scope: String,
    pub amber_search: Option<AmberSearchInfo>,
}

/// The upstream `createSpelunkSearch` options.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchOptions {
    pub objective: Objective,
    pub screening: usize,
    pub trials: usize,
    pub seed: u32,
    pub scope: ScopeRequest,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            objective: Objective::Depth,
            screening: 2,
            trials: 12,
            seed: 1,
            scope: ScopeRequest::default(),
        }
    }
}

/// The upstream `createSpelunkAmberSearch` options.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AmberSearchOptions {
    pub objective: Objective,
    pub screening: usize,
    pub trials: usize,
    pub max_trials: usize,
    pub min_rope_pct: usize,
    pub from: usize,
    pub to: Option<usize>,
    pub seed: u32,
}

impl Default for AmberSearchOptions {
    fn default() -> Self {
        Self {
            objective: Objective::Amber,
            screening: 2,
            trials: 12,
            max_trials: 128,
            min_rope_pct: 0,
            from: 1,
            to: None,
            seed: 1,
        }
    }
}

fn config_key(config: &Config) -> String {
    format!(
        "{:?}|{:?}|{}|{}|{}|{:?}|{:?}|{:?}|{}|{}|{:?}|{:?}|{:?}",
        config.cave,
        config.party,
        config.manic,
        config.policy.id(),
        config.speed,
        config.elixirs,
        config.memorized,
        config.tool,
        config.entry.id(),
        config.full_clear,
        config.amber,
        config.reaction_ms,
        config.stamina
    )
}

fn measure(
    input: &Inputs,
    config: &Config,
    seed: u32,
    fraction: f32,
    phase: &str,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Result<Option<RunResult>, ConfigError> {
    simulate(input, config.clone(), RunOptions { seed }, &mut |_| {
        report(Progress {
            fraction,
            message: phase.to_string(),
        })
    })
}

/// The upstream `createSpelunkSearch`: screen every allowed configuration and
/// compare the finalists on fresh seeds.
pub fn search(
    input: &Inputs,
    requested: &Requested,
    options: &SearchOptions,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Result<Option<SearchResult>, ConfigError> {
    let base = normalize(input, requested)?;
    let screening = count(Some(options.screening), 2, 1, 8, "screening trials")?;
    let final_trials = count(Some(options.trials), 12, 2, 64, "final trials")?;
    let scope = normalize_scope(input, &base, &options.scope)?;
    let occupancy = belt_occupancy(input, &scope);
    let allowed = |config: &Config| {
        in_scope(config, &scope, occupancy, &base)
            && config.stamina == base.stamina
            && config.reaction_ms == base.reaction_ms
    };
    let configuration_count = configuration_count(input, &base, &scope);
    // The captured baseline is always measured; when the scope cannot enumerate
    // it, it is the one reference outside the exhaustive count.
    let total = configuration_count + u128::from(!allowed(&base));
    let mut top: Vec<Summary> = Vec::new();
    let mut screened = 1u128;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    seen.insert(config_key(&base));
    let fraction =
        |screened: u128, offset: f64| 0.75 * (screened as f64 + offset) / total.max(1) as f64;
    let mut fatal: Option<ConfigError> = None;
    let mut stopped = false;
    enumerate_configurations(input, &base, &scope, &mut |config| {
        let key = config_key(&config);
        if !seen.insert(key) {
            return Control::Continue;
        }
        let mut runs = Vec::new();
        for trial in 0..screening {
            match measure(
                input,
                &config,
                seed_of(options.seed, trial, false),
                fraction(screened, trial as f64 / screening as f64) as f32,
                &format!("Screening configuration {} of {}", screened + 1, total),
                report,
            ) {
                Ok(Some(run)) => runs.push(run),
                Ok(None) => {
                    stopped = true;
                    return Control::Stop;
                }
                Err(error) => {
                    fatal = Some(error);
                    return Control::Stop;
                }
            }
        }
        let summary = summarize(config, runs, input, options.objective);
        top.push(summary);
        top.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        top.truncate(5);
        screened += 1;
        Control::Continue
    });
    if let Some(error) = fatal {
        return Err(error);
    }
    if stopped {
        return Ok(None);
    }
    if screened != total {
        return Err(ConfigError(
            "Expedition configuration count did not match exhaustive screening.".into(),
        ));
    }
    let mut finalists: Vec<Summary> = vec![summarize(
        base.clone(),
        Vec::new(),
        input,
        options.objective,
    )];
    for entry in &top {
        if !finalists
            .iter()
            .any(|finalist| config_key(&finalist.config) == config_key(&entry.config))
        {
            finalists.push(entry.clone());
        }
    }
    let final_count = finalists.len();
    for (index, entry) in finalists.iter_mut().enumerate() {
        entry.runs.clear();
        for trial in 0..final_trials {
            let Some(run) = measure(
                input,
                &entry.config,
                seed_of(options.seed, trial, true),
                (0.75
                    + 0.15 * (trial * final_count + index) as f64
                        / (final_trials * final_count) as f64) as f32,
                "Fresh-seed finalist comparison",
                report,
            )?
            else {
                return Ok(None);
            };
            entry.runs.push(run);
        }
        *entry = summarize(
            entry.config.clone(),
            std::mem::take(&mut entry.runs),
            input,
            options.objective,
        );
    }
    let rows: Vec<Row> = finalists
        .iter()
        .map(|entry| Row {
            label: if config_key(&entry.config) == config_key(&base) {
                "Selected baseline".into()
            } else {
                "Screened strategy".into()
            },
            reference_only: !allowed(&entry.config),
            summary: entry.clone(),
            party_analysis: None,
            risk_eligible: true,
            matching_strategies: 0,
        })
        .collect();
    let families = match measure_party_families(input, &rows, options.objective, report)? {
        Some(families) => families,
        None => return Ok(None),
    };
    let mut rows = rows;
    for (row, family) in rows.iter_mut().zip(&families) {
        row.party_analysis = Some(party_summary(family, options.objective, input, &allowed));
    }
    let mut measured: BTreeSet<String> = rows
        .iter()
        .map(|row| config_key(&row.summary.config))
        .collect();
    for family in &families {
        for (index, candidate) in family.iter().enumerate() {
            let key = config_key(&candidate.config);
            if !allowed(&candidate.config) || !measured.insert(key) {
                continue;
            }
            rows.push(Row {
                label: "Measured shorter party".into(),
                reference_only: false,
                summary: candidate.clone(),
                party_analysis: Some(party_summary(
                    &family[..=index],
                    options.objective,
                    input,
                    &allowed,
                )),
                risk_eligible: true,
                matching_strategies: 0,
            });
        }
    }
    let baseline = rows[0].clone();
    let winner = rows
        .iter()
        .filter(|row| !row.reference_only)
        .max_by(|a, b| {
            a.summary
                .score
                .partial_cmp(&b.summary.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();
    let (delta, interval) = match &winner {
        Some(winner) => paired_change(winner, &baseline, input, options.objective),
        None => (None, None),
    };
    Ok(Some(SearchResult {
        kind: "search".into(),
        objective: options.objective,
        seed: options.seed,
        rows,
        winner,
        baseline,
        screened,
        configurations: configuration_count,
        exhaustive: true,
        search_scope: Some(scope),
        delta,
        interval,
        claim: "Every allowed configuration screened under the experimental model; fresh-seed finalists are sampled, not a proof of the stochastic optimum.".into(),
        scope: SEARCH_SCOPE_TEXT.into(),
        amber_search: None,
    }))
}

const SEARCH_SCOPE_TEXT: &str = "Eligible parties from the selected character pool within the selected size range are screened. With Order doesn't matter, each group is tested once in character-slot order; expedition outcomes can still depend on that order. Otherwise every party permutation is screened. Permitted policies/clear choices/Auto speeds, every maximally filled physical belt arrangement (including unlocked duplicates), unique memorized subsets, manual tools and both entry choices are screened. Empty belt slots are allowed only when the selected pool and duplicate limit cannot fill them. Cave, Normal/Manic mode, starting-stamina mode, reaction time and account upgrades stay fixed. Auto tool choice is automatic; unused manual Auto settings do not multiply the search. References outside the selected scope, including underfilled belts and unsearched party orders, cannot win. The strongest five screening setups and reference configurations receive fresh finalist trials, followed by paired shorter-prefix measurements. Eligible measured prefixes also compete on the original objective; out-of-scope prefixes are diagnostic only. Separate party efficiency advice retains 99% of the best observed prefix objective (gained floors for Depth), not proven equivalence, and never lowers the highest-objective winner. Intervals are descriptive, not selection-adjusted guarantees.";

fn paired_change(
    winner: &Row,
    baseline: &Row,
    input: &Inputs,
    objective: Objective,
) -> (Option<f64>, Option<[f64; 2]>) {
    let n = winner.summary.runs.len().min(baseline.summary.runs.len());
    if n == 0 {
        return (None, None);
    }
    let differences: Vec<f64> = (0..n)
        .map(|index| {
            run_metric(&winner.summary.runs[index], input, objective)
                - run_metric(&baseline.summary.runs[index], input, objective)
        })
        .collect();
    let delta = mean(differences.iter().copied());
    let interval = if n > 1 {
        let se = (differences
            .iter()
            .map(|value| (value - delta).powi(2))
            .sum::<f64>()
            / (n - 1) as f64
            / n as f64)
            .sqrt();
        Some([delta - 1.96 * se, delta + 1.96 * se])
    } else {
        None
    };
    (Some(delta), interval)
}

fn measure_party_families(
    input: &Inputs,
    rows: &[Row],
    objective: Objective,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Result<Option<Vec<Vec<Summary>>>, ConfigError> {
    if rows.is_empty() || rows.iter().any(|row| row.summary.runs.is_empty()) {
        return Err(ConfigError(
            "Party analysis requires completed expedition trials.".into(),
        ));
    }
    struct Pending {
        config: Config,
        seeds: Vec<u32>,
        runs: Vec<RunResult>,
    }
    let key = |config: &Config, seeds: &[u32]| format!("{}|{:?}", config_key(config), seeds);
    let mut measured: BTreeMap<String, Summary> = BTreeMap::new();
    let mut pending: BTreeMap<String, Pending> = BTreeMap::new();
    let mut families: Vec<Vec<String>> = Vec::new();
    for row in rows {
        let seeds: Vec<u32> = row.summary.runs.iter().map(|run| run.seed).collect();
        let unique: BTreeSet<u32> = seeds.iter().copied().collect();
        if unique.len() != seeds.len() {
            return Err(ConfigError(
                "Party analysis requires distinct paired trial seeds.".into(),
            ));
        }
        measured.insert(
            key(&row.summary.config, &seeds),
            summarize(
                row.summary.config.clone(),
                row.summary.runs.clone(),
                input,
                objective,
            ),
        );
    }
    for row in rows {
        let seeds: Vec<u32> = row.summary.runs.iter().map(|run| run.seed).collect();
        let mut family = Vec::new();
        for size in 1..=row.summary.config.party.len() {
            let prefix = Config {
                party: row.summary.config.party[..size].to_vec(),
                ..row.summary.config.clone()
            };
            let id = key(&prefix, &seeds);
            family.push(id.clone());
            if !measured.contains_key(&id) && !pending.contains_key(&id) {
                pending.insert(
                    id,
                    Pending {
                        config: prefix,
                        seeds: seeds.clone(),
                        runs: Vec::new(),
                    },
                );
            }
        }
        families.push(family);
    }
    let requests: Vec<(String, u32)> = pending
        .iter()
        .flat_map(|(id, entry)| entry.seeds.iter().map(move |seed| (id.clone(), *seed)))
        .collect();
    for (index, (id, run_seed)) in requests.iter().enumerate() {
        let fraction = 0.9 + 0.1 * index as f64 / requests.len().max(1) as f64;
        let config = pending[id].config.clone();
        let Some(run) = measure(
            input,
            &config,
            *run_seed,
            fraction as f32,
            "Measuring shorter parties on paired seeds",
            report,
        )?
        else {
            return Ok(None);
        };
        pending
            .get_mut(id)
            .expect("request owns a pending entry")
            .runs
            .push(run);
    }
    for (id, entry) in pending {
        measured.insert(id, summarize(entry.config, entry.runs, input, objective));
    }
    Ok(Some(
        families
            .into_iter()
            .map(|family| family.into_iter().map(|id| measured[&id].clone()).collect())
            .collect(),
    ))
}

fn enumerate_configurations(
    input: &Inputs,
    base: &Config,
    scope: &SearchScope,
    visit: &mut dyn FnMut(Config) -> Control,
) {
    let ids = eligible(input, base, &scope.characters, scope.ignore_order);
    let mut memory_ids = scope.memorized.clone();
    memory_ids.sort_unstable();
    let belts: Vec<Vec<i32>> = if scope.enabled("elixirs") {
        let mut belts = Vec::new();
        belts_into(
            &scope.elixirs,
            input.slots,
            input.dupes,
            belt_occupancy(input, scope),
            &mut Vec::new(),
            &mut BTreeMap::new(),
            &mut belts,
        );
        belts
    } else {
        vec![base.elixirs.clone()]
    };
    let memories: Vec<Vec<usize>> = if scope.enabled("memorized") {
        let mut memories = Vec::new();
        memories_into(
            &memory_ids,
            input.memory_slots,
            0,
            &mut Vec::new(),
            &mut memories,
        );
        memories
    } else {
        vec![base.memorized.clone()]
    };
    'configs: {
        for policy in policies(scope, base) {
            let speeds: Vec<u32> = if policy == Policy::Auto {
                scope.speeds.clone()
            } else {
                vec![base.speed]
            };
            let clears: Vec<bool> = if policy == Policy::Auto {
                auto_clears(scope)
                    .into_iter()
                    .map(|clear| clear == Clear::Full)
                    .collect()
            } else {
                vec![base.full_clear]
            };
            for speed in speeds {
                for full_clear in &clears {
                    let tools: Vec<ToolChoice> =
                        if policy == Policy::Auto || !scope.enabled("tools") {
                            vec![base.tool]
                        } else {
                            std::iter::once(ToolChoice::Adaptive)
                                .chain(input.tools.iter().map(|tool| ToolChoice::Tool(tool.id)))
                                .collect()
                        };
                    for tool in tools {
                        let entries: Vec<super::Entry> = if scope.enabled("entry") {
                            vec![super::Entry::Surface, super::Entry::Shortcut]
                        } else {
                            vec![base.entry]
                        };
                        for entry in &entries {
                            let mut emit = |party: &[usize]| -> Control {
                                for elixirs in &belts {
                                    for memorized in &memories {
                                        let config = Config {
                                            policy,
                                            speed,
                                            full_clear: *full_clear,
                                            tool,
                                            entry: *entry,
                                            party: party.to_vec(),
                                            elixirs: elixirs.clone(),
                                            memorized: memorized.clone(),
                                            amber: (policy == Policy::AmberFarm)
                                                .then(|| base.amber.unwrap_or_default()),
                                            ..base.clone()
                                        };
                                        if visit(config) == Control::Stop {
                                            return Control::Stop;
                                        }
                                    }
                                }
                                Control::Continue
                            };
                            let result = if scope.enabled("party") {
                                walk_parties(
                                    &ids,
                                    scope.party_min,
                                    scope.party_max.min(ids.len()),
                                    scope.ignore_order,
                                    &mut Vec::new(),
                                    0,
                                    &mut emit,
                                )
                            } else {
                                emit(&base.party)
                            };
                            if result == Control::Stop {
                                break 'configs;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
fn parties(ids: &[usize], min: usize, max: usize, ignore_order: bool) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    walk_parties(
        ids,
        min,
        max,
        ignore_order,
        &mut Vec::new(),
        0,
        &mut |party| {
            out.push(party.to_vec());
            Control::Continue
        },
    );
    out
}

fn walk_parties(
    ids: &[usize],
    min: usize,
    max: usize,
    ignore_order: bool,
    party: &mut Vec<usize>,
    start: usize,
    visit: &mut dyn FnMut(&[usize]) -> Control,
) -> Control {
    if party.len() >= min && visit(party) == Control::Stop {
        return Control::Stop;
    }
    if party.len() == max {
        return Control::Continue;
    }
    for index in start..ids.len() {
        if party.contains(&ids[index]) {
            continue;
        }
        party.push(ids[index]);
        if walk_parties(
            ids,
            min,
            max,
            ignore_order,
            party,
            if ignore_order { index + 1 } else { 0 },
            visit,
        ) == Control::Stop
        {
            party.pop();
            return Control::Stop;
        }
        party.pop();
    }
    Control::Continue
}

fn belts_into(
    ids: &[usize],
    slots: usize,
    dupes: usize,
    remaining: usize,
    belt: &mut Vec<i32>,
    counts: &mut BTreeMap<i32, usize>,
    out: &mut Vec<Vec<i32>>,
) {
    if belt.len() == slots {
        if remaining == 0 {
            out.push(belt.clone());
        }
        return;
    }
    // An empty slot is legal only while the unfilled slots can still hold every
    // remaining elixir. Upstream's enumerator rejects an underfilled pool; this
    // keeps the exhaustive count and the enumeration in agreement.
    if slots - belt.len() > remaining {
        belt.push(-1);
        belts_into(ids, slots, dupes, remaining, belt, counts, out);
        belt.pop();
    }
    if remaining == 0 {
        return;
    }
    for id in ids {
        let used = counts.get(&(*id as i32)).copied().unwrap_or(0);
        if used >= dupes {
            continue;
        }
        belt.push(*id as i32);
        counts.insert(*id as i32, used + 1);
        belts_into(ids, slots, dupes, remaining - 1, belt, counts, out);
        belt.pop();
        counts.insert(*id as i32, used);
    }
}

fn memories_into(
    ids: &[usize],
    slots: usize,
    start: usize,
    memory: &mut Vec<usize>,
    out: &mut Vec<Vec<usize>>,
) {
    out.push(memory.clone());
    if memory.len() == slots {
        return;
    }
    for index in start..ids.len() {
        memory.push(ids[index]);
        memories_into(ids, slots, index + 1, memory, out);
        memory.pop();
    }
}

fn configuration_count(input: &Inputs, base: &Config, scope: &SearchScope) -> u128 {
    let n = eligible(input, base, &scope.characters, scope.ignore_order).len();
    let mut parties: u128 = 0;
    let mut ordered: u128 = 1;
    for k in 1..=scope.party_max.min(n) {
        ordered = ordered.saturating_mul((n - k + 1) as u128);
        if k >= scope.party_min {
            parties = parties.saturating_add(if scope.ignore_order {
                choose(n, k)
            } else {
                ordered
            });
        }
    }
    if !scope.enabled("party") {
        parties = 1;
    }
    let mut belts = vec![0u128; input.slots + 1];
    belts[0] = 1;
    for _ in 0..scope.elixirs.len() {
        let mut next = vec![0u128; input.slots + 1];
        for occupied in 0..=input.slots {
            for copies in 0..=input.dupes.min(input.slots - occupied) {
                next[occupied + copies] = next[occupied + copies].saturating_add(
                    belts[occupied].saturating_mul(choose(input.slots - occupied, copies)),
                );
            }
        }
        belts = next;
    }
    let mut memories: u128 = 0;
    for k in 0..=input.memory_slots.min(scope.memorized.len()) {
        memories = memories.saturating_add(choose(scope.memorized.len(), k));
    }
    let policy_count: u128 = policies(scope, base)
        .into_iter()
        .map(|policy| {
            if policy == Policy::Auto {
                scope.speeds.len() as u128 * auto_clears(scope).len() as u128
            } else if scope.enabled("tools") {
                input.tools.len() as u128 + 1
            } else {
                1
            }
        })
        .sum();
    parties
        .saturating_mul(if scope.enabled("elixirs") {
            belts[belt_occupancy(input, scope)]
        } else {
            1
        })
        .saturating_mul(if scope.enabled("memorized") {
            memories
        } else {
            1
        })
        .saturating_mul(policy_count)
        .saturating_mul(if scope.enabled("entry") { 2 } else { 1 })
}

fn choose(n: usize, k: usize) -> u128 {
    if k > n {
        return 0;
    }
    let mut value: u128 = 1;
    for index in 1..=k {
        value = value
            .saturating_mul((n - index + 1) as u128)
            .saturating_div(index as u128);
    }
    value
}

/// The configuration count the upstream page prints before a search.
pub fn search_space(
    input: &Inputs,
    requested: &Requested,
    scope: &ScopeRequest,
) -> Result<(u128, SearchScope), ConfigError> {
    let base = normalize(input, requested)?;
    let scope = normalize_scope(input, &base, scope)?;
    Ok((configuration_count(input, &base, &scope), scope))
}

/// The upstream `spelunkRunMetric`, public for callers that rank stored runs.
pub fn metric(run: &RunResult, input: &Inputs, objective: Objective) -> f64 {
    run_metric(run, input, objective)
}

/// The plan text the section copies: the configuration, its party, belt and
/// memory, and the Amber instructions when it farms.
pub fn plan_text(input: &Inputs, config: &Config) -> String {
    let cave = input
        .caves
        .get(config.cave)
        .map_or("Unknown cave", |cave| cave.name.as_str());
    let characters = |id: usize| {
        input.characters.get(id).map_or_else(
            || format!("Character {id}"),
            |character| character.name.clone(),
        )
    };
    let elixir = |id: i32| {
        if id < 0 {
            "empty".to_string()
        } else {
            input
                .elixirs
                .get(id as usize)
                .map_or_else(|| format!("Elixir {id}"), |elixir| elixir.name.clone())
        }
    };
    let mut text = format!(
        "{cave} / {} / {} / {} / {} / party {} / belt {} / memory {} / {} / entry {}",
        if config.manic { "Manic" } else { "Normal" },
        config.policy.label(),
        if config.policy == Policy::Auto {
            format!(
                "speed {} / {}",
                config.speed,
                if config.full_clear {
                    "full clear"
                } else {
                    "first exit"
                }
            )
        } else {
            format!("{} ms", config.reaction_ms)
        },
        if config.stamina == super::StaminaMode::Full {
            "fully recovered stamina"
        } else {
            "saved stamina"
        },
        config
            .party
            .iter()
            .map(|id| characters(*id))
            .collect::<Vec<_>>()
            .join(" -> "),
        config
            .elixirs
            .iter()
            .map(|id| elixir(*id))
            .collect::<Vec<_>>()
            .join(", "),
        if config.memorized.is_empty() {
            "none".to_string()
        } else {
            config
                .memorized
                .iter()
                .map(|id| elixir(*id as i32))
                .collect::<Vec<_>>()
                .join(", ")
        },
        match config.tool {
            ToolChoice::Adaptive => "adaptive tools".to_string(),
            ToolChoice::Tool(id) => input
                .tools
                .iter()
                .find(|tool| tool.id == id)
                .map_or_else(|| format!("tool {id}"), |tool| tool.name.clone()),
        },
        config.entry.label(),
    );
    if let Some(amber) = config.amber {
        text.push_str(&format!(
            "\nPlanned: look for stamina burn from D{}; preserve the last {} character(s), first at {}% stamina. {} Seek a rope sooner below {}% finishing stamina.",
            amber.dump_depth,
            (amber.finishers as usize).min(config.party.len()),
            amber.reserve_pct,
            if amber.harvest_depths == 0 {
                "After the oranges, seek the next visible rope immediately.".to_string()
            } else {
                format!(
                    "After the oranges, target +{} floors before seeking a rope.",
                    amber.harvest_depths
                )
            },
            amber.rope_reserve_pct
        ));
    }
    text
}

/// A text plan for the recommendation, or a note that the search found none.
pub fn result_text(input: &Inputs, result: &SearchResult) -> String {
    match &result.winner {
        Some(winner) => format!(
            "Best measured {}: {:.4} (baseline {:.4}, change {:+.4}).\n{}",
            result.objective.label(),
            winner.summary.score,
            result.baseline.summary.score,
            result.delta.unwrap_or(0.0),
            plan_text(input, &winner.summary.config)
        ),
        None => "No recommendation is available.".to_string(),
    }
}

struct Measured {
    config: Config,
    runs: Vec<RunResult>,
}

fn amber_config(config: &Config, amber: super::AmberPolicy) -> Config {
    Config {
        policy: Policy::AmberFarm,
        full_clear: true,
        amber: Some(amber),
        ..config.clone()
    }
}

struct MeasuredCache<'a> {
    input: &'a Inputs,
    seed: u32,
    objective: Objective,
    entries: BTreeMap<String, Measured>,
}

fn measure_configs(
    state: &mut MeasuredCache<'_>,
    configs: &[Config],
    count: usize,
    from: f64,
    to: f64,
    phase: &str,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Result<Option<Vec<Summary>>, ConfigError> {
    let mut requests: Vec<(String, u32)> = Vec::new();
    for config in configs {
        let key = config_key(config);
        let entry = state
            .entries
            .entry(key.clone())
            .or_insert_with(|| Measured {
                config: config.clone(),
                runs: Vec::new(),
            });
        for trial in entry.runs.len()..count {
            requests.push((key.clone(), seed_of(state.seed, trial, false)));
        }
    }
    for (index, (key, run_seed)) in requests.iter().enumerate() {
        let fraction = from + (to - from) * index as f64 / requests.len().max(1) as f64;
        let config = state.entries[key].config.clone();
        let Some(run) = measure(
            state.input,
            &config,
            *run_seed,
            fraction as f32,
            phase,
            report,
        )?
        else {
            return Ok(None);
        };
        state
            .entries
            .get_mut(key)
            .expect("request owns a cache entry")
            .runs
            .push(run);
    }
    Ok(Some(
        configs
            .iter()
            .map(|config| {
                let entry = &state.entries[&config_key(config)];
                summarize(
                    config.clone(),
                    entry.runs.clone(),
                    state.input,
                    state.objective,
                )
            })
            .collect(),
    ))
}

fn outcome_key(summary: &Summary, count: usize) -> String {
    summary
        .runs
        .iter()
        .take(count)
        .map(|run| {
            format!(
                "{}|{}|{}|{}|{:?}|{:?}|{:?}|{:?}",
                run.status.id(),
                run.depth,
                run.seconds,
                run.spent,
                run.rewards,
                run.stamina
                    .iter()
                    .map(|member| (member.id, member.end))
                    .collect::<Vec<_>>(),
                run.amber,
                run.exit.as_ref().map(|exit| exit.kind.id())
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn cx_score(a: &Summary, b: &Summary) -> std::cmp::Ordering {
    b.score
        .partial_cmp(&a.score)
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn amber_leaders(rows: &[Summary], limit: usize) -> Vec<Summary> {
    if rows.is_empty() {
        return Vec::new();
    }
    let mut ranked: Vec<Summary> = rows.to_vec();
    ranked.sort_by(cx_score);
    let count = rows.iter().map(|row| row.runs.len()).min().unwrap_or(0);
    let keys: BTreeMap<String, String> = rows
        .iter()
        .map(|row| (config_key(&row.config), outcome_key(row, count)))
        .collect();
    let mut selected: Vec<Summary> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let take = |ordered: &[Summary],
                target: usize,
                selected: &mut Vec<Summary>,
                seen: &mut BTreeSet<String>| {
        for row in ordered {
            if selected.len() >= target {
                break;
            }
            let key = keys[&config_key(&row.config)].clone();
            if !seen.insert(key) {
                continue;
            }
            selected.push(row.clone());
        }
    };
    take(&ranked, 3.min(limit), &mut selected, &mut seen);
    let mut by_rope = ranked.clone();
    by_rope.sort_by(|a, b| {
        (b.rope as f64 / b.total.max(1) as f64)
            .partial_cmp(&(a.rope as f64 / a.total.max(1) as f64))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| cx_score(a, b))
    });
    take(&by_rope, 5.min(limit), &mut selected, &mut seen);
    let depth_span = 1.0_f64.max(
        rows.iter()
            .map(|row| row.config.amber.map_or(0, |amber| amber.dump_depth) as f64)
            .fold(f64::NEG_INFINITY, f64::max)
            - rows
                .iter()
                .map(|row| row.config.amber.map_or(0, |amber| amber.dump_depth) as f64)
                .fold(f64::INFINITY, f64::min),
    );
    let distance = |a: &Summary, b: &Summary| -> f64 {
        let x = a.config.amber.unwrap_or_default();
        let y = b.config.amber.unwrap_or_default();
        (x.dump_depth as f64 - y.dump_depth as f64).abs() / depth_span
            + (x.finishers as f64 - y.finishers as f64).abs()
            + (x.reserve_pct as f64 - y.reserve_pct as f64).abs() / 75.0
            + (x.harvest_depths as f64 - y.harvest_depths as f64).abs() / 16.0
            + (x.rope_reserve_pct as f64 - y.rope_reserve_pct as f64).abs() / 75.0
    };
    let spread = |pool: &[Summary], selected: &[Summary]| -> Option<Summary> {
        pool.iter()
            .map(|row| {
                let gap = selected
                    .iter()
                    .map(|picked| distance(row, picked))
                    .fold(f64::INFINITY, f64::min);
                (row, gap)
            })
            .fold(
                None,
                |best: Option<(&Summary, f64)>, (row, gap)| match best {
                    Some((_, best_gap)) if gap <= best_gap => best,
                    _ => Some((row, gap)),
                },
            )
            .map(|(row, _)| row.clone())
    };
    while selected.len() < limit {
        let pool: Vec<Summary> = ranked
            .iter()
            .filter(|row| !seen.contains(&keys[&config_key(&row.config)]))
            .cloned()
            .collect();
        let Some(row) = spread(&pool, &selected) else {
            break;
        };
        seen.insert(keys[&config_key(&row.config)].clone());
        selected.push(row);
    }
    let target = limit.min(selected.len() + 2);
    while selected.len() < target {
        let pool: Vec<Summary> = ranked
            .iter()
            .filter(|row| {
                !selected
                    .iter()
                    .any(|picked| config_key(&picked.config) == config_key(&row.config))
            })
            .cloned()
            .collect();
        let Some(row) = spread(&pool, &selected) else {
            break;
        };
        selected.push(row);
    }
    selected
}

fn amber_difference(
    a: &Summary,
    b: &Summary,
    input: &Inputs,
    objective: Objective,
) -> (Option<f64>, Option<[f64; 2]>) {
    let differences: Vec<f64> = a
        .runs
        .iter()
        .zip(&b.runs)
        .map(|(run, other)| run_metric(run, input, objective) - run_metric(other, input, objective))
        .collect();
    if differences.is_empty() {
        return (None, None);
    }
    let delta = mean(differences.iter().copied());
    let interval = if differences.len() > 1 {
        let se = (differences
            .iter()
            .map(|value| (value - delta).powi(2))
            .sum::<f64>()
            / (differences.len() - 1) as f64
            / differences.len() as f64)
            .sqrt();
        Some([delta - 1.96 * se, delta + 1.96 * se])
    } else {
        None
    };
    (Some(delta), interval)
}

fn amber_rope_uncertain(row: &Summary, minimum: usize) -> bool {
    if minimum == 0 {
        return false;
    }
    let n = row.total as f64;
    if n == 0.0 {
        return true;
    }
    let p = row.rope as f64 / n;
    let z2 = 1.96_f64.powi(2);
    let center = (p + z2 / (2.0 * n)) / (1.0 + z2 / n);
    let radius = 1.96 * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / (1.0 + z2 / n);
    let target = minimum as f64 / 100.0;
    center - radius <= target && center + radius >= target
}

/// The upstream `createSpelunkAmberSearch`: a bounded diverse phase search with
/// two local-refinement rounds and a fresh-seed adaptive comparison.
pub fn amber_search(
    input: &Inputs,
    requested: &Requested,
    options: &AmberSearchOptions,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Result<Option<SearchResult>, ConfigError> {
    let base = normalize(input, requested)?;
    if !base.elixirs.contains(&3) {
        return Err(ConfigError(
            "Amber farming needs at least one equipped orange elixir. Apply the Amber belt preset or choose a belt.".into(),
        ));
    }
    if !options.objective.amber() {
        return Err(ConfigError(
            "Choose a collected-Amber objective for Amber farming.".into(),
        ));
    }
    let objective = options.objective;
    let screening = count(Some(options.screening), 2, 1, 8, "screening trials")?;
    let trials = count(Some(options.trials), 12, 2, 64, "final trials")?;
    let max_trials = count(
        Some(options.max_trials),
        trials.max(128),
        trials,
        256,
        "adaptive Amber trial cap",
    )?;
    let min_rope_pct = count(
        Some(options.min_rope_pct),
        0,
        0,
        100,
        "minimum sampled rope exit percentage",
    )?;
    let from = count(Some(options.from), 1, 1, 10_000, "first Amber dump depth")?;
    let requested_to = match options.to {
        Some(to) => Some(count(Some(to), 100, from, 10_000, "last Amber dump depth")?),
        None => None,
    };
    let seed = options.seed;
    let mut state = MeasuredCache {
        input,
        seed,
        objective,
        entries: BTreeMap::new(),
    };
    let baseline_screen = match measure_configs(
        &mut state,
        std::slice::from_ref(&base),
        screening,
        0.0,
        0.03,
        "Measuring current configuration",
        report,
    )? {
        Some(rows) => rows,
        None => return Ok(None),
    };
    let baseline_runs = &baseline_screen[0].runs;
    let to = requested_to.unwrap_or_else(|| {
        from.max(
            baseline_runs
                .iter()
                .map(|run| run.depth)
                .max()
                .unwrap_or(from)
                .min(10_000),
        )
    });
    let effective_from = to.min(
        from.max(
            baseline_runs
                .iter()
                .map(|run| run.start_depth)
                .min()
                .unwrap_or(from),
        ),
    );
    let depths: Vec<usize> = (0..5)
        .map(|index| {
            let value =
                effective_from as f64 + (to as f64 - effective_from as f64) * index as f64 / 4.0;
            value.round() as usize
        })
        .collect();
    let mut configs: Vec<Config> = Vec::new();
    let mut config_keys: BTreeSet<String> = BTreeSet::new();
    let mut add = |configs: &mut Vec<Config>, config: Config| -> Option<Config> {
        let key = config_key(&config);
        if !config_keys.insert(key) {
            return None;
        }
        configs.push(config.clone());
        Some(config)
    };
    for dump_depth in &depths {
        for finishers in [1u32, 2]
            .into_iter()
            .filter(|n| *n as usize <= base.party.len())
        {
            for reserve_pct in [25u32, 50, 75, 100] {
                for harvest_depths in [0u32, 3, 8] {
                    add(
                        &mut configs,
                        amber_config(
                            &base,
                            super::AmberPolicy {
                                dump_depth: *dump_depth as u32,
                                finishers,
                                reserve_pct,
                                harvest_depths,
                                rope_reserve_pct: 25,
                            },
                        ),
                    );
                }
            }
        }
    }
    for dump_depth in &depths {
        for finishers in [1u32, 2]
            .into_iter()
            .filter(|n| *n as usize <= base.party.len())
        {
            for (reserve_pct, harvest_depths, rope_reserve_pct) in [
                (25u32, 0u32, 5u32),
                (50, 1, 10),
                (75, 3, 50),
                (100, 8, 75),
                (50, 16, 50),
            ] {
                add(
                    &mut configs,
                    amber_config(
                        &base,
                        super::AmberPolicy {
                            dump_depth: *dump_depth as u32,
                            finishers,
                            reserve_pct,
                            harvest_depths,
                            rope_reserve_pct,
                        },
                    ),
                );
            }
        }
    }
    if base.policy == Policy::AmberFarm {
        add(&mut configs, base.clone());
    }
    let coarse_count = configs.len();
    let coarse = match measure_configs(
        &mut state,
        &configs,
        screening,
        0.03,
        0.3,
        "Screening diverse Amber phase strategies",
        report,
    )? {
        Some(rows) => rows,
        None => return Ok(None),
    };
    let racing_trials = 8.max(screening);
    let leaders_configs: Vec<Config> = amber_leaders(&coarse, 12)
        .into_iter()
        .map(|row| row.config)
        .collect();
    let mut leaders = match measure_configs(
        &mut state,
        &leaders_configs,
        racing_trials,
        0.3,
        0.4,
        "Checking distinct Amber behaviors on more shared seeds",
        report,
    )? {
        Some(rows) => rows,
        None => return Ok(None),
    };
    for round in 0..2 {
        let mut neighbors: Vec<Config> = Vec::new();
        let step = if round == 0 {
            1.max(((to - effective_from) as f64 / 8.0).round() as usize)
        } else {
            1
        };
        for row in amber_leaders(&leaders, 6) {
            let amber = row.config.amber.unwrap_or_default();
            let mut proposed: Vec<super::AmberPolicy> = Vec::new();
            let mut dump_depths: Vec<usize> = vec![
                (amber.dump_depth as usize).saturating_sub(step),
                amber.dump_depth as usize + step,
            ];
            for run in &row.runs {
                if let Some(result) = &run.amber
                    && let Some(first) = result.activations.first()
                {
                    dump_depths.push(first.depth.saturating_sub(1));
                    dump_depths.push(first.depth);
                    dump_depths.push(first.depth + 1);
                }
            }
            for dump_depth in dump_depths {
                proposed.push(super::AmberPolicy {
                    dump_depth: dump_depth.clamp(effective_from, to) as u32,
                    ..amber
                });
            }
            for sign in [-1i64, 1] {
                let shift = |value: u32, delta: i64, lo: u32, hi: u32| -> u32 {
                    (value as i64 + sign * delta).clamp(lo as i64, hi as i64) as u32
                };
                proposed.push(super::AmberPolicy {
                    reserve_pct: shift(
                        amber.reserve_pct,
                        if round == 0 { 25 } else { 10 },
                        25,
                        100,
                    ),
                    ..amber
                });
                proposed.push(super::AmberPolicy {
                    harvest_depths: shift(
                        amber.harvest_depths,
                        if round == 0 { 3 } else { 1 },
                        0,
                        100,
                    ),
                    ..amber
                });
                proposed.push(super::AmberPolicy {
                    rope_reserve_pct: shift(
                        amber.rope_reserve_pct,
                        if round == 0 { 15 } else { 5 },
                        1,
                        100,
                    ),
                    ..amber
                });
                proposed.push(super::AmberPolicy {
                    reserve_pct: shift(amber.reserve_pct, 25, 25, 100),
                    rope_reserve_pct: (amber.rope_reserve_pct as i64 - sign * 15).clamp(1, 100)
                        as u32,
                    ..amber
                });
            }
            if base.party.len() > 1 {
                proposed.push(super::AmberPolicy {
                    finishers: 3 - amber.finishers,
                    ..amber
                });
            }
            for amber in proposed {
                if let Some(config) = add(&mut configs, amber_config(&base, amber)) {
                    neighbors.push(config);
                }
            }
        }
        let start = 0.4 + round as f64 * 0.18;
        let screened = match measure_configs(
            &mut state,
            &neighbors,
            screening,
            start,
            start + 0.1,
            &format!("Refining Amber depths and reserves ({}/2)", round + 1),
            report,
        )? {
            Some(rows) => rows,
            None => return Ok(None),
        };
        let raced_configs: Vec<Config> = amber_leaders(&screened, 12)
            .into_iter()
            .map(|row| row.config)
            .collect();
        let raced = match measure_configs(
            &mut state,
            &raced_configs,
            racing_trials,
            start + 0.1,
            start + 0.18,
            &format!("Remeasuring refined Amber strategies ({}/2)", round + 1),
            report,
        )? {
            Some(rows) => rows,
            None => return Ok(None),
        };
        leaders.extend(raced);
        leaders = amber_leaders(&leaders, 12);
    }
    let mut finalists: Vec<Config> = vec![base.clone()];
    for row in amber_leaders(&leaders, 8) {
        if !finalists
            .iter()
            .any(|config| config_key(config) == config_key(&row.config))
        {
            finalists.push(row.config);
        }
    }
    let finalist_strategies = finalists.len();
    let mut final_runs: Vec<Vec<RunResult>> = vec![Vec::new(); finalists.len()];
    let mut count = 0usize;
    let mut target = trials;
    let mut adaptive_rounds = 0usize;
    let (rows, unresolved, risk_uncertain) = loop {
        for trial in count..target {
            for (index, config) in finalists.iter().enumerate() {
                let fraction = 0.76
                    + 0.23 * (trial as f64 + index as f64 / finalists.len() as f64)
                        / max_trials as f64;
                let Some(run) = measure(
                    input,
                    config,
                    seed_of(seed, trial, true),
                    fraction as f32,
                    &format!("Fresh-seed Amber and rope-risk comparison ({target} trials)"),
                    report,
                )?
                else {
                    return Ok(None);
                };
                final_runs[index].push(run);
            }
        }
        count = target;
        let rows: Vec<Summary> = finalists
            .iter()
            .enumerate()
            .map(|(index, config)| {
                summarize(config.clone(), final_runs[index].clone(), input, objective)
            })
            .collect();
        let display_rows: Vec<Row> = rows
            .iter()
            .enumerate()
            .map(|(index, summary)| Row {
                label: if index == 0 {
                    "Selected baseline".into()
                } else {
                    "Phased Amber strategy".into()
                },
                reference_only: summary.config.policy != Policy::AmberFarm,
                risk_eligible: summary.rope as f64 / summary.total.max(1) as f64 * 100.0
                    >= min_rope_pct as f64,
                summary: summary.clone(),
                party_analysis: None,
                matching_strategies: 0,
            })
            .collect();
        let mut ranked: Vec<&Row> = display_rows
            .iter()
            .filter(|row| !row.reference_only && row.risk_eligible)
            .collect();
        ranked.sort_by(|a, b| cx_score(&a.summary, &b.summary));
        let best = ranked.first().copied();
        let mut close_different = false;
        let mut unresolved = false;
        if let Some(best) = best {
            for row in display_rows.iter().filter(|row| {
                !std::ptr::eq(*row, best)
                    && (std::ptr::eq(*row, &display_rows[0]) || !row.reference_only)
            }) {
                let (_, interval) = amber_difference(&best.summary, &row.summary, input, objective);
                let overlap = interval
                    .map(|[low, high]| low <= 0.0 && high >= 0.0)
                    .unwrap_or(false);
                if overlap
                    && row
                        .summary
                        .runs
                        .iter()
                        .zip(&best.summary.runs)
                        .any(|(run, other)| {
                            run_metric(run, input, objective) != run_metric(other, input, objective)
                        })
                {
                    close_different = true;
                }
                unresolved |= overlap;
            }
        }
        let risk_uncertain = display_rows.iter().any(|row| {
            !row.reference_only
                && amber_rope_uncertain(&row.summary, min_rope_pct)
                && best.is_none_or(|best| {
                    row.summary.score + 1.96 * row.summary.se
                        >= best.summary.score - 1.96 * best.summary.se
                })
        });
        if count >= max_trials
            || (!close_different && !risk_uncertain && (!unresolved || count >= 32.min(max_trials)))
        {
            break (rows, unresolved, risk_uncertain);
        }
        target = max_trials.min((count * 2).max(32.min(max_trials)));
        adaptive_rounds += 1;
    };
    let baseline_row = display_row(&rows[0], 0, min_rope_pct);
    let mut grouped: BTreeMap<String, usize> = BTreeMap::new();
    let mut display: Vec<Row> = vec![baseline_row.clone()];
    if !baseline_row.reference_only {
        grouped.insert(outcome_key(&rows[0], rows[0].runs.len()), 0);
    }
    for (index, row) in rows.iter().enumerate().skip(1) {
        let key = outcome_key(row, row.runs.len());
        if let Some(previous) = grouped.get(&key) {
            display[*previous].matching_strategies += 1;
            continue;
        }
        grouped.insert(key, display.len());
        display.push(display_row(row, index, min_rope_pct));
    }
    let winner = display
        .iter()
        .filter(|row| !row.reference_only && row.risk_eligible)
        .max_by(|a, b| {
            a.summary
                .score
                .partial_cmp(&b.summary.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();
    let (delta, interval) = winner
        .as_ref()
        .map(|winner| amber_difference(&winner.summary, &baseline_row.summary, input, objective))
        .unwrap_or((None, None));
    let observed_behaviors = coarse
        .iter()
        .map(|row| outcome_key(row, screening))
        .collect::<BTreeSet<_>>()
        .len();
    Ok(Some(SearchResult {
        kind: "amber-search".into(),
        objective,
        seed,
        rows: display,
        winner,
        baseline: baseline_row,
        screened: configs.len() as u128,
        configurations: configs.len() as u128,
        exhaustive: false,
        search_scope: None,
        delta,
        interval,
        claim: "Bounded diverse phase search with two local-refinement rounds and fresh-seed adaptive comparison. Observed ties are unresolved; rope percentages and change intervals are descriptive, not guaranteed safety or a global optimum.".into(),
        scope: String::new(),
        amber_search: Some(AmberSearchInfo {
            from,
            to,
            effective_from,
            depths,
            min_rope_pct,
            screening,
            trials: count,
            initial_trials: trials,
            max_trials,
            coarse: coarse_count,
            refined: configs.len().saturating_sub(coarse_count),
            racing_trials,
            adaptive_rounds,
            observed_behaviors,
            finalist_strategies,
            unresolved,
            risk_uncertain,
        }),
    }))
}

fn display_row(summary: &Summary, index: usize, min_rope_pct: usize) -> Row {
    Row {
        label: if index == 0 {
            "Selected baseline".into()
        } else {
            "Phased Amber strategy".into()
        },
        reference_only: summary.config.policy != Policy::AmberFarm,
        risk_eligible: summary.rope as f64 / summary.total.max(1) as f64 * 100.0
            >= min_rope_pct as f64,
        summary: summary.clone(),
        party_analysis: None,
        matching_strategies: 0,
    }
}

#[cfg(test)]
mod tests;
