//! The Coin Multiplier Calculator's save slice.
//!
//! Reads the save-derived coin groups, plus the card values (`CurrentMap_<n>`,
//! `AFKtarget_<n>`, `TaskZZ0[1][0]`). A group whose source this build does not
//! model is left `None`; the caller reports it as not modelled instead of
//! guessing.
//!
//! The Divinity/Crop Scientist group is resolved by
//! [`idleon_account::divinity`].

use idleon_account::Decoder;
use idleon_account::divinity;
use idleon_account::sets::set_bonus;
use idleon_account::talent;
use idleon_account::vault_killz::vault_killz;
use idleon_save::fields;
use idleon_stats as stats;
use serde_json::{Map, Value};

use crate::{Group, Term};

/// The number of groups the coin multiplier multiplies.
const GROUP_COUNT: usize = 23;

/// Decode every character's coin groups, in [`crate::GROUPS`] order.
fn coin_groups(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[idleon_save::Character],
    simulation: &idleon_research::model::Simulation,
) -> Vec<Vec<Group>> {
    let mut warnings = Vec::new();
    let mut decoder = Decoder::new(save, root, characters, simulation, &mut warnings);

    let killz = vault_killz(&decoder);
    // Gambit is account-wide, so it is decoded once for every character row.
    let gambit = decoder.gambit_bonus7();
    let mut rows = Vec::with_capacity(characters.len());
    for index in 0..characters.len() {
        let mut groups: Vec<Group> = (0..GROUP_COUNT).map(|i| crate::group(i, None)).collect();
        if let Some(cash) = idleon_stats_engine::cash_bubbles(
            &idleon_save::Save::new(serde_json::Value::Object(root.clone())),
            simulation,
            index,
        ) {
            let mut terms = Vec::new();
            let mut total = 0.0;
            for (stat, value, part) in &cash {
                total += part;
                let key = match stat {
                    idleon_stats_engine::Stat::Str => "CashSTR",
                    idleon_stats_engine::Stat::Agi => "CashAGI",
                    idleon_stats_engine::Stat::Wis => "CashWIS",
                    idleon_stats_engine::Stat::Luk => "CashLUK",
                };
                let name = idleon_stats_engine::bubble_label(key)
                    .map(|name| format!("Bubble: {name}"))
                    .unwrap_or_else(|| format!("Bubble: {key}"));
                terms.push(crate::Term::annotated(
                    name,
                    *part,
                    format!("{} {value:.0}", stat.id()),
                ));
            }
            groups[0] = crate::group(0, Some(1.0 + total / 100.0)).with_terms(terms, 100.0);
        }
        groups[1] = crate::group(1, Some(1.0 + decoder.companion(24).min(4.0)));
        groups[2] = crate::group(2, Some(1.0 + decoder.companion(45).min(4.0)));
        groups[3] = crate::group(3, Some(1.0 + decoder.companion(159).min(4.0)));
        groups[4] = crate::group(4, Some(1.0 + decoder.companion(38)));
        groups[5] = crate::group(5, Some(1.0 + 0.5 * decoder.event_shop(9)));
        groups[6] = crate::group(6, Some(1.0 + 0.6 * decoder.event_shop(20)));
        let etc77 = decoder.etc(77, index);
        let mut terms = Vec::new();
        push(&mut terms, "Combined Item Sources: BONUS MONEY", etc77);
        groups[7] = crate::group(7, Some(1.0 + etc77 / 100.0)).with_terms(terms, 100.0);
        let rog18 = decoder.rog_qty(18);
        groups[8] = crate::group(8, Some(1.0 + rog18 / 100.0)).with_note(format!("{rog18}%"));
        let rog37 = decoder.rog_qty(37);
        groups[9] = crate::group(9, Some(1.0 + rog37 / 100.0)).with_note(format!("{rog37}%"));
        let (grid149, grid169) = (decoder.grid_bonus(149), decoder.grid_bonus(169));
        let mut terms = Vec::new();
        push(&mut terms, "Grid: J5 Glimbo Linguistics", grid149);
        push(&mut terms, "Grid: J4 Glimbo BOGO Offer", grid169);
        groups[10] =
            crate::group(10, Some(1.0 + (grid149 + grid169) / 100.0)).with_terms(terms, 100.0);
        let etc100 = decoder.etc(100, index);
        let mut terms = Vec::new();
        push(&mut terms, "Combined Item Sources: EXTRA MONEY", etc100);
        groups[11] = crate::group(11, Some(1.0 + etc100 / 100.0)).with_terms(terms, 100.0);
        groups[12] = crate::group(
            12,
            Some(1.0 + set_bonus(&decoder, "GOLD_SET", index) / 100.0),
        );
        groups[13] = crate::group(13, Some(1.0 + gambit / 100.0)).with_note(if gambit > 0.0 {
            format!("{gambit:.1}%")
        } else {
            "Not unlocked".into()
        });
        groups[14] = crate::group(
            14,
            Some(1.0 + 2.5 * if decoder.bundle("bun_y") { 1.0 } else { 0.0 }),
        );
        // The talent pays at least one when it is unlearned, and the cooking
        // level scales it through `getLOG`. The group multiplies its two
        // factors rather than summing terms, so it expands into neither.
        let chip433 = talent::max_bonus(&decoder, 433, index);
        let cooking = idleon_common::curve::log(decoder.option(362));
        groups[15] = crate::group(15, Some(1.0 + chip433.max(1.0) * cooking / 100.0));
        // Voting pays only while the vote is the server-wide winner, which a
        // save alone does not carry, so the term reads zero on most imports.
        let (meal, maneki, poppy, vote) = (
            decoder.meal("Cash"),
            decoder.maneki_kat(),
            decoder.roo(6),
            decoder.voting(34),
        );
        let mut terms = Vec::new();
        push(&mut terms, "Meals: Monster Cash", meal);
        push(&mut terms, "Artifact: Maneki Kat", maneki);
        push(&mut terms, "Poppy: Monster Cash", poppy);
        push(&mut terms, "Voting: Monster Cash", vote);
        groups[16] = crate::group(16, Some(1.0 + (meal + maneki + poppy + vote) / 100.0))
            .with_terms(terms, 100.0);
        let (friend5, arena5, arena14, statue19) = (
            decoder.friend(5),
            decoder.pet_arena(5),
            decoder.pet_arena(14),
            decoder.statue(19),
        );
        let mut terms = Vec::new();
        push(&mut terms, "Friend Bonus: Monster Cash", friend5);
        push(&mut terms, "Pet Arena: Cash I", 0.5 * arena5);
        push(&mut terms, "Pet Arena: Cash II", arena14);
        if statue19 > 0.0 {
            terms.push(Term::annotated(
                "Statue: Pecunia",
                statue19 / 100.0,
                format!("Statue bonus {statue19:.1}"),
            ));
        }
        // This group's terms are already fractions, so it is `1 + Σ terms`.
        groups[17] = crate::group(
            17,
            Some(1.0 + 0.5 * arena5 + friend5 + arena14 + statue19 / 100.0),
        )
        .with_terms(terms, 1.0);
        let (mf9, vault34, vault37) = (
            decoder.mainframe(9).unwrap_or(0.0),
            decoder.vault(34),
            decoder.vault(37),
        );
        let mut terms = Vec::new();
        push(&mut terms, "Mainframe: Fungi Finger Pocketer", mf9);
        if vault34 * killz[8] > 0.0 {
            terms.push(Term::annotated(
                "Vault: Recipe for Profit",
                vault34 * killz[8],
                format!("{} completed tasks", killz[8]),
            ));
        }
        if vault37 * killz[9] > 0.0 {
            terms.push(Term::annotated(
                "Vault: Bubble Money",
                vault37 * killz[9],
                format!("{} capped bubble levels", killz[9]),
            ));
        }
        groups[18] = crate::group(
            18,
            Some(1.0 + (mf9 + vault34 * killz[8] + vault37 * killz[9]) / 100.0),
        )
        .with_terms(terms, 100.0);
        groups[19] = crate::group(19, Some(1.0 + decoder.pristine_charm(16) / 100.0));
        // Prayer 8's bonus, shared with the All Stats capacity breakdown.
        groups[20] = crate::group(
            20,
            Some(1.0 + idleon_account::prayer(decoder.save(), index, 8, false) / 100.0),
        );
        let div_minor = divinity::minor(&decoder, 3);
        let crop_scientist = divinity::crop_scientist(&mut decoder);
        let mut terms = Vec::new();
        push(&mut terms, "Divinity Minor: Harriep", div_minor);
        if crop_scientist > 0.0 {
            terms.push(Term::annotated(
                divinity::CROP_SCIENTIST_NAME,
                crop_scientist,
                format!("{} crops", divinity::crop_count(&decoder)),
            ));
        }
        groups[21] = crate::group(21, Some(1.0 + (div_minor + crop_scientist) / 100.0))
            .with_terms(terms, 100.0);
        rows.push(groups);
    }

    rows
}

/// The "Other Additive Sources" group, at one overkill tier.
pub(crate) fn other_group(other: &idleon_stats_engine::OtherSources, tier: f64) -> Group {
    let mut terms = Vec::new();
    push(&mut terms, "Talent: Cash Money", other.talent657);
    push(&mut terms, "Vial: Monster Cash", other.vial);
    push(&mut terms, "Combined Item Sources: MONEY", other.etc3);
    push(&mut terms, "Cards: Monster Cash", other.cards);
    push(&mut terms, "Card: Molti", other.card_w5b1);
    push(&mut terms, "Talent: Chaching!", other.talent22);
    push(&mut terms, "Flurbo Shop: Monster Cash", other.flurbo);
    push(&mut terms, "Arcade: Cash from Mobs", other.arcade);
    push(&mut terms, "Box Rewards: Monster Cash", other.box_reward);
    push(&mut terms, "Guild: Coin Supercharger x map", other.guild);
    push(
        &mut terms,
        "Talent: Coins For Charon",
        other.talent643 * tier,
    );
    push(&mut terms, "Talent: American Tipper", other.talent644);
    push(&mut terms, "Golden Food: Monster Cash", other.gold_food);
    push(&mut terms, "Vault: Mining Payday$", other.vault17);
    push(&mut terms, "Achievements", other.achievements);
    push(&mut terms, "Vault: Monster Tax", other.vault2);
    push(&mut terms, "Vault: Bored to Death", other.vault14);
    push(&mut terms, "Vault: Dirty Money", other.vault31);
    push(&mut terms, "Ola: Monster Cash Bonus", other.ola420);
    push(
        &mut terms,
        "Vault: Collection Cashout x Cards",
        other.vault70,
    );
    let sum: f64 = terms.iter().map(|term| term.value).sum();
    crate::group(22, Some(1.0 + sum / 100.0)).with_terms(terms, 100.0)
}

/// Adds a term only when its value is above zero.
fn push(terms: &mut Vec<Term>, name: &'static str, value: f64) {
    if value > 0.0 {
        terms.push(Term::new(name, value));
    }
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
        let mut groups = decoded.get(index).cloned().unwrap_or_default();
        let other = idleon_stats_engine::other_sources(
            &idleon_save::Save::new(serde_json::Value::Object(root.clone())),
            simulation,
            index,
        );
        if let Some(other) = &other
            && let Some(slot) = groups.get_mut(22)
        {
            *slot = other_group(other, 1.0);
        }
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
            groups,
            other,
        });
    }
    crate::Snapshot { characters: rows }
}
