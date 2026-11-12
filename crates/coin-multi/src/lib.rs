//! The Coin Multiplier Calculator: the inputs its cards carry, the overkill
//! tier and the total multiplier.
//!
//! Ported from `coin-multi-calc.html` (which reads `CurrentMap_<n>`,
//! `AFKtarget_<n>` and the target's `MonsterHPTotal`) and
//! `js/stats/defs/coin-multi.js`, whose total multiplies 23 groups of
//! `1 + bonus / 100`. Each group is either resolved from the save or reported as
//! not modelled with the upstream source that owns it, so the section explains
//! a partial total instead of hiding it.
//!
//! [`save`] decodes the save-derived groups through the account view; this
//! module owns the card inputs, their validation and the overkill tier the
//! damage-scaled talent reads.

pub mod overkill;
mod save;

use idleon_stats as stats;

pub use overkill::Overkill;
pub use save::coin;

/// One of the 23 multiplicative groups in upstream's `coin-multi` descriptor.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// The upstream descriptor's child name, e.g. `Companion 24`.
    pub name: &'static str,
    /// The group's factor, or `None` when this build cannot resolve it.
    pub value: Option<f64>,
    /// The upstream source a missing group needs; empty when it is resolved.
    pub reason: &'static str,
}

impl Group {
    /// Whether the save resolved this group.
    pub fn is_modelled(&self) -> bool {
        self.value.is_some()
    }
}

/// The 23 groups, in the order `coin-multi.js` multiplies them.
pub(crate) const GROUPS: [(&str, &str); 23] = [
    (
        "Cash Bubbles from Main Stats",
        "needs the total STR/AGI/WIS stat engine (stats-all category breakdowns)",
    ),
    ("Companion 24", ""),
    ("Companion 45", ""),
    ("Companion 159", ""),
    ("Companion 38", ""),
    ("Event Shop: Monster Cash I", ""),
    ("Event Shop: Monster Cash II", ""),
    ("Equip Bonus Money", ""),
    ("RoG 18 I", ""),
    ("RoG 37 II", ""),
    ("Grid Bonus", ""),
    ("Equip Extra Money", ""),
    ("Gold Equipment Set", ""),
    (
        "Gambit 7",
        "needs the Hole gambit chain (js/stats/systems/w5/hole.js)",
    ),
    ("Bundle: Monster Cash", ""),
    (
        "Talent 433 and Cooking Level",
        "needs the talent system (js/stats/systems/common/talent.js)",
    ),
    ("Meals, Maneki Kat, Poppy, and Voting", ""),
    ("Pet Arena, Friends, and Pecunia", ""),
    ("Mainframe and Vault Progress", ""),
    ("Pristine 16", ""),
    ("Prayer 8", ""),
    (
        "Divinity and Crop Scientist",
        "needs the Divinity minor bonus and its bubble chain (js/stats/systems/w5/divinity.js)",
    ),
    (
        "Other Additive Sources",
        "needs talents 657/22/643/644, cards, golden food, guild, flurbo, box rewards and the Monster Cash vial",
    ),
];

/// One character's save-derived card values and coin groups.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Character {
    /// The roster name, or `Character <n>` when the save has none.
    pub name: String,
    /// The class id's upstream name, e.g. `BOWMAN`.
    pub class_name: &'static str,
    /// `CurrentMap_<n>`.
    pub map: usize,
    /// The map's display name with its underscores replaced.
    pub map_name: String,
    /// `AFKtarget_<n>`: the saved target's key.
    pub target: String,
    /// The target's display name, `Unavailable` when the game data lacks it.
    pub target_name: String,
    /// The target's `MonsterHPTotal`, `0` when the target is unavailable.
    pub monster_hp: f64,
    /// `TaskZZ0[1][0]`, the save's highest recorded damage.
    pub highest_damage: f64,
    /// The 23 groups, in upstream's order.
    pub groups: Vec<Group>,
}

/// Every character's save-derived coin data from one import.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub characters: Vec<Character>,
}

impl Snapshot {
    /// The cards' values for one character.
    pub fn character(&self, index: usize) -> Option<&Character> {
        self.characters.get(index)
    }

    /// The inputs a freshly loaded save opens with: the character's saved
    /// target, its monster HP and the save's highest damage.
    pub fn inputs(&self, index: usize) -> Option<Inputs> {
        let character = self.character(index)?;
        Some(Inputs {
            character: index,
            target: character.target.clone(),
            monster_hp: character.monster_hp,
            damage: character.highest_damage,
            overkill: None,
        })
    }
}

/// The values the page's cards carry: the saved target, its HP and the combat
/// values the overkill tier needs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inputs {
    /// The character row the groups and the map come from.
    pub character: usize,
    /// The monster key the overkill tier compares against.
    pub target: String,
    /// The monster's HP (`MonsterHPTotal`).
    pub monster_hp: f64,
    /// The maximum damage the overkill thresholds compare with.
    pub damage: f64,
    /// An explicit tier override; the page clamps it to 1..=51.
    pub overkill: Option<u32>,
}

/// What one evaluation produced.
#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation {
    /// The product of the resolved groups.
    pub total: f64,
    /// Whether every one of the 23 groups is resolved.
    pub complete: bool,
    /// The 23 groups, in upstream's order.
    pub groups: Vec<Group>,
    /// The names of the groups this build cannot resolve.
    pub excluded: Vec<&'static str>,
    pub overkill: Overkill,
    /// Clamped inputs and unavailable targets, named for the cards.
    pub warnings: Vec<String>,
}

/// Evaluate one character's coin multiplier.
///
/// The character must be inside the save. Numeric inputs are clamped to the
/// ranges the page allows (`damage` and `monster_hp` at 0, the tier override at
/// 1..=51) and every clamp is named in [`Evaluation::warnings`]. A non-finite
/// value is an error, because there is no sane clamp for it.
pub fn evaluate(snapshot: &Snapshot, inputs: &Inputs) -> Result<Evaluation, String> {
    let character = snapshot
        .character(inputs.character)
        .ok_or_else(|| format!("character {} is outside the save", inputs.character + 1))?;

    for (name, value) in [("monster HP", inputs.monster_hp), ("damage", inputs.damage)] {
        if !value.is_finite() {
            return Err(format!("{name} must be a finite number"));
        }
    }
    if let Some(tier) = inputs.overkill
        && !(1..=51).contains(&tier)
    {
        return Err(format!("overkill tier {tier} is outside 1..51"));
    }

    let mut warnings = Vec::new();
    let monster_hp = inputs.monster_hp.max(0.0);
    if monster_hp != inputs.monster_hp {
        warnings.push("Monster HP cannot be negative; 0 is used.".into());
    }
    let damage = inputs.damage.max(0.0);
    if damage != inputs.damage {
        warnings.push("Damage cannot be negative; 0 is used.".into());
    }
    let computed = overkill::tier(character.map, monster_hp, damage);
    let overkill = match inputs.overkill {
        Some(tier) => Overkill { tier, ..computed },
        None => computed,
    };
    if overkill.partial {
        warnings.push(
            "The saved target or its monster data is unavailable; the overkill tier is 1.".into(),
        );
    }
    if !inputs.target.is_empty() && stats::monster(&inputs.target).is_none() {
        warnings.push(format!(
            "{} is not in the monster table; the target's own row is unavailable.",
            inputs.target
        ));
    }

    let groups = character.groups.clone();
    let mut total = 1.0;
    let mut excluded = Vec::new();
    for group in &groups {
        match group.value {
            Some(value) => total *= value,
            None => excluded.push(group.name),
        }
    }

    Ok(Evaluation {
        total,
        complete: excluded.is_empty(),
        groups,
        excluded,
        overkill,
        warnings,
    })
}

/// The 23 groups one character's save resolves; the rest carry their source.
pub(crate) fn character_groups(values: [Option<f64>; 23]) -> Vec<Group> {
    GROUPS
        .iter()
        .zip(values)
        .map(|((name, reason), value)| Group {
            name,
            value,
            reason,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character(map: usize, target: &str) -> Character {
        Character {
            name: "Ana".into(),
            class_name: "BOWMAN",
            map,
            map_name: stats::map_name(map),
            target: target.into(),
            target_name: stats::target_name(target),
            monster_hp: overkill::target_hp(target),
            highest_damage: 0.0,
            groups: character_groups([Some(2.0); 23]),
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            characters: vec![character(1, "mushG")],
        }
    }

    #[test]
    fn a_complete_product_is_the_total() {
        let evaluation = evaluate(&snapshot(), &snapshot().inputs(0).unwrap()).unwrap();
        assert!(evaluation.complete);
        assert!(evaluation.excluded.is_empty());
        assert_eq!(evaluation.total, 2_f64.powi(23));
        assert_eq!(evaluation.overkill.monster_hp, 32.0);
    }

    #[test]
    fn a_missing_group_is_named_and_makes_the_total_partial() {
        let mut snapshot = snapshot();
        snapshot.characters[0].groups = character_groups([None; 23]);
        let evaluation = evaluate(&snapshot, &snapshot.inputs(0).unwrap()).unwrap();
        assert!(!evaluation.complete);
        assert_eq!(evaluation.excluded.len(), 23);
        assert_eq!(evaluation.excluded[0], "Cash Bubbles from Main Stats");
        assert_eq!(evaluation.total, 1.0);
    }

    #[test]
    fn an_unknown_character_is_an_error() {
        let mut inputs = snapshot().inputs(0).unwrap();
        inputs.character = 3;
        assert_eq!(
            evaluate(&snapshot(), &inputs),
            Err("character 4 is outside the save".into())
        );
    }

    #[test]
    fn a_non_finite_or_out_of_range_input_is_rejected() {
        let snapshot = snapshot();
        let mut inputs = snapshot.inputs(0).unwrap();
        inputs.damage = f64::NAN;
        assert!(evaluate(&snapshot, &inputs).unwrap_err().contains("finite"));
        inputs.damage = 0.0;
        inputs.overkill = Some(52);
        assert!(evaluate(&snapshot, &inputs).unwrap_err().contains("1..51"));
    }

    #[test]
    fn negative_values_clamp_with_a_warning() {
        let snapshot = snapshot();
        let mut inputs = snapshot.inputs(0).unwrap();
        inputs.monster_hp = -5.0;
        inputs.damage = -1.0;
        let evaluation = evaluate(&snapshot, &inputs).unwrap();
        assert_eq!(evaluation.overkill.monster_hp, 0.0);
        assert_eq!(evaluation.overkill.damage, 0.0);
        assert_eq!(evaluation.warnings.len(), 3, "{:?}", evaluation.warnings);
    }

    #[test]
    fn an_override_sets_the_tier_the_damage_would_not_reach() {
        let snapshot = snapshot();
        let mut inputs = snapshot.inputs(0).unwrap();
        inputs.overkill = Some(7);
        let evaluation = evaluate(&snapshot, &inputs).unwrap();
        assert_eq!(evaluation.overkill.tier, 7);
        assert_eq!(evaluation.overkill.damage, 0.0);
    }
}
