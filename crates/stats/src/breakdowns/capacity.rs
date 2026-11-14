//! Carry capacity, ported from `js/stats/defs/carry-capacity.js` and
//! `js/stats/systems/common/carry-capacity.js`.
//!
//! `computeMaxCapacity` reads the saved row, the account flat sources, the
//! category and all-carry stamps, the gem-shop multiplier, the material talent
//! and the common multiplier chain. Every contributor whose formula this build
//! does not port is named in the row's partial reason instead of being guessed;
//! the value is the product of exactly the terms the tree lists.

use super::{Inputs, Resolution};
use idleon_common::curve::eval;
use idleon_common::formula::{FormulaNode, ValueFormat};
use idleon_save::{Save, at, number, row_values};
use idleon_stats_data::{self as data, StampRow};
use serde_json::Value;

/// The hard cap `computeMaxCapacity` applies after flooring.
const HARD_CAP: f64 = 2_050_000_000.0;

/// The eight carry-capacity descriptors, in the upstream `CAPACITY_ROWS` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapacityKind {
    Mining,
    Chopping,
    Fishing,
    Catching,
    Food,
    Critters,
    Souls,
    Materials,
}

impl CapacityKind {
    /// The suffix of each descriptor id, in `CAPACITY_ROWS` order.
    const IDS: [&'static str; 8] = [
        "mining",
        "chopping",
        "fishing",
        "catching",
        "food",
        "critters",
        "souls",
        "materials",
    ];

    /// Every capacity descriptor.
    pub const ALL: [CapacityKind; 8] = [
        CapacityKind::Mining,
        CapacityKind::Chopping,
        CapacityKind::Fishing,
        CapacityKind::Catching,
        CapacityKind::Food,
        CapacityKind::Critters,
        CapacityKind::Souls,
        CapacityKind::Materials,
    ];

    /// The descriptor the id names, when it is a capacity row.
    pub fn from_id(id: &str) -> Option<Self> {
        let suffix = id.strip_prefix("carry-capacity-")?;
        let index = Self::IDS
            .iter()
            .position(|candidate| *candidate == suffix)?;
        Self::ALL.get(index).copied()
    }

    /// The upstream descriptor id.
    pub fn id(self) -> String {
        format!("carry-capacity-{}", Self::IDS[self.index()])
    }

    /// The upstream descriptor name.
    pub fn name(self) -> &'static str {
        match self {
            CapacityKind::Mining => "Mining Carry Capacity",
            CapacityKind::Chopping => "Chopping Carry Capacity",
            CapacityKind::Fishing => "Fishing Carry Capacity",
            CapacityKind::Catching => "Catching Carry Capacity",
            CapacityKind::Food => "Food Carry Capacity",
            CapacityKind::Critters => "Critter Carry Capacity",
            CapacityKind::Souls => "Soul Carry Capacity",
            CapacityKind::Materials => "Material Carry Capacity",
        }
    }

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|kind| *kind == self)
            .unwrap_or_default()
    }

    /// The `MaxCarryCap_<char>` field the row reads.
    fn field(self) -> &'static str {
        match self {
            CapacityKind::Mining => "Mining",
            CapacityKind::Chopping => "Chopping",
            CapacityKind::Fishing => "Fishing",
            CapacityKind::Catching => "Bugs",
            CapacityKind::Food => "Foods",
            CapacityKind::Critters => "Critters",
            CapacityKind::Souls => "Souls",
            CapacityKind::Materials => "bCraft",
        }
    }

    /// The stamp group upstream's `TYPE_CONFIG` names, when the row has one.
    fn stamp(self) -> Option<&'static str> {
        match self {
            CapacityKind::Mining => Some("MinCap"),
            CapacityKind::Chopping => Some("ChopCap"),
            CapacityKind::Fishing => Some("FishCap"),
            CapacityKind::Catching => Some("CatchCap"),
            CapacityKind::Materials => Some("MatCap"),
            CapacityKind::Food | CapacityKind::Critters | CapacityKind::Souls => None,
        }
    }

    /// Whether the row applies talent 78, the material carry talent.
    fn material(self) -> bool {
        self == CapacityKind::Materials
    }
}

/// Resolve one capacity descriptor, when the id is one of the eight.
pub(super) fn resolve(inputs: &Inputs<'_>, id: &str) -> Option<Resolution> {
    let kind = CapacityKind::from_id(id)?;
    Some(capacity(inputs, kind))
}

fn capacity(inputs: &Inputs<'_>, kind: CapacityKind) -> Resolution {
    let save = inputs.save;
    let key = format!("MaxCarryCap_{}", inputs.character);
    let Some(row) = save.get(&key) else {
        return Resolution::Unavailable {
            reason: format!("{key} is missing from the imported save."),
        };
    };
    let saved_base = row.get(kind.field()).and_then(number).unwrap_or(0.0);
    let mut partial = Partial::default();

    let vault = idleon_account::vault_bonus(save);
    let vault_level = row_number(save, "UpgVault", 11).unwrap_or(0.0);
    let bundle = bundle_flat(save);
    let account_flat = vault + bundle;
    let gem = row_number(save, "GemItemsPurchased", 58).unwrap_or(0.0);
    let all_carry_stamp = stamps(save, inputs.character, "AllCarryCap", &mut partial);
    let stars = carry_stars(save, inputs.character, &mut partial);
    let all_carry_pct = all_carry_stamp.value + stars;
    let category_stamp = match kind.stamp() {
        Some(group) => stamps(save, inputs.character, group, &mut partial),
        None => StampTotal::default(),
    };
    let material_talent = if kind.material() {
        talent(save, inputs.character, 78)
    } else {
        0.0
    };
    let talent_634 = talent(save, inputs.character, 634);
    if material_talent > 0.0 || talent_634 > 0.0 {
        partial.add(TALENT_LEVELS);
    }
    let guild = guild_bonus(save, 2);
    let companion = companion_bonus(save, 18, &mut partial);
    let shrine = shrine_bonus(inputs, 3, &mut partial);
    let prayer_curse = prayer(save, inputs.character, 4, true);
    let prayer_bonus = prayer(save, inputs.character, 12, false);
    let bribe = bribe_bonus(save, 23);
    let common = (1.0 + (guild + talent_634) / 100.0)
        * (1.0 + companion / 100.0)
        * (1.0 + shrine / 100.0)
        * (1.0 - prayer_curse / 100.0).max(0.4)
        * (1.0 + (prayer_bonus + bribe) / 100.0);
    let factors = Factors {
        category_stamp: category_stamp.value,
        gem_purchases: gem,
        all_carry_pct,
        material_talent,
        common_multiplier: common,
    };
    let value = capacity_from_factors(account_flat + saved_base, &factors);

    let tree = FormulaNode::new(kind.name(), value)
        .with_format(ValueFormat::Count)
        .with_input(format!(
            "{} saved + {} account flat, then the factors below, floored at the hard cap",
            ValueFormat::Count.format(saved_base),
            ValueFormat::Count.format(account_flat)
        ))
        .with_source("js/stats/systems/common/carry-capacity.js · computeMaxCapacity")
        .with_children([
            FormulaNode::leaf("Saved bag capacity", saved_base, ValueFormat::Count)
                .with_source(format!("{key} · {}", kind.field())),
            FormulaNode::new("Account flat capacity", account_flat)
                .with_format(ValueFormat::Count)
                .with_children([
                    FormulaNode::leaf("Vault upgrade 11", vault, ValueFormat::Count)
                        .with_input(format!("Level {vault_level}"))
                        .with_source("UpgVault[11] · Carry Capacity"),
                    FormulaNode::leaf("Bundles", bundle, ValueFormat::Count)
                        .with_source("BundlesReceived · bon_w + bon_x + bon_y × 1000"),
                ]),
            FormulaNode::new("Category stamp", category_stamp.value)
                .with_format(ValueFormat::Count)
                .with_source(match kind.stamp() {
                    Some(group) => format!("StampLv · {group}"),
                    None => "No stamp group feeds this capacity".to_string(),
                })
                .with_children(category_stamp.children),
            FormulaNode::new("All carry sources", all_carry_pct)
                .with_format(ValueFormat::Percent)
                .with_children([
                    FormulaNode::new("All Carry Cap stamp", all_carry_stamp.value)
                        .with_format(ValueFormat::Count)
                        .with_source("StampLv · AllCarryCap")
                        .with_children(all_carry_stamp.children),
                    FormulaNode::new("Carry Capacity star signs", stars)
                        .with_format(ValueFormat::Count)
                        .with_source("PVtStarSign_<char> and StarSg"),
                ]),
            FormulaNode::leaf(
                "Gem shop multiplier",
                1.0 + 0.25 * gem,
                ValueFormat::Multiplier,
            )
            .with_input(format!("{gem} purchases"))
            .with_source("GemItemsPurchased[58]"),
            FormulaNode::leaf(
                "Material talent multiplier",
                1.0 + material_talent / 100.0,
                ValueFormat::Multiplier,
            )
            .with_input(format!("{material_talent}%"))
            .with_source("Talent 78 · +% Mat Carry Cap"),
            FormulaNode::new("Common multiplier", common)
                .with_format(ValueFormat::Multiplier)
                .with_children([
                    FormulaNode::leaf(
                        "Guild 2 and talent 634",
                        1.0 + (guild + talent_634) / 100.0,
                        ValueFormat::Multiplier,
                    )
                    .with_input(format!("Guild {guild}% + talent {talent_634}%"))
                    .with_source("Guild[0][2] · Rucksack and SL_<char>[634]"),
                    FormulaNode::leaf(
                        "Companion 18",
                        1.0 + companion / 100.0,
                        ValueFormat::Multiplier,
                    )
                    .with_source("Roster · companion 18"),
                    FormulaNode::leaf(
                        "Pantheon Shrine",
                        1.0 + shrine / 100.0,
                        ValueFormat::Multiplier,
                    )
                    .with_source("Shrine[3] · Pantheon Shrine"),
                    FormulaNode::leaf(
                        "Prayer 4 curse",
                        (1.0 - prayer_curse / 100.0).max(0.4),
                        ValueFormat::Multiplier,
                    )
                    .with_input(format!("Zerg Rushogen {prayer_curse}%, floored at ×0.4")),
                    FormulaNode::leaf(
                        "Prayer 12 and bribe 23",
                        1.0 + (prayer_bonus + bribe) / 100.0,
                        ValueFormat::Multiplier,
                    )
                    .with_input(format!(
                        "Ruck Sack {prayer_bonus}% + Bottomless Bags {bribe}%"
                    )),
                ]),
            FormulaNode::leaf("Hard cap", HARD_CAP, ValueFormat::Count)
                .with_input("The value is floored after the cap"),
        ]);
    Resolution::Value {
        tree,
        partial: partial.into_reason(),
    }
}

/// The factors `capacityFromFactors` multiplies.
struct Factors {
    category_stamp: f64,
    gem_purchases: f64,
    all_carry_pct: f64,
    material_talent: f64,
    common_multiplier: f64,
}

/// `capacityFromFactors`: the base and flat sources scaled by every factor.
fn capacity_from_factors(base_capacity: f64, factors: &Factors) -> f64 {
    let raw = base_capacity.max(0.0)
        * (1.0 + factors.category_stamp / 100.0)
        * (1.0 + 25.0 * factors.gem_purchases / 100.0)
        * (1.0 + factors.all_carry_pct / 100.0)
        * (1.0 + factors.material_talent / 100.0)
        * factors.common_multiplier;
    raw.min(HARD_CAP).floor()
}

/// The unmodelled upstream systems a row met, named once each.
#[derive(Default)]
struct Partial {
    reasons: Vec<&'static str>,
}

impl Partial {
    fn add(&mut self, reason: &'static str) {
        if !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
    }

    fn into_reason(self) -> Option<String> {
        if self.reasons.is_empty() {
            return None;
        }
        Some(format!(
            "Partial total: {}. The value is the product of the terms this build decodes.",
            self.reasons.join("; ")
        ))
    }
}

const LAB_STAMP: &str =
    "the Lab layout is missing, so the Certified Stamp Book (mainframe 7) cannot double stamps";
const SERAPH_MERITOC: &str =
    "the Meritocracy 22 factor of Seraph Cosmos is not decoded and is left at x1";
const SHRINE_TOUR: &str =
    "the Shrine World Tour needs the lab mainframe 5 node, which is not decoded";
const NO_COMPANION_ROSTER: &str =
    "the companion roster beside the save is missing, so companion 18 reads zero";
const TALENT_LEVELS: &str = "the all-talent bonus levels are not decoded, so talents 78 and 634 \
    read their purchased levels";

/// A stamp group's total and the rows that add up to it.
#[derive(Default)]
struct StampTotal {
    value: f64,
    children: Vec<FormulaNode>,
}

/// `computeCarryStampBonus` for one stamp group.
fn stamps(save: &Save, character: usize, group: &str, partial: &mut Partial) -> StampTotal {
    let table = data::capacity();
    let levels = save.get("StampLv");
    let compass = save.get("Compass");
    let pristine = pristine_bonus(save, 17);
    let lab_double = match idleon_account::mainframe_bonus(save, 7) {
        Some(2.0) => 2.0,
        Some(_) => 1.0,
        None => {
            partial.add(LAB_STAMP);
            1.0
        }
    };
    let mut doubler: Option<f64> = None;
    let mut total = StampTotal::default();
    for stamp in table.stamps.iter().filter(|stamp| stamp.kind == group) {
        let level = levels
            .as_ref()
            .and_then(|rows| at(rows, stamp.cat))
            .map(|row| row_number_in(row, stamp.idx))
            .unwrap_or(0.0);
        if level <= 0.0 {
            continue;
        }
        let effective = stamp_over_level(save, character, stamp, level).floor();
        let raw = eval(&stamp.formula, stamp.x1, stamp.x2, effective);
        let exalted = is_exalted(compass.as_ref(), stamp);
        let category = if stamp.cat < 2 {
            lab_double * (1.0 + pristine / 100.0)
        } else {
            1.0
        };
        let doubler_total = if exalted {
            *doubler.get_or_insert_with(|| stamp_doubler(save))
        } else {
            0.0
        };
        let value = raw * category * (1.0 + doubler_total / 100.0);
        total.value += value;
        total.children.push(
            FormulaNode::leaf(stamp_key(stamp), value, ValueFormat::Count)
                .with_input(format!(
                    "Lv {level} → {effective}; {} on {}",
                    stamp.formula, stamp.item
                ))
                .with_multiplier(category * (1.0 + doubler_total / 100.0))
                .with_source(format!("ITEMS.{} · desc_line1", stamp.item)),
        );
    }
    total
}

/// The stamp key upstream's `stampKey` builds: category letter plus index.
fn stamp_key(stamp: &StampRow) -> String {
    format!("{}{}", number_letter(stamp.cat).unwrap_or('_'), stamp.idx)
}

/// `computeStampDoublerSources`: the exalted multiplier behind one stamp.
///
/// Upstream adds ten account sources to a 100% base. The four this build does
/// not decode are named in the partial reason; the rest are plain save reads.
fn stamp_doubler(save: &Save) -> f64 {
    let table = data::capacity();
    let mut total = 100.0;
    // Atoms[12], the Stamp Supercharger.
    total += row_number(save, "Atoms", 12).unwrap_or(0.0);
    total += pristine_bonus(save, 20);
    let compass = save.get("Compass");
    let compass76 = compass
        .as_ref()
        .and_then(|rows| at(rows, 0).cloned())
        .map(|row| row_number_in(&row, 76))
        .unwrap_or(0.0);
    total += compass76 * table.compass_per_level.get(76).copied().unwrap_or(0.0);
    if row_text(save, "OptLacc", 379).contains("EMPEROR_SET") {
        total += table.emperor_set;
    }
    if event_shop_owned(save, 18) {
        total += 20.0;
    }
    total += nested_number(save, "Spelunk", &[4, 3])
        .unwrap_or(0.0)
        .round();
    total += rog_bonus(save, 17, table);
    total += legend_bonus(save, 36, table);
    total += palette_bonus(save, 23, table);
    let jelly_obstructions = nested_number(save, "Research", &[7, 9]).unwrap_or(0.0);
    if jelly_obstructions > 50.0 {
        total += table.jelly_reward50;
    }
    let exotic_level = row_number(save, "FarmUpg", 49 + 20).unwrap_or(0.0);
    if exotic_level > 0.0 {
        total += if table.exotic49.decay {
            table.exotic49.base * exotic_level / (1000.0 + exotic_level)
        } else {
            table.exotic49.base * exotic_level
        };
    }
    total
}

/// `computePaletteBonus(23)`: the palette's own level and its multipliers.
fn palette_bonus(save: &Save, index: usize, table: &data::CapacityData) -> f64 {
    let level = nested_number(save, "Spelunk", &[9, index]).unwrap_or(0.0);
    if level <= 0.0 {
        return 0.0;
    }
    let param = &table.palette23;
    let raw = if param.decay {
        level / (level + param.denom) * param.coeff
    } else {
        level * param.coeff
    };
    let legend = 1.0 + legend_bonus(save, 10, table) / 100.0;
    let star = nested_number(save, "Spelunk", &[49, index]).unwrap_or(0.0);
    let black_star = (1.0 + star + (star / 2.0).floor()).max(1.0);
    let lore = if nested_number(save, "Spelunk", &[0, 8]).unwrap_or(0.0) >= 1.0 {
        1.5
    } else {
        1.0
    };
    raw * legend * black_star * lore
}

/// `legendPTSbonus(idx)`: the points invested in a Legend talent.
fn legend_bonus(save: &Save, index: usize, table: &data::CapacityData) -> f64 {
    let points = nested_number(save, "Spelunk", &[18, index]).unwrap_or(0.0);
    let per_point = table.legend_per_point.get(index).copied().unwrap_or(0.0);
    (points * per_point).round().max(0.0)
}

/// `computeShinyBonusS(cat)`: the Breeding shiny levels that raise a category.
pub(crate) fn shiny_bonus(save: &Save, category: u64) -> f64 {
    let table = data::capacity();
    let unlock = save
        .get("Menagerie")
        .and_then(|rows| at(&rows, 2).cloned())
        .map(|row| row_number_in(&row, 11))
        .unwrap_or(0.0);
    let widen: f64 = if unlock >= 1.0 { 10.0 } else { 0.0 };
    let thresholds = 19.0 + widen;
    let breeding = save.get("Breeding");
    let mut total = 0.0;
    for (world, pets) in table.pet_shiny_type.iter().enumerate() {
        let Some(shiny) = breeding
            .as_ref()
            .and_then(|rows| at(rows, 22 + world).cloned())
        else {
            continue;
        };
        for (pet, kind) in pets.iter().enumerate() {
            let exp = row_number_in(&shiny, pet);
            if exp <= 0.0 {
                continue;
            }
            let Some(cat) = table.shiny_type_to_cat.get(*kind as usize) else {
                continue;
            };
            if *cat != category {
                continue;
            }
            let level = shiny_pet_level(exp, thresholds as usize);
            let per_level = table
                .shiny_bonus_per_level
                .get(*kind as usize)
                .copied()
                .unwrap_or(0.0);
            total += (level * per_level).round();
        }
    }
    total
}

/// `shinyPetLevel`: the level a shiny EXP value reaches on the game's curve.
pub(crate) fn shiny_pet_level(exp: f64, thresholds: usize) -> f64 {
    let mut level = 1.0;
    for step in 0..thresholds {
        let e = step as f64;
        if exp > ((1.0 + (e + 1.0).powf(1.6)) * 1.7f64.powf(e + 1.0)).floor() {
            level = e + 2.0;
        }
    }
    level
}

/// The number of shiny pets in each Breeding world, in `PET_STATS` order.
pub(crate) fn shiny_pet_worlds() -> Vec<usize> {
    data::capacity()
        .pet_shiny_type
        .iter()
        .map(Vec::len)
        .collect()
}

/// `rogBonusQTY(idx, uniqueSushi)`: the Ring of Glory bonus once its tier is met.
fn rog_bonus(save: &Save, index: usize, table: &data::CapacityData) -> f64 {
    let sushi = save.get("Sushi");
    let unique = sushi
        .as_ref()
        .and_then(|rows| at(rows, 5).cloned())
        .map(|row| {
            let mut count = 0.0;
            for (position, value) in row_values(&row).iter().enumerate() {
                if value.as_f64().is_none() && number(value).is_none() {
                    break;
                }
                count = position as f64 + 1.0;
            }
            count
        })
        .unwrap_or(0.0);
    if unique > index as f64 {
        table.rog_bonus.get(index).copied().unwrap_or(0.0)
    } else {
        0.0
    }
}

/// `eventShopOwned(t, OptLacc[311])`: `N2L` membership in the event shop string.
fn event_shop_owned(save: &Save, index: usize) -> bool {
    let flags = row_text(save, "OptLacc", 311);
    number_letter(index).is_some_and(|letter| flags.contains(letter))
}

/// `_stampOverLevel`: over-level stamps scale by the skill level behind them.
fn stamp_over_level(save: &Save, character: usize, stamp: &StampRow, raw_level: f64) -> f64 {
    if stamp.skill_idx <= 0 || emporium_owned(save, 5) {
        return raw_level;
    }
    if stamp.overlevel_step <= 1.0 {
        return raw_level;
    }
    let skill_level =
        row_number(save, &format!("Lv0_{character}"), stamp.skill_idx as usize).unwrap_or(0.0);
    let scaled = raw_level * (10.0 / stamp.overlevel_step);
    let mut adjusted = scaled;
    if scaled > 3.0 {
        adjusted = 3.0 + (scaled - 3.0) * (skill_level / (scaled - 3.0).max(1e-300)).powf(0.75);
        adjusted *= stamp.overlevel_step / 10.0;
    }
    adjusted.min(raw_level)
}

/// `emporiumBonus(t, Ninja[102][9])`: `N2L` membership.
fn emporium_owned(save: &Save, index: usize) -> bool {
    let flags = save
        .get("Ninja")
        .and_then(|rows| at(&rows, 102).cloned())
        .map(|row| row_values(&row))
        .and_then(|row| row.get(9).cloned())
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    number_letter(index).is_some_and(|letter| flags.contains(letter))
}

/// `N2L`: the game's `Number2Letter` alphabet.
fn number_letter(index: usize) -> Option<char> {
    data::capacity().number_letter.chars().nth(index)
}

/// `isExalted`: the compass's exalted stamp list.
fn is_exalted(compass: Option<&Value>, stamp: &StampRow) -> bool {
    let Some(row) = compass.and_then(|compass| at(compass, 4)) else {
        return false;
    };
    let key = stamp_key(stamp);
    row_values(row).iter().any(|value| match value {
        Value::String(text) => *text == key,
        _ => false,
    })
}

/// `pristineBon(idx)`: the charm bonus when `Ninja[107][idx]` is set.
fn pristine_bonus(save: &Save, index: usize) -> f64 {
    let owned = save
        .get("Ninja")
        .and_then(|rows| at(&rows, 107).cloned())
        .map(|row| row_number_in(&row, index));
    if owned != Some(1.0) {
        return 0.0;
    }
    data::capacity()
        .pristine_charms
        .get(&index.to_string())
        .copied()
        .unwrap_or(0.0)
}

/// `computeStarSignBonus('CarryCap', char)`.
///
/// The sign values and the active-sign rules are upstream's; the Seraph Cosmos
/// multiplier applies when its extra stars are unlocked, with the one factor
/// this build does not decode named in the partial reason.
fn carry_stars(save: &Save, character: usize, partial: &mut Partial) -> f64 {
    let enabled = enabled_star_signs(save);
    let equipped = save
        .get(&format!("PVtStarSign_{character}"))
        .map(|value| {
            csv_numbers(&value)
                .into_iter()
                .map(|sign| sign as usize)
                .collect::<Vec<usize>>()
        })
        .unwrap_or_default();
    let unlocked = save.get("StarSg");
    let mut total = 0.0;
    for [sign, value] in data::capacity()
        .carry_star_signs
        .iter()
        .map(|row| [row[0], row[1]])
    {
        let sign = sign as usize;
        let owned = star_owned(unlocked.as_ref(), sign, enabled);
        if !equipped.contains(&sign) && !owned {
            continue;
        }
        if value < 0.0 && sign < enabled {
            continue;
        }
        total += value;
    }
    if total > 0.0 {
        total *= seraph_multi(save, character, enabled, partial);
    }
    total
}

/// `getEnabledStarSigns`: infinite signs start at five on a Rift level 10,
/// and every Breeding shiny level in category 3 widens the limit.
fn enabled_star_signs(save: &Save) -> usize {
    if row_number(save, "Rift", 0).unwrap_or(0.0) < 10.0 {
        return 0;
    }
    (5.0 + shiny_bonus(save, 3)) as usize
}

/// `isStarSignActive`: an unlocked sign below the infinite-sign limit.
fn star_owned(unlocked: Option<&Value>, sign: usize, enabled: usize) -> bool {
    let Some(unlocked) = unlocked else {
        return false;
    };
    if sign >= enabled {
        return false;
    }
    let name = data::capacity()
        .star_sign_names
        .get(sign)
        .map(String::as_str)
        .unwrap_or_default();
    unlocked
        .as_object()
        .is_some_and(|fields| fields.contains_key(name))
}

/// `computeSeraphMulti`: chip and Seraph Cosmos factors on the sign total.
fn seraph_multi(save: &Save, character: usize, enabled: usize, partial: &mut Partial) -> f64 {
    if save.get("StarSg").is_none() {
        return 1.0;
    }
    let arcane40 = row_number(save, "Arcane", 40).unwrap_or(0.0);
    let summon_level = row_number(save, &format!("Lv0_{character}"), 18).unwrap_or(0.0);
    let base = 1.1 + arcane40.min(10.0) / 100.0;
    let exponent = ((summon_level + 1.0) / 20.0).ceil();
    let seraph = base.powf(exponent).min(5.0);
    let chip = if enabled >= 1 && star_chip(save, character) {
        2.0
    } else {
        1.0
    };
    if meritoc_star_vote(save, character) {
        partial.add(SERAPH_MERITOC);
    }
    chip * seraph
}

/// Whether the Seraph Cosmos factor the build does not decode can apply:
/// Meritocracy option 22 is the active vote and map 250 is cleared.
fn meritoc_star_vote(save: &Save, character: usize) -> bool {
    let vote = row_number(save, "OptLacc", 453).unwrap_or(0.0);
    if vote != 22.0 {
        return false;
    }
    nested_number(save, &format!("KLA_{character}"), &[250, 0]).unwrap_or(0.0) <= 0.0
}

/// `hasStarChip`: the Silkrode Nanochip sits in one of a character's lab slots.
fn star_chip(save: &Save, character: usize) -> bool {
    save.get("Lab")
        .and_then(|rows| at(&rows, 1 + character).cloned())
        .map(|row| row_values(&row))
        .is_some_and(|row| row.iter().take(7).any(|slot| number(slot) == Some(15.0)))
}

/// `guild.resolve(2)`: Rucksack's guild-points curve.
fn guild_bonus(save: &Save, index: usize) -> f64 {
    let Some(curve) = data::capacity().guilds.get(index) else {
        return 0.0;
    };
    let points = save
        .get("Guild")
        .and_then(|rows| at(&rows, 0).cloned())
        .map(|row| row_number_in(&row, index))
        .unwrap_or(0.0);
    if points <= 0.0 {
        return 0.0;
    }
    eval(&curve.formula, curve.x1, curve.x2, points)
}

/// `talent.resolve(id)`: the saved level on the talent's own curve.
fn talent(save: &Save, character: usize, index: usize) -> f64 {
    let Some(curve) = data::capacity().talents.get(&index.to_string()) else {
        return 0.0;
    };
    let level = save
        .get(&format!("SL_{character}"))
        .map(|row| row_number_in(&row, index))
        .unwrap_or(0.0);
    if level <= 0.0 {
        return 0.0;
    }
    eval(&curve.formula, curve.x1, curve.x2, level)
}

/// `companionBonusForSave(id)` from the wrapper's roster.
fn companion_bonus(save: &Save, index: usize, partial: &mut Partial) -> f64 {
    let roster = idleon_save::Roster::read(
        save.root()
            .as_object()
            .and_then(|root| root.get("companion")),
    );
    if !roster.available {
        partial.add(NO_COMPANION_ROSTER);
        return 0.0;
    }
    if !roster.owns(index as u32) {
        return 0.0;
    }
    idleon_account::companion_bonus(index, roster.is_enhanced(index as u32))
}

/// `shrine.resolve(3)`: the Pantheon Shrine, gated on the selected map.
fn shrine_bonus(inputs: &Inputs<'_>, index: usize, partial: &mut Partial) -> f64 {
    let save = inputs.save;
    let Some([base, per_level]) = data::capacity().shrines.get(index).copied() else {
        return 0.0;
    };
    let Some(row) = save
        .get("Shrine")
        .and_then(|rows| at(&rows, index).cloned())
    else {
        return 0.0;
    };
    let shrine = row_values(&row);
    let level = shrine.get(3).and_then(number).unwrap_or(0.0);
    if level <= 0.0 {
        return 0.0;
    }
    let shrine_map = shrine.first().and_then(number).unwrap_or(0.0) as usize;
    // Sailing artifact 0 is the Moai Head, which makes shrine effects global.
    let artifact_global = save
        .get("Sailing")
        .and_then(|rows| at(&rows, 3).cloned())
        .map(|row| row_number_in(&row, 0) > 0.0)
        .unwrap_or(false);
    if !artifact_global && inputs.map != shrine_map {
        partial.add(SHRINE_TOUR);
        return 0.0;
    }
    let card = 1.0 + data::capacity().boss3b_card_pct * boss3b_card_level(save) / 100.0;
    ((level - 1.0) * per_level + base) * card
}

/// `computeCardLv('Boss3B')`: the critter card's level behind the shrine boost.
fn boss3b_card_level(save: &Save) -> f64 {
    let Some(cards) = save.get("Cards0") else {
        return 0.0;
    };
    let qty = cards.get("Boss3B").and_then(number).unwrap_or(0.0);
    if qty <= 0.0 {
        return 0.0;
    }
    let five_star: f64 = if row_number(save, "Rift", 0).unwrap_or(0.0) >= 45.0 {
        1.0
    } else {
        0.0
    };
    let six_star: f64 = if nested_number(save, "Spelunk", &[0, 2]).unwrap_or(0.0) >= 1.0 {
        1.0
    } else {
        0.0
    };
    let max_stars = (4.0 + five_star + six_star).round() as u32;
    let mut level = 1.0;
    for star in 0..max_stars {
        if qty > 1.5 * (star as f64 + 1.0 + (star / 3) as f64).powi(2) {
            level = star as f64 + 2.0;
        }
    }
    for (option, floor) in [(155, 6.0), (603, 7.0)] {
        let text = row_text(save, "OptLacc", option);
        if text.split(',').any(|entry| entry == "Boss3B") && level < floor {
            level = floor;
        }
    }
    level
}

/// `computePrayerReal(idx, costIdx, char)`: equipped, else the no-prayer passive.
fn prayer(save: &Save, character: usize, index: usize, curse_column: bool) -> f64 {
    save.as_object().map_or(0.0, |fields| {
        idleon_account::prayer(fields, character, index, curse_column)
    })
}

/// `getBribeBonus(idx)`: the bribe's flat percentage when it is active.
pub(crate) fn bribe_bonus(save: &Save, index: usize) -> f64 {
    if row_number(save, "BribeStatus", index).unwrap_or(0.0) != 1.0 {
        return 0.0;
    }
    data::capacity().bribes.get(index).copied().unwrap_or(0.0)
}

/// The bundle capacity rows: `bon_w + bon_x + bon_y`, 1000 each.
fn bundle_flat(save: &Save) -> f64 {
    let Some(bundles) = save.get("BundlesReceived") else {
        return 0.0;
    };
    let holder = bundles
        .get("h")
        .filter(|inner| inner.is_object())
        .unwrap_or(&bundles);
    1000.0
        * ["bon_w", "bon_x", "bon_y"]
            .iter()
            .filter_map(|key| holder.get(*key).and_then(number))
            .sum::<f64>()
}

/// A nested row's entry, e.g. `Spelunk[4][3]`.
fn nested_number(save: &Save, key: &str, path: &[usize]) -> Option<f64> {
    let mut value = save.get(key)?;
    for index in path {
        value = at(&value, *index)?.clone();
    }
    number(&value)
}

/// A flat row's entry, or `None` when the save has no such row.
fn row_number(save: &Save, key: &str, index: usize) -> Option<f64> {
    save.get(key).map(|row| row_number_in(&row, index))
}

/// A flat row's text entry, empty when the row or a string there is missing.
fn row_text(save: &Save, key: &str, index: usize) -> String {
    save.get(key)
        .map(|row| row_values(&row))
        .and_then(|row| row.get(index).and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default()
}

/// The numbers in a comma-separated save string, or in a row.
///
/// `PVtStarSign_<char>` is the one field the game writes as `69,55,20,`
/// rather than JSON; upstream splits it before reading the signs.
fn csv_numbers(value: &Value) -> Vec<f64> {
    match value {
        Value::String(text) => text
            .split(',')
            .filter_map(|part| part.trim().parse().ok())
            .collect(),
        _ => row_values(value).iter().filter_map(number).collect(),
    }
}

/// A row's entry, read as upstream's `rowValues` reads it.
fn row_number_in(row: &Value, index: usize) -> f64 {
    row_values(row).get(index).and_then(number).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_research::model::{SimContext, Simulation};
    use serde_json::json;

    fn flags(length: usize, set: &[(usize, f64)]) -> Value {
        let mut row = vec![Value::Null; length];
        for (index, value) in set {
            row[*index] = json!(value);
        }
        Value::Array(row)
    }

    /// A save with one point in every source the capacity row reads.
    fn save() -> Save {
        let mut ninja = vec![Value::Array(Vec::new()); 108];
        ninja[102] = json!(vec![
            json!(1),
            json!("0"),
            json!(315),
            json!(1),
            json!(0),
            json!(1),
            json!(1),
            json!(1),
            json!(68),
            json!("_e"),
            json!(2)
        ]);
        ninja[107] = flags(23, &[(17, 1.0)]);
        Save::new(json!({
            "charNames": ["Ana"],
            "companion": { "l": ["18,Glunko,_,_,1"] },
            "MaxCarryCap_0": "{\"Mining\":35000,\"Chopping\":30000,\"Fishing\":30000,\"Bugs\":30000,\"Foods\":30000,\"Critters\":30000,\"Souls\":30000,\"bCraft\":30000}",
            "SL_0": "{\"634\":100,\"78\":100}",
            "UpgVault": flags(90, &[(11, 100.0)]),
            "BundlesReceived": "{\"bon_w\":1}",
            "GemItemsPurchased": flags(90, &[(58, 4.0)]),
            "Guild": Value::Array(vec![flags(18, &[(2, 50.0)])]),
            "BribeStatus": flags(41, &[(23, 1.0)]),
            "PrayOwned": flags(25, &[(4, 20.0), (12, 20.0)]),
            "Prayers_0": flags(12, &[]),
            "StampLv": Value::Array(vec![json!({}), json!({}), json!({"1": 100})]),
            "Lv0_0": flags(25, &[(1, 10.0), (3, 10.0), (2, 10.0), (4, 10.0), (6, 10.0), (18, 100.0)]),
            "Ninja": Value::Array(ninja),
            "PVtStarSign_0": "11,25,",
            "Rift": flags(10, &[(0, 0.0)]),
        }))
    }

    fn resolve_row(save: &Save, kind: CapacityKind, map: usize) -> Resolution {
        let simulation = Simulation::default();
        let context = SimContext::default();
        let inputs = Inputs {
            map,
            ..Inputs::for_test(save, &simulation, &context)
        };
        capacity(&inputs, kind)
    }

    fn value(resolution: &Resolution) -> f64 {
        resolution.value().expect("a value")
    }

    fn warning(resolution: &Resolution) -> String {
        match resolution {
            Resolution::Value { partial, .. } => partial.clone().unwrap_or_default(),
            Resolution::Unavailable { reason } => reason.clone(),
        }
    }

    #[test]
    fn every_capacity_id_maps_to_its_row() {
        for kind in CapacityKind::ALL {
            assert_eq!(CapacityKind::from_id(&kind.id()), Some(kind));
        }
        assert_eq!(CapacityKind::from_id("carry-capacity-gold"), None);
        assert_eq!(CapacityKind::from_id("drop-rate"), None);
    }

    #[test]
    fn a_missing_row_is_unavailable_and_names_the_field() {
        let save = Save::new(json!({ "charNames": ["Ana"] }));
        let resolution = resolve_row(&save, CapacityKind::Mining, 0);
        assert!(matches!(resolution, Resolution::Unavailable { .. }));
        assert!(resolution.value().is_none());
        assert!(warning(&resolution).contains("MaxCarryCap_0"));
    }

    #[test]
    fn the_value_agrees_with_the_tree_root() {
        let save = save();
        for kind in CapacityKind::ALL {
            let resolution = resolve_row(&save, kind, 0);
            let tree = resolution.tree().expect("a tree");
            assert_eq!(tree.value, value(&resolution), "{}", kind.id());
            assert_eq!(tree.format, ValueFormat::Count);
            assert_eq!(tree.children.len(), 8);
        }
    }

    #[test]
    fn the_flat_sources_and_factors_multiply_as_the_formula_says() {
        let save = save();
        let resolution = resolve_row(&save, CapacityKind::Mining, 0);
        // Vault 11 at level 100 pays 5 per level; one bundle pays 1000.
        let vault = idleon_account::vault_bonus(&save);
        assert_eq!(vault, 500.0);
        let base = 35_000.0 + vault + 1000.0;
        // The All Carry Cap stamp is 100 levels of the flat +1% curve; signs
        // 11 and 25 are equipped and add 10 and 5.
        let all_carry = 100.0 + 15.0;
        // At map 0 no shrine row exists and no prayer is equipped.
        let guild = eval("decay", 70.0, 50.0, 50.0);
        let talent = eval("decay", 30.0, 60.0, 100.0);
        let companion = idleon_account::companion_bonus(18, true);
        let common = (1.0 + (guild + talent) / 100.0)
            * (1.0 + companion / 100.0)
            * (1.0 + (0.0 + 5.0) / 100.0);
        let expected = (base * 2.0 * (1.0 + all_carry / 100.0) * common)
            .min(HARD_CAP)
            .floor();
        assert_eq!(value(&resolution), expected);
        // Mining carries no stamp group in this save, so only the shared
        // sources move its value above the flat base.
        assert!(value(&resolution) > base);
    }

    #[test]
    fn the_pantheon_shrine_counts_only_on_its_own_map() {
        let save = Save::new(json!({
            "charNames": ["Ana"],
            "MaxCarryCap_0": "{\"Mining\":1000}",
            "Shrine": Value::Array(vec![
                flags(6, &[]),
                flags(6, &[]),
                flags(6, &[]),
                flags(6, &[(0, 0.0), (3, 5.0)]),
            ]),
        }));
        let on_map = resolve_row(&save, CapacityKind::Mining, 0);
        let off_map = resolve_row(&save, CapacityKind::Mining, 7);
        // Shrine 3 is worth 10 + 4 × 2 = 18% at level five.
        assert!(value(&off_map) < value(&on_map));
        assert!(warning(&off_map).contains("Shrine World Tour"));
        assert!(!warning(&on_map).contains("Shrine World Tour"));
    }

    #[test]
    fn prayer_curse_floors_at_forty_percent() {
        let save = Save::new(json!({
            "charNames": ["Ana"],
            "MaxCarryCap_0": "{\"Mining\":1000}",
            "PrayOwned": flags(25, &[(4, 200.0)]),
            "Prayers_0": "[4,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1]",
        }));
        let equipped = prayer(&save, 0, 4, true);
        assert!(equipped > 12.0);
        let multiplier = (1.0 - equipped / 100.0).max(0.4);
        assert_eq!(multiplier, 0.4);
    }
}
