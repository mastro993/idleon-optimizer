//! The Sneaking formula breakdown trees.
//!
//! Ported from `buildStealthBreakdown`, `buildJadeBreakdown`,
//! `buildSneakingExpBreakdown`, `buildDoorRateBreakdown` and the runtime tree
//! in `_renderFormulaBreakdowns` (`sneaking-calc.html`). Every group names the
//! upstream formula it rewrites; additive groups' children sum to the group's
//! value and multiplier groups' children multiply to it.

use super::{ActionMode, Loadout};
use crate::formula::{FormulaNode, ValueFormat};

/// `1 + bonus / 100`, with the bonus shown under the factor.
fn pct_factor(label: &str, bonus: f64, source: &str) -> FormulaNode {
    FormulaNode::new(label, 1.0 + bonus / 100.0)
        .with_format(ValueFormat::Multiplier)
        .with_input(format!("1 + {}%", ValueFormat::Raw.format(bonus)))
        .with_source(source)
        .with_children([FormulaNode::leaf("Bonus", bonus, ValueFormat::Percent)])
}

/// The stealth tree from `buildStealthBreakdown`.
pub fn stealth(loadout: &Loadout, twin: usize) -> FormulaNode {
    let Some(entry) = loadout.twin(twin) else {
        return FormulaNode::new("Stealth", 0.0).with_format(ValueFormat::Raw);
    };
    let account = loadout.account();
    let base = 10.0 + loadout.ninja_bonus(13) * f64::from(entry.level);
    let bonuses = loadout.bonuses(twin);
    let charm = (1.0 + bonuses.get(4) / 100.0)
        * (1.0 + bonuses.get(7) / 100.0)
        * (1.0 + bonuses.get(17) / 100.0);
    let shh = 1.0 + loadout.ninja_bonus(23) / 100.0;
    let core = loadout.account_multi(twin) * charm * shh;
    let ally_children = loadout.ally_contributions(twin);
    let ally = 1.0 + ally_children.iter().map(|entry| entry.total).sum::<f64>() / 100.0;
    let flowers = loadout.funeral_percent(entry.floor);
    let funeral = 1.0 + flowers / 100.0;

    let base_node = FormulaNode::new("Base Stealth", base)
        .with_format(ValueFormat::Raw)
        .with_note("10 + Way of Stealth × Sneaking level")
        .with_children([
            FormulaNode::leaf("Base", 10.0, ValueFormat::Raw),
            FormulaNode::new(
                "Way of Stealth contribution",
                loadout.ninja_bonus(13) * f64::from(entry.level),
            )
            .with_format(ValueFormat::Percent)
            .with_children([
                FormulaNode::leaf("Bonus per level", loadout.ninja_bonus(13), ValueFormat::Raw)
                    .with_source("NK 13 · Way of Stealth"),
                FormulaNode::leaf("Sneaking level", f64::from(entry.level), ValueFormat::Raw)
                    .with_source(format!("Lv0_{twin}[17]")),
            ]),
        ]);

    let mut core_children = vec![
        FormulaNode::new(
            "Land Rank Stealth",
            1.0 + account.farm_rank_bonus * account.farm_level / 100.0,
        )
        .with_format(ValueFormat::Multiplier)
        .with_note(
            "1 + Land Rank bonus × login Farming level / 100. Talent 207 is read at its raw \
             level; the All-Talent levels are not modelled.",
        )
        .with_children([
            FormulaNode::leaf(
                "Land Rank bonus per Farming level",
                account.farm_rank_bonus,
                ValueFormat::Percent,
            )
            .with_source("FarmRank[2][4] · NinjaInfo[36] · exotic 14 · talent 207"),
            FormulaNode::leaf(
                "Login character Farming level",
                account.farm_level,
                ValueFormat::Raw,
            )
            .with_source("Lv0_<login>[16]"),
            FormulaNode::leaf(
                "Combined bonus",
                account.farm_rank_bonus * account.farm_level,
                ValueFormat::Percent,
            ),
        ]),
        pct_factor(
            "Equipped Charm: EXP, Jade & Stealth",
            bonuses.get(7),
            "Charm bonus type 7",
        ),
        pct_factor(
            "Compass: Sneaking Stealth",
            account.compass_45,
            "CompassUpg 45",
        ),
        pct_factor(
            "Gambit: Sneaking Stealth",
            account.gambit_11,
            "HolesInfo 71 · Gambit milestone 11",
        )
        .with_note("The Gambit point chain is not modelled; this term reads zero."),
        pct_factor(
            "Equipped Charm: Total Stealth",
            bonuses.get(4),
            "Charm bonus type 4",
        ),
        pct_factor(
            "Equipped Charm: Total Stealth II",
            bonuses.get(17),
            "Charm bonus type 17",
        ),
        pct_factor(
            "Gold Inventory: Total Stealth",
            loadout.gold_bonuses().get(20),
            "Gold inventory bonus 20",
        ),
        FormulaNode::new(
            "Alchemy + Star Sign",
            1.0 + (account.bubble_a10 + account.star_sign_73) / 100.0,
        )
        .with_format(ValueFormat::Multiplier)
        .with_note("1 + (bubble + star sign) / 100")
        .with_children([
            FormulaNode::leaf(
                "A10AllCharz bubble",
                account.bubble_a10,
                ValueFormat::Percent,
            )
            .with_source("CauldronInfo[2][10] · MATTY STAFFORD"),
            FormulaNode::leaf(
                "S. Tealio star sign",
                account.star_sign_73,
                ValueFormat::Percent,
            )
            .with_source("StarSigns 73 · Stealth"),
            FormulaNode::leaf(
                "Combined bonus",
                account.bubble_a10 + account.star_sign_73,
                ValueFormat::Percent,
            ),
        ]),
        pct_factor(
            "Sneaking Statue",
            account.statue_26,
            "Statue 26 · computeStatueBonusGiven",
        ),
        FormulaNode::new(
            "Crystal 5 Card",
            1.0 + 4.0 * account.card_crystal5_level / 100.0,
        )
        .with_format(ValueFormat::Multiplier)
        .with_note("1 + 4 × card level / 100")
        .with_children([
            FormulaNode::leaf("Card level", account.card_crystal5_level, ValueFormat::Raw)
                .with_source("Cards0 · Crystal5"),
            FormulaNode::leaf("Bonus per level", 4.0, ValueFormat::Percent),
            FormulaNode::leaf(
                "Combined bonus",
                4.0 * account.card_crystal5_level,
                ValueFormat::Percent,
            ),
        ]),
        FormulaNode::new(
            "Achievement 368",
            1.0 + 5.0 * account.achievement_368 / 100.0,
        )
        .with_format(ValueFormat::Multiplier)
        .with_children([
            FormulaNode::leaf(
                "Achievement status",
                account.achievement_368,
                ValueFormat::Raw,
            )
            .with_source("AchieveReg[368]"),
            FormulaNode::leaf(
                "Combined bonus",
                5.0 * account.achievement_368,
                ValueFormat::Percent,
            ),
        ]),
        pct_factor(
            "Gemstone: Stealth",
            loadout.gemstone(0),
            "NjGem0 at OptLacc[233]",
        ),
        pct_factor(
            "Voting: Sneaking Stealth",
            account.voting_25,
            "Voting bonus 25",
        ),
        pct_factor(
            "Lamp: Sneaking Stealth",
            account.lamp_21,
            "Holes[21][6] · lamp",
        ),
        FormulaNode::new("Holes Upgrade 54", account.hole_upgrade_54.max(1.0))
            .with_format(ValueFormat::Multiplier)
            .with_note("max(1, B_UPG 54)")
            .with_children([FormulaNode::leaf(
                "Raw multiplier",
                account.hole_upgrade_54,
                ValueFormat::Multiplier,
            )
            .with_source("Holes[9][15] · Holes[13][54]")]),
        FormulaNode::new(
            "Fractal Island",
            1.0 + 2.0 * account.fractal_unlocked * f64::from(entry.level) / 100.0,
        )
        .with_format(ValueFormat::Multiplier)
        .with_note("1 + 2 × unlock × Sneaking level / 100")
        .with_children([
            FormulaNode::leaf("Unlock status", account.fractal_unlocked, ValueFormat::Raw)
                .with_source("OptLacc[184]"),
            FormulaNode::leaf("Sneaking level", f64::from(entry.level), ValueFormat::Raw),
            FormulaNode::leaf(
                "Combined bonus",
                2.0 * account.fractal_unlocked * f64::from(entry.level),
                ValueFormat::Percent,
            ),
        ]),
        pct_factor(
            "Emperor: Sneaking Stealth",
            account.emperor_0,
            "Emperor bonus 0",
        ),
        pct_factor("RoG: Sneaking Stealth", account.rog_32, "RoG bonus 32"),
        pct_factor(
            "Jelly Operator: Sneaking Stealth",
            account.jelly_16,
            "Jelly reward 16",
        ),
        pct_factor(
            "Shh! Ninja Knowledge",
            loadout.ninja_bonus(23),
            "NK 23 · Shh!",
        ),
        FormulaNode::new("Companion 163", 1.0 + 39.0 * account.companion_163)
            .with_format(ValueFormat::Multiplier)
            .with_note("1 + 39 × owned")
            .with_children([
                FormulaNode::leaf("Owned", account.companion_163, ValueFormat::Raw)
                    .with_source("companion roster"),
                FormulaNode::leaf(
                    "Multiplier contribution",
                    39.0 * account.companion_163,
                    ValueFormat::Percent,
                ),
            ]),
    ];
    for name in ["Alchemy + Star Sign"] {
        if let Some(index) = core_children.iter().position(|node| node.label == name) {
            let node = core_children.remove(index);
            core_children.insert(5, node);
        }
    }
    let core_node = FormulaNode::new("Core Multipliers", core)
        .with_format(ValueFormat::Multiplier)
        .with_note("Product of all listed factors")
        .with_children(core_children);

    let mut ally_children: Vec<FormulaNode> = ally_children
        .iter()
        .map(|ally| {
            FormulaNode::new(ally.name.clone(), ally.total)
                .with_format(ValueFormat::Percent)
                .with_children([
                    FormulaNode::leaf("Floor Stealth charm", ally.type8, ValueFormat::Percent),
                    FormulaNode::leaf("Floor Stealth charm II", ally.type16, ValueFormat::Percent),
                ])
        })
        .collect();
    if ally_children.is_empty() {
        ally_children.push(FormulaNode::leaf(
            "No other twins on this floor",
            0.0,
            ValueFormat::Percent,
        ));
    }
    let ally_node = FormulaNode::new("Ally Stealth Multiplier", ally)
        .with_format(ValueFormat::Multiplier)
        .with_note("1 + sum of ally floor-stealth charm bonuses / 100")
        .with_children(ally_children);

    let flower_node = FormulaNode::new("Funeral Flowers Multiplier", funeral)
        .with_format(ValueFormat::Multiplier)
        .with_note("1 + flowers × bonus per flower / 100")
        .with_children([
            FormulaNode::leaf(
                format!("Flowers on floor {}", entry.floor),
                loadout.flowers.get(entry.floor).copied().unwrap_or(0.0),
                ValueFormat::Raw,
            )
            .with_source("Spelunk[15]"),
            FormulaNode::leaf(
                "Bonus per flower",
                loadout.ninja_bonus(21),
                ValueFormat::Percent,
            )
            .with_source("NK 21 · Funeral Flowers"),
            FormulaNode::leaf("Combined bonus", flowers, ValueFormat::Percent),
        ]);

    FormulaNode::new(format!("{} Stealth", entry.name), loadout.stealth(twin))
        .with_format(ValueFormat::Raw)
        .with_note("Base Stealth × Core Multipliers × Ally Multiplier × Funeral Multiplier")
        .with_input(format!(
            "{} × {:.4} × {:.4} × {:.4}",
            ValueFormat::Raw.format(base),
            core,
            ally,
            funeral
        ))
        .with_children([base_node, core_node, ally_node, flower_node])
}

/// The Jade tree from `buildJadeBreakdown`.
pub fn jade(loadout: &Loadout, twin: usize) -> FormulaNode {
    let Some(entry) = loadout.twin(twin) else {
        return FormulaNode::new("Jade per Hour", 0.0).with_format(ValueFormat::Rate);
    };
    let hour = loadout.jade_hour(twin);
    let coin = hour.coin;
    let name = entry.name.clone();

    let floor_node = FormulaNode::new("Floor Base Jade", coin.base_jade)
        .with_format(ValueFormat::Raw)
        .with_children([
            FormulaNode::leaf("Floor", entry.floor as f64, ValueFormat::Raw),
            FormulaNode::leaf("Mastery", f64::from(loadout.mastery().0), ValueFormat::Raw),
        ]);

    let u = FormulaNode::new("Sneaking Level Jade", coin.u)
        .with_format(ValueFormat::Multiplier)
        .with_note("1 + NK 8 × Sneaking level / 100")
        .with_children([
            FormulaNode::leaf(
                "Ninja Knowledge 8 bonus per level",
                coin.nk8,
                ValueFormat::Percent,
            ),
            FormulaNode::leaf("Sneaking level", coin.sneak_level, ValueFormat::Raw),
            FormulaNode::leaf(
                "Combined bonus",
                coin.nk8 * coin.sneak_level,
                ValueFormat::Percent,
            ),
        ]);
    let solo = FormulaNode::new(
        "Solo Jade Charms",
        1.0 + (coin.charm14 + coin.charm19) / 100.0,
    )
    .with_format(ValueFormat::Multiplier)
    .with_children([
        FormulaNode::leaf("Jade charm (type 14)", coin.charm14, ValueFormat::Percent).with_note(
            if coin.solo {
                "3× solo multiplier applied"
            } else {
                "Twin is not solo"
            },
        ),
        FormulaNode::leaf(
            "EXP + Jade charm (type 19)",
            coin.charm19,
            ValueFormat::Percent,
        )
        .with_note(if coin.solo {
            "3× solo multiplier applied"
        } else {
            "Twin is not solo"
        }),
        FormulaNode::leaf(
            "Solo status",
            f64::from(u8::from(coin.solo)),
            ValueFormat::Raw,
        ),
    ]);
    let vial_group = FormulaNode::new(
        "Vial + Meal + Card",
        1.0 + (coin.vial + coin.meal + 4.0 * coin.card_level) / 100.0,
    )
    .with_format(ValueFormat::Multiplier)
    .with_children([
        FormulaNode::leaf("Jade Vial", coin.vial, ValueFormat::Percent)
            .with_source("Alchemy vial · 6Jade"),
        FormulaNode::leaf("Jade Meal", coin.meal, ValueFormat::Percent)
            .with_source("Meal · zJade · cooking multiplier"),
        FormulaNode::new("W6B4 Card", 4.0 * coin.card_level)
            .with_format(ValueFormat::Percent)
            .with_children([
                FormulaNode::leaf("Card level", coin.card_level, ValueFormat::Raw),
                FormulaNode::leaf("Bonus per level", 4.0, ValueFormat::Percent),
            ]),
    ]);
    let primary = FormulaNode::new("Primary Jade Multipliers", coin.o)
        .with_format(ValueFormat::Multiplier)
        .with_note("Product of the listed factors")
        .with_children([
            FormulaNode::leaf(
                "Gem Shop Jade Multiplier",
                coin.gem_shop,
                ValueFormat::Multiplier,
            )
            .with_source("2^GemItemsPurchased[10]"),
            solo,
            pct_factor("Monument: Jade", coin.monument, "MonumentROGbonuses(2, 2)"),
            pct_factor("Pristine Charm: Jade", coin.pristine, "Pristine charm 13"),
            pct_factor("Ninja Knowledge 28", coin.nk28, "NK 28 · Jade"),
            pct_factor("Gold Inventory: Jade", coin.gold9, "Gold inventory bonus 9"),
            FormulaNode::new("Killroy Jade Multiplier", coin.killroy.max(1.0))
                .with_format(ValueFormat::Multiplier)
                .with_source("KillroyBonuses(2) · OptLacc[230]"),
            vial_group,
        ]);
    let equipped = FormulaNode::new(
        "Equipped Jade Charms",
        1.0 + (coin.charm6 + coin.charm7 + coin.charm15) / 100.0,
    )
    .with_format(ValueFormat::Multiplier)
    .with_note("1 + sum / 100")
    .with_children([
        FormulaNode::leaf(
            "Jade at 0% Detection (type 6)",
            coin.charm6,
            ValueFormat::Percent,
        ),
        FormulaNode::leaf(
            "EXP, Jade & Stealth (type 7)",
            coin.charm7,
            ValueFormat::Percent,
        ),
        FormulaNode::leaf("Jade (type 15)", coin.charm15, ValueFormat::Percent),
    ]);
    let slab = FormulaNode::new("Slab + Stamp", coin.g)
        .with_format(ValueFormat::Multiplier)
        .with_note("1 + (Slab + Stamp) / 100")
        .with_children([
            FormulaNode::leaf("Slab Bonus", coin.slab, ValueFormat::Percent)
                .with_source("SlabboBonus(4) · Cards1"),
            FormulaNode::leaf("Jade Coin Stamps", coin.stamp, ValueFormat::Percent)
                .with_source("StampLv · JadeCoin"),
        ]);
    let mut account_children = vec![
        FormulaNode::new(
            "MSA + Sigil + Arcade + Vault",
            1.0 + (coin.msa + coin.sigil + coin.arcade + coin.vault) / 100.0,
        )
        .with_format(ValueFormat::Multiplier)
        .with_children([
            FormulaNode::leaf("MSA Bonus", coin.msa, ValueFormat::Percent)
                .with_source("MSA_Bonus(7) · TotemInfo"),
            FormulaNode::leaf("Sigil Bonus", coin.sigil, ValueFormat::Percent)
                .with_source("CauldronP2W[4] · sigil 23"),
            FormulaNode::leaf("Arcade Bonus", coin.arcade, ValueFormat::Percent)
                .with_source("ArcadeUpg[35]"),
            FormulaNode::leaf("Vault Bonus", coin.vault, ValueFormat::Percent)
                .with_source("UpgVault[81]"),
        ]),
        pct_factor("Jade Star Sign", coin.star, "StarSigns 75 · Jade"),
        FormulaNode::new("Task Shop Jade", 1.0 + coin.task / 100.0)
            .with_format(ValueFormat::Multiplier)
            .with_children([
                FormulaNode::leaf(
                    "Bonus per level",
                    loadout.account().jade.task_base,
                    ValueFormat::Percent,
                )
                .with_source("TaskShopDesc[5][3][11]"),
                FormulaNode::leaf(
                    "Task level",
                    loadout.account().jade.task_level,
                    ValueFormat::Raw,
                )
                .with_source("TaskZZ2[5][3]"),
                FormulaNode::leaf("Combined bonus", coin.task, ValueFormat::Percent),
            ]),
    ];
    if let Some(index) = account_children
        .iter()
        .position(|node| node.label == "Jade Star Sign")
    {
        let node = account_children.remove(index);
        account_children.push(node);
    }
    let account_node = FormulaNode::new("Account Jade Multipliers", coin.n)
        .with_format(ValueFormat::Multiplier)
        .with_children(account_children);
    let rift = FormulaNode::new("Rift + Compass + Companion", coin.y)
        .with_format(ValueFormat::Multiplier)
        .with_children([
            FormulaNode::new("Rift Skill Bonus", 1.0 + 10.0 * coin.rift / 100.0)
                .with_format(ValueFormat::Multiplier)
                .with_children([
                    FormulaNode::leaf("Rift value", coin.rift, ValueFormat::Raw)
                        .with_source("computeRiftSkillBonus(16, 1)"),
                    FormulaNode::leaf("Combined bonus", 10.0 * coin.rift, ValueFormat::Percent),
                ]),
            pct_factor("Compass: Jade", coin.compass, "CompassUpg 41"),
            FormulaNode::new("Companion 163", 1.0 + 99.0 * coin.companion)
                .with_format(ValueFormat::Multiplier)
                .with_children([FormulaNode::leaf("Owned", coin.companion, ValueFormat::Raw)]),
        ]);
    let achievements = FormulaNode::new("Jade Achievements", coin.r)
        .with_format(ValueFormat::Multiplier)
        .with_children([
            FormulaNode::leaf(
                "Achievement 366",
                3.0 * coin.achievement_366,
                ValueFormat::Percent,
            ),
            FormulaNode::leaf(
                "Achievement 369",
                5.0 * coin.achievement_369,
                ValueFormat::Percent,
            ),
            FormulaNode::leaf(
                "Achievement 367",
                7.0 * coin.achievement_367,
                ValueFormat::Percent,
            ),
        ]);
    let gem = FormulaNode::new("Gemstone + Meritoc + Exotic", coin.a)
        .with_format(ValueFormat::Multiplier)
        .with_children([
            pct_factor("Jade Gemstone", coin.gemstone, "NjGem1"),
            pct_factor("Meritoc: Jade", coin.meritoc, "MeritocBonus 6"),
            pct_factor("Exotic Crop: Jade", coin.exotic, "ExoticBonus 58"),
        ]);
    let gold = FormulaNode::new("Gold Inventory per 10 Sneaking Levels", coin.g_final)
        .with_format(ValueFormat::Multiplier)
        .with_children([
            FormulaNode::leaf("Bonus per 10 levels", coin.gold23, ValueFormat::Percent)
                .with_source("Gold inventory bonus 23"),
            FormulaNode::new("Sneaking level groups", 0.0)
                .with_format(ValueFormat::Raw)
                .with_children([FormulaNode::leaf(
                    "Sneaking level",
                    coin.sneak_level,
                    ValueFormat::Raw,
                )]),
            FormulaNode::leaf(
                "Combined bonus",
                coin.gold23 * (coin.sneak_level / 10.0).floor(),
                ValueFormat::Percent,
            ),
        ]);
    let multiplier = FormulaNode::new("Total Jade Multiplier", coin.multiplier)
        .with_format(ValueFormat::Multiplier)
        .with_note("Product of all Jade multiplier groups")
        .with_children([
            u,
            primary,
            equipped,
            slab,
            pct_factor("Crop Scientist: Jade", coin.crop, "computeCropSC(2)"),
            pct_factor("Summoning Winner: Jade", coin.win, "Sigil bonus 1"),
            account_node,
            rift,
            achievements,
            gem,
            gold,
        ]);
    let coin_node = FormulaNode::new("Jade per Successful Action", coin.coin)
        .with_format(ValueFormat::Raw)
        .with_note("Floor base Jade × total multiplier")
        .with_children([floor_node, multiplier]);
    let rate = FormulaNode::new("Game Display Actions per Hour", hour.actions_per_hour)
        .with_format(ValueFormat::Rate)
        .with_note(
            "Matches the game's Jade/hour display estimate; the runtime door throughput is shown in \
             the Door tab",
        )
        .with_children([
            FormulaNode::leaf(
                "Action Speed",
                loadout.action_speed(twin),
                ValueFormat::Raw,
            )
            .with_note("Raw actions per hour without detection or KO")
            .with_source("NLbonuses(4) + charm 2 + glove + gold 24"),
            FormulaNode::new("Action Hit Chance", 100.0 * hour.hit_chance)
                .with_format(ValueFormat::Percent)
                .with_note("1 - detection chance")
                .with_children([
                    FormulaNode::leaf(
                        "Detection Chance",
                        100.0 * coin.detection,
                        ValueFormat::Percent,
                    ),
                    FormulaNode::leaf(
                        "A detected action is consumed even when dodged",
                        0.0,
                        ValueFormat::Raw,
                    ),
                ]),
            FormulaNode::new("KO Chance per Attempt", 100.0 * hour.ko_chance)
                .with_format(ValueFormat::Percent)
                .with_note("Detection × (1 - dodge)")
                .with_children([
                    FormulaNode::leaf(
                        "Detection Chance",
                        100.0 * coin.detection,
                        ValueFormat::Percent,
                    ),
                    FormulaNode::leaf(
                        "Dodge Chance (capped at 90%)",
                        100.0 * hour.dodge,
                        ValueFormat::Percent,
                    ),
                ]),
            FormulaNode::leaf(
                "KO Duration (internal seconds)",
                loadout.ko_seconds(twin),
                ValueFormat::Raw,
            ),
            FormulaNode::leaf(
                "Expected KO cycles per successful action",
                if hour.detection_cycles.is_finite() {
                    hour.detection_cycles
                } else {
                    0.0
                },
                ValueFormat::Raw,
            )
            .with_note(if hour.detection_cycles.is_finite() {
                ""
            } else {
                "Infinite at 100% detection; displayed as 0 to keep the tree finite"
            }),
        ]);
    FormulaNode::new(format!("{name} Jade per Hour"), hour.jade_per_hour)
        .with_format(ValueFormat::Rate)
        .with_note("Jade per successful action × successful actions per hour")
        .with_children([coin_node, rate])
}

/// The EXP tree from `buildSneakingExpBreakdown`.
pub fn exp(loadout: &Loadout, twin: usize) -> FormulaNode {
    let Some(entry) = loadout.twin(twin) else {
        return FormulaNode::new("Sneaking EXP", 0.0).with_format(ValueFormat::Raw);
    };
    let detail = loadout.exp_action(twin);
    let recipient = loadout.twin(detail.recipient).map_or_else(
        || format!("Twin {}", detail.recipient + 1),
        |twin| twin.name.clone(),
    );
    let floor = FormulaNode::new("Floor Base EXP", detail.base)
        .with_format(ValueFormat::Raw)
        .with_children([
            FormulaNode::leaf("Floor", entry.floor as f64, ValueFormat::Raw),
            FormulaNode::leaf("Mastery", f64::from(loadout.mastery().0), ValueFormat::Raw),
        ]);
    let nk = FormulaNode::new("Ninja Knowledge", detail.nk)
        .with_format(ValueFormat::Multiplier)
        .with_note("(1 + NK 2 / 100) × (1 + NK 19 / 100)")
        .with_children([
            FormulaNode::leaf("Sneaking EXP", detail.nk2, ValueFormat::Percent),
            FormulaNode::leaf("Mastery Sneaking EXP", detail.nk19, ValueFormat::Percent),
        ]);
    let additive = FormulaNode::new("Additive Account Sources", detail.simple)
        .with_format(ValueFormat::Multiplier)
        .with_note("1 + sum / 100")
        .with_children([
            FormulaNode::leaf("Sneaking EXP Vial", detail.vial, ValueFormat::Percent)
                .with_source("Alchemy vial · 6SneakEXP"),
            FormulaNode::leaf("Sneaking EXP Meal", detail.meal, ValueFormat::Percent)
                .with_source("Meal · zSneakExp"),
            FormulaNode::leaf("Rift Skill Bonus", 25.0 * detail.rift, ValueFormat::Percent),
            FormulaNode::leaf("Pristine Charm", detail.pristine, ValueFormat::Percent),
            FormulaNode::leaf("W6A4 Card", 3.0 * detail.card, ValueFormat::Percent),
            FormulaNode::leaf("Mainframe", detail.mainframe, ValueFormat::Percent)
                .with_source("Lab mainframe 16"),
            FormulaNode::leaf("Sneaking EXP Stamps", detail.stamp, ValueFormat::Percent),
            FormulaNode::leaf("Sneaking EXP Star Sign", detail.star, ValueFormat::Percent),
            FormulaNode::leaf("Guild Bonus", detail.guild, ValueFormat::Percent)
                .with_source("Guild[0][14] · decay(30, 120)"),
            FormulaNode::leaf(
                "Talent 431 additive bonus",
                detail.talent_add,
                ValueFormat::Percent,
            )
            .with_note("Raw talent level; the All-Talent levels are not modelled"),
            FormulaNode::leaf("Achievement 370", detail.achievement, ValueFormat::Percent),
            FormulaNode::leaf("Voting Bonus", detail.vote, ValueFormat::Percent),
            FormulaNode::leaf("Vault Upgrade", detail.vault, ValueFormat::Percent),
        ]);
    let charms = FormulaNode::new("Equipped EXP Charms", detail.charms)
        .with_format(ValueFormat::Multiplier)
        .with_note("Solo bonuses include the native 3× multiplier when applicable")
        .with_children([
            FormulaNode::leaf("Solo Sneaking EXP", detail.charm13, ValueFormat::Percent),
            FormulaNode::leaf("Solo EXP + Jade", detail.charm19, ValueFormat::Percent),
            FormulaNode::leaf(
                "Highest-twin EXP transfer",
                detail.charm3,
                ValueFormat::Percent,
            ),
            FormulaNode::leaf("EXP, Jade & Stealth", detail.charm7, ValueFormat::Percent),
        ]);
    let penalty = FormulaNode::new("No-item-find Penalty", detail.no_item_penalty)
        .with_format(ValueFormat::Multiplier)
        .with_note("max(0, 1 - 100 × Item Find charm value)")
        .with_children([FormulaNode::leaf(
            "Item Find charm value",
            detail.charm5,
            ValueFormat::Raw,
        )]);
    FormulaNode::new(
        format!("{} Sneaking EXP per Successful Action", entry.name),
        detail.exp,
    )
    .with_format(ValueFormat::Raw)
    .with_note("Awarded only by successful normal and training-floor Sneaking actions")
    .with_children([
        floor,
        FormulaNode::leaf(
            "Shared Skill EXP Multiplier",
            detail.all_skill_multi,
            ValueFormat::Multiplier,
        )
        .with_source("Meritoc 10 · legend 20 · companion 32"),
        nk,
        pct_factor("Arcade", detail.arcade_52, "ArcadeUpg 52"),
        pct_factor(
            "Gold Inventory: Sneaking EXP",
            detail.gold_10,
            "Gold inventory bonus 10",
        ),
        pct_factor("Compass: Sneaking EXP", detail.compass_49, "CompassUpg 49"),
        pct_factor("Gemstone: Sneaking EXP", detail.gem_4, "NjGem4"),
        additive,
        charms,
        penalty,
        pct_factor("Summoning Winner", detail.win_bonus, "Sigil bonus 6"),
        FormulaNode::new("Companion: Smoke Devil", detail.companion)
            .with_format(ValueFormat::Multiplier)
            .with_note("1 + 1.5 × Companions(163)")
            .with_children([FormulaNode::leaf(
                "Effective companion value",
                detail.companion_163,
                ValueFormat::Raw,
            )]),
        FormulaNode::new("Talent 431 Multiplier", detail.talent_multi)
            .with_format(ValueFormat::Multiplier)
            .with_note("Raw level; the All-Talent levels are not modelled")
            .with_children([FormulaNode::leaf(
                "Best tab-2 talent value",
                detail.talent_multi,
                ValueFormat::Multiplier,
            )]),
        FormulaNode::leaf(
            format!("EXP Recipient: {recipient}"),
            1.0,
            ValueFormat::Multiplier,
        )
        .with_note("Charm transfer scans Sneaking level and saved Sneaking EXP in username order"),
    ])
}

/// The door tree from `buildDoorRateBreakdown`.
pub fn door(loadout: &Loadout, twin: usize) -> FormulaNode {
    let Some(entry) = loadout.twin(twin) else {
        return FormulaNode::new("Door Damage per Hour", 0.0).with_format(ValueFormat::Raw);
    };
    let damage = loadout.door_damage(twin);
    let state = loadout.door_state(entry.floor);
    let rate = loadout.door_rate(twin);
    let active = state.active && damage.can_hit_door;
    let hits = if active { rate.hits_per_hour } else { 0.0 };
    let per_hour = damage.damage * hits;
    let reason = if !damage.can_hit_door {
        "No Nunchaku equipped"
    } else if !state.active {
        "No active door at this floor or mastery"
    } else {
        ""
    };
    let mut children = vec![damage_tree(&damage, reason)];
    children.push(
        FormulaNode::new(
            "Hit Chance per Action",
            if active { rate.hit_chance } else { 0.0 },
        )
        .with_format(ValueFormat::Multiplier)
        .with_note(reason)
        .with_children([
            FormulaNode::leaf(
                "Detection Chance",
                100.0 * loadout.detection(twin),
                ValueFormat::Percent,
            ),
            FormulaNode::leaf(
                "Detected actions deal no damage, even when dodged",
                0.0,
                ValueFormat::Raw,
            ),
        ]),
    );
    children.push(
        FormulaNode::new("Door Hits per Hour", hits)
            .with_format(ValueFormat::Rate)
            .with_children([
                FormulaNode::leaf("Action Speed", loadout.action_speed(twin), ValueFormat::Raw),
                FormulaNode::new("Runtime Attempts per Hour", rate.attempts_per_hour)
                    .with_format(ValueFormat::Rate)
                    .with_children([
                        FormulaNode::leaf("Base progress per attempt", 3600.0, ValueFormat::Raw),
                        FormulaNode::new("Net KO progress per attempt", rate.expected_ko_progress)
                            .with_format(ValueFormat::Raw)
                            .with_children([
                                FormulaNode::leaf(
                                    "KO Chance per Attempt",
                                    100.0 * rate.ko_chance,
                                    ValueFormat::Percent,
                                ),
                                FormulaNode::leaf(
                                    "KO Duration (internal seconds)",
                                    loadout.ko_seconds(twin),
                                    ValueFormat::Raw,
                                ),
                            ]),
                    ]),
            ]),
    );
    FormulaNode::new(format!("{} Door Damage per Hour", entry.name), per_hour)
        .with_format(ValueFormat::Raw)
        .with_note(if reason.is_empty() {
            "Only a Nunchaku wielder attacks an undefeated floor door"
        } else {
            reason
        })
        .with_children(children)
}

fn damage_tree(damage: &super::DoorDamage, reason: &str) -> FormulaNode {
    if !damage.can_hit_door {
        return FormulaNode::new("No Nunchaku equipped", 0.0)
            .with_format(ValueFormat::Raw)
            .with_note(reason)
            .with_children([FormulaNode::new("Equipped weapon", 0.0)
                .with_format(ValueFormat::Raw)
                .with_note(damage.item_key.clone())
                .with_children([FormulaNode::leaf(
                    "Item level",
                    f64::from(damage.item_level),
                    ValueFormat::Raw,
                )])]);
    }
    FormulaNode::new("Nunchaku Weapon Damage", damage.damage)
        .with_format(ValueFormat::Raw)
        .with_note("Only a Nunchaku wielder attacks an undefeated floor door")
        .with_children([
            FormulaNode::new("Nunchaku Weapon Damage", damage.weapon_stat)
                .with_format(ValueFormat::Raw)
                .with_children([FormulaNode::new("Equipped weapon", 0.0)
                    .with_format(ValueFormat::Raw)
                    .with_note(damage.item_key.clone())
                    .with_children([FormulaNode::leaf(
                        "Item level",
                        f64::from(damage.item_level),
                        ValueFormat::Raw,
                    )])]),
            pct_factor(
                "Mahjong Boosters",
                damage.mahjong,
                "NK 5 · Mahjong Boosters",
            ),
            FormulaNode::new(
                "Gold Inventory: Damage per 10 Sneaking Levels",
                1.0 + damage.gold_per_ten * damage.sneak_tens / 100.0,
            )
            .with_format(ValueFormat::Multiplier)
            .with_children([
                FormulaNode::leaf(
                    "Bonus per 10 levels",
                    damage.gold_per_ten,
                    ValueFormat::Percent,
                ),
                FormulaNode::new("Sneaking level groups", damage.sneak_tens)
                    .with_format(ValueFormat::Raw)
                    .with_children([FormulaNode::leaf(
                        "Sneaking level",
                        damage.sneak_level,
                        ValueFormat::Raw,
                    )]),
                FormulaNode::leaf(
                    "Combined bonus",
                    damage.gold_per_ten * damage.sneak_tens,
                    ValueFormat::Percent,
                ),
            ]),
            pct_factor(
                "Gold Inventory: Door Damage",
                damage.gold_damage,
                "Gold inventory bonus 18",
            ),
            pct_factor("Hinge Buster Bubble", damage.y8, "Alchemy bubble Y8"),
            pct_factor("Gemstone: Door Damage", damage.gemstone, "NjGem2"),
            pct_factor(
                "True Battering",
                damage.true_battering,
                "NK 27 · True Battering",
            ),
        ])
}

/// The runtime tree from `_renderFormulaBreakdowns`.
pub fn runtime(loadout: &Loadout, twin: usize, item_batch_hours: f64) -> FormulaNode {
    let Some(entry) = loadout.twin(twin) else {
        return FormulaNode::new("Runtime", 0.0).with_format(ValueFormat::Raw);
    };
    let row = loadout.runtime(twin, item_batch_hours);
    let mode = match row.mode {
        ActionMode::Captive => "captive",
        ActionMode::Training => "training",
        ActionMode::Door => "door",
        ActionMode::Untie => "untie",
        ActionMode::Normal => "normal",
    };
    FormulaNode::new(format!("{} Runtime", entry.name), 0.0)
        .with_format(ValueFormat::Raw)
        .with_note(
            "Diagnostic root; child nodes use their own units under the saved floor and loadout",
        )
        .with_children([
            FormulaNode::leaf("Action Mode", 0.0, ValueFormat::Raw).with_note(mode),
            FormulaNode::leaf(
                "Runtime Attempts per Hour",
                row.attempts_per_hour,
                ValueFormat::Rate,
            ),
            FormulaNode::leaf(
                "Successful Actions per Hour",
                row.successful_actions_per_hour,
                ValueFormat::Rate,
            ),
            FormulaNode::leaf("Flowers per Hour", row.flowers_per_hour, ValueFormat::Rate)
                .with_note("Native-order probability states"),
            FormulaNode::new("Jade per Hour", row.jade_per_hour)
                .with_format(ValueFormat::Raw)
                .with_children([
                    FormulaNode::leaf(
                        "Jade per Successful Action",
                        row.jade_per_action,
                        ValueFormat::Raw,
                    ),
                    FormulaNode::leaf(
                        "Game Display Jade per Hour",
                        row.game_display_jade_per_hour,
                        ValueFormat::Raw,
                    )
                    .with_note("Native panel estimate; the runtime value is the parent node"),
                ]),
            FormulaNode::leaf(
                "Expected Items per Hour",
                row.items_per_hour,
                ValueFormat::Rate,
            )
            .with_note(format!(
                "{item_batch_hours} hour successful-action batch; expected-rate model"
            )),
            FormulaNode::new("Door Damage per Hour", row.door_damage_per_hour)
                .with_format(ValueFormat::Raw)
                .with_children([
                    FormulaNode::leaf("Damage per Hit", row.door_damage_per_hit, ValueFormat::Raw),
                    FormulaNode::leaf(
                        "Door Hits per Hour",
                        row.door_hits_per_hour,
                        ValueFormat::Rate,
                    ),
                ]),
            FormulaNode::new("Untie Progress per Hour", row.untie_progress_per_hour)
                .with_format(ValueFormat::Raw)
                .with_children([FormulaNode::leaf(
                    "Progress per Hit",
                    row.untie_progress_per_hit,
                    ValueFormat::Raw,
                )]),
            FormulaNode::leaf(
                "Taunting Mark Coupling",
                f64::from(u8::from(row.coupled)),
                ValueFormat::Raw,
            )
            .with_note(if row.coupled {
                "Expected team-progress model"
            } else {
                "Exact independent runtime formula"
            }),
        ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::Save;
    use serde_json::json;

    fn fixture() -> Loadout {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
        let text = std::fs::read_to_string(path).expect("the fixture is committed");
        let value = serde_json::from_str(&text).expect("the fixture is JSON");
        Loadout::from_save(&Save::new(value)).expect("the fixture carries Ninja rows")
    }

    fn small() -> Loadout {
        let ninja = vec![
            json!(["NjItem0", 0]),
            json!(["NjGl2", 10]),
            json!(["NjTr4", 100]),
            json!(["NjTr5", 50]),
        ];
        let mut rows = vec![json!([0, 0, 0, 0]); 12];
        rows.extend(ninja);
        rows.resize(103, json!(["Blank", 0]));
        rows.push(json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
        let save = json!({
            "Ninja": rows,
            "Spelunk": [null, null, null, null, null, null, null, null, null, null, null, null, null, null, json!([0, 0, 0, 0]), json!([0, 0, 0, 0]), null, null, json!([0, 0, 0, 0, 0, 0, 0])],
            "Lv0_0": json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100]),
            "Exp0_0": json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        });
        Loadout::from_save(&Save::new(save)).expect("the small save reads")
    }

    #[test]
    fn every_tree_has_nodes_and_a_root() {
        let loadout = fixture();
        for twin in 0..loadout.twins().len() {
            for tree in [
                stealth(&loadout, twin),
                jade(&loadout, twin),
                exp(&loadout, twin),
                door(&loadout, twin),
                runtime(&loadout, twin, 1.0),
            ] {
                assert!(tree.node_count() > 5);
                assert!(!tree.copy_text().is_empty());
                assert_eq!(tree.match_count("no such formula node"), 0);
            }
        }
    }

    #[test]
    fn the_stealth_group_products_reproduce_the_root() {
        let loadout = fixture();
        for twin in 0..loadout.twins().len() {
            let tree = stealth(&loadout, twin);
            assert!((tree.value - loadout.stealth(twin)).abs() < 1e-9);
            let product: f64 = tree.children.iter().map(|child| child.value).product();
            assert!(
                (product - tree.value).abs() <= 1e-6 * tree.value.abs().max(1.0),
                "twin {twin}: {product} != {}",
                tree.value
            );
        }
    }

    #[test]
    fn the_jade_root_matches_the_coin_times_the_rate() {
        let loadout = fixture();
        let tree = jade(&loadout, 0);
        let hour = loadout.jade_hour(0);
        assert!((tree.value - hour.jade_per_hour).abs() < 1e-6);
        let coin = &tree.children[0];
        assert!((coin.value - hour.coin.coin).abs() < 1e-6);
        assert!((coin.children[0].value - hour.coin.base_jade).abs() < 1e-6);
    }

    #[test]
    fn the_exp_root_matches_the_action_value() {
        let loadout = fixture();
        let tree = exp(&loadout, 0);
        let action = loadout.exp_action(0);
        assert!((tree.value - action.exp).abs() < 1e-6);
        assert_eq!(tree.match_count("Twin"), tree.match_count("Twin"));
    }

    #[test]
    fn the_door_root_matches_damage_times_hits() {
        let loadout = fixture();
        let tree = door(&loadout, 0);
        let damage = loadout.door_damage(0);
        let state = loadout.door_state(loadout.twin(0).unwrap().floor);
        let rate = loadout.door_rate(0);
        let expected = if state.active && damage.can_hit_door {
            damage.damage * rate.hits_per_hour
        } else {
            0.0
        };
        assert!((tree.value - expected).abs() < 1e-6);
    }

    #[test]
    fn search_finds_names_and_sources() {
        let loadout = fixture();
        let tree = stealth(&loadout, 0);
        assert!(tree.match_count("Compass") >= 1);
        assert_eq!(tree.match_count("no such node"), 0);
        let tree = jade(&loadout, 0);
        assert!(tree.match_count("Jade") > 0);
        assert!(tree.match_count("Gold Inventory") > 0);
    }

    #[test]
    fn a_save_without_the_account_fields_still_builds_every_tree() {
        let loadout = small();
        for tree in [
            stealth(&loadout, 0),
            jade(&loadout, 0),
            exp(&loadout, 0),
            door(&loadout, 0),
            runtime(&loadout, 0, 1.0),
        ] {
            assert!(tree.node_count() > 5);
            assert!(tree.value.is_finite());
        }
    }

    #[test]
    fn the_runtime_mode_follows_the_weapon_and_floor() {
        let loadout = fixture();
        assert_eq!(
            loadout.runtime(0, 1.0).mode,
            loadout.action_mode(0),
            "the runtime row names the native action mode"
        );
    }

    #[test]
    fn the_mastery_hp_is_read_from_the_floor_table() {
        assert!(crate::sneaking::data::data().info.jade_base.len() > 10);
        assert!(crate::sneaking::door_hp(1, 0) > 0.0);
    }
}
