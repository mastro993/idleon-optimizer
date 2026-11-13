//! The rates the breakdown trees read: full stealth, Jade coin and hour, EXP
//! per action, door damage and the runtime action loop.
//!
//! Ported from `computeTwinStealth`, `computeJadeCoin`, `computeJadeRate`,
//! `computeDoorDamageDetailed`, `computeUntieProgressDetailed`,
//! `computeDoorHitsPerHour`, `computeTeamDoorRates` and
//! `computeSneakingTeamScenario` in `js/stats/systems/w6/sneaking-*.js`.

use super::math::{
    DoorRate, TeamMember, door_hits_per_hour, exp_base, item_find_per_hour, jade_actions_per_hour,
    jade_base, team_door_rates, weapon_stat,
};
use super::placement::door_hp;
use super::{FLOOR_COUNT, GoldBonuses, Loadout, SlotKind};

/// One twin's Jade per successful action.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JadeCoin {
    pub coin: f64,
    pub base_jade: f64,
    pub multiplier: f64,
    pub detection: f64,
    pub sneak_level: f64,
    pub nk8: f64,
    pub nk28: f64,
    pub solo: bool,
    pub charm6: f64,
    pub charm7: f64,
    pub charm14: f64,
    pub charm15: f64,
    pub charm19: f64,
    pub gold9: f64,
    pub gold23: f64,
    pub u: f64,
    pub o: f64,
    pub e: f64,
    pub g: f64,
    pub d: f64,
    pub b: f64,
    pub n: f64,
    pub y: f64,
    pub r: f64,
    pub a: f64,
    pub g_final: f64,
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
    pub task: f64,
    pub rift: f64,
    pub compass: f64,
    pub companion: f64,
    pub achievement_366: f64,
    pub achievement_367: f64,
    pub achievement_369: f64,
    pub gemstone: f64,
    pub meritoc: f64,
    pub exotic: f64,
    pub gem_shop: f64,
}

/// One twin's Jade per hour.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JadeHour {
    pub coin: JadeCoin,
    pub jade_per_hour: f64,
    pub actions_per_hour: f64,
    pub detection_cycles: f64,
    pub hit_chance: f64,
    pub ko_chance: f64,
    pub dodge: f64,
}

/// One twin's EXP per successful action.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ExpAction {
    pub exp: f64,
    pub base: f64,
    pub floor: usize,
    pub mastery: u32,
    pub recipient: usize,
    pub all_skill_multi: f64,
    pub nk: f64,
    pub nk2: f64,
    pub nk19: f64,
    pub arcade: f64,
    pub gold: f64,
    pub compass: f64,
    pub gemstone: f64,
    pub simple: f64,
    pub charms: f64,
    pub no_item_penalty: f64,
    pub win: f64,
    pub companion: f64,
    pub talent: f64,
    pub arcade_52: f64,
    pub gold_10: f64,
    pub compass_49: f64,
    pub gem_4: f64,
    pub vial: f64,
    pub meal: f64,
    pub rift: f64,
    pub pristine: f64,
    pub card: f64,
    pub mainframe: f64,
    pub stamp: f64,
    pub star: f64,
    pub guild: f64,
    pub talent_add: f64,
    pub achievement: f64,
    pub vote: f64,
    pub vault: f64,
    pub charm13: f64,
    pub charm19: f64,
    pub charm3: f64,
    pub charm7: f64,
    pub charm5: f64,
    pub win_bonus: f64,
    pub companion_163: f64,
    pub talent_multi: f64,
}

/// One twin's door damage per hit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DoorDamage {
    pub damage: f64,
    pub can_hit_door: bool,
    pub weapon_stat: f64,
    pub item_key: String,
    pub item_level: u32,
    pub sneak_level: f64,
    pub sneak_tens: f64,
    pub mahjong: f64,
    pub gold_per_ten: f64,
    pub gold_damage: f64,
    pub y8: f64,
    pub gemstone: f64,
    pub true_battering: f64,
}

/// One twin's untie progress per hit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UntieProgress {
    pub progress: f64,
    pub can_untie: bool,
    pub weapon_stat: f64,
    pub mahjong: f64,
    pub vial: f64,
}

/// The saved door at a floor and the current mastery.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DoorState {
    pub active: bool,
    pub floor: usize,
    /// The floor whose door is the one unfinished floor, when the save has one.
    pub active_floor: Option<usize>,
    pub max_hp: f64,
    pub progress: f64,
    pub remaining_hp: f64,
}

/// The action each twin's native loop takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActionMode {
    Captive,
    Training,
    Door,
    Untie,
    #[default]
    Normal,
}

impl ActionMode {
    pub fn label(self) -> &'static str {
        match self {
            ActionMode::Captive => "captive",
            ActionMode::Training => "training",
            ActionMode::Door => "door",
            ActionMode::Untie => "untie",
            ActionMode::Normal => "normal",
        }
    }
}

impl std::fmt::Display for ActionMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

/// One twin's runtime row under the saved floor and loadout.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Runtime {
    pub mode: ActionMode,
    pub attempts_per_hour: f64,
    pub successful_actions_per_hour: f64,
    pub flowers_per_hour: f64,
    pub jade_per_action: f64,
    pub jade_per_hour: f64,
    pub game_display_actions_per_hour: f64,
    pub game_display_jade_per_hour: f64,
    pub items_per_hour: f64,
    pub door_damage_per_hit: f64,
    pub door_hits_per_hour: f64,
    pub door_damage_per_hour: f64,
    pub untie_progress_per_hit: f64,
    pub untie_progress_per_hour: f64,
    pub coupled: bool,
}

impl Loadout {
    /// The account-wide stealth multiplier, without the charm, ally and funeral
    /// factors the loadout itself applies.
    pub(super) fn account_multi(&self, twin: usize) -> f64 {
        self.account_multi_with(twin, &self.gold_bonuses())
    }

    /// [`Self::account_multi`] with the Gold inventory scan already resolved.
    pub(super) fn account_multi_with(&self, twin: usize, gold: &GoldBonuses) -> f64 {
        let account = &self.account;
        let farm = 1.0 + account.farm_rank_bonus * account.farm_level / 100.0;
        let gold_20 = gold.get(20);
        let card = 1.0 + 4.0 * account.card_crystal5_level / 100.0;
        let achievement = 1.0 + 5.0 * account.achievement_368 / 100.0;
        let gemstone = 1.0 + self.gemstone(0) / 100.0;
        let voting = 1.0 + account.voting_25 / 100.0;
        let emperor = 1.0 + account.emperor_0 / 100.0;
        let rog = 1.0 + account.rog_32 / 100.0;
        let jelly = 1.0 + account.jelly_16 / 100.0;
        let companion = 1.0 + 39.0 * account.companion_163;
        let sneak_level = self
            .twins
            .get(twin)
            .map_or(0.0, |entry| f64::from(entry.level));
        let fractal = 1.0 + 2.0 * account.fractal_unlocked * sneak_level / 100.0;
        farm * (1.0 + account.compass_45 / 100.0)
            * (1.0 + account.gambit_11 / 100.0)
            * (1.0 + gold_20 / 100.0)
            * (1.0 + (account.bubble_a10 + account.star_sign_73) / 100.0)
            * (1.0 + account.statue_26 / 100.0)
            * card
            * achievement
            * gemstone
            * voting
            * (1.0 + account.lamp_21 / 100.0)
            * account.hole_upgrade_54.max(1.0)
            * fractal
            * emperor
            * rog
            * jelly
            * companion
    }

    /// The charm bonus map's dodge percentage.
    pub(crate) fn dodge_pct(&self, twin: usize) -> f64 {
        self.bonuses(twin).get(0)
    }

    /// The charm bonus map's item-find value.
    pub(crate) fn item_find(&self, twin: usize) -> f64 {
        self.bonuses(twin).get(5)
    }

    /// The twin's `y8`-style door factor and the saved door state.
    pub(crate) fn door_state(&self, floor: usize) -> DoorState {
        if floor == 0 || floor >= FLOOR_COUNT {
            return DoorState::default();
        }
        let max_hp = door_hp(floor, self.mastery);
        if max_hp <= 0.0 {
            return DoorState {
                floor,
                ..DoorState::default()
            };
        }
        // A projected mastery is a what-if: the game has not relocked a floor,
        // so upstream's `doorState` shows every door fresh at its full HP. A
        // projection below the saved maximum has no door at all.
        if self.projected {
            if self.mastery < self.mastery_max {
                return DoorState {
                    floor,
                    ..DoorState::default()
                };
            }
            return DoorState {
                active: true,
                floor,
                active_floor: Some(floor),
                max_hp,
                progress: 0.0,
                remaining_hp: max_hp,
            };
        }
        if self.mastery < self.mastery_max {
            return DoorState {
                floor,
                ..DoorState::default()
            };
        }
        let active_floor = (1..FLOOR_COUNT).find(|candidate| {
            let candidate_max = door_hp(*candidate, self.mastery);
            let progress = self.doors.get(*candidate).copied().unwrap_or(0.0);
            candidate_max > progress
        });
        let progress = self.doors.get(floor).copied().unwrap_or(0.0);
        let remaining = (max_hp - progress).max(0.0);
        let active = active_floor == Some(floor) && remaining > 0.0;
        DoorState {
            active,
            floor,
            active_floor,
            max_hp,
            progress,
            remaining_hp: if active { remaining } else { 0.0 },
        }
    }

    /// `computeJadeCoin`.
    pub(crate) fn jade_coin(&self, twin: usize) -> JadeCoin {
        self.jade_coin_with(twin, &self.gold_bonuses())
    }

    /// [`Self::jade_coin`] with the Gold inventory scan already resolved.
    pub(super) fn jade_coin_with(&self, twin: usize, gold: &GoldBonuses) -> JadeCoin {
        let Some(entry) = self.twins.get(twin) else {
            return JadeCoin::default();
        };
        let jade = self.account.jade;
        let bonuses = self.bonuses_with(twin, gold);
        let solo = self.is_solo(twin);
        let detection = self.detection_with(twin, gold);
        let charm14 = bonuses.get(14) * if solo { 3.0 } else { 1.0 };
        let charm19 = bonuses.get(19) * if solo { 3.0 } else { 1.0 };
        let charm6 = bonuses.get(6) * if detection == 0.0 { 2.0 } else { 1.0 };
        let charm7 = bonuses.get(7);
        let charm15 = bonuses.get(15);
        let sneak_level = f64::from(entry.level);
        let nk8 = self.ninja_bonus(8);
        let nk28 = self.ninja_bonus(28);
        let u = jade.level_factor(nk8, sneak_level);
        let o = jade.primary(charm14 + charm19, nk28);
        let e = 1.0 + (charm6 + charm7 + charm15) / 100.0;
        let g = 1.0 + (jade.slab + jade.stamp) / 100.0;
        let d = 1.0 + jade.crop / 100.0;
        let b = 1.0 + jade.win / 100.0;
        let n = jade.account();
        let y = jade.rift_group();
        let r = jade.achievements();
        let a = jade.gem_meritoc();
        let g_final = 1.0 + jade.gold_23 * (sneak_level / 10.0).floor() / 100.0;
        let multiplier = u * o * e * g * d * b * n * y * r * a * g_final;
        let base_jade = jade_base(entry.floor, self.mastery);
        JadeCoin {
            coin: base_jade * multiplier,
            base_jade,
            multiplier,
            detection,
            sneak_level,
            nk8,
            nk28,
            solo,
            charm6,
            charm7,
            charm14,
            charm15,
            charm19,
            gold9: jade.gold_9,
            gold23: jade.gold_23,
            u,
            o,
            e,
            g,
            d,
            b,
            n,
            y,
            r,
            a,
            g_final,
            monument: jade.monument,
            pristine: jade.pristine,
            vial: jade.vial,
            meal: jade.meal,
            card_level: jade.card_level,
            killroy: jade.killroy,
            slab: jade.slab,
            stamp: jade.stamp,
            crop: jade.crop,
            win: jade.win,
            msa: jade.msa,
            sigil: jade.sigil,
            arcade: jade.arcade,
            vault: jade.vault,
            star: jade.star,
            task: jade.task_base * jade.task_level,
            rift: jade.rift,
            compass: jade.compass,
            companion: jade.companion_163,
            achievement_366: jade.achievement_366,
            achievement_367: jade.achievement_367,
            achievement_369: jade.achievement_369,
            gemstone: jade.gemstone_1,
            meritoc: jade.meritoc,
            exotic: jade.exotic,
            gem_shop: jade.gem_shop,
        }
    }

    /// `computeJadeRate`.
    pub(crate) fn jade_hour(&self, twin: usize) -> JadeHour {
        let coin = self.jade_coin(twin);
        let rate = jade_actions_per_hour(
            self.action_speed(twin),
            coin.detection,
            self.ko_seconds(twin),
            self.dodge_pct(twin),
        );
        JadeHour {
            jade_per_hour: coin.coin * rate.actions_per_hour,
            actions_per_hour: rate.actions_per_hour,
            detection_cycles: rate.detection_cycles,
            hit_chance: rate.hit_chance,
            ko_chance: rate.ko_chance,
            dodge: rate.dodge,
            coin,
        }
    }

    /// `computeSneakingExpPerAction`.
    pub(crate) fn exp_action(&self, twin: usize) -> ExpAction {
        self.exp_action_with(twin, &self.gold_bonuses())
    }

    /// [`Self::exp_action`] with the Gold inventory scan already resolved.
    pub(super) fn exp_action_with(&self, twin: usize, gold: &GoldBonuses) -> ExpAction {
        let Some(entry) = self.twins.get(twin) else {
            return ExpAction::default();
        };
        let exp = self.account.exp;
        let bonuses = self.bonuses_with(twin, gold);
        let solo = self.is_solo(twin);
        let nk2 = self.ninja_bonus(2);
        let nk19 = self.ninja_bonus(19);
        let charm13 = bonuses.get(13) * if solo { 3.0 } else { 1.0 };
        let charm19 = bonuses.get(19) * if solo { 3.0 } else { 1.0 };
        let charm3 = bonuses.get(3);
        let charm7 = bonuses.get(7);
        let charm5 = bonuses.get(5);
        let base = exp_base(entry.floor, self.mastery);
        let nk = (1.0 + nk2 / 100.0) * (1.0 + nk19 / 100.0);
        let simple = 1.0
            + (exp.vial
                + exp.meal
                + 25.0 * exp.rift
                + exp.pristine
                + 3.0 * exp.card_level
                + exp.mainframe
                + exp.stamp
                + exp.star
                + exp.guild
                + exp.talent_add
                + exp.achievement_370
                + exp.vote
                + exp.vault)
                / 100.0;
        let charms = 1.0 + (charm13 + charm19 + charm3 + charm7) / 100.0;
        let no_item_penalty = (1.0 - 100.0 * charm5).max(0.0);
        let win = 1.0 + exp.win / 100.0;
        let companion = 1.0 + 1.5 * exp.companion_163;
        let result = base
            * exp.all_skill_multi
            * nk
            * (1.0 + exp.arcade_52 / 100.0)
            * (1.0 + exp.gold_10 / 100.0)
            * (1.0 + exp.compass_49 / 100.0)
            * (1.0 + exp.gemstone_4 / 100.0)
            * simple
            * charms
            * no_item_penalty
            * win
            * companion
            * exp.talent_multi;
        ExpAction {
            exp: result,
            base,
            floor: entry.floor,
            mastery: self.mastery,
            recipient: self.exp_recipient(twin, charm3),
            all_skill_multi: exp.all_skill_multi,
            nk,
            nk2,
            nk19,
            arcade: 1.0 + exp.arcade_52 / 100.0,
            gold: 1.0 + exp.gold_10 / 100.0,
            compass: 1.0 + exp.compass_49 / 100.0,
            gemstone: 1.0 + exp.gemstone_4 / 100.0,
            simple,
            charms,
            no_item_penalty,
            win,
            companion,
            talent: exp.talent_multi,
            arcade_52: exp.arcade_52,
            gold_10: exp.gold_10,
            compass_49: exp.compass_49,
            gem_4: exp.gemstone_4,
            vial: exp.vial,
            meal: exp.meal,
            rift: exp.rift,
            pristine: exp.pristine,
            card: exp.card_level,
            mainframe: exp.mainframe,
            stamp: exp.stamp,
            star: exp.star,
            guild: exp.guild,
            talent_add: exp.talent_add,
            achievement: exp.achievement_370,
            vote: exp.vote,
            vault: exp.vault,
            charm13,
            charm19,
            charm3,
            charm7,
            charm5,
            win_bonus: exp.win,
            companion_163: exp.companion_163,
            talent_multi: exp.talent_multi,
        }
    }

    /// The twin that receives the EXP when a transfer charm is equipped.
    fn exp_recipient(&self, twin: usize, charm3: f64) -> usize {
        if charm3 <= 0.0 {
            return twin;
        }
        let selected_level = self.twins.get(twin).map_or(0, |entry| entry.level);
        let mut selected = twin;
        let count = self.twins.len().max(self.exp.len());
        for candidate in 0..count {
            let level = self.twins.get(candidate).map_or(0, |entry| entry.level);
            if level <= selected_level {
                continue;
            }
            let exp = self.exp.get(candidate).copied().unwrap_or(0.0);
            let selected_exp = self.exp.get(selected).copied().unwrap_or(0.0);
            if exp > selected_exp {
                selected = candidate;
            }
        }
        selected
    }

    /// `computeDoorDamageDetailed`.
    pub(crate) fn door_damage(&self, twin: usize) -> DoorDamage {
        self.door_damage_with(twin, &self.gold_bonuses())
    }

    /// [`Self::door_damage`] with the Gold inventory scan already resolved.
    pub(super) fn door_damage_with(&self, twin: usize, gold: &GoldBonuses) -> DoorDamage {
        let Some(entry) = self.twins.get(twin) else {
            return DoorDamage::default();
        };
        let weapon = entry.slot(SlotKind::Weapon);
        let is_nunchaku = weapon
            .def()
            .is_some_and(|def| def.kind == 1 && def.bonus_type == 1);
        let sneak_level = f64::from(entry.level);
        let sneak_tens = (sneak_level / 10.0).floor();
        let mahjong = self.ninja_bonus(5);
        let gold_per_ten = gold.get(22);
        let gold_damage = gold.get(18);
        let weapon_stat = if is_nunchaku {
            weapon_stat(&weapon.key, f64::from(weapon.level))
        } else {
            0.0
        };
        let factors = (1.0 + mahjong / 100.0)
            * (1.0 + gold_per_ten * sneak_tens / 100.0)
            * (1.0 + gold_damage / 100.0)
            * (1.0 + self.account.door.y8 / 100.0)
            * (1.0 + self.account.door.gemstone_2 / 100.0)
            * (1.0 + self.account.door.true_battering / 100.0);
        DoorDamage {
            damage: weapon_stat * factors,
            can_hit_door: is_nunchaku,
            weapon_stat,
            item_key: weapon.key.clone(),
            item_level: weapon.level,
            sneak_level,
            sneak_tens,
            mahjong,
            gold_per_ten,
            gold_damage,
            y8: self.account.door.y8,
            gemstone: self.account.door.gemstone_2,
            true_battering: self.account.door.true_battering,
        }
    }

    /// `computeUntieProgressDetailed`.
    pub(crate) fn untie_progress(&self, twin: usize) -> UntieProgress {
        let Some(entry) = self.twins.get(twin) else {
            return UntieProgress::default();
        };
        let weapon = entry.slot(SlotKind::Weapon);
        let is_kunai = weapon
            .def()
            .is_some_and(|def| def.kind == 1 && def.bonus_type == 2);
        let weapon_stat = if is_kunai {
            weapon_stat(&weapon.key, f64::from(weapon.level))
        } else {
            0.0
        };
        let mahjong = self.ninja_bonus(5);
        let vial = self.account.door.untie_vial;
        UntieProgress {
            progress: weapon_stat * (1.0 + mahjong / 100.0) * (1.0 + vial / 100.0),
            can_untie: is_kunai,
            weapon_stat,
            mahjong,
            vial,
        }
    }

    /// `computeDoorHitsPerHour`.
    pub(crate) fn door_rate(&self, twin: usize) -> DoorRate {
        self.door_rate_with(twin, &self.gold_bonuses())
    }

    /// [`Self::door_rate`] with the Gold inventory scan already resolved.
    pub(super) fn door_rate_with(&self, twin: usize, gold: &GoldBonuses) -> DoorRate {
        door_hits_per_hour(
            self.action_speed_with(twin, gold),
            self.detection_with(twin, gold),
            self.ko_seconds_with(twin, gold),
            self.bonuses_with(twin, gold).get(0),
        )
    }

    /// `computeTeamDoorRates` for the twins on this twin's floor.
    pub(crate) fn team_door_rates(&self, twin: usize) -> Vec<DoorRate> {
        let floor = self.twins.get(twin).map_or(0, |entry| entry.floor);
        self.floor_rates(floor, &self.gold_bonuses())
            .into_iter()
            .map(|(_, rate)| rate)
            .collect()
    }

    /// The team rates for every twin standing on one floor, in roster order.
    ///
    /// The saved floor is what groups a team, so a search that only moves charms
    /// reads one solve per floor instead of one per twin.
    pub(super) fn floor_rates(&self, floor: usize, gold: &GoldBonuses) -> Vec<(usize, DoorRate)> {
        let members: Vec<usize> = self
            .twins
            .iter()
            .take(self.players.min(self.twins.len()))
            .filter(|entry| entry.floor == floor)
            .map(|entry| entry.index)
            .collect();
        let team: Vec<TeamMember> = members
            .iter()
            .map(|index| {
                let bonuses = self.bonuses_with(*index, gold);
                TeamMember {
                    action_speed: self.action_speed_with(*index, gold),
                    detection: self.detection_with(*index, gold),
                    ko_time: self.ko_seconds_with(*index, gold),
                    dodge_pct: bonuses.get(0),
                    taunting: bonuses.get(1) > 0.0,
                    flower_gain: 1.0 + self.ninja_bonus(30) / 100.0,
                }
            })
            .collect();
        team_door_rates(&team)
            .into_iter()
            .zip(members)
            .map(|(rate, index)| (index, rate))
            .collect()
    }

    /// The team rate for one twin, or its independent rate when no teammate.
    pub(crate) fn team_door_rate(&self, twin: usize) -> DoorRate {
        let floor = self.twins.get(twin).map_or(0, |entry| entry.floor);
        let position = self
            .twins
            .iter()
            .take(self.players.min(self.twins.len()))
            .filter(|entry| entry.floor == floor)
            .position(|entry| entry.index == twin);
        let rates = self.team_door_rates(twin);
        position
            .and_then(|position| rates.get(position).copied())
            .unwrap_or_else(|| self.door_rate(twin))
    }

    /// `sneakingActionMode`.
    pub(crate) fn action_mode(&self, twin: usize) -> ActionMode {
        let Some(entry) = self.twins.get(twin) else {
            return ActionMode::Normal;
        };
        if !entry.untied {
            return ActionMode::Captive;
        }
        if entry.floor == 0 {
            return ActionMode::Training;
        }
        let door_active = self.door_state(entry.floor).active;
        let subtype = entry.weapon_subtype().unwrap_or(-1);
        if subtype == 1 && door_active {
            return ActionMode::Door;
        }
        if subtype == 2 && self.captive_target(entry.floor).is_some() {
            return ActionMode::Untie;
        }
        ActionMode::Normal
    }

    /// `computeSneakingTeamScenario`'s row for one twin.
    pub fn runtime(&self, twin: usize, item_batch_hours: f64) -> Runtime {
        let Some(entry) = self.twins.get(twin) else {
            return Runtime::default();
        };
        let mode = self.action_mode(twin);
        let rate = self.team_door_rate(twin);
        let normal_successes = matches!(mode, ActionMode::Normal | ActionMode::Training)
            .then_some(rate.hits_per_hour)
            .unwrap_or(0.0);
        let jade = self.jade_coin(twin);
        let door = self.door_damage(twin);
        let door_active = self.door_state(entry.floor).active && door.can_hit_door;
        let untie = self.untie_progress(twin);
        let display = jade_actions_per_hour(
            self.action_speed(twin),
            jade.detection,
            self.ko_seconds(twin),
            self.dodge_pct(twin),
        );
        let hours = item_batch_hours.max(0.01);
        let items = item_find_per_hour(
            self.ninja_bonus(14),
            self.item_find(twin),
            normal_successes,
            hours,
            self.account.bubble_y9,
            f64::from(self.item_find_phase),
            &self.omni,
        ) / hours;
        Runtime {
            mode,
            attempts_per_hour: rate.attempts_per_hour,
            successful_actions_per_hour: normal_successes,
            flowers_per_hour: rate.flowers_per_hour,
            jade_per_action: jade.coin,
            jade_per_hour: if mode == ActionMode::Normal {
                jade.coin * normal_successes
            } else {
                0.0
            },
            game_display_actions_per_hour: if mode == ActionMode::Normal {
                display.actions_per_hour
            } else {
                0.0
            },
            game_display_jade_per_hour: if mode == ActionMode::Normal {
                jade.coin * display.actions_per_hour
            } else {
                0.0
            },
            items_per_hour: items,
            door_damage_per_hit: if door_active { door.damage } else { 0.0 },
            door_hits_per_hour: if door_active { rate.hits_per_hour } else { 0.0 },
            door_damage_per_hour: if door_active {
                door.damage * rate.hits_per_hour
            } else {
                0.0
            },
            untie_progress_per_hit: if mode == ActionMode::Untie {
                untie.progress
            } else {
                0.0
            },
            untie_progress_per_hour: if mode == ActionMode::Untie {
                untie.progress * rate.hits_per_hour
            } else {
                0.0
            },
            coupled: rate.coupled,
        }
    }
}
