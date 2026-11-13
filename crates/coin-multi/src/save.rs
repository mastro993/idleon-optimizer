//! The Coin Multiplier Calculator's save slice.
//!
//! Ports the save-derived groups of `js/stats/defs/coin-multi.js`, plus the
//! grep-free reads of `coin-multi-calc.html` (`CurrentMap_<n>`, `AFKtarget_<n>`,
//! `TaskZZ0[1][0]`). A group whose upstream source this build does not port is
//! left `None`; the caller reports it as not modelled instead of guessing.
//!
//! The vault kill-count tiers follow `js/stats/systems/common/vaultKillz.js`,
//! and the equipped-set check follows `checkSetEquipped` in
//! `js/stats/systems/w3/setBonus.js`.

use idleon_account::Decoder;
use idleon_save::fields;
use idleon_stats as stats;
use serde_json::{Map, Value};

/// The number of groups `coin-multi.js` multiplies.
const GROUP_COUNT: usize = 23;

/// Decode every character's coin groups, indexed like [`crate::GROUPS`].
fn coin_groups(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[idleon_save::Character],
    simulation: &idleon_research::model::Simulation,
) -> Vec<[Option<f64>; GROUP_COUNT]> {
    let mut warnings = Vec::new();
    let mut decoder = Decoder::new(save, root, characters, simulation, &mut warnings);

    let killz = vault_killz(&decoder);
    // Gambit is account-wide, so it is decoded once for every character row.
    let gambit = decoder.gambit_bonus7();
    let mut groups = vec![[None; GROUP_COUNT]; characters.len()];
    for (index, values) in groups.iter_mut().enumerate() {
        values[1] = Some(1.0 + decoder.companion(24).min(4.0));
        values[2] = Some(1.0 + decoder.companion(45).min(4.0));
        values[3] = Some(1.0 + decoder.companion(159).min(4.0));
        values[4] = Some(1.0 + decoder.companion(38));
        values[5] = Some(1.0 + 0.5 * decoder.event_shop(9));
        values[6] = Some(1.0 + 0.6 * decoder.event_shop(20));
        values[7] = Some(1.0 + decoder.etc(77, index) / 100.0);
        values[8] = Some(1.0 + decoder.rog_qty(18) / 100.0);
        values[9] = Some(1.0 + decoder.rog_qty(37) / 100.0);
        values[10] = Some(1.0 + (decoder.grid_bonus(149) + decoder.grid_bonus(169)) / 100.0);
        values[11] = Some(1.0 + decoder.etc(100, index) / 100.0);
        values[12] = Some(1.0 + gold_set_bonus(&decoder, index) / 100.0);
        values[13] = Some(1.0 + gambit / 100.0);
        values[14] = Some(1.0 + 2.5 * if decoder.bundle("bun_y") { 1.0 } else { 0.0 });
        // Voting pays only while the vote is the server-wide winner, which a
        // save alone does not carry, so the term reads zero on most imports.
        let vote = decoder.voting(34);
        values[16] = Some(
            1.0 + (decoder.meal("Cash") + decoder.maneki_kat() + decoder.roo(6) + vote) / 100.0,
        );
        values[17] = Some(
            1.0 + 0.5 * decoder.pet_arena(5)
                + decoder.friend(5)
                + decoder.pet_arena(14)
                + decoder.statue(19) / 100.0,
        );
        values[18] = Some(
            1.0 + (decoder.mainframe(9).unwrap_or(0.0)
                + decoder.vault(34) * killz[8]
                + decoder.vault(37) * killz[9])
                / 100.0,
        );
        values[19] = Some(1.0 + decoder.pristine_charm(16) / 100.0);
        // `computePrayerReal`, shared with the All Stats capacity breakdown.
        values[20] = Some(1.0 + idleon_account::prayer(decoder.save(), index, 8, false) / 100.0);
    }

    groups
}

/// The whole coin snapshot, cards included.
///
/// `root` is the `it.json` wrapper the save may sit in: the character names,
/// the companion roster and the winning vote live beside `data`, so the decoder
/// reads them there.
pub fn coin(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[idleon_save::Character],
    simulation: &idleon_research::model::Simulation,
) -> crate::Snapshot {
    let decoded = coin_groups(save, root, characters, simulation);
    let highest_damage = fields::at(save, "TaskZZ0", &[1, 0]).unwrap_or(0.0);
    let names: Vec<String> = fields::field(root, "charNames")
        .or_else(|| fields::field(save, "charNames"))
        .and_then(|value| {
            value.as_array().map(|names| {
                names
                    .iter()
                    .map(|name| name.as_str().unwrap_or_default().to_owned())
                    .collect()
            })
        })
        .unwrap_or_default();
    let mut rows = Vec::with_capacity(characters.len());
    for index in 0..characters.len() {
        let class = fields::char_field(save, "CharacterClass", index)
            .and_then(|value| idleon_save::json::number(&value))
            .unwrap_or(0.0)
            .max(0.0) as usize;
        let map = fields::char_field(save, "CurrentMap", index)
            .and_then(|value| idleon_save::json::number(&value))
            .unwrap_or(0.0)
            .max(0.0) as usize;
        // `AFKtarget_<n>` is a plain monster key, not JSON, so it is read raw:
        // `parsed_field` would reject the unquoted text.
        let target = save
            .get(&format!("AFKtarget_{index}"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        rows.push(crate::Character {
            name: names
                .get(index)
                .filter(|name| !name.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("Character {}", index + 1)),
            class_name: stats::class_name(class),
            map,
            map_name: stats::map_name(map),
            monster_hp: crate::overkill::target_hp(&target),
            target_name: stats::target_name(&target),
            highest_damage,
            target,
            groups: crate::character_groups(
                decoded.get(index).copied().unwrap_or([None; GROUP_COUNT]),
            ),
        });
    }
    crate::Snapshot { characters: rows }
}

/// `getSetBonus('GOLD_SET', ci)`: the permanent unlock, or the equipped set.
fn gold_set_bonus(decoder: &Decoder<'_>, character: usize) -> f64 {
    if decoder.ola_text(379).contains("GOLD_SET") || set_equipped(decoder, "GOLD_SET", character) {
        stats::set_bonus("GOLD_SET")
    } else {
        0.0
    }
}

/// `checkSetEquipped` from `js/stats/systems/w3/setBonus.js`.
fn set_equipped(decoder: &Decoder<'_>, name: &str, character: usize) -> bool {
    let Some(set) = stats::equipment_set(name) else {
        return false;
    };
    let Some(order) = fields::char_field(decoder.save(), "EquipOrder", character) else {
        return false;
    };
    let row = |index: usize| -> &[Value] {
        order
            .get(index)
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    };
    let (armor, tools) = (row(0), row(1));
    let item = |row: &[Value], slot: usize| -> Option<String> {
        row.get(slot).and_then(Value::as_str).map(str::to_owned)
    };

    let mut parts = 0;
    for slot in 0..16 {
        if item(armor, slot).is_some_and(|name| set.armor.contains(&name)) {
            parts += 1;
        }
    }
    if set.tools_cap > 0.0 {
        let mut found = 0;
        for slot in 0..8 {
            if found as f64 >= set.tools_cap {
                break;
            }
            if item(tools, slot).is_some_and(|name| set.tools.contains(&name)) {
                parts += 1;
                found += 1;
            }
        }
    }
    if set.special_cap == 1.0
        && (0..16).any(|slot| item(armor, slot).is_some_and(|name| set.special.contains(&name)))
    {
        parts += 1;
    }
    let required = set.armor.len() as f64 + set.tools_cap + set.special_cap;
    parts as f64 >= required
}

/// `computeVaultKillzTotal` from `js/stats/systems/common/vaultKillz.js`.
///
/// Indices 0..3 are the raw kills on maps 14, 24, 13 and 8; 4..7 their floored
/// logs; 8 the completed tasks; 9 the capped bubble levels; 10 and 11 the same
/// pair for map 101.
fn vault_killz(decoder: &Decoder<'_>) -> [f64; 12] {
    let mut result = [0.0; 12];
    let kills = |map: usize| -> f64 {
        let mut total = 0.0;
        for character in 0..decoder.characters().len() {
            let Some(row) = fields::char_field(decoder.save(), "KLA", character) else {
                continue;
            };
            let Some(row) = fields::object_row(&row, map) else {
                continue;
            };
            let left = row.first().copied().flatten().unwrap_or(0.0);
            total += (stats::kill_requirement(map) - left).max(0.0);
        }
        total
    };

    for (index, map) in [14usize, 24, 13, 8].into_iter().enumerate() {
        result[index] = kills(map);
    }
    for index in 0..4 {
        result[4 + index] = floored_log(result[index]);
    }

    // `tasksGlobalData[3]` is `TaskZZ3`: its rows hold the completed flag per task.
    let mut completed = 0.0;
    if let Some(rows) = fields::field(decoder.save(), "TaskZZ3") {
        for world in 0..4 {
            let Some(tasks) = fields::object_row(&rows, world) else {
                continue;
            };
            completed += tasks.iter().filter(|task| **task == Some(1.0)).count() as f64;
        }
    }
    result[8] = completed;

    let mut bubbles = 0.0;
    if let Some(cauldrons) = fields::field(decoder.save(), "CauldronInfo") {
        for cauldron in 0..4 {
            let Some(levels) = fields::object_row(&cauldrons, cauldron) else {
                continue;
            };
            for level in levels.into_iter().flatten() {
                bubbles += level.min(100.0);
            }
        }
    }
    result[9] = bubbles;

    result[10] = kills(101);
    result[11] = floored_log(result[10]);
    result
}

/// `getLOG(value)`: the floored base-10 log, `0` below ten.
fn floored_log(value: f64) -> f64 {
    if value < 10.0 {
        0.0
    } else {
        value.log10().floor()
    }
}
