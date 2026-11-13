//! The Hole's Gambit chain, from `js/stats/systems/w5/hole.js`.
//!
//! Gambit points are the six saved scores, each weighted by its world and
//! multiplied by `gambitPTSmulti`: measurement 13, the Bolaia study, cavern
//! upgrade 78, the W5 monument slot, the two DR jars, legend 29 and arcane
//! upgrade 47. [`Decoder::gambit_bonus7`] pays `HolesInfo[71][7]`'s base times
//! the log of those points once they clear the bonus-8 threshold, which is the
//! percentage the Coin Multiplier's `Gambit 7` group reads.

use super::Decoder;
use idleon_common::curve;
use idleon_save::fields;

/// `deathNoteRank(kills, 0, rift)`: the overkill tier one mob's kills buy.
fn death_note_rank(kills: f64, rift: f64) -> f64 {
    if kills < 25_000.0 {
        0.0
    } else if kills < 100_000.0 {
        1.0
    } else if kills < 250_000.0 {
        2.0
    } else if kills < 500_000.0 {
        3.0
    } else if kills < 1_000_000.0 {
        4.0
    } else if kills < 5_000_000.0 {
        5.0
    } else if kills < 100_000_000.0 {
        7.0
    } else if kills > 1_000_000_000.0 && rift >= 20.0 {
        20.0
    } else {
        10.0
    }
}

/// `marbleBon`: the multiplier a marble level grants.
fn marble_bonus(level: f64) -> f64 {
    if level <= 0.0 { 1.0 } else { 1.5 + 0.5 * level }
}

impl Decoder<'_> {
    /// `computeGambitBonus7`: the Monster Cash percentage Gambit tier 7 pays.
    pub fn gambit_bonus7(&self) -> f64 {
        let points = self.gambit_points();
        let threshold = 2000.0 + 1000.0 * 8.0 * (1.0 + 7.0 / 5.0) * 1.26_f64.powf(7.0);
        if points < threshold {
            return 0.0;
        }
        let base = self
            .data
            .gambit_bonus(7)
            .split('|')
            .next()
            .and_then(|value| value.trim().parse::<f64>().ok())
            .unwrap_or(10.0);
        base * curve::log(points)
    }

    /// The six scores' weighted total, scaled by [`Decoder::gambit_multi`].
    fn gambit_points(&self) -> f64 {
        let mut raw = 0.0;
        for index in 0..6 {
            let score = self.nested("Holes", &[11, 65 + index]);
            let base = score + 3.0 * (score / 10.0).floor() + 10.0 * (score / 60.0).floor();
            raw += if index == 0 { 100.0 } else { 200.0 } * base;
        }
        raw * self.gambit_multi()
    }

    /// `gambitPTSmulti`: the multiplier the Gambit point total reads.
    fn gambit_multi(&self) -> f64 {
        let legend = self.legend(29);
        let jar = |index: usize| {
            self.nested("Holes", &[24, index])
                * self.data.jar_bonus_per_level(index)
                * (1.0 + legend / 100.0)
        };
        let cavern_78 = if self.nested("Holes", &[13, 78]) == 1.0 {
            10.0
        } else {
            0.0
        };
        let sum = self.gambit_measurement(13)
            + self.nested("Holes", &[26, 13]) * idleon_fountain::bolaia_per_level(13)
            + cavern_78
            + self.monument_rog(2, 7)
            + jar(23)
            + jar(30)
            + self.arcane(47);
        1.0 + sum / 100.0
    }

    /// One measurement's bonus: its base, the cosmo multiplier and the account
    /// quantity it counts.
    fn gambit_measurement(&self, index: usize) -> f64 {
        let level = self.nested("Holes", &[22, index]);
        let Some((base, total)) = idleon_fountain::measurement(index) else {
            return 0.0;
        };
        let cosmo = 1.0 + self.cosmo_bonus(1, 3) / 100.0;
        let base = if total {
            cosmo * (base * level / (100.0 + level))
        } else {
            cosmo * base * level
        };
        base * self.measurement_multiplier(idleon_fountain::measurement_type(index))
    }

    /// `_measurementMulti`: 18 % a unit, then 8 % more past the fifth.
    ///
    /// Only the Death Note quantity is ported, because measurement 13 is its one
    /// `HolesInfo[52]` type in the tables this build carries.
    /// shortcut: measurement types 0, 1, 3, 8 and 9 read zero; port them when a
    /// Gambit bonus starts reading one.
    fn measurement_multiplier(&self, kind: u32) -> f64 {
        let qty = if kind == 6 {
            self.overkill_qty() / 125.0
        } else {
            0.0
        };
        if qty < 5.0 {
            1.0 + 18.0 * qty / 100.0
        } else {
            1.0 + (18.0 * qty + 8.0 * (qty - 5.0)) / 100.0
        }
    }

    /// `_computeOverkillQTY`: every Death Note mob's overkill tier, summed.
    fn overkill_qty(&self) -> f64 {
        let rift = self.row_at("Rift", 0);
        let rift_bonus = if rift >= 20.0 { rift } else { 0.0 };
        let mut total = 0.0;
        for world in &self.data.death_note_mobs {
            for (mob, requirement) in world {
                if *mob < 0 {
                    continue;
                }
                let mut kills = 0.0;
                for character in 0..self.characters.len() {
                    let key = format!("KLA_{character}");
                    kills += requirement
                        - fields::at(self.save, &key, &[*mob as usize, 0]).unwrap_or(0.0);
                }
                total += death_note_rank(kills.max(0.0), rift_bonus);
            }
        }
        total
    }

    /// `computeMonumentROGbonus(tier, index)`: one W5 monument slot's bonus.
    fn monument_rog(&self, tier: usize, index: usize) -> f64 {
        let slot = 10 * tier + index;
        let level = self.nested("Holes", &[15, slot]);
        let bonus = self.data.monument_bonus(slot);
        if level <= 0.0 || bonus <= 0.0 {
            return 0.0;
        }
        let mut holeoz = 1.0;
        if index != 9 {
            holeoz = 1.0 + self.monument_rog(tier, 9) / 100.0 + self.cosmo_bonus(0, 0) / 100.0;
        }
        holeoz *= 1.0 + self.fountain_monument(tier) / 100.0;
        if bonus < 30.0 {
            level * bonus * holeoz.max(1.0)
        } else {
            0.1 * (level / (250.0 + level) * 10.0 * bonus * holeoz.max(1.0)).ceil()
        }
    }

    /// `bonTOT(Holes[31], Holes[32], tier, 13)`: the monument upgrade's level,
    /// scaled by its marble level.
    fn fountain_monument(&self, tier: usize) -> f64 {
        let level = self.nested("Holes", &[31, tier, 13]);
        let marble = self.nested("Holes", &[32, tier, 13]);
        let per_level = idleon_fountain::upgrade(tier, 13)
            .map(|upgrade| upgrade.per_level)
            .unwrap_or(0.0);
        (marble_bonus(marble) * level * per_level).round()
    }

    /// `cosmoBonus(tier, index)`: the bonus one cosmo upgrade level grants.
    fn cosmo_bonus(&self, tier: usize, index: usize) -> f64 {
        let level = self.nested("Holes", &[4 + tier, index]);
        (idleon_fountain::cosmo_base(tier, index) * level).floor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_research::model::Simulation;
    use serde_json::{Map, Value};

    /// A decoder over a save holding only the Hole rows at `holes`.
    fn bonus(holes: Value) -> f64 {
        let mut save = Map::new();
        save.insert(
            "Holes".into(),
            Value::String(serde_json::to_string(&holes).expect("an array")),
        );
        let root = Map::new();
        let simulation = Simulation::default();
        let mut warnings = Vec::new();
        let view = Decoder::new(&save, &root, &[], &simulation, &mut warnings);
        view.gambit_bonus7()
    }

    /// The `Holes` rows the chain reads, all neutral except the six scores.
    fn holes(scores: [f64; 6]) -> Value {
        let mut rows: Vec<Value> = (0..35).map(|_| Value::from(Vec::<f64>::new())).collect();
        let mut scores_row = vec![0.0; 71];
        for (offset, score) in scores.iter().enumerate() {
            scores_row[65 + offset] = *score;
        }
        rows[11] = Value::from(scores_row);
        Value::Array(rows)
    }

    #[test]
    fn points_below_the_bonus_eight_threshold_pay_nothing() {
        assert_eq!(bonus(holes([0.0; 6])), 0.0);
        // 2000 points: below the 4105 the threshold asks for.
        assert_eq!(bonus(holes([20.0, 0.0, 0.0, 0.0, 0.0, 0.0])), 0.0);
    }

    #[test]
    fn points_past_the_threshold_pay_the_base_times_the_log() {
        // Six scores of 95: each is worth 95 + 27 + 10 = 132 points.
        let value = bonus(holes([95.0; 6]));
        assert!(value > 0.0);
        let points = 100.0 * 132.0 + 5.0 * 200.0 * 132.0;
        assert_eq!(value, 10.0 * curve::log(points));
    }

    #[test]
    fn the_measurement_base_reads_its_leading_number_and_the_tot_suffix() {
        assert_eq!(idleon_fountain::measurement(13), Some((10.0, true)));
        assert_eq!(idleon_fountain::measurement(0), Some((45.0, true)));
        assert_eq!(idleon_fountain::measurement(1), Some((2.0, false)));
        assert_eq!(idleon_fountain::measurement_type(13), 6);
    }

    #[test]
    fn the_marble_multiplier_starts_at_one() {
        assert_eq!(marble_bonus(0.0), 1.0);
        assert_eq!(marble_bonus(2.0), 2.5);
    }

    #[test]
    fn the_death_note_ranks_follow_the_upstream_thresholds() {
        assert_eq!(death_note_rank(24_999.0, 0.0), 0.0);
        assert_eq!(death_note_rank(25_000.0, 0.0), 1.0);
        assert_eq!(death_note_rank(100_000_000.0, 0.0), 10.0);
        assert_eq!(death_note_rank(2_000_000_000.0, 19.0), 10.0);
        assert_eq!(death_note_rank(2_000_000_000.0, 20.0), 20.0);
    }
}
