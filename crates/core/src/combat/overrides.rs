//! Mechanics overrides: the curated combat mechanics the simulator can disable,
//! set to a value, or scale relative to the save.
//!
//! Ported from `js/stats/systems/common/combat-sim-toggles.js`. The catalog names
//! every mechanic once, with the input keys it owns, its neutral value and the
//! modes it supports; a [`State`] holds one [`Rule`] per mechanic id, and
//! [`State::build`] turns the rules plus the run's saved inputs into the override
//! map the simulator applies. `raw:<input>` ids address any other scalar input.
//!
//! The catalog lives in `crates/core/data/combat-overrides.json`, written by
//! `script/combat-overrides-oracle.mjs` from the upstream module, so the data and
//! the oracle fixture it also writes can never drift apart. The saved inputs
//! (`entries` upstream) arrive with the encounter run; until then [`Entries`] is
//! empty, which is exactly the upstream behaviour when the saved values are
//! unavailable: `off` and `set` still apply, while `scale` and the object
//! mechanic wait for a base value.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const OVERRIDES_JSON: &str = include_str!("../../data/combat-overrides.json");

/// What a mechanic's saved input means, which decides its neutral value and the
/// modes it supports.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A multiplier; its neutral value is 1.
    Mult,
    /// A percentage added to a stat.
    Pct,
    /// A 0-1 probability.
    Chance,
    /// A count of items or kills.
    Count,
    /// A plain value with no neutral.
    Value,
    /// An on/off flag.
    Flag,
    /// Follow the save when untouched, else force on or off.
    Tri,
    /// An object of per-monster numbers.
    ObjScale,
}

/// One input a flag with its own keys writes.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixed {
    /// The input the value lands on.
    pub key: String,
    #[serde(flatten)]
    pub value: FixedValue,
}

/// The value a flag writes into one of its inputs.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FixedValue {
    /// A constant number.
    Number { number: f64 },
    /// A fixed list, such as every medallion type or none.
    List { items: Vec<String> },
}

impl FixedValue {
    /// The override value this fixed input resolves to.
    pub fn resolve(&self) -> Value {
        match self {
            FixedValue::Number { number } => Value::Number(*number),
            FixedValue::List { items } => Value::List(items.clone()),
        }
    }
}

/// One curated mechanic.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Toggle {
    /// The id the state keys on.
    pub id: String,
    /// The group the editor lists it under.
    pub group: String,
    pub kind: Kind,
    pub label: String,
    pub desc: String,
    /// The inputs this mechanic owns; the first is the one the state addresses.
    pub keys: Vec<String>,
    /// The value `off` writes, when the mechanic has a neutral value.
    pub neutral: Option<f64>,
    /// The inputs `on` writes for a flag with its own keys.
    #[serde(default)]
    pub on: Vec<Fixed>,
    /// The inputs `off` writes for a flag with its own keys.
    #[serde(default)]
    pub off: Vec<Fixed>,
}

impl Toggle {
    /// The modes this mechanic supports, in display order.
    pub fn modes(&self) -> Vec<Mode> {
        match self.kind {
            Kind::Flag | Kind::Tri => vec![Mode::On, Mode::Off],
            Kind::ObjScale => vec![Mode::Off, Mode::Scale],
            _ if self.neutral.is_some() => vec![Mode::Off, Mode::Set, Mode::Scale],
            _ => vec![Mode::Set, Mode::Scale],
        }
    }

    /// The input the state addresses first.
    pub fn key(&self) -> &str {
        self.keys.first().map(String::as_str).unwrap_or(&self.id)
    }

    /// Whether the flag writes keys beyond the one it is filed under.
    pub fn has_fixed_keys(&self) -> bool {
        !self.on.is_empty() || !self.off.is_empty()
    }
}

/// The catalog: the groups the editor lists and every mechanic.
#[derive(Debug, Deserialize)]
pub struct Catalog {
    pub groups: Vec<String>,
    pub toggles: Vec<Toggle>,
}

/// The parsed catalog, read once.
pub fn data() -> &'static Catalog {
    static DATA: OnceLock<Catalog> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(OVERRIDES_JSON)
            .expect("crates/core/data/combat-overrides.json is valid")
    })
}

/// The group labels, in catalog order.
pub fn groups() -> &'static [String] {
    &data().groups
}

/// The curated mechanic with this id.
pub fn toggle(id: &str) -> Option<&'static Toggle> {
    data().toggles.iter().find(|toggle| toggle.id == id)
}

/// One override mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// The mechanic's neutral value: disabled.
    Off,
    /// Forced on; flags and tristate mechanics only.
    On,
    /// The value replaces the saved input.
    Set,
    /// The saved input is multiplied by the value.
    Scale,
}

impl Mode {
    /// The mode's id in the project JSON.
    pub fn id(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::On => "on",
            Mode::Set => "set",
            Mode::Scale => "scale",
        }
    }

    /// Its label in the editor's mode picker.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Off => "Off",
            Mode::On => "On",
            Mode::Set => "Set",
            Mode::Scale => "Scale ×",
        }
    }

    /// Every mode a persisted state may carry.
    pub const ALL: [Mode; 4] = [Mode::Off, Mode::On, Mode::Set, Mode::Scale];
}

/// The input key a state id addresses: the mechanic's first key, or the input
/// named after `raw:`.
pub fn input_key(id: &str) -> Option<&str> {
    match toggle(id) {
        Some(toggle) => Some(toggle.key()),
        None => id.strip_prefix("raw:").filter(|key| !key.is_empty()),
    }
}

/// A saved sim input's value: a number, or the per-monster object the respawn
/// mechanic scales.
#[derive(Clone, Debug, Deserialize, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum SavedValue {
    Number(f64),
    Object(BTreeMap<String, f64>),
}

/// The run's saved inputs, keyed by input name: the part of upstream's
/// `buildCombatInputs(...).entries` the overrides read.
pub type Entries = BTreeMap<String, SavedValue>;

/// One resolved input override.
#[derive(Clone, Debug, Deserialize, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum Value {
    Number(f64),
    List(Vec<String>),
    Object(BTreeMap<String, f64>),
}

/// The override map the simulator applies on top of the saved inputs.
pub type Overrides = BTreeMap<String, Value>;

/// One override rule, as the project JSON stores it.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, Deserialize)]
pub struct Rule {
    pub mode: Mode,
    /// The set value or the scale factor; absent for `off` and `on`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
}

impl Rule {
    /// A rule that disables or forces on a mechanic.
    pub fn new(mode: Mode) -> Self {
        Self { mode, value: None }
    }

    /// A rule that replaces or scales the saved input.
    pub fn with_value(mode: Mode, value: f64) -> Self {
        Self {
            mode,
            value: Some(value),
        }
    }

    /// The rule the editor starts from when a mode is picked: a set keeps the
    /// value typed so far (or the saved value the caller passes), a scale is 1.
    pub fn seeded(mode: Mode, current: Option<&Rule>, saved: Option<f64>) -> Self {
        match mode {
            Mode::Off | Mode::On => Rule::new(mode),
            Mode::Set => Rule {
                mode,
                value: Some(match current {
                    Some(rule) if rule.mode == Mode::Set => {
                        rule.value.unwrap_or(saved.unwrap_or(0.0))
                    }
                    _ => saved.unwrap_or(0.0),
                }),
            },
            Mode::Scale => Rule {
                mode,
                value: Some(match current {
                    Some(rule) if rule.mode == Mode::Scale => rule.value.unwrap_or(1.0),
                    _ => 1.0,
                }),
            },
        }
    }
}

/// The override state: one rule per mechanic id, `raw:<input>` for any other.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct State(BTreeMap<String, Rule>);

impl State {
    /// No overrides: every input keeps its saved value.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads the project's JSON shape, keeping every row whose mode the engine
    /// knows. Unsupported modes and missing values are dropped by
    /// [`Self::normalize`], the way upstream's `normalizeToggleState` drops them.
    pub fn from_json(value: &serde_json::Value) -> Self {
        let mut state = Self::new();
        let Some(rows) = value.as_object() else {
            return state;
        };
        for (id, row) in rows {
            let Some(row) = row.as_object() else {
                continue;
            };
            let Some(mode) = row
                .get("mode")
                .and_then(serde_json::Value::as_str)
                .and_then(mode_by_id)
            else {
                continue;
            };
            let value = row.get("value").and_then(serde_json::Value::as_f64);
            state.0.insert(id.clone(), Rule { mode, value });
        }
        state
    }

    /// The rows whose mode the mechanic supports and whose set or scale value is
    /// a finite number, with `off`/`on` rows stripped of any stray value.
    pub fn normalize(&self) -> Self {
        let mut state = Self::new();
        for (id, rule) in &self.0 {
            if input_key(id).is_none() {
                continue;
            }
            let supported = match toggle(id) {
                Some(toggle) => toggle.modes(),
                None => vec![Mode::Set, Mode::Scale],
            };
            if !supported.contains(&rule.mode) {
                continue;
            }
            let normalized = match rule.mode {
                Mode::Set | Mode::Scale => match rule.value {
                    Some(value) if value.is_finite() => Rule::with_value(rule.mode, value),
                    _ => continue,
                },
                Mode::Off | Mode::On => Rule::new(rule.mode),
            };
            state.0.insert(id.clone(), normalized);
        }
        state
    }

    /// One row as stored, before normalization.
    pub fn rule(&self, id: &str) -> Option<Rule> {
        self.0.get(id).copied()
    }

    /// Whether one mechanic currently has a valid override.
    pub fn active(&self, id: &str) -> bool {
        self.normalize().0.contains_key(id)
    }

    /// How many mechanics have a valid override.
    pub fn len(&self) -> usize {
        self.normalize().0.len()
    }

    /// Whether no mechanic has a valid override.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether any rule scales a saved input, upstream `toggleNeedsBase`. The
    /// object mechanic reads the saved inputs too, but `off` resolves to an empty
    /// object without them, so only scale has to wait.
    pub fn needs_base(&self) -> bool {
        self.normalize()
            .0
            .values()
            .any(|rule| rule.mode == Mode::Scale)
    }

    /// Set one mechanic's rule.
    pub fn set(&mut self, id: &str, rule: Rule) {
        self.0.insert(id.to_string(), rule);
    }

    /// Forget one mechanic's rule, putting its input back to the save.
    pub fn remove(&mut self, id: &str) {
        self.0.remove(id);
    }

    /// Put every mechanic back to its saved value.
    pub fn reset(&mut self) {
        self.0.clear();
    }

    /// The overrides these rules resolve to against `entries`.
    pub fn build(&self, entries: &Entries) -> Overrides {
        self.normalize().build_valid(entries)
    }

    /// The rules resolved against `entries`, with `base` applied last so an
    /// explicit run override always wins, exactly upstream `resolveToggleOverrides`.
    pub fn resolve(&self, entries: &Entries, base: &Overrides) -> Overrides {
        let valid = self.normalize();
        if valid.0.is_empty() {
            return base.clone();
        }
        let mut resolved = valid.build_valid(entries);
        resolved.extend(base.clone());
        resolved
    }

    /// One `(id, line)` per active rule, for the editor's chips: the catalog's
    /// order first, so a chip sits above the row it edits, then the raw inputs.
    pub fn chips(&self) -> Vec<(String, String)> {
        let valid = self.normalize();
        let mut chips = Vec::with_capacity(valid.0.len());
        for toggle in &data().toggles {
            if let Some(rule) = valid.0.get(&toggle.id) {
                chips.push((toggle.id.clone(), chip(&toggle.label, rule)));
            }
        }
        for (id, rule) in &valid.0 {
            if toggle(id).is_none() {
                let name = raw_name(input_key(id).unwrap_or(id));
                chips.push((id.clone(), chip(&name, rule)));
            }
        }
        chips
    }

    /// The chip lines alone, in [`Self::chips`] order.
    pub fn describe(&self) -> Vec<String> {
        self.chips().into_iter().map(|(_, line)| line).collect()
    }

    /// [`Self::build`] for a state [`Self::normalize`] already produced.
    fn build_valid(&self, entries: &Entries) -> Overrides {
        let mut overrides = Overrides::new();
        for (id, rule) in &self.0 {
            let Some(key) = input_key(id) else {
                continue;
            };
            let base = entries.get(key);
            let toggle = toggle(id);
            let kind = toggle.map(|toggle| toggle.kind);
            let scale = rule.value.filter(|value| value.is_finite());
            match (toggle, rule.mode) {
                (Some(toggle), Mode::Off | Mode::On) if toggle.has_fixed_keys() => {
                    let fixed = if rule.mode == Mode::On {
                        &toggle.on
                    } else {
                        &toggle.off
                    };
                    for entry in fixed {
                        overrides.insert(entry.key.clone(), entry.value.resolve());
                    }
                }
                (Some(_), Mode::Off | Mode::On) if matches!(kind, Some(Kind::Flag | Kind::Tri)) => {
                    let value = if rule.mode == Mode::On { 1.0 } else { 0.0 };
                    overrides.insert(key.to_string(), Value::Number(value));
                }
                (Some(_), Mode::Off) if kind == Some(Kind::ObjScale) => {
                    let zeroed = match base {
                        Some(SavedValue::Object(object)) => {
                            object.keys().map(|name| (name.clone(), 0.0)).collect()
                        }
                        _ => BTreeMap::new(),
                    };
                    overrides.insert(key.to_string(), Value::Object(zeroed));
                }
                (Some(_), Mode::Set) => {
                    if let Some(value) = scale {
                        overrides.insert(key.to_string(), Value::Number(value));
                    }
                }
                (Some(_), Mode::Scale) => match (base, scale) {
                    (Some(SavedValue::Number(base)), Some(scale)) if base.is_finite() => {
                        overrides.insert(key.to_string(), Value::Number(base * scale));
                    }
                    (Some(SavedValue::Object(object)), Some(scale))
                        if kind == Some(Kind::ObjScale) =>
                    {
                        let scaled = object
                            .iter()
                            .map(|(name, value)| (name.clone(), value * scale))
                            .collect();
                        overrides.insert(key.to_string(), Value::Object(scaled));
                    }
                    _ => {}
                },
                (Some(toggle), Mode::Off) => {
                    if let Some(neutral) = toggle.neutral {
                        overrides.insert(key.to_string(), Value::Number(neutral));
                    }
                }
                (None, Mode::Set) => {
                    if let Some(value) = scale {
                        overrides.insert(key.to_string(), Value::Number(value));
                    }
                }
                (None, Mode::Scale) => {
                    if let (Some(SavedValue::Number(base)), Some(scale)) = (base, scale)
                        && base.is_finite()
                    {
                        overrides.insert(key.to_string(), Value::Number(base * scale));
                    }
                }
                _ => {}
            }
        }
        overrides
    }
}

impl<'de> Deserialize<'de> for State {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self::from_json(&value).normalize())
    }
}

/// The friendly name of a `raw:<input>` id, upstream `rawInputName`: the talent
/// families it knows, then the camel-case split with its first letter raised.
pub fn raw_name(key: &str) -> String {
    if let Some((id, value)) = split_talent_value(key) {
        return format!("{} (value {value})", crate::combat::data::talent_name(id));
    }
    if let Some(id) = key
        .strip_prefix("talentEnh")
        .and_then(|rest| rest.parse::<u32>().ok())
    {
        return format!("{} (enhanced)", crate::combat::data::talent_name(id));
    }
    if let Some(id) = key
        .strip_prefix("talent")
        .and_then(|rest| rest.strip_suffix("Level"))
        .and_then(|id| id.parse::<u32>().ok())
    {
        return format!("{} level", crate::combat::data::talent_name(id));
    }
    if let Some(id) = key
        .strip_prefix("buff")
        .and_then(|rest| rest.parse::<u32>().ok())
    {
        return format!("{} buff", crate::combat::data::talent_name(id));
    }
    let mut out = String::with_capacity(key.len() + 4);
    let mut previous_lower = false;
    for ch in key.chars() {
        if previous_lower && (ch.is_ascii_uppercase() || ch.is_ascii_digit()) {
            out.push(' ');
        }
        previous_lower = ch.is_ascii_lowercase();
        if out.is_empty() {
            out.extend(ch.to_uppercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// A `talent<id>_<value>` key's talent id and value digit.
fn split_talent_value(key: &str) -> Option<(u32, char)> {
    let (id, value) = key.strip_prefix("talent")?.split_once('_')?;
    let digits: Vec<char> = value.chars().collect();
    if digits.len() != 1 || !digits[0].is_ascii_digit() {
        return None;
    }
    Some((id.parse().ok()?, digits[0]))
}

/// One chip line: the mechanic's name and what its rule does to the input.
fn chip(name: &str, rule: &Rule) -> String {
    let what = match rule.mode {
        Mode::Off => "off".to_string(),
        Mode::On => "on".to_string(),
        Mode::Set => format!("= {}", fmt_value(rule.value.unwrap_or(0.0))),
        Mode::Scale => format!("\u{d7} {}", fmt_value(rule.value.unwrap_or(1.0))),
    };
    format!("{name} {what}")
}

/// A rule value as a chip reads it: integers without a fraction, others in their
/// shortest round-trip form.
fn fmt_value(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// The mode with this id, as the project JSON stores it.
fn mode_by_id(id: &str) -> Option<Mode> {
    Mode::ALL.into_iter().find(|mode| mode.id() == id)
}
