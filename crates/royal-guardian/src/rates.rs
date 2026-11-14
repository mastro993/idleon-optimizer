//! The collection rate: what one outpost harvests per hour before the node's
//! grade, and the account-wide multiplier every outpost shares.
//!
//! Ported from `TotalStatz`, `globalResourceRateBreakdown`,
//! `outpostResourceRateBreakdown`, `resourceProductionWithGrade`,
//! `supportCollection`, `savageCollection` and the collection halves of
//! `royalGuardianDerivedInputs`.
//!
//! The upstream folds the whole account into ten factors. Two of them read
//! systems this port does not decode yet - the W14 alchemy bubble's Prisma
//! multiplier and the companion roster - so [`CollectionInputs`] reports them as
//! missing and leaves their factor at one instead of guessing. The rest come
//! from the Royal Guardian rows, the four talents the section extracts and the
//! three single-row cross-system constants in `royal-guardian.json`.
//!
//! Supplying the inputs directly is how the oracle test compares the port with
//! the upstream JS: [`RoyalGuardian::global_collection_rate_with`] and its
//! outpost and production siblings take the whole set, so the two unread sources
//! can be handed in without the port claiming to read them.

use super::*;

/// How much the additive collection bonuses cap the W14 bubble at.
const ALCHEMY_CAP: f64 = 50.0;

/// The account-wide counts the global rate multiplies in: upstream `TotalStatz`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TotalStats {
    /// Every resource grade the save holds, summed.
    pub grades: f64,
    /// Every built outpost's three upgrade levels, summed.
    pub levels: f64,
    pub purified: f64,
    /// The log of each world's first currency balance, summed.
    pub log_currencies: f64,
    pub outposts: f64,
}

/// The cross-system inputs a collection rate reads: upstream
/// `royalGuardianDerivedInputs`, narrowed to the six the rate uses.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollectionInputs {
    /// The Royal Guardian character's level.
    pub char_level: f64,
    /// Talent 225's and 226's effects, as their curves read them.
    pub talent_225: f64,
    pub talent_226: f64,
    /// Talent 230's multiplier.
    pub talent_230: f64,
    /// The RoG quantity sixty unique sushi unlock.
    pub sushi_60: f64,
    /// Companion 141's collection bonus, which the roster lives outside the save.
    pub companion_141: f64,
    /// The Arcade upgrade 70 bonus.
    pub arcade_70: f64,
    /// The W14 alchemy bubble's value, which the rate caps at 50.
    pub alchemy_w14: f64,
    /// `floor(ZenithMarket[10] × Spelunk[45][10])`.
    pub zenith_10: f64,
    /// The sources the port does not read yet, and so leaves at zero.
    pub missing: Vec<String>,
}

/// One multiplier in a rate, with the upstream factor name and its label.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Factor {
    pub name: &'static str,
    pub label: &'static str,
    pub value: f64,
}

/// A rate and the factors it multiplies.
#[derive(Clone, Debug, PartialEq)]
pub struct Rate {
    pub value: f64,
    pub factors: Vec<Factor>,
    /// The factors left at one because their source is not read.
    pub missing: Vec<String>,
    /// Whether any factor is missing.
    pub partial: bool,
}

impl RoyalGuardian {
    /// A saved talent's purchased level across the characters.
    pub fn talent_level(&self, talent: usize) -> f64 {
        self.talents.get(&talent).copied().unwrap_or(0.0)
    }

    /// A saved talent's effect: its curve at the highest purchased level.
    ///
    /// Upstream adds the all-talent bonus levels before the curve; those are not
    /// decoded yet, so this reads the purchased level alone.
    pub fn talent_value(&self, talent: usize) -> f64 {
        if !self.has_character() {
            return 0.0;
        }
        let level = self.talent_level(talent);
        if level <= 0.0 {
            return 0.0;
        }
        let Some(curve) = data::talent(talent) else {
            return 0.0;
        };
        idleon_common::curve::eval(&curve.formula, curve.x1, curve.x2, level)
    }

    /// The unique sushi the save has discovered: upstream `computeUniqueSushi`.
    fn unique_sushi(&self) -> f64 {
        let Some(tiers) = self.sushi.get(5).and_then(Value::as_array) else {
            return 0.0;
        };
        let mut count = 0.0;
        for (index, tier) in tiers.iter().enumerate() {
            if json::number(tier).unwrap_or(0.0) < 0.0 {
                break;
            }
            count = index as f64 + 1.0;
        }
        count
    }

    /// The saved arcade upgrade's level.
    fn arcade_level(&self, index: usize) -> f64 {
        self.arcade.get(index).and_then(json::number).unwrap_or(0.0)
    }

    /// An arcade upgrade's effect: upstream `arcadeBonus`, with the companion 27
    /// doubling it starts from left unread like the other companion factors.
    fn arcade_bonus(&self, index: usize) -> f64 {
        if index != 70 {
            return 0.0;
        }
        let arcade = &data::data().collection.arcade70;
        let level = self.arcade_level(index);
        if level <= 0.0 {
            return 0.0;
        }
        let raw = if arcade.kind == "add" {
            if arcade.denominator != 0.0 {
                ((arcade.base + arcade.denominator) / arcade.denominator + 0.5 * (level - 1.0))
                    / (arcade.base / arcade.denominator)
                    * level
                    * arcade.base
            } else {
                arcade.base * level
            }
        } else {
            arcade.base * level / (level + arcade.denominator)
        };
        // The game doubles the bonus at 101 and multiplies it by six at 102.
        let maxed = match level as i64 {
            101 => 2.0,
            102 => 6.0,
            _ => 1.0,
        };
        maxed * raw
    }

    /// The three cross-system constants the rate reads.
    pub fn collection_inputs(&self) -> CollectionInputs {
        // The W14 bubble carries a Prisma multiplier from the alchemy section,
        // and the companion roster lives in `it.json`, which the import does not
        // read: both stay out of the rate and are named instead.
        let missing = vec![
            "Alchemy W14 bubble (Prisma multiplier)".to_owned(),
            "Companion roster (it.json)".to_owned(),
        ];
        let collection = &data::data().collection;
        let sushi = if self.unique_sushi() > 60.0 {
            collection.rog_bonus60
        } else {
            0.0
        };
        CollectionInputs {
            char_level: self.character_level(),
            talent_225: self.talent_value(225),
            talent_226: self.talent_value(226),
            talent_230: self.talent_value(230),
            sushi_60: sushi,
            companion_141: 0.0,
            arcade_70: self.arcade_bonus(70),
            alchemy_w14: 0.0,
            zenith_10: (collection.zenith_market10 * self.spelunk_cell(45, 10)).floor(),
            missing,
        }
    }

    /// The account-wide counts the global rate multiplies in.
    pub fn total_stats(&self) -> TotalStats {
        let grades = self
            .g
            .get(row::GRADES)
            .and_then(Value::as_array)
            .map(|row| row.iter().filter_map(json::number).sum())
            .unwrap_or(0.0);
        let mut levels = 0.0;
        let mut purified = 0.0;
        let mut outposts = 0.0;
        for map in 0..self.maps.len() {
            if !self.outpost(map).built {
                continue;
            }
            outposts += 1.0;
            levels += self.map_cell(map, 0) + self.map_cell(map, 1) + self.map_cell(map, 2);
            if self.rank(map, 4) >= 1.0 {
                purified += 1.0;
            }
        }
        // Upstream sums eight worlds' first currency, one past the seven outpost
        // worlds: the eighth slot is a real row `RoyalG[1]` carries.
        let log_currencies = (0..WORLDS + 1)
            .map(|world| idleon_common::curve::log(self.g_cell(row::CURRENCIES, 10 * world)))
            .sum();
        TotalStats {
            grades,
            levels,
            purified,
            log_currencies,
            outposts,
        }
    }

    /// What one Support outpost adds to a linked outpost: upstream
    /// `supportCollection`.
    pub fn support_collection(&self) -> f64 {
        200.0 * (1.0 + self.armory_bonus(43) / 100.0) + self.jelly_bonus(41)
    }

    /// What a Savage outpost multiplies its drain by: upstream `savageCollection`.
    pub fn savage_collection(&self) -> f64 {
        5.0 * (1.0 + self.armory_bonus(69) / 100.0)
    }

    /// The account-wide collection multiplier: upstream
    /// `globalResourceRateBreakdown`.
    pub fn global_collection_rate(&self) -> Rate {
        self.global_collection_rate_with(&self.collection_inputs())
    }

    /// The account-wide collection multiplier with every input supplied, which
    /// is how the oracle compares the port against the upstream JS.
    pub fn global_collection_rate_with(&self, inputs: &CollectionInputs) -> Rate {
        let totals = self.total_stats();
        let orblet = self.orblet_bonus(1);
        // The W14 bubble is an alchemy value the port does not decode, so it
        // stays at zero and the rate names it as missing.
        let alchemy = inputs.alchemy_w14.min(ALCHEMY_CAP);
        let additive =
            orblet + inputs.sushi_60 + alchemy + inputs.arcade_70 + self.armory_bonus(70);
        let hundreds = ((inputs.char_level - 1_000.0).max(0.0) / 100.0).floor();
        let factors = vec![
            Factor {
                name: "companion141",
                label: "Companion 141",
                value: 1.0 + inputs.companion_141,
            },
            Factor {
                name: "orbletBonus1+sushi60+alchemyW14+arcade70+armory70",
                label: "Collection bonuses",
                value: 1.0 + additive / 100.0,
            },
            Factor {
                name: "TotalStatz0*armory18",
                label: "Resource grades",
                value: 1.0 + totals.grades * self.armory_bonus(18) / 100.0,
            },
            Factor {
                name: "TotalStatz1*armory50",
                label: "Outpost upgrade levels",
                value: 1.0 + totals.levels * self.armory_bonus(50) / 100.0,
            },
            Factor {
                name: "TotalStatz2*armory51",
                label: "Purified outposts",
                value: 1.0 + totals.purified * self.armory_bonus(51) / 100.0,
            },
            Factor {
                name: "charLevel*armory52",
                label: "Royal Guardian level",
                value: 1.0 + hundreds * self.armory_bonus(52) / 100.0,
            },
            Factor {
                name: "talent225+talent226",
                label: "Collection talents",
                value: 1.0 + (inputs.talent_225 + inputs.talent_226) / 100.0,
            },
            Factor {
                name: "talent230Multi",
                label: "Collection multiplier talent",
                value: inputs.talent_230.max(1.0),
            },
            Factor {
                name: "TotalStatz3*armory53",
                label: "Currency logs",
                value: 1.0 + totals.log_currencies * self.armory_bonus(53) / 100.0,
            },
            Factor {
                name: "zenith10",
                label: "Zenith Market",
                value: 1.0 + inputs.zenith_10 / 100.0,
            },
        ];
        rate(factors, inputs.missing.clone())
    }

    /// What one built outpost collects per hour, by world and node link:
    /// upstream `outpostResourceRateBreakdown`.
    pub fn outpost_collection_rate(&self, map: usize) -> Rate {
        self.outpost_collection_rate_with(map, &self.collection_inputs())
    }

    /// One outpost's collection rate with every input supplied.
    pub fn outpost_collection_rate_with(&self, map: usize, inputs: &CollectionInputs) -> Rate {
        let global = self.global_collection_rate_with(inputs);
        self.outpost_collection_rate_given(map, &global)
    }

    /// One outpost's collection rate with the account-wide rate already
    /// computed, which a search over many layouts evaluates once per state.
    pub(crate) fn outpost_collection_rate_given(&self, map: usize, global: &Rate) -> Rate {
        let global = global.clone();
        if !self.outpost(map).built {
            let mut missing = global.missing.clone();
            missing.push(format!("map {map} outpost"));
            return Rate {
                value: 0.0,
                factors: Vec::new(),
                missing,
                partial: true,
            };
        }
        let units = self.units_by_type(map);
        let logistics = self.map_cell(map, cell::LOGISTICS);
        let factors = vec![
            Factor {
                name: "base",
                label: "Base collection",
                value: 125.0,
            },
            Factor {
                name: "globalResourceRateMultiplier",
                label: "Kingdom multiplier",
                value: global.value,
            },
            Factor {
                name: "outpostPurifyBonus",
                label: "Purification",
                value: 1.0 + (200.0 + self.armory_bonus(1)) * self.purification(map) / 100.0,
            },
            Factor {
                name: "supportCollection",
                label: "Support outposts",
                value: 1.0 + self.support_collection() * self.support_count(map) as f64 / 100.0,
            },
            Factor {
                name: "mapLevel1",
                label: "Advanced Logistics",
                value: 1.0 + logistics * 5.0 / 100.0,
            },
            Factor {
                name: "commandRank*armory73",
                label: "Command Rank",
                value: 1.0 + self.rank(map, 2) * self.armory_bonus(73) / 100.0,
            },
            Factor {
                name: "unitSpec0*totalUnits0",
                label: "Worker units",
                value: 1.0 + self.unit_spec(0) * units[0] / 100.0,
            },
            Factor {
                name: "mapLevel0Cap",
                label: "Expanded Barracks cap",
                value: (1.0 + 10.0 * ((self.map_cell(map, 0) - 6.0).round()).max(0.0) / 100.0)
                    .min(5.0),
            },
        ];
        rate(factors, global.missing)
    }

    /// What one outpost drains from a node once its grade is applied, with every
    /// input supplied: upstream `resourceProductionWithGrade`.
    pub fn resource_production_with(
        &self,
        map: usize,
        resource: usize,
        inputs: &CollectionInputs,
    ) -> Rate {
        let global = self.global_collection_rate_with(inputs);
        let mut rate = self.outpost_collection_rate_given(map, &global);
        let grade = self.resource_grade(resource);
        rate.factors.push(Factor {
            name: "resourceGrade",
            label: "Resource grade",
            value: 1.0 + grade * 25.0 / 100.0,
        });
        rate.value *= 1.0 + grade * 25.0 / 100.0;
        rate
    }

    /// How full an outpost's purification rank leaves it, as a 0/1 flag: upstream
    /// `outpostPurification`.
    pub fn purification(&self, map: usize) -> f64 {
        if self.purified(map) { 1.0 } else { 0.0 }
    }
}

/// Multiply a factor list into a rate.
fn rate(factors: Vec<Factor>, missing: Vec<String>) -> Rate {
    let value = factors.iter().map(|factor| factor.value).product();
    Rate {
        value,
        factors,
        partial: !missing.is_empty(),
        missing,
    }
}
