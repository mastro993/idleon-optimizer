//! The account-wide terms the breakdown trees multiply in.
//!
//! The loadout owns the terms the `Ninja` rows carry: the base stealth, the
//! equipped charm factors, the floor allies, the funeral flowers, the Gold
//! inventory scan and the gemstone levels. This struct carries everything the
//! account supplies: the Land Rank, compass, Gambit, Atlantean bubble, star
//! signs, statues, cards, achievements, voting, lamps, Hole upgrades, fractal
//! island, Emperor, RoG, Jelly and companions.
//!
//! The account decoder (`idleon_account`) fills it; every field defaults
//! to the neutral value (zero for a percentage, one for a multiplier) so a save
//! without the system leaves the tree intact. The breakdown nodes name the
//! source field of each term, including the terms a save does not carry.

/// The all-talent multiplier `maxTalentBonus(432)` and the breakdown terms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Account {
    /// The all-talent multiplier that scales every gemstone bonus.
    pub talent_432: f64,
    /// The Land Rank bonus per farming level, in percent.
    pub farm_rank_bonus: f64,
    /// The login character's Farming level, the Land Rank multiplier's level.
    pub farm_level: f64,
    pub compass_45: f64,
    pub gambit_11: f64,
    pub bubble_a10: f64,
    pub star_sign_73: f64,
    pub statue_26: f64,
    pub card_crystal5_level: f64,
    pub achievement_368: f64,
    pub voting_25: f64,
    pub lamp_21: f64,
    pub hole_upgrade_54: f64,
    pub fractal_unlocked: f64,
    pub emperor_0: f64,
    pub rog_32: f64,
    pub jelly_16: f64,
    pub companion_163: f64,
    /// The `LAAARRRRYYYY` (`Y9ACTIVE`) bubble value, the item-find multiplier.
    pub bubble_y9: f64,
    pub jade: JadeAccount,
    pub exp: ExpAccount,
    pub door: DoorAccount,
}

/// The Jade rate's account factors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JadeAccount {
    /// `2^GemItemsPurchased[10]`, the Gem Shop Jade multiplier.
    pub gem_shop: f64,
    pub monument: f64,
    pub pristine: f64,
    pub vial: f64,
    pub meal: f64,
    pub card_level: f64,
    pub killroy: f64,
    pub slab: f64,
    pub stamp: f64,
    pub crop: f64,
    pub win: f64,
    pub msa: f64,
    pub sigil: f64,
    pub arcade: f64,
    pub vault: f64,
    pub star: f64,
    pub task_base: f64,
    pub task_level: f64,
    /// `computeRiftSkillBonus(16, 1)`: 1 when the Rift skill rank clears one.
    pub rift: f64,
    pub compass: f64,
    pub companion_163: f64,
    pub achievement_366: f64,
    pub achievement_367: f64,
    pub achievement_369: f64,
    pub gemstone_1: f64,
    pub meritoc: f64,
    pub exotic: f64,
    pub gold_9: f64,
    pub gold_23: f64,
}

/// The EXP per action's account factors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExpAccount {
    /// `(1 + meritoc 10) × (1 + legend 20) × (1 + companion 32)`.
    pub all_skill_multi: f64,
    pub arcade_52: f64,
    pub gold_10: f64,
    pub compass_49: f64,
    pub gemstone_4: f64,
    pub vial: f64,
    pub meal: f64,
    /// `computeRiftSkillBonus(16, 0)`, worth 25% each.
    pub rift: f64,
    pub pristine: f64,
    pub card_level: f64,
    pub mainframe: f64,
    pub stamp: f64,
    pub star: f64,
    pub guild: f64,
    pub talent_add: f64,
    pub talent_multi: f64,
    pub achievement_370: f64,
    pub vote: f64,
    pub vault: f64,
    pub win: f64,
    pub companion_163: f64,
}

/// The door damage's account factors.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DoorAccount {
    /// `Hinge Buster` bubble (`Y8`), in percent.
    pub y8: f64,
    /// `gemstoneBonus(2)`.
    pub gemstone_2: f64,
    /// `NK 27`, True Battering, in percent.
    pub true_battering: f64,
    /// `computeVialByKey('6Untie')`.
    pub untie_vial: f64,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            talent_432: 1.0,
            farm_rank_bonus: 0.0,
            farm_level: 0.0,
            compass_45: 0.0,
            gambit_11: 0.0,
            bubble_a10: 0.0,
            star_sign_73: 0.0,
            statue_26: 0.0,
            card_crystal5_level: 0.0,
            achievement_368: 0.0,
            voting_25: 0.0,
            lamp_21: 0.0,
            hole_upgrade_54: 0.0,
            fractal_unlocked: 0.0,
            emperor_0: 0.0,
            rog_32: 0.0,
            jelly_16: 0.0,
            companion_163: 0.0,
            bubble_y9: 1.0,
            jade: JadeAccount::default(),
            exp: ExpAccount::default(),
            door: DoorAccount::default(),
        }
    }
}

impl Default for JadeAccount {
    fn default() -> Self {
        Self {
            gem_shop: 1.0,
            monument: 0.0,
            pristine: 0.0,
            vial: 0.0,
            meal: 0.0,
            card_level: 0.0,
            killroy: 1.0,
            slab: 0.0,
            stamp: 0.0,
            crop: 0.0,
            win: 0.0,
            msa: 0.0,
            sigil: 0.0,
            arcade: 0.0,
            vault: 0.0,
            star: 0.0,
            task_base: 0.0,
            task_level: 0.0,
            rift: 0.0,
            compass: 0.0,
            companion_163: 0.0,
            achievement_366: 0.0,
            achievement_367: 0.0,
            achievement_369: 0.0,
            gemstone_1: 0.0,
            meritoc: 0.0,
            exotic: 0.0,
            gold_9: 0.0,
            gold_23: 0.0,
        }
    }
}

impl Default for ExpAccount {
    fn default() -> Self {
        Self {
            all_skill_multi: 1.0,
            arcade_52: 0.0,
            gold_10: 0.0,
            compass_49: 0.0,
            gemstone_4: 0.0,
            vial: 0.0,
            meal: 0.0,
            rift: 0.0,
            pristine: 0.0,
            card_level: 0.0,
            mainframe: 0.0,
            stamp: 0.0,
            star: 0.0,
            guild: 0.0,
            talent_add: 0.0,
            talent_multi: 1.0,
            achievement_370: 0.0,
            vote: 0.0,
            vault: 0.0,
            win: 0.0,
            companion_163: 0.0,
        }
    }
}

impl JadeAccount {
    /// `u`: the Jade per Sneaking level factor.
    pub fn level_factor(&self, nk8: f64, sneak_level: f64) -> f64 {
        1.0 + nk8 * sneak_level / 100.0
    }

    /// `o`: the primary Jade multipliers, before the charm and account groups.
    pub fn primary(&self, solo_charms: f64, nk28: f64) -> f64 {
        self.gem_shop
            * (1.0 + solo_charms / 100.0)
            * (1.0 + self.monument / 100.0)
            * (1.0 + self.pristine / 100.0)
            * (1.0 + nk28 / 100.0)
            * (1.0 + self.gold_9 / 100.0)
            * self.killroy.max(1.0)
            * (1.0 + (self.vial + self.meal + 4.0 * self.card_level) / 100.0)
    }

    /// `N`: the account Jade multiplier group.
    pub fn account(&self) -> f64 {
        (1.0 + (self.msa + self.sigil + self.arcade + self.vault) / 100.0)
            * (1.0 + self.star / 100.0)
            * (1.0 + self.task_base * self.task_level / 100.0)
    }

    /// `y`: the Rift, compass and companion Jade group.
    pub(crate) fn rift_group(&self) -> f64 {
        (1.0 + 10.0 * self.rift / 100.0)
            * (1.0 + self.compass / 100.0)
            * (1.0 + 99.0 * self.companion_163)
    }

    /// `R`: the Jade achievements.
    pub fn achievements(&self) -> f64 {
        1.0 + (3.0 * self.achievement_366 + 5.0 * self.achievement_369 + 7.0 * self.achievement_367)
            / 100.0
    }

    /// `A`: gemstone, meritocracy and exotic crop.
    pub(crate) fn gem_meritoc(&self) -> f64 {
        (1.0 + self.gemstone_1 / 100.0) * (1.0 + self.meritoc / 100.0) * (1.0 + self.exotic / 100.0)
    }
}
