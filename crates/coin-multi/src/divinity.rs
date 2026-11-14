//! The Divinity minor link and Crop Scientist coin terms.
//!
//! Ported from `computeDivinityMinor` in `js/stats/systems/w5/divinity.js` and
//! `computeCropSC` in `js/stats/systems/w6/farming.js`. Both are account-wide,
//! so the coin groups read them once for every character row:
//!
//! - a Harriep minor link pays `max(1, Y2 bubble) × (1 + Coral Kid 3 / 100) ×
//!   Div Lv / (60 + Div Lv) × god base` for every linked character, or for
//!   every character when the deity is universal, pocketed or the server vote
//!   (option 425) makes it account-wide;
//! - Crop Scientist 4 pays `15 × crops × (1 + mainframe 17 / 100) ×
//!   (1 + (Grimoire 22 + exotic 40 + Vault 79) / 100)` behind emporium item 23.
//!
//! The Y2 bubble is `finalBubbleValByKey('Y2ACTIVE', …)`: its own curve times
//! the equipped check. It is a large bubble, so Prisma and the big-bubble pass
//! never apply to it.

use idleon_account::Decoder;
use idleon_save::fields;

/// `Divinity` row 11 pockets, the two gods the Cavern keeps linked.
const POCKET_ROW: usize = 11;
const POCKETS: [usize; 2] = [29, 30];

/// The Crop Scientist bonus the coin group reads is row 4, Coins from
/// Monsters; its emporium gate is item 23.
const CROP_SCIENTIST_EMPORIUM: usize = 23;
const CROP_SCIENTIST_CROPS: f64 = 15.0;

/// `computeDivinityMinor(-1, style, saveData)`: the total minor-link bonus.
pub(crate) fn minor(decoder: &Decoder<'_>, style: i32) -> f64 {
    let Some(target) =
        (0..decoder.god_count()).find(|god| decoder.god_type(*god as f64) == style as f64)
    else {
        return 0.0;
    };
    let save = decoder.save();
    let y2 = y2_bubble(decoder, 0);
    let coral_kid = decoder.option(430).round();
    let account_wide = account_wide(decoder, style);

    let linked = fields::numbers(save, "Divinity").unwrap_or_default();
    let mut total = 0.0;
    for character in 0..decoder.characters().len().min(12) {
        let assigned = fields::number_at(&linked, character + 12);
        if !account_wide && (assigned < 0.0 || decoder.god_type(assigned) != style as f64) {
            continue;
        }
        let god = if account_wide {
            target
        } else {
            assigned as usize
        };
        total += minor_link(decoder, character, god, y2, coral_kid);
    }
    total
}

/// `computeDivinityMinor(character, style, saveData)`: one character's own
/// link. This is the share the talent system's AllTalentLVz reads for Arctis
/// (style 2).
///
/// A link the save has not assigned is `-1` upstream, not zero, so an absent
/// row never pays.
pub(crate) fn minor_for(decoder: &Decoder<'_>, character: usize, style: i32) -> f64 {
    let Some(target) =
        (0..decoder.god_count()).find(|god| decoder.god_type(*god as f64) == style as f64)
    else {
        return 0.0;
    };
    let save = decoder.save();
    let linked = fields::numbers(save, "Divinity").unwrap_or_default();
    let y2 = y2_bubble(decoder, character);
    let coral_kid = decoder.option(430).round();
    // The per-character branch shares the link through the universal
    // companion or a pocket, and the game's own unlock gates for Arctis
    // (research grid 173) and Nobis (gem item 9).
    let shared = decoder.companion(0) == 1.0
        || pocketed(decoder, style)
        || (fields::at(save, "Research", &[0, 173]).unwrap_or(0.0) >= 1.0 && style == 2)
        || (fields::at(save, "GemItemsPurchased", &[9]).unwrap_or(0.0) == 1.0 && style == 0);
    if shared {
        return minor_link(decoder, character, target, y2, coral_kid);
    }

    let mut god = linked
        .get(character + 12)
        .copied()
        .flatten()
        .unwrap_or(-1.0);
    if god < 0.0 {
        return 0.0;
    }
    if decoder.god_type(god) != style as f64 {
        // Talent 505 lets a link carry one extra god, named by its worth.
        let talent = decoder.talent_level(character, 505);
        let extra = talent - 10.0 * (talent / 10.0).floor();
        if !(talent > 0.0
            && fields::number_at(&linked, 25) > extra
            && decoder.god_type(extra) == style as f64)
        {
            return 0.0;
        }
        god = extra;
    }
    minor_link(decoder, character, god as usize, y2, coral_kid)
}

/// The link is account-wide when the universal companion is owned, a Cavern
/// pocket holds the deity (styles 3 and 5), or an option selects it.
fn account_wide(decoder: &Decoder<'_>, style: i32) -> bool {
    (style == 3 || style == 5)
        && (decoder.companion(0) == 1.0
            || pocketed(decoder, style)
            || decoder.option(425) == f64::from(style + 1))
}

/// Whether one of the Cavern pockets holds a deity of `style`.
fn pocketed(decoder: &Decoder<'_>, style: i32) -> bool {
    POCKETS.iter().any(|column| {
        fields::at(decoder.save(), "Holes", &[POCKET_ROW, *column])
            .is_some_and(|god| god >= 0.0 && decoder.god_type(god) == style as f64)
    })
}

/// One character's share: `div Lv / (60 + div Lv)` times the account terms.
fn minor_link(decoder: &Decoder<'_>, character: usize, god: usize, y2: f64, coral_kid: f64) -> f64 {
    let div_lv = fields::at(decoder.save(), &format!("Lv0_{character}"), &[14]).unwrap_or(0.0);
    if div_lv <= 0.0 {
        return 0.0;
    }
    let base = decoder.god_base(decoder.god_type(god as f64).max(0.0) as usize);
    y2.max(1.0) * ((1.0 + coral_kid / 100.0) * div_lv / (60.0 + div_lv)) * base
}

/// `finalBubbleValByKey('Y2ACTIVE', character, saveData)`.
fn y2_bubble(decoder: &Decoder<'_>, character: usize) -> f64 {
    let save = decoder.save();
    // `CauldronInfo` rows can be sparse objects, so read row 3 through the
    // object-aware helper before taking the bubble index.
    let level = fields::field(save, "CauldronInfo")
        .and_then(|rows| fields::object_row(&rows, 3))
        .and_then(|row| row.get(21).copied().flatten())
        .unwrap_or(0.0);
    if level <= 0.0 {
        return 0.0;
    }
    let equipped_by_companion = decoder.companion(4) == 1.0;
    let equipped = fields::field(save, "CauldronBubbles")
        .and_then(|rows| idleon_save::json::index(&rows, &[character]).cloned())
        .and_then(|row| row.as_array().map(|cells| cells.to_vec()))
        .is_some_and(|cells| cells.iter().any(|cell| cell.as_str() == Some("c21")));
    if !equipped_by_companion && !equipped {
        return 0.0;
    }
    decoder.y2_bubble(level)
}

/// `computeCropSC(4, saveData)`: Coins from Monsters.
pub(crate) fn crop_scientist(decoder: &mut Decoder<'_>) -> f64 {
    if decoder.emporium(CROP_SCIENTIST_EMPORIUM) <= 0.0 {
        return 0.0;
    }
    let multiplier = (1.0 + decoder.mainframe(17).unwrap_or(0.0) / 100.0)
        * (1.0 + (decoder.grimoire_22() + decoder.exotic(40) + decoder.vault(79)) / 100.0);
    CROP_SCIENTIST_CROPS * crop_count(decoder) as f64 * multiplier
}

/// `farmCropCount`: `Object.keys(FarmCrop).length`.
pub(crate) fn crop_count(decoder: &Decoder<'_>) -> usize {
    match fields::field(decoder.save(), "FarmCrop") {
        Some(serde_json::Value::Object(crops)) => crops.len(),
        Some(serde_json::Value::Array(crops)) => crops.len(),
        _ => 0,
    }
}

/// The Crop Scientist row's display name, for the group's term.
pub(crate) const CROP_SCIENTIST_NAME: &str = "Crop Scientist: Cash";

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_research::model::Simulation;
    use serde_json::{Map, Value, json};

    fn save(value: Value) -> Map<String, Value> {
        value.as_object().expect("an object").clone()
    }

    /// Two characters at Divinity 120: character 0 has Big P equipped, character
    /// 1 does not. Both are linked to god 7 (style 1, base 200).
    fn linked_save() -> Map<String, Value> {
        let mut cauldron = vec![vec![0.0; 33]; 4];
        cauldron[3][21] = 100.0;
        let mut linked = vec![0; 14];
        linked[12] = 7;
        linked[13] = 7;
        save(json!({
            "Lv0_0": "[1,0,0,0,0,0,0,0,0,0,0,0,0,0,120]",
            "Lv0_1": "[1,0,0,0,0,0,0,0,0,0,0,0,0,0,120]",
            "Divinity": serde_json::to_string(&linked).unwrap(),
            "CauldronInfo": serde_json::to_string(&cauldron).unwrap(),
            "CauldronBubbles": r#"[["c21"],[]]"#,
        }))
    }

    #[test]
    fn a_linked_character_pays_the_divinity_formula_and_an_unlinked_one_does_not() {
        let save = linked_save();
        let characters = idleon_save::characters(&save);
        let simulation = Simulation::default();
        let mut warnings = Vec::new();
        let decoder = Decoder::new(&save, &save, &characters, &simulation, &mut warnings);
        // Both link to god 7: type 1 (Arctis), base 150, 120 / (60 + 120) each.
        // Both shares read character 0's equipped Big P: decayMulti(0.5, 60)
        // at level 100 is 1.3125, over Arctis' 150 base.
        let value = minor(&decoder, 1);
        assert!(
            (value - 2.0 * 1.3125 * 150.0 * 120.0 / 180.0).abs() < 1e-12,
            "got {value}"
        );
        // Neither links to Harriep (type 3).
        assert_eq!(minor(&decoder, 3), 0.0);
    }

    #[test]
    fn the_y2_bubble_needs_a_purchased_and_equipped_big_p() {
        let save = linked_save();
        let characters = idleon_save::characters(&save);
        let simulation = Simulation::default();
        let mut warnings = Vec::new();
        let decoder = Decoder::new(&save, &save, &characters, &simulation, &mut warnings);
        let equipped = y2_bubble(&decoder, 0);
        assert!(equipped > 1.0, "decayMulti 0.5 / 60 at 100 is above one");
        assert_eq!(y2_bubble(&decoder, 1), 0.0);

        let mut unpurchased = save.clone();
        unpurchased.insert("CauldronInfo".into(), json!("[[],[],[],[0]]"));
        let characters = idleon_save::characters(&unpurchased);
        let decoder = Decoder::new(
            &unpurchased,
            &unpurchased,
            &characters,
            &simulation,
            &mut warnings,
        );
        assert_eq!(y2_bubble(&decoder, 0), 0.0);
    }

    #[test]
    fn the_crop_scientist_gate_and_count_match_the_save() {
        let mut ninja = vec![Vec::<Value>::new(); 103];
        ninja[102] = (0..10)
            .map(|index| if index == 9 { json!("w") } else { json!("") })
            .collect();
        let mut crop = save(json!({
            "Ninja": Value::Array(ninja.into_iter().map(Value::Array).collect()),
            "FarmCrop": r#"{"1":0,"2":0,"3":0}"#,
        }));
        let characters = idleon_save::characters(&crop);
        let simulation = Simulation::default();
        let mut warnings = Vec::new();
        let mut decoder = Decoder::new(&crop, &crop, &characters, &simulation, &mut warnings);
        assert_eq!(crop_count(&decoder), 3);
        // 15 per crop, no mainframe/Grimoire/exotic/Vault terms in this save.
        assert_eq!(crop_scientist(&mut decoder), 45.0);
        crop.insert("Ninja".into(), json!("[]"));
        let characters = idleon_save::characters(&crop);
        let mut decoder = Decoder::new(&crop, &crop, &characters, &simulation, &mut warnings);
        assert_eq!(crop_scientist(&mut decoder), 0.0);
    }
}
