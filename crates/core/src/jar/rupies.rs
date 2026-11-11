//! Rupie value: the multiplier the cavern applies to every rupie, the extra
//! rupie chance and what one break pays out.
//!
//! Ported from `rupieValue`, `rupieValueBreakdown`, `extraRupieChance`,
//! `avgRupiesPerBreak` and `rupieTypeRange` in
//! `js/stats/systems/w5/jar.js`.

use super::{Jar, TYPE_CATEGORIES, TYPE_NAMES, TypeCategory, upgrade};
use crate::formula::{FormulaNode, ValueFormat};

/// The tiers the income table projects, counted from the first.
pub const INCOME_TIERS: u32 = 6;

/// The tiers the rupie value table projects, as the upstream page shows five.
pub const VALUE_TIERS: u32 = 5;

/// One multiplier of the rupie value, as `rupieValueBreakdown` lists it.
struct Term {
    label: String,
    multiplier: f64,
    input: String,
    source: &'static str,
}

/// The extra rupie chance one source grants, as its table lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtraRupieTerm {
    /// The system that grants it.
    pub label: &'static str,
    /// The percentage points it adds.
    pub percent: f64,
    /// Where the value comes from, for the row's caption.
    pub source: &'static str,
}

impl Jar {
    /// `rupieValue`: the multiplier every rupie a jar drops is worth.
    pub fn rupie_value(&self) -> f64 {
        self.value_terms()
            .iter()
            .map(|term| term.multiplier)
            .product()
    }

    /// The multiplier sources, in the upstream breakdown order, without the
    /// ones that grant nothing.
    fn value_terms(&self) -> Vec<Term> {
        let mut terms = Vec::new();
        let base = 1.0
            + self.upgrade_value(upgrade::BASE_RUPIE_TWO, 1.0)
            + self.upgrade_value(upgrade::BASE_RUPIE_FIVE, 2.0)
            + self.upgrade_value(upgrade::BASE_RUPIE_EIGHT, 4.0);
        terms.push(Term {
            label: "Big Jar Mach II, V & VIII".into(),
            multiplier: base,
            input: format!("1 + base rupie value at three schematics = {base}"),
            source: "B_UPG_DATA 62, 65 and 68",
        });
        if self.ext.rupie_slugs > 0.0 {
            terms.push(Term {
                label: format!("Rupie Slugs (1.5^{})", self.ext.rupie_slugs),
                multiplier: 1.5_f64.powf(self.ext.rupie_slugs).max(1.0),
                input: format!("1.5 per slug, {} slugs", self.ext.rupie_slugs),
                source: "OptLacc[355]",
            });
        }
        let lamp = self.lamp_bonus();
        if lamp > 0.0 {
            terms.push(Term {
                label: "Infinite Resources (Lamp)".into(),
                multiplier: 1.0 + lamp / 400.0,
                input: format!("+{}% / 400", ValueFormat::Raw.format(lamp)),
                source: "Holes[21][7] and Spelunk[45][2]",
            });
        }
        if self.ext.monument > 0.0 {
            terms.push(Term {
                label: "Wisdom: Jar Rupie Value".into(),
                multiplier: 1.0 + self.ext.monument / 100.0,
                input: format!("+{}%", ValueFormat::Raw.format(self.ext.monument)),
                source: "Monument 2 · 1",
            });
        }
        let measurements = self.measurement_bonus(10) + self.measurement_bonus(14);
        if measurements > 0.0 {
            terms.push(Term {
                label: "Rupie Value Measurements (Studies + Highest Damage)".into(),
                multiplier: 1.0 + measurements / 100.0,
                input: format!("+{}%", ValueFormat::Raw.format(measurements)),
                source: "Holes[22][10] and Holes[22][14]",
            });
        }
        if self.upgrade(upgrade::EVER_TREE_SYNERGY) {
            let evertree = self.evertree_multiplier();
            terms.push(Term {
                label: format!("Evertree ~ Rupie Synergy (1.10^{})", self.evertree_trunks()),
                multiplier: evertree,
                input: "1.10 per trunk".into(),
                source: "B_UPG_DATA 80 and Holes[11][5]",
            });
        }
        if self.doubler_bonus() > 0.0 {
            terms.push(Term {
                label: format!("Gilded Jar (+{}%)", self.doubler_bonus()),
                multiplier: 1.0 + self.doubler_bonus() / 100.0,
                input: "Consumed on the next rupie break".into(),
                source: "Holes[11][60]",
            });
        }
        let legend = self.ext.legend_pts;
        for (index, source) in [(3, "Tortole Rock"), (17, "Blood Glass"), (28, "Mystic Ore")] {
            let bonus = self.collectible_bonus(index, legend);
            if bonus > 0.0 {
                terms.push(Term {
                    label: source.into(),
                    multiplier: 1.0 + bonus / 100.0,
                    input: format!("+{}%", ValueFormat::Raw.format(bonus)),
                    source: "Collectible multiplier",
                });
            }
        }
        let additive = [0, 6, 13, 21, 33]
            .iter()
            .map(|index| self.collectible_bonus(*index, legend))
            .sum::<f64>();
        if additive > 0.0 {
            terms.push(Term {
                label: "Additive Collectibles".into(),
                multiplier: 1.0 + additive / 100.0,
                input: format!("+{}%", ValueFormat::Raw.format(additive)),
                source: "Abnormal Rupie, Amber Square, Dawn Prism, Stained Glassdrop and Tewball Orbstone",
            });
        }
        if self.ext.stamp_cavern_res > 0.0 {
            terms.push(Term {
                label: "Cavern Resource Stamp".into(),
                multiplier: 1.0 + self.ext.stamp_cavern_res / 100.0,
                input: format!("+{}%", ValueFormat::Raw.format(self.ext.stamp_cavern_res)),
                source: "W1 stamps · CavernRes",
            });
        }
        if self.ext.cglunko > 0.0 {
            terms.push(Term {
                label: "All Cavern Resources (Cglunko)".into(),
                multiplier: 1.0 + self.ext.cglunko / 100.0,
                input: format!("+{}%", ValueFormat::Raw.format(self.ext.cglunko)),
                source: "OptLacc[644]",
            });
        }
        terms
    }

    /// The rupie value as the shared breakdown tree: every multiplier is a
    /// child, and their product is the root.
    pub fn rupie_value_tree(&self) -> FormulaNode {
        let terms: Vec<FormulaNode> = self
            .value_terms()
            .into_iter()
            .map(|term| {
                FormulaNode::new(term.label, term.multiplier)
                    .with_format(ValueFormat::Multiplier)
                    .with_input(term.input)
                    .with_source(term.source)
            })
            .collect();
        let mut tree = FormulaNode::new("Rupie Value", self.rupie_value())
            .with_format(ValueFormat::Multiplier)
            .with_input("The multipliers below, multiplied")
            .with_source("jar.js · rupie value")
            .with_children(terms);
        if self.ext.stamp_cavern_res == 0.0 || self.ext.exotic == 0.0 {
            tree = tree.with_note(
                "The Cavern Resource stamp and the farming Exotic 52 are not decoded yet; \
                 the total leaves their bonuses out.",
            );
        }
        tree
    }

    /// `rupiesPerBreak`: what one break of the jar pays at `tier`, counting the
    /// extra rupie chance. A multi-color jar splits it across its colors.
    pub fn rupies_per_break(&self, tier: u32) -> f64 {
        self.avg_rupies_per_break() * self.rupie_value() * 10_f64.powi(tier as i32)
    }

    /// `rupieValue` at one tier: tiers multiply the value by ten each.
    pub fn rupie_value_at_tier(&self, tier: u32) -> f64 {
        self.rupie_value() * 10_f64.powi(tier as i32)
    }

    /// The rupie jars and the colors each pays at the current balances.
    pub fn rupie_colors(&self, jar_type: usize) -> Vec<usize> {
        let (first, last) = self.rupie_color_range(jar_type);
        (first..=last).collect()
    }

    /// `rupieTypeRange`: the colors a rupie jar escalates to at the current
    /// balances.
    fn rupie_color_range(&self, jar_type: usize) -> (usize, usize) {
        match jar_type {
            0 if self.rupies(1) >= 1000.0 => (0, 2),
            0 if self.rupies(0) >= 100.0 => (0, 1),
            3 if self.rupies(4) >= 500_000.0 => (3, 5),
            3 if self.rupies(3) >= 10_000.0 => (3, 4),
            5 => (10, 10),
            6 if self.rupies(7) >= 50_000_000.0 => (6, 8),
            6 if self.rupies(6) >= 6_000_000.0 => (6, 7),
            8 => (11, 11),
            9 => (9, 9),
            other => (other, other),
        }
    }

    /// `extraRupieChance`: the chance a break pays one more rupie, as a fraction.
    pub fn extra_rupie_chance(&self) -> f64 {
        self.extra_rupie_terms()
            .iter()
            .map(|term| term.percent)
            .sum::<f64>()
            / 100.0
    }

    /// The sources of the extra rupie chance, as the upstream table lists them.
    pub fn extra_rupie_terms(&self) -> Vec<ExtraRupieTerm> {
        let mut terms = Vec::new();
        if self.ext.bell > 0.0 {
            terms.push(ExtraRupieTerm {
                label: "Bell: Extra Rupie Chance",
                percent: self.ext.bell,
                source: "Holes[17][4] × 0.5",
            });
        }
        if self.ext.cosmo > 0.0 {
            terms.push(ExtraRupieTerm {
                label: "Rupies Everywhere (Cosmo)",
                percent: self.ext.cosmo,
                source: "Holes[4][3] × 35",
            });
        }
        if self.upgrade(upgrade::BASE_RUPIE_FOUR) {
            terms.push(ExtraRupieTerm {
                label: "Big Jar Mach IV",
                percent: 50.0,
                source: "B_UPG_DATA 64",
            });
        }
        terms
    }

    /// `avgRupiesPerBreak`: `floor(1 + chance + rand())` averages to `1 + chance`.
    pub fn avg_rupies_per_break(&self) -> f64 {
        1.0 + self.extra_rupie_chance()
    }

    /// The rupie jars, in save order, with the colors each pays.
    pub fn rupie_jars(&self) -> Vec<(usize, &'static str, Vec<usize>)> {
        (0..TYPE_NAMES.len())
            .filter(|jar_type| TYPE_CATEGORIES[*jar_type] == TypeCategory::Rupie)
            .map(|jar_type| (jar_type, TYPE_NAMES[jar_type], self.rupie_colors(jar_type)))
            .collect()
    }
}
