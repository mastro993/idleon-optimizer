//! The global breakdown tree: the account-wide rules every outpost, node and
//! unit formula multiplies, as expandable nodes.
//!
//! Ported from `globalBreakdownTree` in `royal-guardian-calc.html` and the
//! upstream breakdown functions it calls: `globalResourceRateBreakdown`,
//! `globalRangeBreakdown`, `globalRankExpBreakdown`, `barExpRateBase`,
//! `globalPointBreakdown`, `globalUnitBreakdowns`, `globalSupportBreakdown` and
//! `globalSavageBreakdown`.
//!
//! Each category returns the same tree the page draws, built out of
//! [`FormulaNode`]s so the shared formula tree renders it. A heading whose value
//! is the sum of its children says so: the group nodes add their terms, and the
//! tests check every sum against the number the section reports.

use super::*;
use idleon_common::formula::{FormulaNode, ValueFormat};

/// The global Range rules: upstream `globalRangeBreakdown`.
const BASE_RANGE: f64 = 80.0;
const MAX_RANGE: f64 = 999.0;
const OUTPOST_ALLOWANCE: f64 = 8.0;

/// Which global rule the breakdown tab shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    /// The kingdom-wide collection multiplier.
    #[default]
    Collection,
    /// The base range every outpost starts from.
    Range,
    /// EXP per contributor, one row per rank bar.
    RankExp,
    /// The outpost points a map can earn.
    Points,
    /// Each unit type's effect.
    Units,
    /// What one Support link adds.
    Support,
    /// What a Savage outpost multiplies its drain by.
    Savage,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Category::Collection,
        Category::Range,
        Category::RankExp,
        Category::Points,
        Category::Units,
        Category::Support,
        Category::Savage,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Category::Collection => "collection",
            Category::Range => "range",
            Category::RankExp => "rank-exp",
            Category::Points => "points",
            Category::Units => "units",
            Category::Support => "support",
            Category::Savage => "savage",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Collection => "Collection",
            Category::Range => "Range",
            Category::RankExp => "Rank EXP",
            Category::Points => "Points",
            Category::Units => "Units",
            Category::Support => "Support",
            Category::Savage => "Savage",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|category| category.id() == id)
    }
}

/// One global breakdown: its tree, the number it reports and what the save is
/// missing for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Breakdown {
    pub category: Category,
    pub root: FormulaNode,
    pub value: f64,
    pub partial: bool,
    pub missing: Vec<String>,
}

/// The rank bars the game tracks, in save order.
pub const RANK_BARS: [&str; 5] = [
    "Trading Rank",
    "Intel Rank",
    "Command Rank",
    "Military Rank",
    "Purity Rank",
];

/// The unit that feeds each rank bar: upstream `outpostRankContributor`.
pub const RANK_CONTRIBUTORS: [&str; 5] = ["Trader", "Surveyor", "Commander", "Knight", "Priest"];

/// One unit rule's static half: its name, stat label, base effect and the
/// Armory row that scales it.
const UNIT_RULES: [(&str, &str, f64, usize); 8] = [
    ("Worker", "Collection", 50.0, 19),
    ("Trader", "Trading EXP", 0.0, 20),
    ("Guard", "Range", 25.0, 21),
    ("Surveyor", "Intel EXP", 0.0, 22),
    ("Militia", "Clearing", 4_000.0, 23),
    ("Commander", "Command EXP", 0.0, 24),
    ("Knight", "Military EXP", 0.0, 25),
    ("Priest", "Purity EXP", 0.0, 26),
];

impl RoyalGuardian {
    /// The global breakdown for one category: upstream `globalBreakdownTree`.
    pub fn global_breakdown(&self, category: Category) -> Breakdown {
        match category {
            Category::Collection => self.collection_breakdown(),
            Category::Range => self.range_breakdown(),
            Category::RankExp => self.rank_exp_breakdown(),
            Category::Points => self.points_breakdown(),
            Category::Units => self.units_breakdown(),
            Category::Support => self.support_breakdown(),
            Category::Savage => self.savage_breakdown(),
        }
    }

    /// One rank bar's EXP per contributor: upstream `barExpRateBase`.
    pub fn bar_exp_rate_base(&self, bar: usize) -> f64 {
        let bar = bar.min(4);
        let mut value = (1.0 + self.orblet_bonus(6) / 100.0) * (1.0 + self.jelly_bonus(1) / 100.0);
        if bar == 4 {
            value *= 1.0 + self.shop_bonus(65) / 100.0;
        }
        if bar as f64 == self.g_cell(row::BANKED, row::SELECTED_BAR_CELL) {
            value *= 1.0 + self.shop_bonus(77) / 100.0;
        }
        value * (1.0 + self.unit_spec([1, 3, 5, 6, 7][bar]) / 100.0)
    }

    /// A Spelunk shop row's bonus: upstream `shopUpgBonus`, narrowed to the two
    /// rank-EXP rows the section reads. Neither row takes one of the source's
    /// per-index multipliers, so the plain level product is exact here.
    pub fn shop_bonus(&self, index: usize) -> f64 {
        data::spelunk_shop_per_level(index) * self.spelunk_cell(5, index).max(0.0)
    }

    /// The global range rules: base, Orblet bonus, cap, resource allowance and
    /// outpost allowance.
    pub fn global_range(&self) -> (f64, f64, f64, f64, f64) {
        (
            BASE_RANGE,
            self.orblet_bonus(8),
            MAX_RANGE,
            RESOURCE_ALLOWANCE,
            OUTPOST_ALLOWANCE,
        )
    }

    /// An Armory row's effect as a subtree: saved level, per-level bonus and the
    /// current effect.
    fn armory_node(&self, index: usize, label: &str, value: f64) -> FormulaNode {
        FormulaNode::leaf(label, value, ValueFormat::Percent)
            .with_source(format!("RoyalG[2][{index}]"))
            .with_children(vec![
                FormulaNode::leaf("Saved level", self.armory_level(index), ValueFormat::Raw)
                    .with_source(format!("RoyalG[2][{index}]")),
                FormulaNode::leaf(
                    "Bonus per level",
                    data::armory(index)
                        .map(|row| row.bonus_per_level)
                        .unwrap_or(0.0),
                    ValueFormat::Raw,
                ),
                FormulaNode::leaf("Current bonus", value, ValueFormat::Percent),
            ])
    }

    fn collection_breakdown(&self) -> Breakdown {
        let inputs = self.collection_inputs();
        let rate = self.global_collection_rate_with(&inputs);
        let totals = self.total_stats();
        let orblet = self.orblet_bonus(1);
        let alchemy = inputs.alchemy_w14.min(50.0);
        let additive =
            orblet + inputs.sushi_60 + alchemy + inputs.arcade_70 + self.armory_bonus(70);
        let hundreds = ((inputs.char_level - 1_000.0).max(0.0) / 100.0).floor();
        let mut factors: Vec<FormulaNode> = rate
            .factors
            .iter()
            .map(|factor| FormulaNode::leaf(factor.label, factor.value, ValueFormat::Multiplier))
            .collect();
        // The additive factor is the one the upstream tree expands: its five
        // terms sum to the amount the factor divides by a hundred.
        factors[1].children = vec![
            FormulaNode::leaf("Orblet collection", orblet, ValueFormat::Percent)
                .with_source("RoyalG[23][1]"),
            FormulaNode::leaf("Sushi", inputs.sushi_60, ValueFormat::Percent)
                .with_note("Sixty unique sushi"),
            FormulaNode::leaf("Alchemy bubble", alchemy, ValueFormat::Percent)
                .with_note("W14, capped at 50"),
            FormulaNode::leaf("Arcade", inputs.arcade_70, ValueFormat::Percent)
                .with_source("ArcadeUpg[70]"),
            FormulaNode::leaf("Armory", self.armory_bonus(70), ValueFormat::Percent)
                .with_source("RoyalG[2][70]"),
        ];
        factors[1].input = Some(format!("sum {additive}%"));
        // The grade, level, purity and currency-log factors name the counts they
        // multiply, so a reader can follow each one back to the save.
        factors[2].input = Some(format!("{} grades", totals.grades));
        factors[3].input = Some(format!("{} upgrade levels", totals.levels));
        factors[4].input = Some(format!("{} purified outposts", totals.purified));
        factors[5].input = Some(format!("{hundreds} hundreds"));
        factors[8].children = (0..WORLDS + 1)
            .map(|world| {
                let balance = self.g_cell(row::CURRENCIES, 10 * world);
                FormulaNode::leaf(
                    format!("W{} currency log", world + 1),
                    idleon_common::curve::log(balance),
                    ValueFormat::Raw,
                )
                .with_source(format!("RoyalG[1][{}]", 10 * world))
                .with_input(format!("balance {balance}"))
            })
            .collect();
        let root = FormulaNode::new("Global Collection Multiplier", rate.value)
            .with_format(ValueFormat::Multiplier)
            .with_children(factors);
        Breakdown {
            category: Category::Collection,
            root,
            value: rate.value,
            partial: rate.partial,
            missing: rate.missing,
        }
    }

    fn range_breakdown(&self) -> Breakdown {
        let (base, orblet, cap, resource, outpost) = self.global_range();
        let value = base + orblet;
        let root = FormulaNode::new("Global Range Rules", value)
            .with_note("The heading sums the base range and the Orblet range")
            .with_children(vec![
                FormulaNode::leaf("Base range", base, ValueFormat::Raw),
                self.armory_like_orblet(8, "Orblet range", orblet),
                FormulaNode::leaf("Maximum range", cap, ValueFormat::Raw),
                FormulaNode::leaf("Resource-link allowance", resource, ValueFormat::Raw),
                FormulaNode::leaf("Outpost-link allowance", outpost, ValueFormat::Raw),
            ]);
        Breakdown {
            category: Category::Range,
            root,
            value,
            partial: false,
            missing: Vec::new(),
        }
    }

    /// An Orblet row's effect as a subtree, the way upstream `effectDetailChildren`
    /// expands it.
    fn armory_like_orblet(&self, index: usize, label: &str, value: f64) -> FormulaNode {
        FormulaNode::leaf(label, value, ValueFormat::Raw)
            .with_source(format!("RoyalG[23][{index}]"))
            .with_children(vec![
                FormulaNode::leaf("Saved level", self.orblet_level(index), ValueFormat::Raw)
                    .with_source(format!("RoyalG[23][{index}]")),
                FormulaNode::leaf(
                    "Bonus per level",
                    data::data()
                        .orblet_bonus_per_level
                        .get(index)
                        .copied()
                        .unwrap_or(0.0),
                    ValueFormat::Raw,
                ),
                FormulaNode::leaf("Current bonus", value, ValueFormat::Raw).with_source("Floored"),
            ])
    }

    fn rank_exp_breakdown(&self) -> Breakdown {
        let orblet = self.orblet_bonus(6);
        let shop_65 = self.shop_bonus(65);
        let shop_77 = self.shop_bonus(77);
        let selected = (self.g_cell(row::BANKED, row::SELECTED_BAR_CELL).max(0.0) as usize).min(4);
        let bars: Vec<FormulaNode> = (0..5)
            .map(|bar| {
                let base = self.bar_exp_rate_base(bar);
                let mut children = vec![
                    FormulaNode::leaf(
                        "Orblet EXP multiplier",
                        1.0 + orblet / 100.0,
                        ValueFormat::Multiplier,
                    )
                    .with_source("RoyalG[23][6]"),
                    FormulaNode::leaf(
                        "Jelly reward multiplier",
                        1.0 + self.jelly_bonus(1) / 100.0,
                        ValueFormat::Multiplier,
                    ),
                ];
                if bar == 4 {
                    children.push(
                        FormulaNode::leaf("Purity Shop bonus", shop_65, ValueFormat::Percent)
                            .with_source("Spelunk[5][65]"),
                    );
                }
                if bar == selected {
                    children.push(
                        FormulaNode::leaf("Selected-bar Shop bonus", shop_77, ValueFormat::Percent)
                            .with_source("Spelunk[5][77]"),
                    );
                }
                children.push(
                    FormulaNode::leaf(
                        format!("{} effect", RANK_CONTRIBUTORS[bar]),
                        self.unit_spec([1, 3, 5, 6, 7][bar]),
                        ValueFormat::Percent,
                    )
                    .with_source("Royal armory unit bonus"),
                );
                FormulaNode::leaf(
                    format!("{} EXP per contributor", RANK_BARS[bar]),
                    base,
                    ValueFormat::Rate,
                )
                .with_children(children)
            })
            .collect();
        let value = self.bar_exp_rate_base(selected);
        let root = FormulaNode::new(
            format!("{} EXP per contributor", RANK_BARS[selected]),
            value,
        )
        .with_format(ValueFormat::Rate)
        .with_note("Each bar multiplies its shop and unit bonuses into the base rate")
        .with_children(bars);
        Breakdown {
            category: Category::RankExp,
            root,
            value,
            partial: false,
            missing: Vec::new(),
        }
    }

    fn points_breakdown(&self) -> Breakdown {
        let base = 2.0;
        let worlds: Vec<FormulaNode> = (0..WORLDS)
            .map(|world| {
                let index = 9 + world;
                let bonus = self.armory_bonus(index);
                self.armory_node(
                    index,
                    format!("W{} Armory bonus", world + 1).as_str(),
                    bonus,
                )
            })
            .collect();
        let bonus: f64 = worlds.iter().map(|node| node.value).sum();
        let value = base + bonus;
        let root = FormulaNode::new("Global Outpost Point Rules", value)
            .with_note("Every map starts with the base and its world's Armory bonus")
            .with_children(vec![
                FormulaNode::leaf("Base points", base, ValueFormat::Raw),
                FormulaNode::leaf("Per-world bonuses", bonus, ValueFormat::Raw)
                    .with_children(worlds),
            ]);
        Breakdown {
            category: Category::Points,
            root,
            value,
            partial: false,
            missing: Vec::new(),
        }
    }

    fn units_breakdown(&self) -> Breakdown {
        let rules: Vec<FormulaNode> = (0..UNIT_RULES.len())
            .map(|unit| {
                let (name, label, base, armory_index) = UNIT_RULES[unit];
                let bonus = self.armory_bonus(armory_index);
                let value = self.unit_spec(unit);
                let format = if unit == 4 {
                    ValueFormat::Rate
                } else {
                    ValueFormat::Raw
                };
                FormulaNode::leaf(format!("{name}: {label}"), value, format).with_children(vec![
                    FormulaNode::leaf("Base effect", base, ValueFormat::Raw),
                    self.armory_node(armory_index, "Armory effect", bonus),
                ])
            })
            .collect();
        let value: f64 = rules.iter().map(|rule| rule.value).sum();
        let root = FormulaNode::new("Unit effects", value)
            .with_note("The heading sums every unit's effect")
            .with_children(rules);
        Breakdown {
            category: Category::Units,
            root,
            value,
            partial: false,
            missing: Vec::new(),
        }
    }

    fn support_breakdown(&self) -> Breakdown {
        let value = self.support_collection();
        let root = FormulaNode::new("Support Bonus Per Link", value)
            .with_format(ValueFormat::Percent)
            .with_note("The heading sums the base and the two bonuses")
            .with_children(vec![
                FormulaNode::leaf("Base support", 200.0, ValueFormat::Percent),
                FormulaNode::leaf(
                    "Armory multiplier bonus",
                    200.0 * self.armory_bonus(43) / 100.0,
                    ValueFormat::Percent,
                )
                .with_source("RoyalG[2][43]"),
                FormulaNode::leaf(
                    "Jelly Operator reward",
                    self.jelly_bonus(41),
                    ValueFormat::Percent,
                )
                .with_source("Research[7][9]"),
            ]);
        Breakdown {
            category: Category::Support,
            root,
            value,
            partial: false,
            missing: Vec::new(),
        }
    }

    fn savage_breakdown(&self) -> Breakdown {
        let value = self.savage_collection();
        let root = FormulaNode::new("Savage Drain Multiplier", value)
            .with_format(ValueFormat::Multiplier)
            .with_note("The base multiplier grows by the armory bonus")
            .with_children(vec![
                FormulaNode::leaf("Base multiplier", 5.0, ValueFormat::Multiplier),
                FormulaNode::leaf(
                    "Armory multiplier bonus",
                    self.armory_bonus(69),
                    ValueFormat::Percent,
                )
                .with_source("RoyalG[2][69]"),
                FormulaNode::leaf("Currency multiplier", 0.0, ValueFormat::Multiplier),
            ]);
        Breakdown {
            category: Category::Savage,
            root,
            value,
            partial: false,
            missing: Vec::new(),
        }
    }
}
