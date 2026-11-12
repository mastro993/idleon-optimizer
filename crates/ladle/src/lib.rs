//! The Ladle Optimizer (W4): ladles per day, claim plans and Cooking EXP balloons.
//!
//! Ported from `js/stats/systems/w4/cooking-ladle.js` in
//! <https://github.com/Corgan/idleon-research-optimizer>. Every function below is
//! the upstream expression with its reads made explicit:
//!
//! - [`ladles_per_day`], [`ladle_breakpoints`] and [`ladle_tier_outlook`] are the
//!   AFK panel rate and how it moves with Cooking efficiency.
//! - [`claim_expectation`], [`claim_minimum`] and [`claim_plan`] turn a claim into
//!   guaranteed and expected ladles, with the Post Office 19c doubling roll and
//!   the Prayer 2 ten-hour claim cap.
//! - [`cooking_exp_req`], [`balloon_gain`], [`balloons_to_level`],
//!   [`levels_from_balloons`] and [`split_balloons`] are the Cooking EXP balloon
//!   model: one balloon at a time, each read at the current level, overflow carried.
//!
//! The four panel values a ladle rate needs (Cooking efficiency, AFK gain rate,
//! EXP multiplier and skill prowess) come from the upstream stats engine
//! (`js/stats/stat-context.js`), which this build does not have yet. They are
//! [`Panel`] inputs here, so the maths is exact and the sourcing is explicit; the
//! section reads them from the player until that engine lands. Talent 148 is the
//! same: [`Panel::ladle_hours`] turns its percentage into the ladle-hours
//! multiplier.
//!
//! Everything else is save-derived: the roster, Cooking level and EXP, the AFK
//! target and pending AFK time, the Post Office 19c roll, the Prayer 2 cap and the
//! ladle and balloon counts across storage and every inventory.
//!
//! [`kitchen`] carries the daily reset: the meal levels and stocks, the meal
//! costs, the reset kitchen layout and the ladle investment instructions.

pub mod kitchen;

use idleon_common::formula::{FormulaNode, ValueFormat};
use idleon_save::Save;

/// `COOKING_SKILL_IDX`: Cooking's index in a `Lv0_<n>` / `Exp0_<n>` row.
pub const COOKING_LEVEL_INDEX: usize = 10;

/// `LADLE_ITEM`: the item the ladle counts read.
pub const LADLE_ITEM: &str = "Ladle";

/// `AFK_CLAIM_CAP_SECONDS`: the longest claim Prayer 2 credits.
pub const AFK_CLAIM_CAP_SECONDS: f64 = 36_000.0;

/// `EXP_BALLOONS`: the three Cooking EXP balloons, with the item ID
/// `balloonGain` multiplies by (`ITEMS[key].ID`).
pub const EXP_BALLOONS: [Balloon; 3] = [
    Balloon {
        key: "ExpBalloon1",
        label: "Balloon 1",
        item_id: 50.0,
    },
    Balloon {
        key: "ExpBalloon2",
        label: "Balloon 2",
        item_id: 120.0,
    },
    Balloon {
        key: "ExpBalloon3",
        label: "Balloon 3",
        item_id: 200.0,
    },
];

/// `PostOffUpgradeInfo[19]`, Chefs Essentials' third slot: the 2x ladle chance is
/// `decay(88, 200, points − 99.9)` above the slot threshold.
const BOX_19C: PostOfficeSlot = PostOfficeSlot {
    box_index: 19,
    threshold: 99.9,
    x1: 88.0,
    x2: 200.0,
};

/// `PrayerInfo[2][4]`: the base bonus of Prayer 2 (Unending Energy).
const PRAYER_2_BASE_BONUS: f64 = 10.0;

/// One Cooking EXP balloon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Balloon {
    /// The save's item key, e.g. `ExpBalloon2`.
    pub key: &'static str,
    /// The label the select shows.
    pub label: &'static str,
    /// `ITEMS[key].ID`, the base `balloonGain` multiplies.
    pub item_id: f64,
}

/// How many of one item the save carries, and what the counts could not see.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Count {
    pub value: f64,
    /// Whether part of the inventory was unreadable, so the count is a floor.
    pub partial: bool,
    /// The inventories that could not be read, when `partial`.
    pub missing: Vec<String>,
}

/// The upstream stats-engine values a ladle rate needs, entered from the game's
/// Cooking panel until the account stats engine is built.
///
/// A value the player reads from the panel gives the exact upstream outcome; the
/// neutral defaults (no efficiency or prowess, an EXP multiplier of 1 and the
/// game's 0.01 AFK floor) give a lower bound instead.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Panel {
    /// `skill-efficiency` for Cooking.
    pub efficiency: f64,
    /// `skill-afk` for Cooking: the AFK gain rate.
    pub afk: f64,
    /// `skill-exp` for Cooking: the EXP multiplier.
    pub exp_multi: f64,
    /// `computeAllSkillProwessDetail`: the prowess the ladle exponent adds.
    pub prowess: f64,
    /// Talent 148 Overflowing Ladle, as a percentage.
    pub talent_148: f64,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            efficiency: 0.0,
            afk: 0.01,
            exp_multi: 1.0,
            prowess: 0.0,
            talent_148: 0.0,
        }
    }
}

impl Panel {
    /// `ladleMultiplier`: ladle hours per ladle, `1 + Talent 148 / 100`.
    pub fn ladle_hours(self) -> f64 {
        1.0 + self.talent_148 / 100.0
    }

    /// The exponent the ladle tier uses, `0.25 + clamp(0, 0.1, prowess)`.
    pub fn exponent(self) -> f64 {
        tier_exponent(self.prowess)
    }
}

/// One character's save-derived Cooking state.
#[derive(Clone, Debug, PartialEq)]
pub struct Character {
    pub index: usize,
    pub name: String,
    /// `AFKtarget_<n>`, the map or skill the character is away in.
    pub afk_target: String,
    /// Cooking level from `Lv0_<n>[10]`.
    pub level: f64,
    /// Cooking EXP from `Exp0_<n>[10]`.
    pub exp: f64,
    /// `TimeAway.GlobalTime − PTimeAway_<n> × 1000` in hours, when known.
    pub pending_hours: Option<f64>,
    /// `computeBoxReward(charIdx, '19c')`, the Post Office 2x ladle chance.
    pub double_chance: f64,
    /// `computePrayerReal(2, 1, charIdx) > 2`: Prayer 2 caps the claim at ten hours.
    pub claim_capped: bool,
}

impl Character {
    /// Whether the character is AFK-ing Cooking, so its claims count by default.
    pub fn cooking(&self) -> bool {
        self.afk_target == "Cooking"
    }
}

/// The ladle section's save-derived state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ladle {
    pub characters: Vec<Character>,
    /// Every `Ladle` the save carries, across the chest and the inventories.
    pub ladles: Count,
    /// One count per entry in [`EXP_BALLOONS`].
    pub balloons: Vec<Count>,
    /// `TimeAway.ShopRestock` in hours, clamped to a day, when the save has it.
    pub reset_hours: Option<f64>,
    /// The Kitchens & Jellyfish reset state, when the save carries `Cooking` and
    /// `Meals`.
    pub kitchen: Option<kitchen::Reset>,
}

impl Ladle {
    /// Read the ladle state from a decoded save.
    ///
    /// `None` when the save carries no character rows, like the other sections
    /// that need a roster.
    pub fn from_save(save: &Save) -> Option<Self> {
        let characters = characters(save);
        if characters.is_empty() {
            return None;
        }
        let inventories = inventories(save);
        Some(Self {
            characters,
            ladles: inventories.count(LADLE_ITEM),
            balloons: EXP_BALLOONS
                .iter()
                .map(|balloon| inventories.count(balloon.key))
                .collect(),
            reset_hours: reset_hours(save),
            kitchen: kitchen::Reset::from_save(save),
        })
    }

    /// One character by index.
    pub fn character(&self, index: usize) -> Option<&Character> {
        self.characters.get(index)
    }

    /// The characters the section counts by default: those AFK-ing Cooking.
    pub fn cooking_characters(&self) -> Vec<usize> {
        self.characters
            .iter()
            .filter(|character| character.cooking())
            .map(|character| character.index)
            .collect()
    }
}

/// One character's ladle rate and Cooking EXP state.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub index: usize,
    pub name: String,
    pub afk_target: String,
    pub cooking: bool,
    pub efficiency: f64,
    pub prowess: f64,
    pub afk: f64,
    pub exp_multi: f64,
    /// `basePerDay / 15`, the ladle multi the panel floors.
    pub ladle_multi: f64,
    pub base_per_day: f64,
    /// In-game AFK panel `LADLES/DAY`.
    pub panel_per_day: f64,
    /// The Post Office 19c doubling chance, as a percentage.
    pub double_chance: f64,
    pub claim_capped: bool,
    pub claim_hours: f64,
    pub effective_claim_seconds: f64,
    pub raw_per_claim: f64,
    pub per_claim: f64,
    pub per_day: f64,
    pub per_hour: f64,
    pub min_per_claim: f64,
    pub min_per_day: f64,
    pub raw_per_hour: f64,
    pub claim_cap_hours: f64,
    pub pending_hours: f64,
    pub pending_known: bool,
    pub level: f64,
    pub exp: f64,
    pub exp_req: f64,
    pub exp_per_hour: f64,
    pub next_level_balloons: f64,
    pub breakpoints: Vec<Breakpoint>,
    pub outlook: Outlook,
}

/// One efficiency step of the ladle tier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breakpoint {
    /// Ladles a day the step unlocks.
    pub ladles_per_day: f64,
    /// The efficiency it needs.
    pub efficiency: f64,
    /// How far that is from the current efficiency, as a factor.
    pub multiplier: f64,
}

/// How the ladle tier responds to more efficiency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outlook {
    /// The floored ladle multi.
    pub tier: f64,
    pub base_per_day: f64,
    pub next: Breakpoint,
    /// Ladle gain from the next floored step, as a percentage.
    pub next_tier_gain_pct: f64,
    /// Efficiency the next step needs, as a percentage.
    pub next_tier_efficiency_pct: f64,
    /// Ladle gain from ten percent more efficiency.
    pub plus_10_efficiency_gain_pct: f64,
    /// Efficiency one percent more ladles needs.
    pub efficiency_pct_for_plus_1_pct: f64,
    /// Whether the tier is small enough that the floored step dominates.
    pub small_tier: bool,
}

/// One claim's cost and result.
#[derive(Clone, Debug, PartialEq)]
pub struct Use {
    /// Balloons spent; `Infinity` when a balloon grants nothing.
    pub balloons: f64,
    pub level: f64,
    pub exp: f64,
}

/// The result of spending a fixed number of balloons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    pub level: f64,
    pub exp: f64,
    pub used: f64,
    pub left: f64,
}

/// One character's share of the split.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Share {
    pub start_level: f64,
    pub level: f64,
    pub exp: f64,
    pub used: f64,
}

/// What sharing a balloon stack across characters produced.
#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    /// One entry per input character, `None` where the caller passed none.
    pub characters: Vec<Option<Share>>,
    pub used: f64,
    pub left: f64,
}

/// One character's claim income, as [`claim_plan`] takes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Claimer {
    pub index: usize,
    pub name: String,
    /// `rawPerHour`: ladles an hour before the claim rolls.
    pub raw_per_hour: f64,
    /// The Post Office 19c doubling chance, as a percentage.
    pub double_chance: f64,
    /// The longest claim Prayer 2 credits, or `Infinity`.
    pub cap_hours: f64,
    pub claim_hours: f64,
    pub pending_hours: f64,
}

/// One claim in the window.
#[derive(Clone, Debug, PartialEq)]
pub struct Claim {
    /// The index in the claimers slice.
    pub claimer: usize,
    pub index: usize,
    pub name: String,
    pub at_hours: f64,
    pub hours: f64,
    pub credited_hours: f64,
    /// An immediate claim for time already accrued.
    pub now: bool,
    pub raw: f64,
    pub min: f64,
    pub expected: f64,
}

/// One character's claims in the window.
#[derive(Clone, Debug, PartialEq)]
pub struct ClaimRow {
    pub claimer: usize,
    pub index: usize,
    pub name: String,
    pub claims: Vec<Claim>,
    pub min_ladles: f64,
    pub expected_ladles: f64,
}

/// Every character's claims before the next daily reset.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClaimPlan {
    pub window_hours: f64,
    pub claims: Vec<Claim>,
    pub per_character: Vec<ClaimRow>,
    pub min_ladles: f64,
    pub expected_ladles: f64,
}

/// The ladle tier's exponent: `0.25 + clamp(0, 0.1, prowess)`.
fn tier_exponent(prowess: f64) -> f64 {
    0.25 + prowess.clamp(0.0, 0.1)
}

/// `ladlesPerDay`: `15 × floor(max((efficiency / 1000)^(0.25 + prowess), 1))`.
pub fn ladles_per_day(efficiency: f64, prowess: f64) -> f64 {
    let exponent = tier_exponent(prowess);
    15.0 * ((efficiency / 1000.0).powf(exponent).max(1.0)).floor()
}

/// `ladleBreakpoints`: the next `count` floored steps and the efficiency each needs.
pub fn ladle_breakpoints(efficiency: f64, prowess: f64, count: usize) -> Vec<Breakpoint> {
    let exponent = tier_exponent(prowess);
    let tier = ladles_per_day(efficiency, prowess) / 15.0;
    (1..=count)
        .map(|step| {
            let next = tier + step as f64;
            let required = 1000.0 * next.powf(1.0 / exponent);
            Breakpoint {
                ladles_per_day: 15.0 * next,
                efficiency: required,
                multiplier: required / efficiency.max(1e-300),
            }
        })
        .collect()
}

/// `ladleTierOutlook`: how the rate answers more efficiency.
pub fn ladle_tier_outlook(efficiency: f64, prowess: f64) -> Outlook {
    let efficiency = efficiency.max(0.0);
    let exponent = tier_exponent(prowess);
    let base = ladles_per_day(efficiency, prowess);
    let tier = base / 15.0;
    let next = ladle_breakpoints(efficiency, prowess, 1)[0];
    let plus_10 = ladles_per_day(efficiency * 1.1, prowess);
    let one_percent_tier = (tier * 1.01).ceil();
    let efficiency_for_one_percent = if efficiency > 0.0 {
        1000.0 * one_percent_tier.powf(1.0 / exponent) / efficiency - 1.0
    } else {
        f64::INFINITY
    };
    Outlook {
        tier,
        base_per_day: base,
        next,
        next_tier_gain_pct: 100.0 / tier,
        next_tier_efficiency_pct: (next.multiplier - 1.0) * 100.0,
        plus_10_efficiency_gain_pct: (plus_10 / base.max(1e-300) - 1.0) * 100.0,
        efficiency_pct_for_plus_1_pct: efficiency_for_one_percent * 100.0,
        small_tier: tier <= 100.0,
    }
}

/// `ladleClaimExpectation`: the average ladles one claim of `value` gives, with
/// the Post Office 19c doubling roll.
pub fn claim_expectation(value: f64, double_chance: f64) -> f64 {
    let chance = (double_chance / 100.0).clamp(0.0, 1.0);
    let value = value.max(0.0);
    if value >= 1.0 {
        (1.0 - chance) * value.floor() + chance * (2.0 * value).floor()
    } else if value > 0.5 {
        value * (1.0 + chance) + (1.0 - value) * chance
    } else {
        value * (1.0 + chance)
    }
}

/// `ladleClaimMinimum`: the guaranteed ladles one claim gives, without the roll.
pub fn claim_minimum(value: f64) -> f64 {
    let value = value.max(0.0);
    if value >= 1.0 { value.floor() } else { 0.0 }
}

/// `ladleClaimPlan`: the claims before the next reset.
///
/// Each claimer claims every `claim_hours`, with the final claim just before the
/// reset; the earliest claim also collects `pending_hours` already accrued. The
/// game only grants a claim after more than 120 seconds away, and Prayer 2 credits
/// at most `cap_hours` per claim. When pending time would be lost to the cap, an
/// immediate claim is added first.
pub fn claim_plan(claimers: &[Claimer], window_hours: f64) -> ClaimPlan {
    let window = window_hours.max(0.0);
    let mut plan = ClaimPlan {
        window_hours: window,
        ..ClaimPlan::default()
    };
    for (claimer_index, claimer) in claimers.iter().enumerate() {
        let rate = claimer.raw_per_hour.max(0.0);
        let every = if claimer.claim_hours > 0.0 {
            claimer.claim_hours
        } else {
            24.0
        };
        let cap = if claimer.cap_hours > 0.0 {
            claimer.cap_hours
        } else {
            f64::INFINITY
        };
        let mut pending = claimer.pending_hours.max(0.0);
        let mut segments: Vec<(f64, f64, bool)> = Vec::new();
        if window > 0.0 {
            let count = (window / every - 1e-9).ceil().max(1.0);
            let first = window - (count - 1.0) * every;
            if pending > 0.0 && pending + first > cap {
                segments.push((0.0, pending, true));
                pending = 0.0;
            }
            for step in 0..count as usize {
                let end = first + step as f64 * every;
                let hours = if step == 0 { first + pending } else { every };
                segments.push((end, hours, false));
            }
        } else if pending > 0.0 {
            segments.push((0.0, pending, true));
        }
        let mut row = ClaimRow {
            claimer: claimer_index,
            index: claimer.index,
            name: claimer.name.clone(),
            claims: Vec::new(),
            min_ladles: 0.0,
            expected_ladles: 0.0,
        };
        for (at, hours, now) in segments {
            let credited = hours.min(cap);
            let raw = if hours * 3600.0 > 120.0 {
                rate * credited
            } else {
                0.0
            };
            row.claims.push(Claim {
                claimer: claimer_index,
                index: claimer.index,
                name: claimer.name.clone(),
                at_hours: at,
                hours,
                credited_hours: credited,
                now,
                raw,
                min: claim_minimum(raw),
                expected: claim_expectation(raw, claimer.double_chance),
            });
        }
        for claim in &row.claims {
            row.min_ladles += claim.min;
            row.expected_ladles += claim.expected;
            plan.claims.push(claim.clone());
        }
        plan.min_ladles += row.min_ladles;
        plan.expected_ladles += row.expected_ladles;
        plan.per_character.push(row);
    }
    plan.claims.sort_by(|a, b| {
        a.at_hours
            .total_cmp(&b.at_hours)
            .then(a.claimer.cmp(&b.claimer))
    });
    plan
}

/// `cookingExpReq`: the EXP one Cooking level costs.
pub fn cooking_exp_req(level: f64) -> f64 {
    let level = level.max(0.0);
    (15.0 + level.powi(2) + 15.0 * level)
        * (1.225 - (0.135 * level / (level + 50.0)).min(0.18)).powf(level)
        - 30.0
}

/// `balloonGain`: `item ID × (level + 1) × EXP multi`, read at the current level.
pub fn balloon_gain(item_id: f64, level: f64, exp_multi: f64) -> f64 {
    item_id * (level + 1.0) * exp_multi
}

/// `balloonsToLevel`: spends balloons one at a time, each read at the current
/// level, carrying the overflow into the next level.
pub fn balloons_to_level(
    level: f64,
    exp: f64,
    target_level: f64,
    exp_multi: f64,
    item_id: f64,
) -> Use {
    let mut level = level.max(0.0);
    let mut exp = exp.max(0.0);
    let target = target_level.max(level);
    let mut used = 0.0;
    let mut limit = 0;
    while level < target && limit < 100_000 {
        limit += 1;
        let required = cooking_exp_req(level);
        if exp >= required {
            exp -= required;
            level += 1.0;
            continue;
        }
        let gain = balloon_gain(item_id, level, exp_multi);
        if gain.is_nan() || gain <= 0.0 {
            return Use {
                balloons: f64::INFINITY,
                level,
                exp,
            };
        }
        let count = ((required - exp) / gain).ceil();
        used += count;
        exp += count * gain;
    }
    while exp >= cooking_exp_req(level) && limit < 100_000 {
        limit += 1;
        exp -= cooking_exp_req(level);
        level += 1.0;
    }
    Use {
        balloons: used,
        level,
        exp,
    }
}

/// `levelsFromBalloons`: spends up to `count` balloons and reports the level.
pub fn levels_from_balloons(
    level: f64,
    exp: f64,
    count: f64,
    exp_multi: f64,
    item_id: f64,
) -> Levels {
    let mut level = level.max(0.0);
    let mut exp = exp.max(0.0);
    let total = count.max(0.0).floor();
    let mut left = total;
    let mut limit = 0;
    while limit < 100_000 {
        limit += 1;
        let required = cooking_exp_req(level);
        if exp >= required {
            exp -= required;
            level += 1.0;
            continue;
        }
        let gain = balloon_gain(item_id, level, exp_multi);
        if gain.is_nan() || gain <= 0.0 || left <= 0.0 {
            break;
        }
        let need = ((required - exp) / gain).ceil();
        if need > left {
            exp += left * gain;
            left = 0.0;
            break;
        }
        left -= need;
        exp += need * gain;
    }
    Levels {
        level,
        exp,
        used: total - left,
        left,
    }
}

/// `splitBalloons`: shares a balloon stack to keep Cooking levels even.
///
/// Each next level-up goes to the lowest-level character (ties: the fewest
/// balloons to its next level, then the lowest index). When the remainder cannot
/// buy that level, it is all given to that character as partial EXP.
pub fn split_balloons(
    characters: &[Option<(f64, f64)>],
    count: f64,
    exp_multi: f64,
    item_id: f64,
) -> Split {
    let total = count.max(0.0).floor();
    let mut left = total;
    let mut shares: Vec<Option<Share>> = characters
        .iter()
        .map(|entry| {
            entry.map(|(level, exp)| {
                let opened = levels_from_balloons(level, exp, 0.0, exp_multi, item_id);
                Share {
                    start_level: level.max(0.0),
                    level: opened.level,
                    exp: opened.exp,
                    used: 0.0,
                }
            })
        })
        .collect();
    let to_next = |share: Share| {
        balloons_to_level(
            share.level,
            share.exp,
            share.level + 1.0,
            exp_multi,
            item_id,
        )
        .balloons
    };
    let mut need: Vec<Option<f64>> = shares.iter().map(|share| share.map(to_next)).collect();
    let mut limit = 0;
    while left > 0.0 && limit < 200_000 {
        limit += 1;
        let mut pick: Option<usize> = None;
        for (index, share) in shares.iter().enumerate() {
            let Some(share) = share else { continue };
            let Some(candidate) = need[index].filter(|need| need.is_finite()) else {
                continue;
            };
            let Some(best) = pick else {
                pick = Some(index);
                continue;
            };
            let best_share = shares[best].expect("a picked index holds a share");
            let best_need = need[best].unwrap_or(f64::INFINITY);
            if share.level < best_share.level
                || (share.level == best_share.level && candidate < best_need)
            {
                pick = Some(index);
            }
        }
        let Some(index) = pick else { break };
        let share = shares[index].expect("a picked index holds a share");
        let give = left.min(need[index].unwrap_or(f64::INFINITY));
        let next = levels_from_balloons(share.level, share.exp, give, exp_multi, item_id);
        let updated = Share {
            start_level: share.start_level,
            level: next.level,
            exp: next.exp,
            used: share.used + give,
        };
        shares[index] = Some(updated);
        left -= give;
        need[index] = Some(to_next(updated));
    }
    Split {
        characters: shares,
        used: total - left,
        left,
    }
}

/// `cookingLadleStats`: one character's rates, from its save row and the panel.
pub fn stats(character: &Character, panel: Panel, claim_hours: f64) -> Stats {
    let efficiency = panel.efficiency;
    let prowess = panel.prowess.clamp(0.0, 0.1);
    let afk = panel.afk;
    let exp_multi = panel.exp_multi;
    let base_per_day = ladles_per_day(efficiency, prowess);
    let ladle_multi = base_per_day / 15.0;
    let panel_per_day = base_per_day * afk;
    let double_chance = character.double_chance;
    let capped = character.claim_capped;
    let claim_hours = if claim_hours > 0.0 { claim_hours } else { 24.0 };
    let claim_seconds = claim_hours * 3600.0;
    let effective_claim_seconds = if capped {
        claim_seconds.min(AFK_CLAIM_CAP_SECONDS)
    } else {
        claim_seconds
    };
    let raw_per_claim = base_per_day * afk * effective_claim_seconds / 86_400.0;
    let per_claim = claim_expectation(raw_per_claim, double_chance);
    let per_day = per_claim * 24.0 / claim_hours;
    let min_per_claim = claim_minimum(raw_per_claim);
    let raw_per_hour = base_per_day * afk / 24.0;
    let claim_cap_hours = if capped {
        AFK_CLAIM_CAP_SECONDS / 3600.0
    } else {
        f64::INFINITY
    };
    let min_day = claim_plan(
        &[Claimer {
            index: character.index,
            name: character.name.clone(),
            raw_per_hour,
            double_chance,
            cap_hours: claim_cap_hours,
            claim_hours,
            pending_hours: 0.0,
        }],
        24.0,
    );
    let level = character.level;
    let exp = character.exp;
    let pending_hours = character.pending_hours.unwrap_or(0.0);
    Stats {
        index: character.index,
        name: character.name.clone(),
        afk_target: character.afk_target.clone(),
        cooking: character.cooking(),
        efficiency,
        prowess,
        afk,
        exp_multi,
        ladle_multi,
        base_per_day,
        panel_per_day,
        double_chance,
        claim_capped: capped,
        claim_hours,
        effective_claim_seconds,
        raw_per_claim,
        per_claim,
        per_day,
        per_hour: per_day / 24.0,
        min_per_claim,
        min_per_day: min_day.min_ladles,
        raw_per_hour,
        claim_cap_hours,
        pending_hours,
        pending_known: character.cooking() && character.pending_hours.is_some(),
        level,
        exp,
        exp_req: cooking_exp_req(level),
        exp_per_hour: afk * 100.0 * exp_multi,
        next_level_balloons: balloons_to_level(
            level,
            exp,
            level + 1.0,
            exp_multi,
            EXP_BALLOONS[0].item_id,
        )
        .balloons,
        breakpoints: ladle_breakpoints(efficiency, prowess, 3),
        outlook: ladle_tier_outlook(efficiency, prowess),
    }
}

impl Stats {
    /// The claimer this character contributes to the section's claim plan.
    pub fn claimer(&self) -> Claimer {
        Claimer {
            index: self.index,
            name: self.name.clone(),
            raw_per_hour: self.raw_per_hour,
            double_chance: self.double_chance,
            cap_hours: self.claim_cap_hours,
            claim_hours: self.claim_hours,
            pending_hours: self.pending_hours,
        }
    }

    /// The AFK panel rate tree: `15 × ladle multi × AFK gain rate`.
    pub fn ladles_tree(&self) -> FormulaNode {
        FormulaNode {
            label: "Ladles / day (in-game AFK panel)".into(),
            value: self.panel_per_day,
            format: ValueFormat::Raw,
            children: vec![
                FormulaNode {
                    label: "Base ladles / day".into(),
                    value: self.base_per_day,
                    format: ValueFormat::Raw,
                    note: Some("15 × Ladle Multi".into()),
                    children: vec![FormulaNode {
                        label: "Ladle Multi (Bonus Ladles from Efficiency)".into(),
                        value: self.ladle_multi,
                        format: ValueFormat::Multiplier,
                        note: Some("floor(max((eff / 1000)^(0.25 + prowess), 1))".into()),
                        children: vec![
                            FormulaNode {
                                label: "Cooking efficiency".into(),
                                value: self.efficiency,
                                format: ValueFormat::Raw,
                                note: Some(
                                    "AFK panel value: the stats engine is not built yet".into(),
                                ),
                                ..FormulaNode::default()
                            },
                            FormulaNode {
                                label: "Exponent 0.25 + prowess".into(),
                                value: 0.25 + self.prowess,
                                format: ValueFormat::Raw,
                                note: Some(format!("prowess {}", self.prowess)),
                                ..FormulaNode::default()
                            },
                        ],
                        ..FormulaNode::default()
                    }],
                    ..FormulaNode::default()
                },
                FormulaNode {
                    label: "AFK gain rate".into(),
                    value: self.afk,
                    format: ValueFormat::Multiplier,
                    note: Some("AFK panel value: the stats engine is not built yet".into()),
                    ..FormulaNode::default()
                },
            ],
            ..FormulaNode::default()
        }
    }

    /// The claim tree: one claim, its rolls, and the guaranteed day total.
    pub fn claims_tree(&self) -> FormulaNode {
        FormulaNode {
            label: "Expected ladles / day from claims".into(),
            value: self.per_day,
            format: ValueFormat::Raw,
            children: vec![
                FormulaNode {
                    label: "Ladles / day (in-game AFK panel)".into(),
                    value: self.panel_per_day,
                    format: ValueFormat::Raw,
                    ..FormulaNode::default()
                },
                FormulaNode {
                    label: "Claim window".into(),
                    value: self.effective_claim_seconds / 86_400.0,
                    format: ValueFormat::Multiplier,
                    note: Some(if self.claim_capped {
                        "Prayer 2 caps claims at 10h".into()
                    } else {
                        format!("{}h claim", self.claim_hours)
                    }),
                    ..FormulaNode::default()
                },
                FormulaNode {
                    label: "Ladles per claim".into(),
                    value: self.raw_per_claim,
                    format: ValueFormat::Raw,
                    note: Some("floored when credited; below 1 may give nothing".into()),
                    ..FormulaNode::default()
                },
                FormulaNode {
                    label: "Post Office 19c doubling chance".into(),
                    value: self.double_chance,
                    format: ValueFormat::Percent,
                    note: Some("claims above 0.5 roll a 2x; each claim then floors".into()),
                    ..FormulaNode::default()
                },
                FormulaNode {
                    label: "Expected ladles per claim".into(),
                    value: self.per_claim,
                    format: ValueFormat::Raw,
                    ..FormulaNode::default()
                },
                FormulaNode {
                    label: "Claims per day".into(),
                    value: 24.0 / self.claim_hours,
                    format: ValueFormat::Multiplier,
                    ..FormulaNode::default()
                },
                FormulaNode {
                    label: "Guaranteed ladles / day".into(),
                    value: self.min_per_day,
                    format: ValueFormat::Raw,
                    note: Some(format!(
                        "{} claims, last one before reset",
                        self.claim_count()
                    )),
                    ..FormulaNode::default()
                },
            ],
            ..FormulaNode::default()
        }
    }

    /// The Cooking EXP balloon tree: what one balloon gives and what a level costs.
    pub fn balloons_tree(
        &self,
        balloon: Balloon,
        owned: f64,
        target: Option<f64>,
        split: Option<&Split>,
    ) -> FormulaNode {
        let mut children = vec![
            FormulaNode {
                label: "Cooking level".into(),
                value: self.level,
                format: ValueFormat::Count,
                ..FormulaNode::default()
            },
            FormulaNode {
                label: "EXP banked".into(),
                value: self.exp,
                format: ValueFormat::Raw,
                ..FormulaNode::default()
            },
            FormulaNode {
                label: "EXP for the next level".into(),
                value: self.exp_req,
                format: ValueFormat::Raw,
                note: Some(
                    "(15 + Lv² + 15 × Lv) × (1.225 − min(0.18, 0.135 × Lv / (Lv + 50)))^Lv − 30"
                        .into(),
                ),
                ..FormulaNode::default()
            },
            FormulaNode {
                label: format!("EXP per {} at this level", balloon.label),
                value: balloon_gain(balloon.item_id, self.level, self.exp_multi),
                format: ValueFormat::Raw,
                note: Some(format!(
                    "item ID {} × (Lv + 1) × EXP multi {}",
                    balloon.item_id, self.exp_multi
                )),
                ..FormulaNode::default()
            },
            FormulaNode {
                label: "Balloons owned".into(),
                value: owned,
                format: ValueFormat::Count,
                ..FormulaNode::default()
            },
        ];
        if let Some(target) = target
            && target > self.level + 1.0
        {
            children.push(FormulaNode {
                label: format!("Balloons to Lv {target}"),
                value: balloons_to_level(
                    self.level,
                    self.exp,
                    target,
                    self.exp_multi,
                    balloon.item_id,
                )
                .balloons,
                format: ValueFormat::Count,
                note: Some("one at a time; overflow carries into the next level".into()),
                ..FormulaNode::default()
            });
        }
        if let Some(split) = split
            && let Some(Some(share)) = split.characters.get(self.index)
        {
            children.push(FormulaNode {
                label: "Level with the split share".into(),
                value: share.level,
                format: ValueFormat::Count,
                note: Some(format!("{} balloons from the even split", share.used)),
                ..FormulaNode::default()
            });
        }
        FormulaNode {
            label: format!("Balloons to the next Cooking level ({})", balloon.label),
            value: self.next_level_balloons,
            format: ValueFormat::Count,
            note: Some("one balloon at a time; overflow carries".into()),
            children,
            ..FormulaNode::default()
        }
    }

    /// How many claims the guaranteed day total assumes.
    fn claim_count(&self) -> usize {
        let window = 24.0;
        (window / self.claim_hours - 1e-9).ceil().max(1.0) as usize
    }

    /// The next-step cell's text, as the upstream outlook column shows it.
    pub fn outlook_label(&self) -> String {
        let outlook = self.outlook;
        if outlook.small_tier {
            format!(
                "+{:.1}% @ +{:.1}% eff",
                outlook.next_tier_gain_pct, outlook.next_tier_efficiency_pct
            )
        } else {
            format!("+10% eff → +{:.2}%", outlook.plus_10_efficiency_gain_pct)
        }
    }
}

/// One Post Office box slot: its index, threshold and the two coefficients of
/// the `decay` formula the reward uses.
struct PostOfficeSlot {
    box_index: usize,
    threshold: f64,
    x1: f64,
    x2: f64,
}

/// `computeBoxReward` for a `decay` slot: the invested points past the slot's
/// threshold, run through `x1 × lv / (lv + x2)`.
fn box_reward(points: f64, slot: &PostOfficeSlot) -> f64 {
    let mut points = points.max(0.0);
    if points <= 0.0 {
        return 0.0;
    }
    points = (points - slot.threshold).round();
    if points <= 0.0 {
        return 0.0;
    }
    slot.x1 * points / (points + slot.x2)
}

/// `prayerValue(2, 1, ci)`: the Prayer 2 bonus a character holds.
///
/// `costIdx` is 1, which is the charge the cap reads; upstream only pays the
/// no-prayer passive on the other costs, so an unequipped character holds none.
fn prayer_2_value(save: &Save, index: usize) -> f64 {
    let level = save
        .get("PrayOwned")
        .as_ref()
        .and_then(|owned| idleon_save::at(owned, 2))
        .and_then(idleon_save::number)
        .unwrap_or(0.0);
    if level <= 0.0 {
        return 0.0;
    }
    let equipped = save
        .get(&format!("Prayers_{index}"))
        .map(|row| idleon_save::row_values(&row))
        .unwrap_or_default()
        .iter()
        .filter_map(idleon_save::number)
        .any(|prayer| prayer == 2.0);
    if !equipped {
        return 0.0;
    }
    let scale = (1.0 + (level - 1.0) / 10.0).max(1.0);
    (PRAYER_2_BASE_BONUS * scale).round()
}

/// `_prayerCapped`: Prayer 2 credits ten hours, which is over the two-hour gate.
fn prayer_capped(save: &Save, index: usize) -> bool {
    prayer_2_value(save, index) > 2.0
}

/// Every stored item the save can see: the chest and each character inventory.
struct Inventories {
    rows: Vec<(Vec<serde_json::Value>, Vec<serde_json::Value>)>,
    missing: Vec<String>,
}

impl Inventories {
    fn count(&self, item: &str) -> Count {
        let mut value = 0.0;
        for (order, quantity) in &self.rows {
            for (slot, name) in order.iter().enumerate() {
                if name.as_str() != Some(item) {
                    continue;
                }
                value += quantity
                    .get(slot)
                    .and_then(idleon_save::number)
                    .unwrap_or(0.0);
            }
        }
        Count {
            value,
            partial: !self.missing.is_empty(),
            missing: self.missing.clone(),
        }
    }
}

/// The chest and every `InventoryOrder_<n>` / `ItemQTY_<n>` pair.
fn inventories(save: &Save) -> Inventories {
    let mut rows = Vec::new();
    let mut missing = Vec::new();
    let chest = save.get("ChestOrder").map(|order| {
        let quantity = save
            .get("ChestQuantity")
            .map(|row| idleon_save::row_values(&row))
            .unwrap_or_default();
        (idleon_save::row_values(&order), quantity)
    });
    match chest {
        Some(chest) => rows.push(chest),
        None => missing.push("storage inventory".into()),
    }
    let mut found_any = false;
    for index in 0..save.character_count() {
        let order = save.get(&format!("InventoryOrder_{index}"));
        let quantity = save.get(&format!("ItemQTY_{index}"));
        match (order, quantity) {
            (Some(order), Some(quantity)) => {
                found_any = true;
                rows.push((
                    idleon_save::row_values(&order),
                    idleon_save::row_values(&quantity),
                ));
            }
            _ => missing.push(format!("character inventory {}", index + 1)),
        }
    }
    if !found_any {
        missing.push("character inventory".into());
    }
    Inventories { rows, missing }
}

/// `TimeAway.ShopRestock` as hours until the daily reset.
fn reset_hours(save: &Save) -> Option<f64> {
    let restock = save
        .get("TimeAway")
        .as_ref()
        .and_then(|time| time.get("ShopRestock"))
        .and_then(idleon_save::number)?;
    restock
        .is_finite()
        .then(|| (restock / 3600.0).clamp(0.0, 24.0))
}

/// Every character's save-derived Cooking state.
fn characters(save: &Save) -> Vec<Character> {
    let global_time = save
        .get("TimeAway")
        .as_ref()
        .and_then(|time| time.get("GlobalTime"))
        .and_then(idleon_save::number)
        .unwrap_or(0.0);
    let post_office = |index: usize| {
        save.get(&format!("POu_{index}"))
            .as_ref()
            .and_then(|row| idleon_save::at(row, BOX_19C.box_index))
            .and_then(idleon_save::number)
            .map(|points| box_reward(points, &BOX_19C))
            .unwrap_or(0.0)
    };
    let cooking = |key: String| {
        save.get(&key)
            .as_ref()
            .and_then(|row| idleon_save::at(row, COOKING_LEVEL_INDEX))
            .and_then(idleon_save::number)
            .unwrap_or(0.0)
    };
    (0..save.character_count())
        .map(|index| {
            let level = cooking(format!("Lv0_{index}"));
            let exp = cooking(format!("Exp0_{index}"));
            let afk_target = save
                .get(&format!("AFKtarget_{index}"))
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_default();
            let afk_start = save
                .get(&format!("PTimeAway_{index}"))
                .and_then(|value| idleon_save::number(&value))
                .filter(|value| value.is_finite() && *value > 0.0)
                .map(|value| value * 1000.0);
            let pending_hours = match (afk_target == "Cooking", afk_start, global_time > 0.0) {
                (true, Some(start), true) => Some(((global_time - start) / 3600.0).max(0.0)),
                _ => None,
            };
            Character {
                index,
                name: save.character_name(index),
                afk_target,
                level,
                exp,
                pending_hours,
                double_chance: post_office(index),
                claim_capped: prayer_capped(save, index),
            }
        })
        .collect()
}
