//! The talent system: a talent's value at the character that reaches the
//! highest one, including the bonus levels the game adds to the purchased
//! level.
//!
//! Ported from `getbonus2`, `getTalentNumber` and `resolveAllTalentLVz` in
//! `js/stats/systems/common/talent.js`. The coin groups read talent 433 (the
//! Dustwalker) alone, and its bonus levels are the AllTalentLVz sum: the
//! Spelunk super talent, talents 149/374/539, achievement 291, the mage family
//! bonus (34) with its talent-144 multiplier, the Rift Slug companion, Arctis'
//! Divinity minor (2), Dream 12, sneaking completions, Grimoire 39, the
//! Kattlekruk set, Arcane map 57 and Super Bit 47.
//!
//! The game only adds those levels to talents at index 100 and above; the set
//! in [`excluded`] is what `AllTalentLVz` returns zero for.

use idleon_account::Decoder;
use idleon_common::curve;
use idleon_save::fields;

use crate::save::set_bonus;

/// `getbonus2(talent, …)`: the talent's value at the best character.
///
/// `context` is the active character: its skill row, Divinity link and level
/// are the ones the bonus-level terms read, exactly as `getbonus2` passes
/// `ctx.charIdx` down.
pub(crate) fn max_bonus(decoder: &Decoder<'_>, talent: usize, context: usize) -> f64 {
    let Some(param) = decoder.talent_param(talent) else {
        return 0.0;
    };
    let mut best: f64 = 0.0;
    for character in 0..character_count(decoder) {
        let raw = decoder.talent_level(character, talent);
        if raw <= 0.0 {
            continue;
        }
        // `getbonus2` applies AllTalentLVz only from talent 100 up.
        let level = if talent >= 100 {
            raw + bonus_levels(decoder, talent, character, context, true, false)
        } else {
            raw
        };
        let value = curve::eval(&param.formula, param.x1, param.x2, level);
        if value > best {
            best = value;
        }
    }
    best
}

/// Every character row the talent loop covers: the save's characters plus any
/// `SL_<n>` row past them.
fn character_count(decoder: &Decoder<'_>) -> usize {
    decoder
        .characters()
        .len()
        .max(decoder.save_character_count())
}

/// The talents `AllTalentLVz` returns zero for: the class-tab talents, the
/// four level-scaled ones, and everything past 614.
fn excluded(talent: usize) -> bool {
    (49..=59).contains(&talent) || [149, 374, 539, 505].contains(&talent) || talent > 614
}

/// `resolveAllTalentLVz`: the levels the game adds to a talent's purchased one.
///
/// `skip_talent_144` mirrors `skipTal144FamMult`: the talent-144 multiplier on
/// the family bonus is skipped while talent 144's own levels are resolved.
fn bonus_levels(
    decoder: &Decoder<'_>,
    talent: usize,
    slot: usize,
    context: usize,
    all_presets: bool,
    skip_talent_144: bool,
) -> f64 {
    if excluded(talent) {
        return 0.0;
    }

    let mut spelunk = 0.0;
    if has_super_talent(decoder, talent, slot, all_presets) {
        spelunk = (50.0 + decoder.spelunk(18, 7) * 10.0 + decoder.spelunk(45, 5)).round();
    }

    let mut ordinary = interval_add(decoder, context, 149)
        + interval_add(decoder, context, 374)
        + interval_add(decoder, context, 539);
    if decoder.achievement(291) > 0.0 {
        ordinary += 1.0;
    }
    ordinary += family_bonus_68(decoder, context, slot, skip_talent_144).floor();
    ordinary += decoder.companion(1);
    ordinary += crate::divinity::minor_for(decoder, context, 2).ceil();
    ordinary += number(decoder, "Dream", 12);
    ordinary += 5.0 * ((97.0 + decoder.option(232)) / 100.0).floor();
    ordinary += number(decoder, "Grimoire", 39);
    ordinary += set_bonus(decoder, "KATTLEKRUK_SET", context);
    ordinary += number(decoder, "Arcane", 57).min(5.0);
    if decoder.superbit(47) == 1.0 {
        let level = class_level(decoder, context);
        ordinary += ((level - 500.0) / 100.0).floor().max(0.0);
    }
    // shortcut: the game caps bonus levels for talents 225..239 at the Armory
    // 55 upgrade; nothing the coin section reads is in that range.
    (spelunk + ordinary).floor()
}

/// `intervalAdd(1, 20, lv)` on one talent: `1 + floor(lv / 20)`, or zero when
/// the active character has not learned it.
fn interval_add(decoder: &Decoder<'_>, context: usize, talent: usize) -> f64 {
    let level = decoder.talent_level(context, talent);
    if level > 0.0 {
        1.0 + (level / 20.0).floor()
    } else {
        0.0
    }
}

/// `FamBonusQTYs[68]`: the mage family bonus at the highest mage, scaled by
/// talent 144 when the active character is that mage.
fn family_bonus_68(
    decoder: &Decoder<'_>,
    context: usize,
    slot: usize,
    skip_talent_144: bool,
) -> f64 {
    let Some(family) = decoder.family_bonus(34) else {
        return 0.0;
    };
    let mut best_level = 0.0;
    let mut best: Option<usize> = None;
    for character in 0..character_count(decoder) {
        if !decoder.class_in_tree(class_of(decoder, character), 34) {
            continue;
        }
        let level = class_level(decoder, character);
        if level > best_level {
            best_level = level;
            best = Some(character);
        }
    }
    let levels = (best_level - family.lv_offset).round().max(0.0);
    let bonus = if levels > 0.0 {
        curve::eval(&family.formula, family.x1, family.x2, levels)
    } else {
        0.0
    };
    if bonus <= 0.0 || skip_talent_144 || best != Some(context) {
        return bonus;
    }
    let raw = decoder.talent_level(context, 144);
    if raw <= 0.0 {
        return bonus;
    }
    let Some(param) = decoder.talent_param(144) else {
        return bonus;
    };
    // The nested `computeAllTalentLVz(144, …)` call passes no
    // `allSpelunkPresets`, so it reads the selected Spelunk preset only.
    let levels = raw + bonus_levels(decoder, 144, slot, context, false, true);
    bonus * (1.0 + curve::eval(&param.formula, param.x1, param.x2, levels) / 100.0)
}

/// Whether the character's Spelunk row lists `talent` as its super talent.
fn has_super_talent(decoder: &Decoder<'_>, talent: usize, slot: usize, all_presets: bool) -> bool {
    let listed = |row: usize| -> bool {
        fields::row(decoder.save(), "Spelunk", &[row])
            .unwrap_or_default()
            .contains(&Some(talent as f64))
    };
    if all_presets {
        return listed(20 + slot) || listed(32 + slot);
    }
    let preset = fields::at(decoder.save(), &format!("PlayerStuff_{slot}"), &[1]).unwrap_or(0.0);
    listed(20 + slot + 12 * preset as usize)
}

/// `CharacterClass_<n>`, the class id whose talent tree the family bonus reads.
fn class_of(decoder: &Decoder<'_>, character: usize) -> usize {
    fields::char_field(decoder.save(), "CharacterClass", character)
        .and_then(|value| idleon_save::json::number(&value))
        .unwrap_or(0.0)
        .max(0.0) as usize
}

/// `Lv0_<n>[0]`, the character's class level.
fn class_level(decoder: &Decoder<'_>, character: usize) -> f64 {
    decoder
        .characters()
        .get(character)
        .map(|character| character.class_level())
        .unwrap_or(0.0)
}

/// One nested number of a save row, `0` when the row is missing.
fn number(decoder: &Decoder<'_>, key: &str, index: usize) -> f64 {
    fields::at(decoder.save(), key, &[index]).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_research::model::Simulation;
    use serde_json::{Map, Value, json};

    fn with_decoder<T>(save: &Map<String, Value>, read: impl FnOnce(&Decoder<'_>) -> T) -> T {
        let characters = idleon_save::characters(save);
        let simulation = Simulation::default();
        let mut warnings = Vec::new();
        let decoder = Decoder::new(save, save, &characters, &simulation, &mut warnings);
        read(&decoder)
    }

    /// A save row of `len` numbers, zero except at `index`.
    fn row(len: usize, index: usize, value: f64) -> Value {
        json!(
            (0..len)
                .map(|cell| if cell == index { value } else { 0.0 })
                .collect::<Vec<f64>>()
        )
    }

    /// Two characters: a mage (class 34) at level 400 who learned talent 433,
    /// and a beginner with nothing.
    fn mage_save(extras: Value) -> Map<String, Value> {
        let mut rows = json!({
            "Lv0_0": "[400,0,0,0,0,0,0,0,0,0,0,0,0,0,0]",
            "Lv0_1": "[1,0,0,0,0,0,0,0,0,0,0,0,0,0,0]",
            "CharacterClass_0": 34,
            "CharacterClass_1": 1,
            "SL_0": "{\"433\":299}",
            "SL_1": "{}",
            "Divinity": "[0,0,0,0,0,0,0,0,0,0,0,0,-1,-1]",
            "OptLacc": "[0]",
        })
        .as_object()
        .cloned()
        .unwrap();
        for (key, value) in extras.as_object().cloned().unwrap_or_default() {
            rows.insert(key, value);
        }
        rows
    }

    #[test]
    fn a_talent_is_read_at_its_purchased_plus_bonus_level() {
        // The mage is the only family-34 character, so family bonus 34 pays
        // floor(decay(20, 350) at 400 - 69) = 9 levels on top of the 299.
        let save = mage_save(json!({}));
        let value = with_decoder(&save, |decoder| max_bonus(decoder, 433, 0));
        assert!((value - 30.0 * 308.0 / 558.0).abs() < 1e-12, "got {value}");
    }

    #[test]
    fn the_bonus_levels_come_from_the_account_terms() {
        // Grimoire 39 pays 30 levels, Arcane 57 pays 5 (its cap), Dream 12 pays
        // 38 and the mage family bonus pays 9.
        let save = mage_save(json!({
            "Grimoire": row(40, 39, 30.0),
            "Arcane": row(58, 57, 5.0),
            "Dream": row(13, 12, 38.0),
        }));
        let bonus = with_decoder(&save, |decoder| {
            bonus_levels(decoder, 433, 0, 0, true, false)
        });
        assert_eq!(bonus, 82.0, "30 + 5 + 38 + 9");
        let value = with_decoder(&save, |decoder| max_bonus(decoder, 433, 0));
        assert!((value - 30.0 * 381.0 / 631.0).abs() < 1e-12, "got {value}");
    }

    #[test]
    fn a_class_tab_talent_has_no_bonus_levels() {
        let save = mage_save(json!({}));
        assert_eq!(
            with_decoder(&save, |decoder| bonus_levels(
                decoder, 55, 0, 0, true, false
            )),
            0.0
        );
        assert_eq!(
            with_decoder(&save, |decoder| bonus_levels(
                decoder, 657, 0, 0, true, false
            )),
            0.0,
            "talents past 614 are excluded"
        );
    }

    #[test]
    fn the_mage_family_bonus_is_the_same_whichever_character_asks() {
        // Family bonus 34 is decay(20, 350) from `Lv - 69`: at 400 that is
        // 20 × 331 / (331 + 350) = 9.72.
        let save = mage_save(json!({}));
        let value = with_decoder(&save, |decoder| family_bonus_68(decoder, 0, 0, false));
        assert!((value - 20.0 * 331.0 / 681.0).abs() < 1e-12, "got {value}");
        let beginner = with_decoder(&save, |decoder| family_bonus_68(decoder, 1, 1, false));
        assert!((beginner - value).abs() < 1e-12, "the bonus is the mage's");
    }

    #[test]
    fn talent_144_scales_the_family_bonus_of_its_own_mage() {
        let mut rows = mage_save(json!({}));
        rows.insert("SL_0".into(), json!("{\"433\":299,\"144\":100}"));
        let characters = idleon_save::characters(&rows);
        let simulation = Simulation::default();
        let mut warnings = Vec::new();
        let decoder = Decoder::new(&rows, &rows, &characters, &simulation, &mut warnings);
        let scaled = family_bonus_68(&decoder, 0, 0, false);
        let plain = family_bonus_68(&decoder, 1, 1, false);
        let base = 20.0 * 331.0 / 681.0;
        // Talent 144 pays decay(40, 100) at its own effective level, which is
        // the 100 purchased levels plus the same 9 family levels.
        let talent = 40.0 * 109.0 / 209.0;
        assert!(
            (scaled - base * (1.0 + talent / 100.0)).abs() < 1e-12,
            "got {scaled}"
        );
        assert!(
            (plain - base).abs() < 1e-12,
            "a beginner context is unscaled"
        );
    }

    #[test]
    fn the_spelunk_super_talent_pays_its_flat_fifty() {
        let mut rows = mage_save(json!({}));
        rows.insert("PlayerStuff_0".into(), json!("[0,0]"));
        let mut spelunk = vec![Vec::<Value>::new(); 46];
        spelunk[20] = vec![json!(433)];
        spelunk[18] = (0..8)
            .map(|index| json!(if index == 7 { 3 } else { 0 }))
            .collect();
        spelunk[45] = (0..6)
            .map(|index| json!(if index == 5 { 7 } else { 0 }))
            .collect();
        rows.insert("Spelunk".into(), json!(spelunk));
        let bonus = with_decoder(&rows, |decoder| {
            bonus_levels(decoder, 433, 0, 0, false, false)
        });
        assert_eq!(bonus, 96.0, "50 + 3 × 10 + 7, plus the 9 family levels");
    }
}
