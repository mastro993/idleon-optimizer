//! The research EXP multiplier tree, ported from
//! `js/stats/defs/research-exp.js`.
//!
//! The additive list and the true multipliers are built in the upstream order
//! so a term missing from a save leaves the rest of the tree intact. The grid,
//! sticker and mastery terms are taken back out of the additive total at the
//! end, because `simTotalExpWith` applies them itself.

use super::{BonusTerm, BonusTree, Decoder, fields};
use crate::data::game;
use crate::model::SimContext;

impl Decoder<'_> {
    pub(super) fn tree(&mut self, context: &mut SimContext) -> BonusTree {
        let additive = self.additive_terms(context);
        let multipliers = self.multiplier_terms(context);
        let additive_pct = additive.iter().map(|term| term.value).sum::<f64>();
        let multiplier = multipliers.iter().map(|term| term.value).product::<f64>();
        let sticker = term_value(&additive, "Sticker Bonus");
        let mastery = term_value(&additive, "Research Skill Mastery");
        let grid_add = self.grid_additive(context);
        // The simulator adds the sticker, the grid nodes and mastery itself.
        context.extra_pct = additive_pct - sticker - grid_add - mastery;
        BonusTree {
            additive,
            multipliers,
            additive_pct,
            multiplier,
        }
    }

    /// The descriptor's additive pool, in upstream order.
    fn additive_terms(&mut self, context: &SimContext) -> Vec<BonusTerm> {
        let mut items = Vec::new();

        // 1. Sticker. Menagerie unlock 22 moves it to the multiplier list.
        let sticker = self.sticker(context);
        items.push(if context.sticker_multiplicative {
            BonusTerm::derived("Sticker Bonus", 0.0)
        } else {
            BonusTerm::derived("Sticker Bonus", sticker)
        });

        // 2. Dancing Coral: `dancingCoralBase(4) * max(0, Tower[22] - 200)`.
        let coral_base = self.data.dancing_coral_base.get(4).copied().unwrap_or(3.0);
        let tower_22 = self.row_at("Tower", 22);
        items.push(BonusTerm::derived(
            "Dancing Coral",
            coral_base * (tower_22 - 200.0).max(0.0),
        ));

        // 3. Zenith Market.
        items.push(BonusTerm::derived(
            "Zenith Market",
            (1.0 * self.spelunk(45, 8)).floor(),
        ));

        // 4. Cards.
        items.push(BonusTerm::derived(
            "Trench Fish Card",
            self.card_level("w7b1").min(10.0),
        ));
        items.push(BonusTerm::derived(
            "Eggroll Card",
            (2.0 * self.card_level("w7b4")).min(15.0),
        ));
        items.push(BonusTerm::derived(
            "Coralcave Crab Card",
            self.card_level("w7a11").min(10.0),
        ));
        items.push(BonusTerm::derived(
            "Seaweed Serpent Card",
            (3.0 * self.card_level("w7b8")).min(20.0),
        ));

        // 5. Paper Hardhat: `shopUpgBonus(63)`.
        items.push(BonusTerm::derived(
            "Paper Hardhat",
            self.spelunk_upgrade(63),
        ));

        // 6. Prehistoric set.
        let prehistoric = if self.ola_text(379).contains("PREHISTORIC_SET") {
            self.data.set_bonus("PREHISTORIC_SET").min(50.0)
        } else {
            0.0
        };
        items.push(BonusTerm::derived("Prehistoric Set", prehistoric));

        // 7. Slab: gated by superbit 34 and fed by mainframe 15, meritoc 23,
        // legend 28 and vault 74.
        let slabbo = match self.mainframe(15) {
            Some(mainframe) if self.superbit(34) > 0.0 => {
                let cards = fields::field(self.save, "Cards1")
                    .and_then(|value| value.as_array().map(Vec::len))
                    .unwrap_or(0);
                let base = (cards.saturating_sub(1300) as f64 / 5.0).floor().max(0.0);
                let multi = (1.0 + mainframe / 100.0)
                    * (1.0 + self.meritoc(23) / 100.0)
                    * (1.0 + self.legend(28) / 100.0)
                    * (1.0 + self.vault(74) / 100.0);
                0.1 * multi * base
            }
            Some(_) => 0.0,
            None => {
                items.push(self.omission(
                    "Slab Bonus",
                    "Lab",
                    "mainframe 15 is not derivable without the lab layout",
                ));
                0.0
            }
        };
        if !items.iter().any(|term| term.name == "Slab Bonus") {
            items.push(BonusTerm::derived("Slab Bonus", slabbo));
        }

        // 8. Meal (Giga Chip).
        match self.cooking_meal_multi() {
            Some(meal) => {
                let level = self.nested("Meals", &[0, 72]);
                let mastery_level = self.nested("CookMaster", &[0, 72]);
                let mastery = 1.0 + mastery_level / (mastery_level + 5.0);
                items.push(BonusTerm::derived(
                    "Meal (Giga Chip)",
                    mastery * meal.value * self.ribbon(100) * level * 0.01,
                ));
            }
            None => items.push(self.omission(
                "Meal (Giga Chip)",
                "Summon",
                "the meal multiplier needs summoning wins and shiny levels",
            )),
        }

        // 9. Crop Scientist.
        if self.emporium(44) > 0.0 {
            let crops = fields::field(self.save, "FarmCrop")
                .map(|value| value.as_object().map(|crops| crops.len()).unwrap_or(0))
                .unwrap_or(0);
            let base = ((crops.saturating_sub(200) as f64) / 10.0).floor().max(0.0);
            match self.mainframe(17) {
                Some(mainframe) => {
                    let multi = (1.0 + mainframe / 100.0)
                        * (1.0 + (self.grimoire_22() + self.exotic(40) + self.vault(79)) / 100.0);
                    items.push(BonusTerm::derived("Crop Scientist", base * multi));
                }
                None => items.push(self.omission(
                    "Crop Scientist",
                    "Lab",
                    "mainframe 17 is not derivable without the lab layout",
                )),
            }
        } else {
            items.push(BonusTerm::derived("Crop Scientist", 0.0));
        }

        // 10. MSA: superbit 44 gates `0.3 * max(0, floor((stars - 300) / 10))`.
        let msa = if self.superbit(44) > 0.0 {
            let stars: f64 = fields::field(self.save, "TotemInfo")
                .and_then(|value| {
                    super::super::json::index(&value, &[0]).map(|row| {
                        row.as_array()
                            .map(|cells| {
                                cells
                                    .iter()
                                    .map(|cell| super::super::json::number(cell).unwrap_or(0.0))
                                    .sum()
                            })
                            .unwrap_or(0.0)
                    })
                })
                .unwrap_or(0.0);
            0.3 * ((stars - 300.0) / 10.0).floor().max(0.0)
        } else {
            0.0
        };
        items.push(BonusTerm::derived("MSA Bonus", msa));

        // 11. Lore episode / tome.
        let tome = self.tome_points();
        let episodes = self.spelunk(13, 2);
        match tome {
            Some(tome_points) if episodes > 7.0 && tome_points > 0.0 => {
                let grimoire = self.row_at("Grimoire", 17);
                let troll = if self.ola_text(379).contains("TROLL_SET") {
                    self.data.set_bonus("TROLL_SET")
                } else {
                    0.0
                };
                let multi = 1.0 + (grimoire + troll) / 100.0;
                let x = ((tome_points - 16000.0) / 100.0).floor().max(0.0);
                let xp = x.powf(0.7);
                items.push(BonusTerm::derived(
                    "Tome Bonus",
                    multi * 20.0 * (xp / (25.0 + xp)).max(0.0),
                ));
            }
            Some(_) => items.push(BonusTerm::derived("Tome Bonus", 0.0)),
            None => items.push(self.omission(
                "Tome Bonus",
                "Tome",
                "the tome score is not derivable without the talent and DR systems",
            )),
        }

        // 12. Grid nodes that add to the pool.
        for (cell, name) in [
            (50, "Grid: Pts Every Ten"),
            (51, "Grid: Sharp Eye"),
            (90, "Grid: Observationalistic"),
            (110, "Grid: All Night Studying"),
            (31, "Grid: Smart Eye"),
        ] {
            items.push(BonusTerm::derived(name, self.grid_bonus(cell)));
        }

        // 13. Grid 112 scales by observations found, 94 by insight levels.
        let (found, insight_levels) = self.observation_totals();
        items.push(BonusTerm::derived(
            "Grid: See 'Em All",
            self.grid_bonus(112) * found,
        ));
        items.push(BonusTerm::derived(
            "Grid: Game Design 102",
            self.grid_bonus(94) * insight_levels,
        ));

        // 14. Pet Arena Crucibles 2 and 8.
        let crucible = 25.0 * self.crucible(2) + 35.0 * self.crucible(8);
        items.push(BonusTerm::derived("Pet Arena Crucible", crucible));

        // 15. Research skill mastery.
        items.push(BonusTerm::derived(
            "Research Skill Mastery",
            self.mastery().additive,
        ));
        items
    }

    /// The descriptor's true multipliers, in upstream order.
    fn multiplier_terms(&mut self, context: &SimContext) -> Vec<BonusTerm> {
        let mut items = Vec::new();

        items.push(BonusTerm::derived(
            "Grid: Takin' Notes",
            1.0 + self.grid_bonus(70) / 100.0,
        ));

        let equipment = self.research_equipment_multi();
        items.push(match equipment {
            Some(value) => BonusTerm::derived("Equipment Research EXP", value),
            None => self.omission(
                "Equipment Research EXP",
                "EMm0_0",
                "equipment stats are not derivable from this save",
            ),
        });

        if context.sticker_multiplicative {
            items.push(BonusTerm::derived(
                "Sticker Bonus",
                1.0 + self.sticker(context) / 100.0,
            ));
        }

        items.push(BonusTerm::derived(
            "Research Skill Mastery multiplier",
            self.mastery().exp_multi,
        ));
        items.push(BonusTerm::derived(
            "ClamWork Promotion 9",
            1.0 + 0.25 * context.clam_work9,
        ));
        items.push(BonusTerm::derived(
            "Nonstop Studies",
            1.0 + 3.0 * context.dream14 / 100.0,
        ));
        items.push(BonusTerm::derived(
            "Button: Monster EXP",
            1.0 + context.button0 / 100.0,
        ));
        items.push(BonusTerm::derived(
            "Arcade: x Research XP",
            1.0 + context.arcade63 / 100.0,
        ));
        items.push(BonusTerm::derived(
            "Killroy: Research EXP",
            context.killroy5.max(1.0),
        ));
        items.push(BonusTerm::derived(
            "Companion: Jellofish (1.5x)",
            1.0 + self.companion(52),
        ));
        items.push(BonusTerm::derived(
            "Companion: Rift Stalker (2x / 3x enhanced)",
            1.0 + self.companion(153) + if self.companion_level2(153) { 1.0 } else { 0.0 },
        ));
        items.push(BonusTerm::derived(
            "Companion: Neonscale (enhanced)",
            1.0 + 0.5 * context.companion_54_level2,
        ));
        items.push(BonusTerm::derived(
            "Ring of Glory: multiplier to Research EXP gain",
            1.0 + context.rog(0) / 100.0,
        ));
        if context.cglunko11 > 0.0 {
            items.push(BonusTerm::derived(
                "Cglunko: Glunko Research",
                1.0 + context.cglunko11 / 100.0,
            ));
        }
        if context.fountain_2_16 > 0.0 {
            items.push(BonusTerm::derived(
                "Fountain: Pen N Paper",
                1.0 + context.fountain_2_16 / 100.0,
            ));
        }
        for index in [0, 23, 46, 54] {
            items.push(BonusTerm::derived(
                "Jelly Reward",
                1.0 + self.jelly(index) / 100.0,
            ));
        }
        items.push(BonusTerm::derived(
            "Menagerie Boost 3",
            1.0 + context.menagerie_b3 / 100.0,
        ));
        items.push(BonusTerm::derived(
            "Royal Guardian: Research Focus",
            context.royal_research.max(1.0),
        ));
        items
    }

    /// The sticker bonus before the additive/multiplier split:
    /// `crownMulti * superbitMulti * level * base`.
    fn sticker(&self, context: &SimContext) -> f64 {
        if context.sticker_fixed <= 0.0 {
            return 0.0;
        }
        let crown = 1.0
            + (self.grid_bonus(68) * context.boony_count + 30.0 * context.event_shop_37) / 100.0;
        crown * context.sticker_fixed
    }

    /// The grid share the simulator applies itself.
    fn grid_additive(&self, _context: &SimContext) -> f64 {
        let (found, insight_levels) = self.observation_totals();
        self.grid_bonus(50)
            + self.grid_bonus(90)
            + self.grid_bonus(110)
            + self.grid_bonus(112) * found
            + self.grid_bonus(94) * insight_levels
            + self.grid_bonus(31)
            + self.grid_bonus(51)
    }

    /// `(observations found, total insight levels)` over the cells that can
    /// hold an observation, as `recomputeDerivedBonuses` counts them.
    fn observation_totals(&self) -> (f64, f64) {
        let level = self.research_level;
        let observations = game().observations.len() as f64;
        let count = if level < 1 {
            0.0
        } else if self.research(2, 0).unwrap_or(0.0) == 0.0 {
            1.0
        } else {
            observations.min(
                5.0 * ((level as f64 + 10.0) / 10.0).floor()
                    - (level as f64 / 20.0).floor()
                    - (level as f64 / 30.0).floor()
                    - (level as f64 / 50.0).floor(),
            )
        } as usize;
        let found = self.observation_row(2);
        let insight = self.observation_row(4);
        let found = (0..count)
            .filter(|index| fields_at(&found, *index) >= 1.0)
            .count() as f64;
        let insight: f64 = (0..count)
            .map(|index| fields_at(&insight, index).max(0.0))
            .filter(|level| *level >= 1.0)
            .sum();
        (found, insight)
    }

    /// `Research[row]` as an observation row.
    fn observation_row(&self, row: usize) -> Vec<Option<f64>> {
        super::fields::row(self.save, "Research", &[row]).unwrap_or_default()
    }
}

fn term_value(terms: &[BonusTerm], name: &str) -> f64 {
    terms
        .iter()
        .find(|term| term.name == name)
        .map(|term| term.value)
        .unwrap_or(0.0)
}

fn fields_at(values: &[Option<f64>], index: usize) -> f64 {
    values.get(index).copied().flatten().unwrap_or(0.0)
}
