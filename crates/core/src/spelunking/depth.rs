//! Depth-limit estimates, ported from `js/stats/systems/w7/spelunk-depth.js`.
//!
//! One row per in-game depth gives the rock mix, each object's expected stamina
//! for the cheapest reference tool, and how likely the floor blocks the party.
//! The two numbers the page graphs are per-floor placement odds, not cumulative
//! run failure; the inspector below the graph owns the exact values.

use super::{
    Config, ConfigError, Inputs, Requested, StaminaMode, Tool, data, game_log, js_round, normalize,
};
use crate::spelunking::data::Rock;
use std::fmt;

/// Which tool the estimate isolates each object with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReferenceTool {
    /// The cheapest unlocked tool for each object, the optimistic reference.
    #[default]
    Best,
    /// One specific unlocked tool.
    Tool(usize),
}

impl serde::Serialize for ReferenceTool {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ReferenceTool::Best => serializer.serialize_str("best"),
            ReferenceTool::Tool(id) => serializer.serialize_u64(*id as u64),
        }
    }
}

impl<'de> serde::Deserialize<'de> for ReferenceTool {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = ReferenceTool;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("`best` or a tool index")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<ReferenceTool, E> {
                if value == "best" {
                    return Ok(ReferenceTool::Best);
                }
                value
                    .parse()
                    .map(ReferenceTool::Tool)
                    .map_err(|_| E::custom("`best` or a tool index"))
            }
            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<ReferenceTool, E> {
                Ok(ReferenceTool::Tool(value as usize))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

impl ReferenceTool {
    pub fn id(self) -> String {
        match self {
            ReferenceTool::Best => "best".into(),
            ReferenceTool::Tool(id) => id.to_string(),
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        if id == "best" {
            return Some(ReferenceTool::Best);
        }
        id.parse().ok().map(ReferenceTool::Tool)
    }
}

/// The depth window and reference tool the estimate covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DepthRange {
    pub from: usize,
    pub to: usize,
    pub tool: ReferenceTool,
}

impl Default for DepthRange {
    fn default() -> Self {
        Self {
            from: 1,
            to: 150,
            tool: ReferenceTool::Best,
        }
    }
}

/// How many exits one floor's rock mix leaves reachable.
#[derive(Clone, Debug, PartialEq)]
pub struct Distribution {
    /// The chance of exactly one, two or three reachable exits.
    pub probabilities: Vec<f64>,
    pub expected: f64,
}

/// One object on a depth, with the cheapest way to break it.
#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    /// The rock's row in the cave table; the boss is the last row.
    pub kind: usize,
    pub name: String,
    pub boss: bool,
    /// The chance this rock fills a normal floor at this depth.
    pub probability: f64,
    pub hp: f64,
    /// The power requirement the hit chance divides entry power by.
    pub requirement: f64,
    pub hit_chance: f64,
    /// The reference tool's id.
    pub tool: usize,
    pub hits: f64,
    /// The tool's stamina per attempt, including elixir effects.
    pub cost: f64,
    /// The attempt cost after the stamina refund.
    pub per_attempt: f64,
    /// Expected stamina to break the object from full.
    pub expected: f64,
    /// Whether the expected cost exceeds the whole party's starting stamina.
    pub over_budget: bool,
}

/// One depth's objects and how likely its exits are unreachable.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The in-game depth, one above the zero-based rock depth.
    pub depth: usize,
    /// The share of normal rocks whose expected cost is over budget.
    pub hard: f64,
    /// The chance every generated exit is blocked, with the extra exits.
    pub blocked_exits: f64,
    pub objects: Vec<Object>,
}

/// The whole estimate for one configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub config: Config,
    pub cave: String,
    /// The party's starting stamina.
    pub budget: f64,
    /// Entry power after the memorized elixirs.
    pub power: f64,
    /// The stamina refund fraction the refund elixir grants.
    pub refund: f64,
    pub holes: Vec<f64>,
    pub rows: Vec<Row>,
    pub partial: bool,
}

/// `spelunkExitDistribution`: the three-hole mix after shop upgrade 18 and the
/// account's extra-exit bonus.
pub fn exit_distribution(input: &Inputs) -> Distribution {
    let bonus = (shop(input, 18) + input.holes_bonus) / 100.0;
    let clamp = |value: f64| value.clamp(0.0, 1.0);
    let probabilities = vec![
        clamp(1.0 - bonus),
        clamp(2.0 - bonus) - clamp(1.0 - bonus),
        1.0 - clamp(2.0 - bonus),
    ];
    let expected = probabilities
        .iter()
        .enumerate()
        .map(|(index, chance)| chance * (index + 1) as f64)
        .sum();
    Distribution {
        probabilities,
        expected,
    }
}

/// The normal-rock type mix on a zero-based depth, excluding the boss slot.
pub(super) fn rock_probabilities(cave: &data::Cave, depth: i64) -> Vec<f64> {
    let mut probabilities = vec![0.0; cave.rocks.len().saturating_sub(1)];
    let mut current = 0usize;
    let mut remaining = 1.0f64;
    for kind in 1..probabilities.len() {
        let threshold = cave.rocks[kind].depth;
        if (depth as f64) < threshold {
            continue;
        }
        let span = depth as f64 - threshold;
        let chance = (span / (span + 2.0 * threshold).max(1.0) * 1.5).clamp(0.0, 1.0);
        probabilities[current] += remaining * (1.0 - chance);
        remaining *= chance;
        current = kind;
    }
    probabilities[current] += remaining;
    probabilities
}

/// `spelunkRockStats`: the HP, power requirement and amber of one rock.
pub(super) fn rock_stats(cave: &data::Cave, kind: usize, depth: i64) -> (f64, f64, f64) {
    let rock = &cave.rocks[kind];
    let span = depth as f64 - rock.depth;
    let hp = if rock.scaled {
        (rock.hp + (rock.growth - 1.0) * span).floor()
    } else {
        js_round(rock.hp)
    };
    let base = cave.rocks[0].base.unwrap_or(0.0);
    let requirement = 100.0
        * base
        * rock.pow
        * if rock.scaled {
            rock.growth.powf(js_round(span.max(0.0)))
        } else {
            1.0
        };
    let amber = (2.0 * (cave.id as f64).min(2.0)).max(1.0)
        * rock.amber
        * if rock.scaled {
            1.0 + 0.25 * (rock.growth - 1.0) * span
        } else {
            1.0
        };
    (hp, requirement, amber)
}

pub(super) fn shop(input: &Inputs, index: usize) -> f64 {
    input.shop.get(index).copied().unwrap_or(0.0)
}

/// `spelunkEntryEffects`: the 13 effect slots the memorized elixirs set at entry.
pub(super) fn entry_effects(input: &Inputs, config: &Config) -> [f64; 13] {
    let mut effects = [0.0; 13];
    let repeats = (1.0 + shop(input, 58) / 100.0).floor().max(0.0);
    for id in &config.memorized {
        match id {
            0 | 2 | 4 | 9 => effects[*id] = 1.0,
            3 | 6 | 7 | 10 => effects[*id] += repeats,
            5 => effects[5] += 2.0 * repeats,
            _ => {}
        }
    }
    effects
}

/// `spelunkDigPower`: entry power after the power and discovery elixirs.
pub(super) fn dig_power(input: &Inputs, effects: &[f64; 13]) -> f64 {
    input.power * (100.0 * effects[10]).max(1.0) / (1.0 + (99.0 * effects[9]).min(99.0))
        * power_boost(input, effects)
}

pub(super) fn power_boost(input: &Inputs, effects: &[f64; 13]) -> f64 {
    if effects[4] > 0.5 {
        let quantity = input.elixirs.get(4).map_or(0.0, |elixir| elixir.quantity);
        5.0 / (1.0 + quantity / 100.0).powf(effects[4] - 1.0)
    } else {
        1.0
    }
}

/// `spelunkStaminaMultiplier`: the Manic and discovery-elixir cost multiplier.
pub(super) fn stamina_multiplier(config: &Config, effects: &[f64; 13]) -> f64 {
    (1.0 + (2.0 * effects[9]).min(2.0))
        * (4.0 * effects[10]).max(1.0)
        * if config.manic { 30.0 } else { 1.0 }
}

/// `spelunkHitDamage`: damage after the power-requirement and flat shop upgrades.
pub(super) fn hit_damage(input: &Inputs, damage: f64, ratio: f64) -> f64 {
    let mut damage = damage;
    if shop(input, 34) >= 1.0 {
        damage *= 1.0 + (game_log(ratio).floor() - 1.0).max(0.0);
    }
    damage * (1.0 + shop(input, 33) / 100.0)
}

fn finite(value: f64, name: &str) -> Result<f64, ConfigError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(ConfigError(format!("{name} is unavailable or invalid.")))
    }
}

/// `spelunkDepthProfile`: build the estimate for a requested configuration.
pub fn profile(
    input: &Inputs,
    requested: &Requested,
    range: &DepthRange,
) -> Result<Profile, ConfigError> {
    let config = normalize(input, requested)?;
    if range.from < 1 || range.to < range.from || range.to > 10_000 || range.to - range.from > 1_000
    {
        return Err(ConfigError(
            "Choose depths 1-10,000 spanning at most 1,001 floors.".into(),
        ));
    }
    let cave = &input.caves[config.cave];
    let tools: Vec<&Tool> = match range.tool {
        ReferenceTool::Best => input.tools.iter().collect(),
        ReferenceTool::Tool(id) => input.tools.iter().filter(|tool| tool.id == id).collect(),
    };
    if tools.is_empty() {
        return Err(ConfigError("Choose an unlocked reference tool.".into()));
    }
    let effects = entry_effects(input, &config);
    let power = finite(dig_power(input, &effects), "Entry Power")?;
    let multiplier = finite(stamina_multiplier(&config, &effects), "Stamina multiplier")?;
    let budget: f64 = config
        .party
        .iter()
        .map(|id| {
            let character = &input.characters[*id];
            if config.stamina == StaminaMode::Full {
                character.max_stamina
            } else {
                character.stamina
            }
        })
        .sum();
    finite(budget, "Party stamina")?;
    let refund = if effects[0] > 0.0 {
        input.elixirs.get(2).map_or(0.0, |elixir| elixir.quantity) / 100.0
    } else {
        0.0
    };
    let holes = exit_distribution(input).probabilities;
    let mut rows = Vec::with_capacity(range.to - range.from + 1);
    for visible in range.from..=range.to {
        let depth = visible as i64 - 1;
        let probabilities = rock_probabilities(cave, depth);
        let mut objects = Vec::new();
        for kind in 0..cave.rocks.len() {
            let boss = kind == cave.rocks.len() - 1;
            let probability = if boss {
                if depth as f64 == cave.boss_depth {
                    1.0
                } else {
                    0.0
                }
            } else {
                probabilities.get(kind).copied().unwrap_or(0.0)
            };
            if probability == 0.0 {
                continue;
            }
            let (hp, requirement, _amber) = rock_stats(cave, kind, depth);
            finite(hp, "Object HP")?;
            finite(requirement, "Object Power requirement")?;
            let ratio = power / requirement;
            let hit_chance = ratio.powf(0.6).min(1.0);
            let mut best: Option<Object> = None;
            for tool in &tools {
                let damage = finite(hit_damage(input, tool.damage, ratio), "Hit damage")?;
                let hits = (hp / damage).ceil();
                let cost = finite(tool.cost * multiplier, "Tool stamina")?;
                let per_attempt = cost * (1.0 - refund).max(0.0);
                let expected = if per_attempt == 0.0 {
                    0.0
                } else {
                    hits / hit_chance * per_attempt
                };
                let object = Object {
                    kind,
                    name: display_name(&cave.rocks[kind]),
                    boss,
                    probability,
                    hp,
                    requirement,
                    hit_chance,
                    tool: tool.id,
                    hits,
                    cost,
                    per_attempt,
                    expected,
                    over_budget: expected > budget,
                };
                if best
                    .as_ref()
                    .is_none_or(|current| object.expected < current.expected)
                {
                    best = Some(object);
                }
            }
            if let Some(object) = best {
                objects.push(object);
            }
        }
        let hard = objects
            .iter()
            .filter(|object| !object.boss && object.over_budget)
            .map(|object| object.probability)
            .sum::<f64>()
            .min(1.0);
        let blocked_exits = holes
            .iter()
            .enumerate()
            .map(|(index, probability)| probability * hard.powi(index as i32 + 1))
            .sum();
        rows.push(Row {
            depth: visible,
            hard,
            blocked_exits,
            objects,
        });
    }
    Ok(Profile {
        config,
        cave: cave.name.clone(),
        budget,
        power,
        refund,
        holes,
        rows,
        partial: input.partial,
    })
}

/// A rock's name with the underscores the game stores turned into spaces.
fn display_name(rock: &Rock) -> String {
    rock.name.replace('_', " ").trim().to_string()
}

#[cfg(test)]
mod tests;
