//! Combat-driven account progress and smelting output (CS-06).
//!
//! Ported from `js/stats/systems/common/combat-sim-progress.js`:
//!
//! - [`State::grant`] is the page's `sim.progress.grant`: forge time advances the
//!   saved furnace, pet time banks toward the shiny-pet batch and alchemy time is
//!   only credited;
//! - [`State::tick`] is `_forgeTick`, the 250 ms furnace task (`_forgeStats`,
//!   `_forgeProduce` and `_refill` are the helpers behind it);
//! - [`State::pet_batch`] is the scene updater that spends banked pet time on
//!   deployed shiny pets (`shinyPetLevel`'s curve included);
//! - [`State::scurge_kill`], [`State::coral_kill`] and [`State::omega_kill`] are
//!   the Clamworks Scurge, Coral Kid and Omega Arcade kill credits;
//! - [`State::summary`] is `combatProgressSummary`, the panel's rows.
//!
//! `crates/combat/data/progress.json` and `fixtures/combat-progress.json` are
//! written by `script/combat-progress-oracle.mjs` from one upstream checkout:
//! the fixture runs the module's own methods on a seeded stream, so the port
//! cannot drift from its source. The save-derived [`Config`] is decoded by
//! `idleon_import::combat` and compared with the same fixture.
//!
//! Not modelled: the Omega ball's dropped item (the reward engine's item
//! actors), the pet boosts outside the shiny table and the Menagerie EXP the
//! Scurge levels would process (menu-gated upstream too). Each one is named in
//! the summary's notices when it is reached.

use idleon_common::Rng;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const PROGRESS_JSON: &str = include_str!("../data/progress.json");

/// The furnace's default interval for an item without a `Cooldown`.
const NO_COOLDOWN: f64 = 9_999_999.0;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    items: BTreeMap<String, Item>,
    /// `Spelunky[28]`: the Scurge upgrade column `scurgeKill` reads.
    scurge_column: Vec<f64>,
    /// `PET_NAMES[world][pet]`, for the shiny table's rows.
    pet_names: Vec<Vec<String>>,
    /// `PET_STATS[world][pet][0]`, the key a `Pets` row names.
    pet_keys: Vec<Vec<String>>,
}

/// One item's forge fields.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Item {
    /// The display name the panel shows.
    pub name: String,
    /// `Cooldown`: the forge interval, or an oil's consumption numerator.
    pub cooldown: f64,
    /// `Amount`: the ore's cost per bar, or a boost's percentage.
    pub amount: f64,
    /// `Effect`: `Ore`, `SpeedForge`, `DoubleBarsForge` or `_`.
    pub effect: String,
}

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(PROGRESS_JSON).expect("progress.json is valid"))
}

/// One item's forge fields; an unknown key reads as the upstream `ITEMS[key] || {}`.
fn item(key: &str) -> Option<&'static Item> {
    data().items.get(key)
}

/// The display name of a forge item or bar, with the game data's underscores as
/// spaces; the upstream `itemName`.
pub fn item_name(key: &str) -> String {
    item(key)
        .map(|item| item.name.clone())
        .unwrap_or_else(|| key.to_string())
}

/// The pet name the shiny table shows, or `World w pet n` when the row is absent.
pub fn pet_name(world: usize, pet: usize) -> String {
    data()
        .pet_names
        .get(world)
        .and_then(|pets| pets.get(pet))
        .cloned()
        .unwrap_or_else(|| format!("World {world} pet {pet}"))
}

/// The saved furnace the run advances.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Forge {
    /// `ForgeItemOrder`/`ForgeItemQty`/`ForgeIntProg` are all long enough.
    pub available: bool,
    /// `ForgeItemOrder`: ore, oil and bar per slot.
    pub order: Vec<String>,
    /// `ForgeItemQty`: the same three rows' quantities.
    pub qty: Vec<f64>,
    /// `ForgeIntProg`: each slot's progress toward its interval.
    pub progress: Vec<f64>,
    /// The saved chest's counts, read by the refill rule.
    pub storage: BTreeMap<String, f64>,
    pub storage_available: bool,
    pub slots: f64,
    pub cap: f64,
    pub refill: bool,
    pub gem_slots: f64,
    pub speed: f64,
    pub bars: f64,
    pub oil_use: f64,
}

impl Default for Forge {
    fn default() -> Self {
        Self {
            available: false,
            order: Vec::new(),
            qty: Vec::new(),
            progress: Vec::new(),
            storage: BTreeMap::new(),
            storage_available: false,
            slots: 0.0,
            cap: 0.0,
            refill: false,
            gem_slots: 0.0,
            speed: 1.0,
            bars: 1.0,
            oil_use: 1.0,
        }
    }
}

impl Forge {
    fn ore(&self, slot: usize) -> &str {
        self.order.get(3 * slot).map_or("", String::as_str)
    }

    fn oil(&self, slot: usize) -> &str {
        self.order.get(3 * slot + 1).map_or("", String::as_str)
    }

    fn bar(&self, slot: usize) -> &str {
        self.order.get(3 * slot + 2).map_or("", String::as_str)
    }

    fn qty_at(&self, at: usize) -> f64 {
        self.qty.get(at).copied().unwrap_or(0.0)
    }

    fn progress_at(&self, slot: usize) -> f64 {
        self.progress.get(slot).copied().unwrap_or(0.0)
    }

    fn set_qty(&mut self, at: usize, value: f64) {
        if let Some(cell) = self.qty.get_mut(at) {
            *cell = value;
        }
    }

    fn set_order(&mut self, at: usize, value: String) {
        if let Some(cell) = self.order.get_mut(at) {
            *cell = value;
        }
    }

    fn set_progress(&mut self, slot: usize, value: f64) {
        if let Some(cell) = self.progress.get_mut(slot) {
            *cell = value;
        }
    }
}

/// The pets a run's shiny progress reads.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Pets {
    pub available: bool,
    /// `Pets` rows: `[key, shinyLevel, …]` per deployed slot.
    pub deployed: Vec<Vec<String>>,
    /// `Breeding`: rows 22.. are each world's shiny EXP per pet.
    pub breeding: Vec<Vec<f64>>,
    /// The shiny EXP per day a deployed shiny pet banks.
    pub shiny_speed: f64,
}

/// The Clamworks Scurge kill credit.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Scurge {
    pub unlocked: bool,
    /// `Menagerie`, row 0 (EXP/level) and row 5 (upgrade levels).
    pub menagerie: Vec<Vec<f64>>,
}

/// The Coral Kid kill credit.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Coral {
    pub unlocked: bool,
    pub kills: f64,
}

/// The Omega Arcade ball credit.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Omega {
    pub unlocked: bool,
    /// Saved ball balance and lifetime drop count were both readable.
    pub available: bool,
    pub balls: f64,
    pub drops: f64,
}

/// The progress inputs a run reads, matching `combatProgressInputs`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub forge: Forge,
    pub pets: Pets,
    pub scurge: Scurge,
    pub coral: Coral,
    pub omega: Omega,
}

/// How a kill credits combat progress, matching the `monsterKill` gates.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Grants {
    /// The character's drop rate, which scales the Omega ball chance.
    pub drop_rate: f64,
    /// `killCreditPerKill`: the KLA credit each kill banks for the Coral Kid.
    pub kill_credit: f64,
    /// The scene the run is on, which gates the Omega roll.
    pub map: usize,
    pub scene: String,
}

impl Default for Grants {
    fn default() -> Self {
        Self {
            drop_rate: 1.0,
            kill_credit: 0.0,
            map: 0,
            scene: String::new(),
        }
    }
}

/// One notice the panel shows, keyed so a repeated path only reports once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub key: &'static str,
    pub text: String,
}

/// One deployed shiny pet's progress.
#[derive(Clone, Debug, PartialEq)]
pub struct PetProgress {
    pub world: usize,
    pub index: usize,
    pub name: String,
    pub start: f64,
    pub gained: f64,
    pub end: f64,
    pub start_level: f64,
    pub level: f64,
}

/// The furnace as the run leaves it.
#[derive(Clone, Debug, PartialEq)]
pub struct ForgeState {
    pub order: Vec<String>,
    pub qty: Vec<f64>,
    pub progress: Vec<f64>,
    pub storage: BTreeMap<String, f64>,
}

/// The Scurge's menu row at the end of the run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScurgeState {
    pub level: f64,
    pub exp: f64,
    pub requirement: f64,
}

/// The panel's rows, matching `combatProgressSummary`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub forge_seconds: f64,
    pub pet_seconds: f64,
    pub alchemy_seconds: f64,
    pub scurge_exp: f64,
    pub coral_kills: f64,
    pub omega_balls: f64,
    pub bars: BTreeMap<String, f64>,
    pub ore_used: BTreeMap<String, f64>,
    pub oil_used: BTreeMap<String, f64>,
    pub storage_used: BTreeMap<String, f64>,
    pub pets: Vec<PetProgress>,
    pub notices: Vec<Notice>,
    pub pet_pending_seconds: f64,
    pub forge_available: bool,
    pub forge: Option<ForgeState>,
    pub scurge: Option<ScurgeState>,
    pub coral: Option<Coral>,
    pub omega: Option<Omega>,
}

impl Summary {
    /// The bars a comparison export carries, keyed by display name.
    pub fn bars_named(&self) -> BTreeMap<String, f64> {
        self.bars
            .iter()
            .map(|(bar, qty)| (item_name(bar), *qty))
            .collect()
    }
}

/// The forge statistics one slot's production reads; `_forgeStats`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Stats {
    interval: f64,
    speed: f64,
    bars: f64,
    cost: f64,
    oil_cost: f64,
}

fn stats(forge: &Forge, slot: usize) -> Stats {
    let ore = item(forge.ore(slot));
    let oil = item(forge.oil(slot));
    let gem = if forge.gem_slots > slot as f64 {
        1.5
    } else {
        1.0
    };
    let oil_effect = oil.map(|oil| (oil.effect.as_str(), oil.amount));
    Stats {
        interval: ore.map_or(0.0, |ore| ore.cooldown),
        speed: 0.25
            * forge.speed
            * match oil_effect {
                Some(("SpeedForge", amount)) => 1.0 + amount / 100.0,
                _ => 1.0,
            }
            * gem,
        bars: forge.bars
            * match oil_effect {
                Some(("DoubleBarsForge", amount)) => 1.0 + amount / 100.0,
                _ => 1.0,
            }
            * gem,
        cost: ore.map_or(0.0, |ore| ore.amount),
        oil_cost: oil.map_or(0.0, |oil| oil.cooldown) * forge.oil_use / 100.0,
    }
}

fn interval(stats: &Stats) -> f64 {
    if stats.interval != 0.0 {
        stats.interval
    } else {
        NO_COOLDOWN
    }
}

/// One run's combat progress: the cloned furnace, the grants and the panels.
#[derive(Clone, Debug)]
pub struct State {
    config: Config,
    bars: BTreeMap<String, f64>,
    ore_used: BTreeMap<String, f64>,
    oil_used: BTreeMap<String, f64>,
    storage_used: BTreeMap<String, f64>,
    pets: Vec<PetProgress>,
    pet_pending_seconds: f64,
    scurge_exp: f64,
    coral_kills: f64,
    omega_balls: f64,
    pet_seconds: f64,
    alchemy_seconds: f64,
    forge_seconds: f64,
    notices: Vec<Notice>,
}

fn add(map: &mut BTreeMap<String, f64>, key: &str, amount: f64) {
    *map.entry(key.to_string()).or_insert(0.0) += amount;
}

impl State {
    /// Start a run from the resolved config; `initCombatProgress`.
    pub fn new(config: Config) -> Self {
        let mut state = Self {
            config,
            bars: BTreeMap::new(),
            ore_used: BTreeMap::new(),
            oil_used: BTreeMap::new(),
            storage_used: BTreeMap::new(),
            pets: Vec::new(),
            pet_pending_seconds: 0.0,
            scurge_exp: 0.0,
            coral_kills: 0.0,
            omega_balls: 0.0,
            pet_seconds: 0.0,
            alchemy_seconds: 0.0,
            forge_seconds: 0.0,
            notices: Vec::new(),
        };
        if state.config.omega.unlocked && !state.config.omega.available {
            state.notice(
                "omegaState",
                "Omega drops are unavailable: saved ball balance or lifetime drop count is missing or invalid. Fresh-account odds are not assumed.",
            );
        }
        state
    }

    fn notice(&mut self, key: &'static str, text: &str) {
        if self.notices.iter().any(|notice| notice.key == key) {
            return;
        }
        self.notices.push(Notice {
            key,
            text: text.to_string(),
        });
    }

    /// `sim.progress.grant`: pay one kill's progress credit.
    pub fn grant(&mut self, rng: &mut Rng, kind: Grant, seconds: f64) {
        if !seconds.is_finite() || seconds < 0.0 {
            return;
        }
        match kind {
            Grant::Forge => {
                self.forge_seconds += seconds;
                if self.config.forge.available {
                    self.forge_grant(rng, seconds);
                } else {
                    self.notice(
                        "forgeState",
                        "Forge time is credited, but saved furnace contents/progress are unavailable.",
                    );
                }
            }
            Grant::Pets => {
                self.pet_seconds += seconds;
                self.pet_pending_seconds += seconds;
                if !self.config.pets.available {
                    self.notice(
                        "petState",
                        "Pet time is credited, but deployed pets and Breeding progress are unavailable.",
                    );
                }
                self.notice(
                    "petNonShiny",
                    "Pet boosts also affect spices, breedability and special pets; only shiny-pet progress is resolved here.",
                );
            }
            Grant::Alchemy => {
                self.alchemy_seconds += seconds;
                self.notice(
                    "alchemyProduction",
                    "Alchemy time is credited; cauldron liquid/bubble production and upgrades are not simulated.",
                );
            }
        }
    }

    /// The reward engine's kill hook: the Coral credit, the Scurge EXP and the
    /// Omega ball roll, in the order `monsterKill` and its tail run them. The
    /// per-kill forge, pet and alchemy grants need the talent formulas and are
    /// reported as missing by the config decoder instead.
    pub fn kill(&mut self, rng: &mut Rng, grants: &Grants, kind: &str) {
        self.coral_kill(grants.kill_credit);
        if kind == "w7a6" {
            self.scurge_kill();
        }
        self.omega_kill(rng, grants);
    }

    /// `sim.progress.scurgeKill`: the Clamworks Scurge's EXP per kill.
    pub fn scurge_kill(&mut self) {
        if !self.config.scurge.unlocked {
            return;
        }
        let bonus = |menagerie: &[Vec<f64>], index: usize| {
            let upgrade = data().scurge_column.get(index + 9).copied().unwrap_or(0.0);
            upgrade
                * menagerie
                    .get(5)
                    .and_then(|row| row.get(index))
                    .copied()
                    .unwrap_or(0.0)
        };
        let menagerie = &self.config.scurge.menagerie;
        let add = (1.0 + bonus(menagerie, 0))
            * (1.0 + bonus(menagerie, 4) / 100.0)
            * (1.0 + bonus(menagerie, 2) / 100.0);
        let nearby = bonus(menagerie, 7) != 0.0;
        if let Some(exp) = self
            .config
            .scurge
            .menagerie
            .get_mut(0)
            .and_then(|row| row.get_mut(7))
        {
            *exp += add;
        }
        self.scurge_exp += add;
        if nearby {
            self.notice(
                "scurgeOtherPlayers",
                "Scurge EXP uses no nearby-player bonus unless Other players count is overridden.",
            );
        }
    }

    /// `sim.progress.coralKill`: the Coral Kid's banked KLA credit.
    pub fn coral_kill(&mut self, credit: f64) {
        if !self.config.coral.unlocked {
            return;
        }
        self.config.coral.kills += credit;
        self.coral_kills += credit;
    }

    /// `sim.progress.omegaKill`: the Omega Arcade ball roll. Returns whether a
    /// ball dropped; the item itself belongs to the reward engine.
    pub fn omega_kill(&mut self, rng: &mut Rng, grants: &Grants) -> bool {
        let omega = &mut self.config.omega;
        if !omega.unlocked
            || !omega.available
            || grants.map == 216
            || grants.scene.contains("Tutorial")
            || grants.scene.contains("wLavaAhole")
        {
            return false;
        }
        let chance =
            1.0 / (5_000.0 * (1.0 + 6.0 * omega.drops.min(8.0)) * 3.5f64.powf(omega.drops));
        if rng.float() >= grants.drop_rate * chance {
            return false;
        }
        omega.balls = (omega.balls + 1.0).round();
        omega.drops = (omega.drops + 1.0).round();
        self.omega_balls += 1.0;
        true
    }

    /// The 250 ms furnace task: `_forgeTick`.
    pub fn tick(&mut self, rng: &mut Rng) {
        if !self.config.forge.available {
            return;
        }
        for slot in 0..16 {
            let forge = &self.config.forge;
            let stats = stats(forge, slot);
            let mut progress = forge.progress_at(slot);
            if forge.slots > slot as f64 {
                if forge.ore(slot) != "Blank" {
                    if forge.qty_at(3 * slot) >= stats.cost {
                        progress += stats.speed;
                    }
                } else if progress > 0.0 {
                    progress = 0.0;
                }
            }
            self.config.forge.set_progress(slot, progress);
            if self.config.forge.progress_at(slot) >= interval(&stats) {
                self.produce(rng, slot, 1.0, false);
            }
        }
    }

    /// `_forgeGrant`: an AFK-style lump of forge seconds.
    fn forge_grant(&mut self, rng: &mut Rng, seconds: f64) {
        for slot in 0..16 {
            let forge = &self.config.forge;
            if forge.ore(slot) == "Blank" {
                continue;
            }
            let mut slot_stats = stats(forge, slot);
            let mut v = interval(&slot_stats) / (4.0 * slot_stats.speed);
            let q = if forge.qty_at(3 * slot + 1) != 0.0 && forge.oil(slot) != "Blank" {
                forge.qty_at(3 * slot + 1) * (1.0 / slot_stats.oil_cost)
            } else {
                0.0
            };
            let q2 = q * v;
            let mut count = (seconds * (1.0 / v))
                .min(forge.qty_at(3 * slot) / slot_stats.cost)
                .floor();
            let passes = if count < q || q == 0.0 { 1 } else { 2 };
            let progress = forge.progress_at(slot);
            self.config.forge.set_progress(
                slot,
                progress
                    + if passes == 1 {
                        4.0 * slot_stats.speed * seconds
                    } else {
                        4.0 * slot_stats.speed * q * v
                    },
            );
            if passes == 2 {
                count = q;
            }
            for pass in 0..passes {
                if count < 1.0 {
                    continue;
                }
                self.produce(rng, slot, count, true);
                if passes == 2 && pass == 0 {
                    slot_stats = stats(&self.config.forge, slot);
                    v = interval(&slot_stats) / (4.0 * slot_stats.speed);
                    count = ((seconds - q2) * (1.0 / v))
                        .min(self.config.forge.qty_at(3 * slot) / slot_stats.cost)
                        .floor();
                    let progress = self.config.forge.progress_at(slot);
                    self.config
                        .forge
                        .set_progress(slot, progress + 4.0 * slot_stats.speed * (seconds - q2));
                }
            }
        }
    }

    /// `_forgeProduce`: one slot pays its bars and spends its ore and oil.
    fn produce(&mut self, rng: &mut Rng, slot: usize, count: f64, afk: bool) {
        let forge = &self.config.forge;
        let at = 3 * slot;
        let stats = stats(forge, slot);
        let ore = forge.ore(slot).to_string();
        let oil = forge.oil(slot).to_string();
        let progress = forge.progress_at(slot);
        self.config
            .forge
            .set_progress(slot, progress - interval(&stats) * count);
        if self.config.forge.bar(slot) == "Blank" {
            self.config.forge.set_order(at + 2, format!("{ore}Bar"));
        }
        let amount = stats.bars * count;
        let previous = self.config.forge.qty_at(at + 2);
        let value = previous + amount;
        let rolled = if rng.float() > amount - amount.floor() {
            value.floor()
        } else {
            value.ceil()
        };
        self.config.forge.set_qty(at + 2, rolled);
        let bar = self.config.forge.bar(slot).to_string();
        let produced = self.config.forge.qty_at(at + 2) - previous;
        add(&mut self.bars, &bar, produced);
        let used = stats.cost.ceil().max(1.0) * count;
        self.config
            .forge
            .set_qty(at, self.config.forge.qty_at(at) - used);
        add(&mut self.ore_used, &ore, used);
        self.refill(slot, 0, afk, true);
        if oil != "Blank" {
            let before = self.config.forge.qty_at(at + 1);
            if afk {
                let use_ = count * stats.oil_cost;
                let value = if rng.float() > use_ - use_.floor() {
                    (before - use_).floor()
                } else {
                    (before - use_).ceil()
                };
                self.config.forge.set_qty(at + 1, value);
            } else if rng.between(0.0, 100.0) < stats.oil_cost * 100.0 {
                self.config.forge.set_qty(at + 1, before - 1.0);
            }
            let spent = before - self.config.forge.qty_at(at + 1);
            add(&mut self.oil_used, &oil, spent);
            self.refill(slot, 1, true, afk);
        }
    }

    /// `_refill`: pull replacement ore or oil from the saved chest.
    fn refill(&mut self, slot: usize, index: usize, empty_only: bool, reset_progress: bool) {
        let at = 3 * slot + index;
        let forge = &self.config.forge;
        let key = forge.order.get(at).cloned().unwrap_or_default();
        let qty = forge.qty_at(at);
        let cost = item(&key).map_or(0.0, |item| item.amount).ceil().max(1.0);
        if qty > 0.0 && (empty_only || qty > cost) {
            return;
        }
        if self.config.forge.refill && !self.config.forge.storage_available {
            self.notice(
                "forgeStorage",
                "Forge refill storage is unavailable; no replacement ore or oil is assumed.",
            );
        }
        let stored = self.config.forge.storage.get(&key).copied().unwrap_or(0.0);
        if self.config.forge.refill && stored > 0.0 {
            let added = self.config.forge.cap.min(stored);
            self.config.forge.set_qty(at, added);
            if let Some(cell) = self.config.forge.storage.get_mut(&key) {
                *cell -= added;
            }
            if reset_progress {
                self.config.forge.set_progress(slot, 0.0);
            }
            add(&mut self.storage_used, &key, added);
        } else if qty <= 0.0 {
            self.config.forge.set_qty(at, 0.0);
            self.config.forge.set_order(at, "Blank".to_string());
        }
    }

    /// The scene updater that spends banked pet time on deployed shiny pets.
    pub fn pet_batch(&mut self) {
        if self.pet_pending_seconds <= 10.0 || !self.config.pets.available {
            return;
        }
        let seconds = self.pet_pending_seconds;
        let speed = self.config.pets.shiny_speed;
        let thresholds = shiny_thresholds(&self.config.scurge.menagerie);
        for slot in 0..27 {
            let Some(pet) = self.config.pets.deployed.get(slot) else {
                continue;
            };
            let Some(kind) = pet.first() else { continue };
            if kind == "none" || pet.get(1).and_then(|level| level.parse::<f64>().ok()) != Some(5.0)
            {
                continue;
            }
            let Some((world, index)) = pet_position(kind) else {
                continue;
            };
            let row = self.config.pets.breeding.get_mut(22 + world);
            let Some(row) = row else {
                self.notice(
                    "shinyProgress",
                    "A deployed shiny pet has no saved progress row.",
                );
                break;
            };
            let Some(cell) = row.get_mut(index) else {
                continue;
            };
            let before = *cell;
            let add = seconds * speed / 86_400.0;
            *cell = before + add;
            let end = *cell;
            let level = shiny_pet_level(end, thresholds);
            if let Some(result) = self
                .pets
                .iter_mut()
                .find(|result| result.world == world && result.index == index)
            {
                result.gained += add;
                result.end = end;
                result.level = level;
            } else {
                self.pets.push(PetProgress {
                    world,
                    index,
                    name: pet_name(world, index),
                    start: before,
                    gained: add,
                    end,
                    start_level: shiny_pet_level(before, thresholds),
                    level,
                });
            }
            break;
        }
        self.pet_pending_seconds = 0.0;
    }

    /// The panel's rows; `combatProgressSummary`.
    pub fn summary(&self) -> Summary {
        let forge = &self.config.forge;
        let scurge = &self.config.scurge;
        Summary {
            forge_seconds: self.forge_seconds,
            pet_seconds: self.pet_seconds,
            alchemy_seconds: self.alchemy_seconds,
            scurge_exp: self.scurge_exp,
            coral_kills: self.coral_kills,
            omega_balls: self.omega_balls,
            bars: self.bars.clone(),
            ore_used: self.ore_used.clone(),
            oil_used: self.oil_used.clone(),
            storage_used: self.storage_used.clone(),
            pets: self.pets.clone(),
            notices: self.notices.clone(),
            pet_pending_seconds: self.pet_pending_seconds,
            forge_available: forge.available,
            forge: forge.available.then(|| ForgeState {
                order: forge.order.clone(),
                qty: forge.qty.clone(),
                progress: forge.progress.clone(),
                storage: forge.storage.clone(),
            }),
            scurge: scurge.unlocked.then(|| {
                let level = scurge
                    .menagerie
                    .first()
                    .and_then(|row| row.get(8))
                    .copied()
                    .unwrap_or(0.0);
                ScurgeState {
                    level,
                    exp: scurge
                        .menagerie
                        .first()
                        .and_then(|row| row.get(7))
                        .copied()
                        .unwrap_or(0.0),
                    requirement: (50.0 + 15.0 * level) * 1.08f64.powf(level),
                }
            }),
            coral: self
                .config
                .coral
                .unlocked
                .then_some(self.config.coral.clone()),
            omega: self
                .config
                .omega
                .unlocked
                .then_some(self.config.omega.clone()),
        }
    }
}

/// The kind of credit a grant pays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grant {
    Forge,
    Pets,
    Alchemy,
}

/// `menagerieUnlockOwned(11)` widens the shiny level curve.
fn shiny_thresholds(menagerie: &[Vec<f64>]) -> usize {
    let unlock = menagerie
        .get(2)
        .and_then(|row| row.get(11))
        .copied()
        .unwrap_or(0.0);
    (19.0 + if unlock >= 1.0 { 10.0 } else { 0.0 }) as usize
}

/// `shinyPetLevel`: the level a shiny EXP value reaches on the game's curve.
pub fn shiny_pet_level(exp: f64, thresholds: usize) -> f64 {
    if exp <= 0.0 {
        return 0.0;
    }
    let mut level = 1.0;
    for step in 0..thresholds {
        let e = step as f64;
        if exp > ((1.0 + (e + 1.0).powf(1.6)) * 1.7f64.powf(e + 1.0)).floor() {
            level = e + 2.0;
        }
    }
    level
}

/// The world and pet index of one `Pets` key.
fn pet_position(kind: &str) -> Option<(usize, usize)> {
    data()
        .pet_keys
        .iter()
        .enumerate()
        .find_map(|(world, pets)| {
            pets.iter()
                .position(|key| key == kind)
                .map(|index| (world, index))
        })
}
