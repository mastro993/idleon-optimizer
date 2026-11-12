//! The expedition configuration: the upstream `normalizeSpelunkConfig` port.
//!
//! The request carries what the controls chose and every field falls back to
//! the save, exactly as upstream does. A request the game would not accept is
//! rejected with the upstream message, so a duplicate party or a locked policy
//! reads the same on both sides.

use super::{AmberPolicy, Entry, Inputs, Policy, StaminaMode, ToolChoice};
use std::fmt;
use std::ops::RangeInclusive;

/// A requested expedition configuration; every field falls back to the save.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Requested {
    pub cave: Option<usize>,
    pub party: Option<Vec<usize>>,
    pub manic: Option<bool>,
    pub policy: Option<Policy>,
    pub speed: Option<u32>,
    /// Belt slots, `-1` for empty. Shorter vectors are padded to the slot count.
    pub elixirs: Option<Vec<i32>>,
    pub memorized: Option<Vec<usize>>,
    pub tool: Option<ToolChoice>,
    pub entry: Option<Entry>,
    pub full_clear: Option<bool>,
    pub amber: Option<AmberPolicy>,
    pub reaction_ms: Option<f64>,
    pub stamina: Option<StaminaMode>,
}

/// A configuration the upstream normaliser accepts.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub cave: usize,
    pub party: Vec<usize>,
    pub manic: bool,
    pub policy: Policy,
    pub speed: u32,
    pub elixirs: Vec<i32>,
    pub memorized: Vec<usize>,
    pub tool: ToolChoice,
    pub entry: Entry,
    pub full_clear: bool,
    /// Present exactly when the policy is [`Policy::AmberFarm`].
    pub amber: Option<AmberPolicy>,
    pub reaction_ms: f64,
    pub stamina: StaminaMode,
}

/// Why a requested configuration cannot run, with the upstream message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError(pub String);

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

fn invalid(message: &str) -> ConfigError {
    ConfigError(message.to_string())
}

/// The upstream `normalizeSpelunkConfig`: fill defaults from the save and
/// reject a party, belt or mode the game would not accept.
///
/// The order of the checks follows the upstream function so a rejected request
/// reports the same message.
pub fn normalize(input: &Inputs, requested: &Requested) -> Result<Config, ConfigError> {
    let cave = requested.cave.unwrap_or(0);
    if cave >= input.caves.len() {
        return Err(invalid("Invalid cave."));
    }
    let capacity = input.caves[cave].capacity;
    let full_stamina = requested.stamina == Some(StaminaMode::Full);
    let party = match &requested.party {
        Some(party) => party.clone(),
        None => input
            .characters
            .iter()
            .filter(|character| full_stamina || character.has_half_stamina())
            .take(capacity)
            .map(|character| character.id)
            .collect(),
    };
    let unique: std::collections::BTreeSet<usize> = party.iter().copied().collect();
    if party.is_empty() || party.len() > capacity || unique.len() != party.len() {
        return Err(invalid(
            "Choose a unique, nonempty party within this cave's capacity.",
        ));
    }
    for id in &party {
        let Some(character) = input.characters.get(*id) else {
            return Err(invalid("Invalid party member."));
        };
        if !full_stamina && !character.has_half_stamina() {
            return Err(ConfigError(format!(
                "{} needs at least half maximum stamina to join.",
                character.name
            )));
        }
    }
    let manic = requested.manic.unwrap_or(input.saved_manic);
    if manic && !input.manic {
        return Err(invalid("Manic expeditions are locked."));
    }
    let policy = requested.policy.unwrap_or(if input.auto {
        Policy::Auto
    } else {
        Policy::Dive
    });
    if policy == Policy::Auto && !input.auto {
        return Err(invalid("This expedition policy is unavailable."));
    }
    let speed = requested.speed.unwrap_or(input.speed_max);
    if speed > input.speed_max {
        return Err(invalid("Invalid Auto speed."));
    }
    let mut elixirs = requested
        .elixirs
        .clone()
        .unwrap_or_else(|| input.saved_elixirs.clone());
    while elixirs.len() > input.slots && elixirs.last() == Some(&-1) {
        elixirs.pop();
    }
    if elixirs.len() > input.slots {
        return Err(invalid("Too many equipped elixirs."));
    }
    while elixirs.len() < input.slots {
        elixirs.push(-1);
    }
    let mut counts = std::collections::BTreeMap::new();
    for id in &elixirs {
        if *id == -1 {
            continue;
        }
        if *id < 0 || *id as usize >= input.elixir_owned {
            return Err(invalid("Invalid elixir."));
        }
        let count = counts.entry(*id as usize).or_insert(0usize);
        *count += 1;
        if *count > input.dupes {
            return Err(invalid("This elixir exceeds the duplicate limit."));
        }
    }
    let mut memorized = requested
        .memorized
        .clone()
        .unwrap_or_else(|| input.memorized.clone());
    memorized.sort_unstable();
    if memorized.len() > input.memory_slots {
        return Err(invalid("Too many memorized elixirs."));
    }
    if memorized.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid("Memorized elixirs must be different."));
    }
    if memorized
        .iter()
        .any(|id| *id >= input.elixir_owned || *id >= input.elixirs.len())
    {
        return Err(invalid("Invalid memorized elixir."));
    }
    let tool = requested.tool.unwrap_or_default();
    if let ToolChoice::Tool(id) = tool
        && !input.tools.iter().any(|tool| tool.id == id)
    {
        return Err(invalid("This tool is locked."));
    }
    let entry = requested.entry.unwrap_or_default();
    let full_clear =
        policy == Policy::AmberFarm || requested.full_clear.unwrap_or(input.full_clear);
    let amber = (policy == Policy::AmberFarm).then(|| requested.amber.unwrap_or_default());
    let amber = match amber {
        Some(amber) => Some(validate_amber(amber)?),
        None => None,
    };
    let reaction_ms = requested.reaction_ms.unwrap_or(350.0);
    if !reaction_ms.is_finite() || !(100.0..=5000.0).contains(&reaction_ms) {
        return Err(invalid("Invalid manual reaction time."));
    }
    let stamina = requested.stamina.unwrap_or_default();
    Ok(Config {
        cave,
        party,
        manic,
        policy,
        speed,
        elixirs,
        memorized,
        tool,
        entry,
        full_clear,
        amber,
        reaction_ms,
        stamina,
    })
}

fn validate_amber(amber: AmberPolicy) -> Result<AmberPolicy, ConfigError> {
    let check = |value: u32, range: RangeInclusive<u32>, message: &str| {
        if range.contains(&value) {
            Ok(())
        } else {
            Err(invalid(message))
        }
    };
    check(amber.dump_depth, 1..=10_000, "Invalid Amber dump depth.")?;
    check(
        amber.finishers,
        1..=2,
        "Invalid Amber finishing characters.",
    )?;
    check(
        amber.reserve_pct,
        25..=100,
        "Invalid Amber finishing reserve.",
    )?;
    check(
        amber.harvest_depths,
        0..=100,
        "Invalid Amber harvest depths.",
    )?;
    check(
        amber.rope_reserve_pct,
        1..=100,
        "Invalid Amber rope reserve.",
    )?;
    Ok(amber)
}
