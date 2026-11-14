//! The damage formula trees: one per layer, node for node from the descriptor's
//! own `_debug` terms.
//!
//! Upstream's expanded tree (`damage.js` with `formulaDetails`) keeps every
//! term it multiplies: the weapon-power chain under `Base Damage`, the additive
//! pool and talent chain under `Percentage Damage Multiplier`, and the
//! pre-softcap groups and post-softcap multipliers under `Multiplicative Damage
//! Bonuses`. This module rebuilds that shape from the values the engine already
//! records, and gives every node the upstream file it came from so the shared
//! tree's search can find it.

use idleon_common::formula::{FormulaNode, ValueFormat};
use idleon_stats_engine::Layers;

use crate::LayerId;

/// The descriptor source every node in one layer comes from.
const DESCRIPTOR: &str = "js/stats/defs/damage.js";

/// Build the selected layer's tree from the engine's terms.
pub fn build(layers: &Layers, layer: LayerId) -> FormulaNode {
    match layer {
        LayerId::Base => base(layers),
        LayerId::Percent => percent(layers),
        LayerId::Multiplier => multiplier(layers),
    }
}

/// One scalar term with its upstream source.
fn term(layers: &Layers, key: &str, label: &str, source: &str) -> FormulaNode {
    FormulaNode::new(label, value(layers, key)).with_source(format!("{DESCRIPTOR} · {source}"))
}

/// A term that reads as a multiplier (`1 + pool / 100`).
fn factor(layers: &Layers, key: &str, label: &str, source: &str) -> FormulaNode {
    FormulaNode::new(label, 1.0 + value(layers, key) / 100.0)
        .with_format(ValueFormat::Multiplier)
        .with_input(format!("1 + {}%", value(layers, key)))
        .with_source(format!("{DESCRIPTOR} · {source}"))
}

fn value(layers: &Layers, key: &str) -> f64 {
    layers.debug.get(key).copied().unwrap_or(0.0)
}

/// DDL0: the primary stat, the weapon-power chain, the flat additions and the
/// HP/MP/speed bubbles.
fn base(layers: &Layers) -> FormulaNode {
    let stat = term(
        layers,
        "base.PrimaryStat",
        "Primary Stat",
        "DamageDealtSTATtype = computeTotalStat(primary) × (1 + talents 95/455/20 + class bubbles)",
    )
    .with_children([
        term(
            layers,
            "base.PrimaryStatRaw",
            "Total primary stat",
            "computeTotalStat(primary)",
        ),
        term(
            layers,
            "base.wp",
            "Weapon Power",
            "wp = wpRaw × (1 + 97/277/457/Cosmo) + talent 5",
        ),
    ]);

    let weapon = term(
        layers,
        "base.wp",
        "Weapon Power",
        "DamageDealtLIST[0] · weapon-power chain",
    )
    .with_children([
        term(layers, "base.wpRaw", "Weapon Power before talents", "wpRaw"),
        term(
            layers,
            "base.equipWP",
            "Equipment Weapon Power",
            "ITEMS[name].Weapon_Power + EMm0_<char>[slot].Weapon_Power",
        ),
        term(
            layers,
            "base.galleryWP",
            "Gallery Weapon Power",
            "computeGalleryBaseStat('Weapon_Power')",
        ),
        term(
            layers,
            "base.obolWP",
            "Obol Weapon Power",
            "ObolEqO0/ObolEqO1 · Menagerie obol multiplier",
        ),
        term(
            layers,
            "base._foodWP",
            "Weapon Power food",
            "TotalFoodBonuses('WeaponPowerBoosts')",
        ),
        term(
            layers,
            "base._box12a",
            "Post Office: Weapon Power",
            "computeBoxReward('12a')",
        ),
        term(
            layers,
            "base._flurbo",
            "Dungeon shop Weapon Power",
            "computeFlurboShop(0)",
        ),
        term(
            layers,
            "base._cardW5b2",
            "W5 card Weapon Power",
            "computeCardLv('w5b2')",
        ),
        term(
            layers,
            "base._card18",
            "Weapon Power cards",
            "computeCardBonusByType(18)",
        ),
        term(
            layers,
            "base._sigil17",
            "Weapon Power sigil",
            "sigilBonus(17)",
        ),
        term(
            layers,
            "base._guild3",
            "Guild Weapon Power",
            "guildBonusParams(3)",
        ),
        term(
            layers,
            "base._chipWP",
            "Weapon Power chip",
            "computeChipBonus('weppow')",
        ),
        term(
            layers,
            "base._tc616",
            "Beginner Weapon Power talent",
            "min(talent 616, floor(highest Beginner level / 10))",
        ),
        term(
            layers,
            "base._vialWP",
            "Weapon Power vial",
            "computeVialByKey('WeaponPOW')",
        ),
        term(
            layers,
            "base._fam16",
            "Family Weapon Power",
            "computeFamBonusQTYs[16]",
        ),
        term(
            layers,
            "base._starWP",
            "Star Signs: Weapon Power",
            "computeStarSignBonus('WepPow')",
        ),
        term(
            layers,
            "base._arc17",
            "Arcade Weapon Power",
            "arcadeBonus(17)",
        ),
        term(
            layers,
            "base._t530",
            "Talent 530 skill scaling",
            "talent(530) × floor(Lv0[12] / 10)",
        ),
        term(
            layers,
            "base._t140",
            "Talent 140 skill scaling",
            "talent(140) × floor(Lv0[10] / 10)",
        ),
        term(
            layers,
            "base._t170",
            "Talent 170 skill scaling",
            "talent(170) × floor(Lv0[15] / 10)",
        ),
        term(
            layers,
            "base._t320",
            "Talent 320 skill scaling",
            "talent(320) × floor(Lv0[13] / 10)",
        ),
        term(
            layers,
            "base._t500",
            "Talent 500 skill scaling",
            "talent(500) × floor(Lv0[14] / 10)",
        ),
        term(
            layers,
            "base._t365",
            "Breeding Weapon Power",
            "talent(365) × log(max(1, PetsStored[0][2]))",
        ),
    ]);

    let additions = FormulaNode::new(
        "Base Damage Additions",
        value(layers, "base.gfBaseDmg")
            + value(layers, "base.arcade0")
            + value(layers, "base.vault0")
            + value(layers, "base.statue0")
            + value(layers, "base.boxBaseDmg")
            + value(layers, "base.card4")
            + value(layers, "base.stampBaseDmg")
            + value(layers, "base.owlBonus1")
            + value(layers, "base.sigil4")
            + value(layers, "base.halloweenCard"),
    )
    .with_source(format!("{DESCRIPTOR} · DDL0 additive sources"))
    .with_children([
        term(
            layers,
            "base.gfBaseDmg",
            "Golden food: Base Damage",
            "goldFoodBonuses('BaseDamage')",
        ),
        term(
            layers,
            "base.arcade0",
            "Arcade Base Damage",
            "arcadeBonus(0)",
        ),
        term(layers, "base.vault0", "Vault 0", "vaultUpgBonus(0)"),
        term(
            layers,
            "base.statue0",
            "Power Statue",
            "computeStatueBonusGiven(0)",
        ),
        term(
            layers,
            "base.boxBaseDmg",
            "Post Office: Base Damage",
            "computeBoxReward('basedmg')",
        ),
        term(
            layers,
            "base.card4",
            "Base Damage cards",
            "computeCardBonusByType(4)",
        ),
        term(
            layers,
            "base.stampBaseDmg",
            "Base Damage stamps",
            "computeStampBonusOfTypeX('BaseDmg')",
        ),
        term(
            layers,
            "base.owlBonus1",
            "Owl Base Damage",
            "computeOwlBonus(1)",
        ),
        term(layers, "base.sigil4", "Base Damage sigil", "sigilBonus(4)"),
        term(
            layers,
            "base.halloweenCard",
            "Halloween card level",
            "computeCardLv('fallEvent2')",
        ),
    ]);

    let bubbles = FormulaNode::new(
        "HP, MP and Speed Bubbles",
        value(layers, "base.bubbleHP")
            + value(layers, "base.bubbleMP")
            + value(layers, "base.bubbleSPD"),
    )
    .with_source(format!("{DESCRIPTOR} · bdmgHP / bdmgMP / bdmgSPD"))
    .with_children([
        term(
            layers,
            "base.bubbleHP",
            "HP-scaled base damage",
            "bdmgHP × log(max(HP - 250, 1))",
        )
        .with_children([term(layers, "base.hpMax", "Max HP", "max-hp")]),
        term(
            layers,
            "base.bubbleMP",
            "MP-scaled base damage",
            "bdmgMP × log(max(MP - 150, 1))",
        )
        .with_children([term(layers, "base.mpMax", "Max MP", "max-mp")]),
        term(
            layers,
            "base.bubbleSPD",
            "Speed-scaled base damage",
            "bdmgSPD × log2(max(speed - 0.1, 0)) / 0.25",
        )
        .with_children([term(
            layers,
            "base.playerSpeed",
            "Movement speed",
            "movement-speed",
        )]),
    ]);

    let softcap = FormulaNode::new("Softcap", value(layers, "base.preSoftcap"))
        .with_format(ValueFormat::Raw)
        .with_input("4000 + excess^0.91, then 15000 + excess^0.84")
        .with_source(format!("{DESCRIPTOR} · DDL0 softcap"))
        .with_children([
            term(
                layers,
                "base.preSoftcapRaw",
                "Before softcaps",
                "_preSoftcapDDL0",
            ),
            term(layers, "base.preSoftcap", "After softcaps", "DDL0"),
        ]);

    FormulaNode::new(
        "Base Damage (with softcap)",
        value(layers, "base.preSoftcap"),
    )
    .with_note("DDL0 · after softcaps")
    .with_source(format!("{DESCRIPTOR} · DamageDealtLIST[0]"))
    .with_children([stat, weapon, additions, bubbles, softcap])
}

/// DDL1: the additive pool, the four multipliers and the softcaps.
fn percent(layers: &Layers) -> FormulaNode {
    let pool = FormulaNode::new(
        "Percentage Damage additive pool",
        value(layers, "percent.addBase"),
    )
    .with_source(format!("{DESCRIPTOR} · DamageDealtLIST[1]"))
    .with_children([
        term(
            layers,
            "percent.statPow",
            "Primary stat scaling",
            "STATtype^0.7",
        ),
        term(
            layers,
            "percent.vault27",
            "Vault 27 × kill count",
            "vaultUpgBonus(27) × VaultKillzTotal(6)",
        ),
        term(
            layers,
            "percent.vault15",
            "Vault 15 × saved progress",
            "vaultUpgBonus(15) × OptLacc[338]",
        ),
        term(
            layers,
            "percent.vault10",
            "Vault 10",
            "0.4 × vaultUpgBonus(10)",
        ),
        term(
            layers,
            "percent.stampPctDmg",
            "Percentage Damage stamps",
            "computeStampBonusOfTypeX('PctDmg')",
        ),
        term(
            layers,
            "percent.farmRank14",
            "Land Rank damage",
            "farmRankUpgBonus(14)",
        ),
        term(
            layers,
            "percent.statue22",
            "Damage Statue",
            "computeStatueBonusGiven(22)",
        ),
        term(
            layers,
            "percent.hpLog",
            "HP-scaled damage talent",
            "log(max(HP, 1)) × talent 86",
        ),
        term(
            layers,
            "percent.mpLog",
            "MP-scaled damage talent",
            "log(max(MP, 1)) × talent 446",
        ),
        term(
            layers,
            "percent.owlBonus2",
            "Owl Damage",
            "computeOwlBonus(2)",
        ),
        term(
            layers,
            "percent.rooBonus4",
            "Roo Damage",
            "computeRooBonus(4)",
        ),
        term(
            layers,
            "percent.bubbaRoG4",
            "Bubba Damage",
            "bubbaRoGBonuses(4)",
        ),
        term(layers, "percent.vault80", "Vault 80", "vaultUpgBonus(80)"),
    ]);

    let chain = FormulaNode::new(
        "Talent and bubble damage pool",
        value(layers, "percent.talentChainSum"),
    )
    .with_source(format!("{DESCRIPTOR} · TalentCalc chain"))
    .with_children([
        term(
            layers,
            "percent.bubbleW12",
            "Bubble: Dmg Of The Sun",
            "bubbleValByKey('W12')",
        ),
        term(
            layers,
            "percent.bubbleA12",
            "Bubble: Dmg Of The Moon",
            "bubbleValByKey('A12')",
        ),
        term(
            layers,
            "percent.bubbleM12",
            "Bubble: Dmg Of The Soul",
            "bubbleValByKey('M12')",
        ),
        term(
            layers,
            "percent.tc31",
            "Lowest-skill damage",
            "TalentCalc(31) = talent 31 × floor(lowest skill level / 5)",
        ),
        term(
            layers,
            "percent.tc110",
            "Kill-threshold damage",
            "TalentCalc(110) = talent 110 × min(100K-kill maps, talent 110 tab 2)",
        ),
        term(
            layers,
            "percent.tc125",
            "Refinery damage",
            "TalentCalc(125) = talent 125 × Refinery[3..8][1]",
        ),
        term(
            layers,
            "percent.tc485",
            "Vial damage",
            "TalentCalc(485) = talent 485 × vials above level 3",
        ),
        term(
            layers,
            "percent.tc305d50",
            "Slab damage",
            "TalentCalc(305) = talent 305 × Cards1 / 50",
        ),
        term(
            layers,
            "percent.tc470d10",
            "Stamp damage",
            "TalentCalc(470) = talent 470 × owned stamps / 10",
        ),
        term(
            layers,
            "percent.bUpg57",
            "Cavern schematic 57",
            "B_UPG(57) = 20 × floor(log(Holes[9][1]))",
        ),
        term(
            layers,
            "percent.t290speed",
            "Speed groups",
            "talent 290 × floor(min(speed - 1, 10) / 0.15)",
        ),
        term(
            layers,
            "percent.tc656",
            "Completed Equinox dreams",
            "TalentCalc(656) = talent 656 × completed dreams",
        ),
        term(
            layers,
            "percent.quests658",
            "Quest cap",
            "min(totalQuestsComplete, talent 658)",
        ),
        term(
            layers,
            "percent.divMinor7",
            "Divinity minor damage",
            "computeDivinityMinor(character, 7)",
        ),
        term(
            layers,
            "percent.talent463",
            "Talent 463",
            "talent 463 tab 2",
        ),
    ]);

    let multipliers = FormulaNode::new(
        "Multipliers",
        value(layers, "percent.smithMult")
            * value(layers, "percent.winMult")
            * value(layers, "percent.talentChainMult")
            * value(layers, "percent.gfDamage"),
    )
    .with_format(ValueFormat::Multiplier)
    .with_source(format!("{DESCRIPTOR} · DDL1 multipliers"))
    .with_children([
        FormulaNode::new("Smithing multiplier", value(layers, "percent.smithMult"))
            .with_format(ValueFormat::Multiplier)
            .with_source(format!(
                "{DESCRIPTOR} · 1 + talent 284 × min(100, Smithing) / 10 / 100"
            )),
        FormulaNode::new("Summoning damage reward", value(layers, "percent.winMult"))
            .with_format(ValueFormat::Multiplier)
            .with_source(format!("{DESCRIPTOR} · 1 + winBonus(0) / 100")),
        FormulaNode::new("Talent chain", value(layers, "percent.talentChainMult"))
            .with_format(ValueFormat::Multiplier)
            .with_source(format!("{DESCRIPTOR} · 1 + talentChainSum / 100"))
            .with_children([chain]),
        FormulaNode::new("Golden food: Damage", value(layers, "percent.gfDamage"))
            .with_format(ValueFormat::Multiplier)
            .with_source(format!(
                "{DESCRIPTOR} · 1 + goldFoodBonuses('Damage') / 100"
            )),
    ]);

    let softcap = FormulaNode::new("Softcap", value(layers, "percent.postSoftcap"))
        .with_input("100 + excess^0.86, then 2M^0.5, then 100M^0.3")
        .with_source(format!("{DESCRIPTOR} · DDL1 softcaps"))
        .with_children([
            term(
                layers,
                "percent.preSoftcap",
                "Before softcaps",
                "_preSoftcapDDL1",
            ),
            term(layers, "percent.postSoftcap", "After softcaps", "DDL1"),
        ]);

    FormulaNode::new(
        "Percentage Damage Multiplier",
        value(layers, "percent.postSoftcap"),
    )
    .with_format(ValueFormat::Multiplier)
    .with_note("DDL1 · after softcaps")
    .with_source(format!("{DESCRIPTOR} · DamageDealtLIST[1]"))
    .with_children([pool, multipliers, softcap])
}

/// DDL2: the pre-softcap groups, the six-stage softcap chain and the
/// post-softcap multipliers.
fn multiplier(layers: &Layers) -> FormulaNode {
    let pre = FormulaNode::new(
        "Multiplicative bonuses before softcaps",
        value(layers, "multiplier.preSoftcap"),
    )
    .with_format(ValueFormat::Multiplier)
    .with_source(format!("{DESCRIPTOR} · DDL2 groups"))
    .with_children([
        factor(
            layers,
            "multiplier.wb",
            "Construction damage floor",
            "1 + talent 508 × log(OLA[152]) / 100 × 1 + talent 208 × log(OLA[329]) / 100",
        ),
        factor(layers, "multiplier.vial7dmg", "Damage vial multiplier", "vial('7dmg')"),
        factor(
            layers,
            "multiplier.eclipse",
            "Eclipse Skull multiplier",
            "computeEclipseSkulls()",
        ),
        factor(
            layers,
            "multiplier.palette34",
            "Palette damage multiplier",
            "computePaletteBonus(34)",
        ),
        factor(
            layers,
            "multiplier.dream6",
            "Equinox dream multiplier",
            "1 + Dream[6] / 10",
        ),
        factor(
            layers,
            "multiplier.pristine0",
            "Pristine multiplier",
            "pristineBon(0)",
        ),
        factor(
            layers,
            "multiplier.summUpg79",
            "Summoning upgrade multiplier",
            "computeSummUpgBonus(79)",
        ),
        factor(
            layers,
            "multiplier.cglunkoDmg",
            "Cove multiplier",
            "Cglunko_upgBon(19)",
        ),
        factor(
            layers,
            "multiplier.group1",
            "Friends, Star Signs & Divinity",
            "active buff 108 + friend(0) + star sign PctDmg + Divinity 25 × talent 507 + level × talent 50",
        ),
        factor(
            layers,
            "multiplier.group2",
            "Grimoire, Lustre Set & Account Bonuses",
            "grimoire 35 + Lustre set + shrine 0 + monument 0,6 + box 12c/21c/23c + family 20 + artifact 25 + atom 9 + shiny 5 + MSA 0 + shimmer + crop + vault 41",
        ),
        factor(
            layers,
            "multiplier.group3",
            "Rift, Damage & Main Stats",
            "rift skill ETC + pctDmg bubbles + artifacts 2/8 + class stat bubbles + construction mastery + tome 0 + compass 48 + exotic 41 + OLA[419]",
        ),
        factor(
            layers,
            "multiplier.group4",
            "Talents, Salt Lick & Mainframe",
            "talent 6 + Salt Lick 9 + etc 45 + prayer 15 + mainframe 0/11/110 + artifacts 27/29 + B_UPG(84) + arcade 46 + OLA[435]",
        ),
        factor(
            layers,
            "multiplier.group5",
            "Companions & Cards",
            "companions 10/156 + damage cards 42 + card set 5",
        ),
        factor(
            layers,
            "multiplier.group6",
            "Pets, Meals, Achievements & Divinity",
            "Pet Arena 2/15 + damage chip + TotDmg meal + achievements + Divinity blessings 7/8",
        ),
        FormulaNode::new("Damage penalties", value(layers, "multiplier.penaltyMult"))
            .with_format(ValueFormat::Multiplier)
            .with_source(format!("{DESCRIPTOR} · talent 24, buff 124, prayers 6/13")),
        factor(layers, "multiplier.minehead0", "Minehead multiplier", "mineheadBonusQTY(0)"),
        factor(
            layers,
            "multiplier.mineheadWp",
            "Minehead weapon multiplier",
            "mineheadBonusQTY(4) × weapon Weapon_Power",
        ),
        factor(
            layers,
            "multiplier.jelly37",
            "Jelly reward multiplier",
            "jellyRewardBonus(37)",
        ),
        factor(
            layers,
            "multiplier.royalStatue0",
            "Royal Statue multiplier",
            "statueBonus(0)",
        ),
        FormulaNode::new(
            "Coral Kid Singularity",
            value(layers, "multiplier.coralMulti"),
        )
        .with_format(ValueFormat::Multiplier)
        .with_source(format!(
            "{DESCRIPTOR} · (1 + coralKidUpgradeEffect(11) / 100)^(Divinity 25 - 35)"
        )),
        factor(
            layers,
            "multiplier.weekly",
            "Weekly boss multiplier",
            "min(150, WeeklyBoss.g)",
        ),
    ]);

    let softcap = FormulaNode::new(
        "Multiplicative bonuses after softcaps",
        value(layers, "multiplier.postSoftcap"),
    )
    .with_format(ValueFormat::Multiplier)
    .with_input("100 + excess^0.86, then 20M^0.8, 500M^0.6, 2B^0.45, 15B^0.36, 60B^0.28")
    .with_source(format!("{DESCRIPTOR} · DDL2 softcaps"))
    .with_children([
        term(
            layers,
            "multiplier.preSoftcap",
            "Before softcaps",
            "_ddl2PreSoftcap",
        ),
        term(
            layers,
            "multiplier.postSoftcap",
            "After softcaps",
            "_debug.postSoftcap",
        ),
    ]);

    let post = FormulaNode::new(
        "Post-softcap multipliers",
        value(layers, "multiplier.final"),
    )
    .with_format(ValueFormat::Multiplier)
    .with_source(format!("{DESCRIPTOR} · DDL2 post-softcap chain"))
    .with_children([
        factor(layers, "multiplier.etc72", "Etc 72", "etcBonus('72')"),
        factor(layers, "multiplier.etc75", "Etc 75", "etcBonus('75')"),
        factor(layers, "multiplier.etc104", "Etc 104", "etcBonus('104')"),
        factor(
            layers,
            "multiplier.votingDmg",
            "Server vote",
            "votingBonusz(1)",
        ),
        factor(
            layers,
            "multiplier.fountainDmg",
            "Fountain",
            "bonTOT(Holes[31], Holes[32], 1, 16)",
        ),
        factor(
            layers,
            "multiplier.rog49",
            "Sushi reward",
            "rogBonusQTY(49)",
        ),
        FormulaNode::new("Sticker multiplier", value(layers, "multiplier.stickerDmg"))
            .with_format(ValueFormat::Multiplier)
            .with_source(format!("{DESCRIPTOR} · computeStickerDmgMulti()")),
        factor(
            layers,
            "multiplier.ola232Bonus",
            "Account progression multiplier",
            "10 × floor((96 + OLA[232]) / 100)",
        ),
        factor(
            layers,
            "multiplier.card96",
            "Card multiplier",
            "computeCardBonusByType(96)",
        ),
        factor(
            layers,
            "multiplier.tomeBonus6",
            "Tome and achievements",
            "tome 6 + achievements 371/384",
        ),
        FormulaNode::new(
            "Killroy multiplier",
            1.0 + value(layers, "multiplier.killroy"),
        )
        .with_format(ValueFormat::Multiplier)
        .with_source(format!("{DESCRIPTOR} · computeKillroyDMG")),
        FormulaNode::new(
            "Post-softcap companions",
            value(layers, "multiplier.compMult"),
        )
        .with_format(ValueFormat::Multiplier)
        .with_source(format!("{DESCRIPTOR} · companions 12/33/160/168/170")),
        FormulaNode::new(
            "Crystal card multiplier",
            1.0 + (1.5 * value(layers, "multiplier.crystal6Bonus")).min(15.0) / 100.0,
        )
        .with_format(ValueFormat::Multiplier)
        .with_source(format!("{DESCRIPTOR} · min(1.5 × CardLv('Crystal6'), 15)")),
        factor(
            layers,
            "multiplier.meritoc5",
            "Meritocracy multiplier",
            "computeMeritocBonusz(5)",
        ),
        FormulaNode::new(
            "Eclectic Sigil multiplier",
            1.0 + value(layers, "multiplier.green") / 100.0,
        )
        .with_format(ValueFormat::Multiplier)
        .with_source(format!(
            "{DESCRIPTOR} · min(100, totalGreenSigils × gridBonusFinal(127))"
        )),
        factor(
            layers,
            "multiplier.famBonus80",
            "Family multiplier",
            "FamBonusQTYs[80]",
        ),
        FormulaNode::new(
            "Clamworks Scurge divisor",
            value(layers, "multiplier.scurgeDivisor"),
        )
        .with_format(ValueFormat::Multiplier)
        .with_source(format!("{DESCRIPTOR} · 100^Scurge at map 306")),
    ]);

    FormulaNode::new(
        "Multiplicative Damage Bonuses",
        value(layers, "multiplier.final"),
    )
    .with_format(ValueFormat::Multiplier)
    .with_note("DDL2 · after softcaps")
    .with_source(format!("{DESCRIPTOR} · DamageDealtLIST[2]"))
    .with_children([pre, softcap, post])
}
