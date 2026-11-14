//! The All Stats Dashboard's category breakdowns.
//!
//! Ported from the descriptor registry in `js/stats/registry.js` and the
//! category tabs in `stats-all.html`: every upstream descriptor keeps its id,
//! name, scope and category, and the tabs read the upstream `CATEGORY_ORDER`
//! and `CATEGORY_LABELS` tables. A descriptor whose formula this build does
//! not port resolves to [`Resolution::Unavailable`] with the upstream source
//! named, so a category renders every row instead of hiding the gaps.
//!
//! The ported formulas live beside this module: [`capacity`] carries the eight
//! carry-capacity descriptors (`js/stats/defs/carry-capacity.js` and
//! `js/stats/systems/common/carry-capacity.js`), [`research`] the two research
//! descriptors the simulator already computes.

pub(crate) mod capacity;
mod research;

use idleon_common::formula::FormulaNode;
use idleon_research::model::{SimContext, Simulation};
use idleon_save::Save;

pub use capacity::CapacityKind;

/// A breakdown category, in the upstream `CATEGORY_ORDER`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Combat,
    Stat,
    Multiplier,
    Rate,
    Capacity,
    Construction,
    Research,
    Currency,
    Progression,
}

impl Category {
    /// Every category, in the order the dashboard's tabs show them.
    pub const ALL: [Category; 9] = [
        Category::Combat,
        Category::Stat,
        Category::Multiplier,
        Category::Rate,
        Category::Capacity,
        Category::Construction,
        Category::Research,
        Category::Currency,
        Category::Progression,
    ];

    /// The upstream category key.
    pub fn id(self) -> &'static str {
        match self {
            Category::Combat => "combat",
            Category::Stat => "stat",
            Category::Multiplier => "multiplier",
            Category::Rate => "rate",
            Category::Capacity => "capacity",
            Category::Construction => "construction",
            Category::Research => "research",
            Category::Currency => "currency",
            Category::Progression => "progression",
        }
    }

    /// The upstream tab label.
    pub fn label(self) -> &'static str {
        match self {
            Category::Combat => "Combat",
            Category::Stat => "Stats",
            Category::Multiplier => "Multipliers",
            Category::Rate => "Rates",
            Category::Capacity => "Capacity",
            Category::Construction => "Construction",
            Category::Research => "Research",
            Category::Currency => "Currency",
            Category::Progression => "Progression",
        }
    }

    /// The category an id names, or `None` for an id this build does not know.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|category| category.id() == id)
    }
}

/// What a descriptor resolves against: upstream's `scope`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Character,
    CharacterMap,
    CharacterMapSkill,
    Account,
}

impl Scope {
    /// The badge text upstream's `_scopeInfo` uses.
    pub fn label(self) -> &'static str {
        match self {
            Scope::Character => "Character",
            Scope::CharacterMap => "Char + Map",
            Scope::CharacterMapSkill => "Char + Map + Skill",
            Scope::Account => "Account",
        }
    }
}

/// One descriptor's result: its value and formula tree, or why it has none.
#[derive(Clone, Debug, PartialEq)]
pub enum Resolution {
    /// The ported formula's value and its formula tree.
    Value {
        tree: FormulaNode,
        /// The upstream `partial` reason, when the total leaves a system out.
        partial: Option<String>,
    },
    /// The descriptor's formula is not ported; the reason names the source.
    Unavailable {
        /// Why no value is shown.
        reason: String,
    },
}

impl Resolution {
    /// The descriptor's value, when it has one.
    pub fn value(&self) -> Option<f64> {
        match self {
            Resolution::Value { tree, .. } => Some(tree.value),
            Resolution::Unavailable { .. } => None,
        }
    }

    /// The formula tree, when the descriptor has one.
    pub fn tree(&self) -> Option<&FormulaNode> {
        match self {
            Resolution::Value { tree, .. } => Some(tree),
            Resolution::Unavailable { .. } => None,
        }
    }
}

/// The repository every ported definition is read from.
pub const UPSTREAM: &str = "https://github.com/Corgan/idleon-research-optimizer/blob/main/";

/// One row of the dashboard: an upstream descriptor and its result.
#[derive(Clone, Debug, PartialEq)]
pub struct Breakdown {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub scope: Scope,
    /// The upstream definition this row ports, such as `js/stats/defs/damage.js`.
    pub source: &'static str,
    pub resolution: Resolution,
}

impl Breakdown {
    /// Where the row's upstream definition lives: the repository the port
    /// reads, at the path the row names.
    pub fn source_url(&self) -> String {
        format!("{UPSTREAM}{}", self.source)
    }

    /// Whether `query` names this row, its category, its upstream source or one
    /// of its formula terms, case-insensitively.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return false;
        }
        [self.id, self.name, self.source, self.scope.label()]
            .into_iter()
            .chain(std::iter::once(self.category.label()))
            .any(|field| field.to_lowercase().contains(&query))
            || self
                .resolution
                .tree()
                .is_some_and(|tree| tree.match_count(&query) > 0)
    }

    /// The value as the row shows it, when it has one.
    pub fn display_value(&self) -> Option<String> {
        self.resolution
            .tree()
            .map(|tree| tree.format.format(tree.value))
    }

    /// Why the row shows a warning instead of a value.
    pub fn warning(&self) -> Option<&str> {
        match &self.resolution {
            Resolution::Value { partial, .. } => partial.as_deref(),
            Resolution::Unavailable { reason } => Some(reason),
        }
    }
}

/// The save slice a sheet resolves against.
pub struct Inputs<'a> {
    pub save: &'a Save,
    pub simulation: &'a Simulation,
    pub sim_context: &'a SimContext,
    /// The context's character, as its save row index.
    pub character: usize,
    /// The context's map, as its game index.
    pub map: usize,
}

#[cfg(test)]
impl<'a> Inputs<'a> {
    /// Character 0 on map 0, the context the unit tests resolve against.
    fn for_test(save: &'a Save, simulation: &'a Simulation, sim_context: &'a SimContext) -> Self {
        Self {
            save,
            simulation,
            sim_context,
            character: 0,
            map: 0,
        }
    }
}

/// The dashboard's descriptors, one per upstream row, in registry order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sheet {
    descriptors: Vec<Breakdown>,
}

impl Sheet {
    /// Resolve every upstream descriptor against the save and the context.
    pub fn build(inputs: &Inputs<'_>) -> Self {
        let descriptors = SPECS
            .iter()
            .map(|spec| {
                let resolution = capacity::resolve(inputs, spec.id)
                    .or_else(|| research::resolve(inputs, spec.id))
                    .unwrap_or_else(|| Resolution::Unavailable {
                        reason: format!(
                            "{} is not modelled yet: it needs the shared stat pipeline \
                             (talents, stamps, bubbles, cards and equipment), which this build \
                             does not decode.",
                            spec.source
                        ),
                    });
                Breakdown {
                    id: spec.id,
                    name: spec.name,
                    category: spec.category,
                    scope: spec.scope,
                    source: spec.source,
                    resolution,
                }
            })
            .collect();
        Self { descriptors }
    }

    /// Every descriptor, in registry order.
    pub fn descriptors(&self) -> &[Breakdown] {
        &self.descriptors
    }

    /// The descriptors of one category, in registry order.
    pub fn in_category(&self, category: Category) -> impl Iterator<Item = &Breakdown> {
        self.descriptors
            .iter()
            .filter(move |row| row.category == category)
    }

    /// One descriptor by its upstream id.
    pub fn get(&self, id: &str) -> Option<&Breakdown> {
        self.descriptors.iter().find(|row| row.id == id)
    }

    /// How many descriptors a category holds.
    pub fn count(&self, category: Category) -> usize {
        self.in_category(category).count()
    }

    /// How many rows are computed rather than warned about.
    pub fn resolved(&self) -> usize {
        self.descriptors
            .iter()
            .filter(|row| matches!(row.resolution, Resolution::Value { .. }))
            .count()
    }
}

/// A descriptor's fixed metadata, as the upstream registry declares it.
struct Spec {
    id: &'static str,
    name: &'static str,
    category: Category,
    scope: Scope,
    source: &'static str,
}

const fn spec(
    id: &'static str,
    name: &'static str,
    category: Category,
    scope: Scope,
    source: &'static str,
) -> Spec {
    Spec {
        id,
        name,
        category,
        scope,
        source,
    }
}

/// Every descriptor `allDescriptors()` returns, in its own order.
const SPECS: [Spec; 60] = [
    spec(
        "drop-rate",
        "Drop Rate",
        Category::Combat,
        Scope::CharacterMap,
        "js/stats/defs/drop-rate.js",
    ),
    spec(
        "voting-multi",
        "Voting Multiplier",
        Category::Multiplier,
        Scope::Account,
        "js/stats/defs/voting-multi.js",
    ),
    spec(
        "gfood-multi",
        "Golden Food Multi",
        Category::Multiplier,
        Scope::Character,
        "js/stats/defs/gfood-multi.js",
    ),
    spec(
        "research-exp",
        "Research EXP Multiplier",
        Category::Research,
        Scope::Account,
        "js/stats/defs/research-exp.js",
    ),
    spec(
        "research-afk-gains",
        "Research AFK Gains Rate",
        Category::Research,
        Scope::Account,
        "js/stats/defs/research-afk-gains.js",
    ),
    spec(
        "sushi-bucks",
        "Sushi Bucks/hr",
        Category::Currency,
        Scope::Account,
        "js/stats/defs/sushi-bucks.js",
    ),
    spec(
        "minehead-currency",
        "Minehead Currency/hr",
        Category::Currency,
        Scope::Account,
        "js/stats/defs/minehead-currency.js",
    ),
    spec(
        "coin-multi",
        "Coin Multiplier",
        Category::Multiplier,
        Scope::CharacterMap,
        "js/stats/defs/coin-multi.js",
    ),
    spec(
        "monster-exp",
        "Monster EXP Multiplier",
        Category::Multiplier,
        Scope::CharacterMap,
        "js/stats/defs/monster-exp.js",
    ),
    spec(
        "kill-per-kill",
        "Kill Per Kill (Multikill)",
        Category::Multiplier,
        Scope::CharacterMap,
        "js/stats/defs/kill-per-kill.js",
    ),
    spec(
        "fighting-afk",
        "Fighting AFK Rate",
        Category::Rate,
        Scope::CharacterMap,
        "js/stats/defs/fighting-afk.js",
    ),
    spec(
        "skill-afk",
        "Skill AFK Rate",
        Category::Rate,
        Scope::CharacterMapSkill,
        "js/stats/defs/skill-afk.js",
    ),
    spec(
        "damage",
        "Damage (Normal)",
        Category::Stat,
        Scope::CharacterMap,
        "js/stats/defs/damage.js",
    ),
    spec(
        "spelunking-pow",
        "Spelunking Power",
        Category::Stat,
        Scope::Account,
        "js/stats/defs/spelunking-pow.js",
    ),
    spec(
        "spelunking-max-stamina",
        "Spelunking Max Stamina",
        Category::Stat,
        Scope::Character,
        "js/stats/defs/spelunking-max-stamina.js",
    ),
    spec(
        "amber-supply-swap",
        "Ambery Supply Swap: Expected Primary Amber Yield",
        Category::Stat,
        Scope::Account,
        "js/stats/defs/amber-supply-swap.js",
    ),
    spec(
        "skill-efficiency",
        "Skill Efficiency",
        Category::Stat,
        Scope::CharacterMapSkill,
        "js/stats/defs/skill-efficiency.js",
    ),
    spec(
        "skill-exp",
        "Skill EXP Multiplier",
        Category::Multiplier,
        Scope::CharacterMapSkill,
        "js/stats/defs/skill-exp.js",
    ),
    spec(
        "tome-score",
        "Tome Score",
        Category::Progression,
        Scope::Account,
        "js/stats/defs/tome-score.js",
    ),
    spec(
        "build-rate",
        "Player Build Speed/hr",
        Category::Construction,
        Scope::Character,
        "js/stats/defs/build-rate.js",
    ),
    spec(
        "construction-exp",
        "Player Construction EXP/hr",
        Category::Construction,
        Scope::Character,
        "js/stats/defs/construction-exp.js",
    ),
    spec(
        "flaggy-speed",
        "Flaggy Rate Multiplier",
        Category::Construction,
        Scope::Account,
        "js/stats/defs/flaggy-speed.js",
    ),
    spec(
        "total-str",
        "Total STR",
        Category::Stat,
        Scope::Character,
        "js/stats/defs/base-stats.js",
    ),
    spec(
        "total-agi",
        "Total AGI",
        Category::Stat,
        Scope::Character,
        "js/stats/defs/base-stats.js",
    ),
    spec(
        "total-wis",
        "Total WIS",
        Category::Stat,
        Scope::Character,
        "js/stats/defs/base-stats.js",
    ),
    spec(
        "total-luk",
        "Total LUK",
        Category::Stat,
        Scope::Character,
        "js/stats/defs/base-stats.js",
    ),
    spec(
        "crit-chance",
        "Crit Chance",
        Category::Stat,
        Scope::CharacterMap,
        "js/stats/defs/crit-chance.js",
    ),
    spec(
        "crit-damage",
        "Crit Damage",
        Category::Stat,
        Scope::Character,
        "js/stats/defs/crit-damage.js",
    ),
    spec(
        "max-hp",
        "Max HP",
        Category::Stat,
        Scope::CharacterMap,
        "js/stats/defs/max-hp.js",
    ),
    spec(
        "max-mp",
        "Max MP",
        Category::Stat,
        Scope::Character,
        "js/stats/defs/max-mp.js",
    ),
    spec(
        "accuracy",
        "Accuracy",
        Category::Combat,
        Scope::CharacterMap,
        "js/stats/defs/accuracy.js",
    ),
    spec(
        "defence",
        "Defence",
        Category::Combat,
        Scope::Character,
        "js/stats/defs/defence.js",
    ),
    spec(
        "movement-speed",
        "Movement Speed",
        Category::Combat,
        Scope::Character,
        "js/stats/defs/movement-speed.js",
    ),
    spec(
        "giant-mob",
        "Giant Mob Odds",
        Category::Combat,
        Scope::CharacterMap,
        "js/stats/defs/giant-mob.js",
    ),
    spec(
        "crystal-spawn",
        "Crystal Mob Chance",
        Category::Combat,
        Scope::CharacterMap,
        "js/stats/defs/crystal-spawn.js",
    ),
    spec(
        "hit-chance",
        "Hit Chance",
        Category::Combat,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "accuracy-required",
        "Accuracy Required (100% Hit)",
        Category::Combat,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "overkill-tier",
        "Overkill Tier",
        Category::Combat,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "multikill-base",
        "Multikill Base",
        Category::Combat,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "multikill-per-tier",
        "Multikill Per Tier",
        Category::Combat,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "multikill-total",
        "Multikill Total",
        Category::Combat,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "divinity-points-per-hour",
        "Divinity Points/hr",
        Category::Rate,
        Scope::Character,
        "js/stats/defs/divinity-rates.js",
    ),
    spec(
        "divinity-exp-per-hour",
        "Divinity EXP/hr",
        Category::Rate,
        Scope::Character,
        "js/stats/defs/divinity-rates.js",
    ),
    spec(
        "raw-combat-kills-per-hour",
        "Raw Combat Kills/hr",
        Category::Rate,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "printer-sample-size",
        "Printer Sample Size",
        Category::Progression,
        Scope::Character,
        "js/stats/defs/utility-stats.js",
    ),
    spec(
        "card-drop-chance-multiplier",
        "Card Drop Chance Multiplier",
        Category::Multiplier,
        Scope::Character,
        "js/stats/defs/utility-stats.js",
    ),
    spec(
        "combat-survivability",
        "Combat Survivability",
        Category::Combat,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "reward-roll-kills-per-hour",
        "Reward-Roll Kills/hr",
        Category::Rate,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "credited-kills-per-hour",
        "Credited Kills/hr",
        Category::Rate,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "class-exp-per-hour",
        "Class EXP/hr",
        Category::Rate,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "panel-coins-per-hour",
        "Panel Coins/hr",
        Category::Rate,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "direct-card-average-time",
        "Direct Card Average Time",
        Category::Rate,
        Scope::CharacterMap,
        "js/stats/defs/combat-outcomes.js",
    ),
    spec(
        "carry-capacity-mining",
        "Mining Carry Capacity",
        Category::Capacity,
        Scope::CharacterMap,
        "js/stats/defs/carry-capacity.js",
    ),
    spec(
        "carry-capacity-chopping",
        "Chopping Carry Capacity",
        Category::Capacity,
        Scope::CharacterMap,
        "js/stats/defs/carry-capacity.js",
    ),
    spec(
        "carry-capacity-fishing",
        "Fishing Carry Capacity",
        Category::Capacity,
        Scope::CharacterMap,
        "js/stats/defs/carry-capacity.js",
    ),
    spec(
        "carry-capacity-catching",
        "Catching Carry Capacity",
        Category::Capacity,
        Scope::CharacterMap,
        "js/stats/defs/carry-capacity.js",
    ),
    spec(
        "carry-capacity-food",
        "Food Carry Capacity",
        Category::Capacity,
        Scope::CharacterMap,
        "js/stats/defs/carry-capacity.js",
    ),
    spec(
        "carry-capacity-critters",
        "Critter Carry Capacity",
        Category::Capacity,
        Scope::CharacterMap,
        "js/stats/defs/carry-capacity.js",
    ),
    spec(
        "carry-capacity-souls",
        "Soul Carry Capacity",
        Category::Capacity,
        Scope::CharacterMap,
        "js/stats/defs/carry-capacity.js",
    ),
    spec(
        "carry-capacity-materials",
        "Material Carry Capacity",
        Category::Capacity,
        Scope::CharacterMap,
        "js/stats/defs/carry-capacity.js",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sheet() -> Sheet {
        let save = Save::new(json!({
            "charNames": ["Ana"],
            "SL_0": "{\"634\":100,\"78\":100}",
            "MaxCarryCap_0": "{\"Mining\":35000,\"Chopping\":30000,\"Fishing\":30000,\"Bugs\":30000,\"Foods\":30000,\"Critters\":30000,\"Souls\":30000,\"bCraft\":30000}",
            "UpgVault": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            "BundlesReceived": "{\"bon_w\":1}",
            "GemItemsPurchased": "[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,4]",
            "Guild": "[[0,0,50]]",
            "BribeStatus": "[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1]",
            "StampLv": "[{},{\"3\":100},{}]",
            "PVtStarSign_0": "11,",
        }));
        let simulation = Simulation::default();
        let context = SimContext::default();
        Sheet::build(&Inputs::for_test(&save, &simulation, &context))
    }

    #[test]
    fn the_sheet_carries_every_upstream_descriptor_once() {
        let sheet = sheet();
        assert_eq!(sheet.descriptors().len(), 60);
        let mut ids: Vec<&str> = sheet.descriptors().iter().map(|row| row.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 60, "descriptor ids must be unique");
        assert!(
            sheet
                .descriptors()
                .iter()
                .all(|row| row.source.ends_with(".js"))
        );
    }

    #[test]
    fn every_upstream_category_holds_at_least_one_descriptor() {
        let sheet = sheet();
        for category in Category::ALL {
            assert!(
                sheet.count(category) > 0,
                "{} holds no descriptor",
                category.label()
            );
        }
        assert_eq!(
            Category::from_id("progression"),
            Some(Category::Progression)
        );
        assert_eq!(Category::from_id("frogs"), None);
    }

    #[test]
    fn a_category_with_no_ported_formula_still_warns_on_every_row() {
        let sheet = sheet();
        let combat: Vec<&Breakdown> = sheet.in_category(Category::Combat).collect();
        assert!(!combat.is_empty());
        for row in combat {
            let warning = row.warning().expect("combat rows warn in this build");
            assert!(warning.contains(row.source), "{warning}");
            assert!(row.display_value().is_none());
        }
        assert!(
            sheet
                .in_category(Category::Capacity)
                .all(|row| { matches!(row.resolution, Resolution::Value { .. }) })
        );
    }

    #[test]
    fn every_ported_value_agrees_with_its_formula_tree() {
        let sheet = sheet();
        for row in sheet.descriptors() {
            if let Some(tree) = row.resolution.tree() {
                assert_eq!(
                    row.resolution.value(),
                    Some(tree.value),
                    "{} disagrees with its tree",
                    row.id
                );
            }
        }
        assert!(sheet.resolved() >= sheet.count(Category::Capacity));
    }

    #[test]
    fn a_missing_carry_row_is_reported_not_guessed() {
        let save = Save::new(json!({ "charNames": ["Ana"] }));
        let simulation = Simulation::default();
        let context = SimContext::default();
        let sheet = Sheet::build(&Inputs::for_test(&save, &simulation, &context));
        let capacity = sheet.get("carry-capacity-mining").expect("the row exists");
        let warning = capacity.warning().expect("the missing row is named");
        assert!(warning.contains("MaxCarryCap_0"), "{warning}");
        assert!(capacity.display_value().is_none());
    }
}
