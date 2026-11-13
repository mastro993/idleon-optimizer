//! The collectibles tab: the forty collectibles, their bonuses and the order an
//! enchant should target them in.
//!
//! Ported from `COLLECTIBLE_DATA`, `collectibleBonus`, `enchantPriority` and
//! `_collectibleEffect` in `jar-calc.html`. The data rows are extracted from the
//! upstream JS into `crates/jar/data/jar.json`.

use super::{COLLECTIBLES, Jar, data};

/// One collectible at its saved level.
#[derive(Clone, Debug, PartialEq)]
pub struct Collectible {
    /// The save index, `0..40`.
    pub index: usize,
    pub name: &'static str,
    /// The upstream category key, e.g. `rupie-mul`.
    pub category: &'static str,
    pub category_label: &'static str,
    pub category_color: &'static str,
    /// `Holes[24][index]`.
    pub level: u32,
    /// `max(0, level - 1)`: the levels an enchant added.
    pub enchant_levels: u32,
    /// `COLLECTIBLE_DATA[index][1]`.
    pub per_level: f64,
    /// `collectibleBonus`: the total effect at the saved level and legend points.
    pub bonus: f64,
    /// The effect text at the saved bonus, e.g. `+180.0% rupie value`.
    pub effect: String,
    /// The effect text one more level grants.
    pub per_level_effect: String,
}

/// The collectibles of one category, in the upstream display order.
#[derive(Clone, Debug, PartialEq)]
pub struct CollectibleGroup {
    pub category: &'static str,
    pub label: &'static str,
    pub color: &'static str,
    pub items: Vec<Collectible>,
}

/// One row of `enchantPriority`: a found collectible and what one more enchant
/// level adds.
#[derive(Clone, Debug, PartialEq)]
pub struct EnchantPriority {
    pub index: usize,
    pub name: &'static str,
    pub category: &'static str,
    pub category_label: &'static str,
    pub category_color: &'static str,
    pub level: u32,
    pub enchant_levels: u32,
    pub bonus: f64,
    /// What one more enchant level adds to the total bonus.
    pub marginal: f64,
}

impl Jar {
    /// `collectibleLevels` for the slots this save can find.
    pub fn collectibles(&self) -> Vec<Collectible> {
        (0..self.collectible_slots())
            .filter_map(|index| self.collectible(index))
            .collect()
    }

    /// The collectibles grouped by category, in the order their first row
    /// appears (the upstream `_renderCollectibles` scan).
    pub fn collectible_groups(&self) -> Vec<CollectibleGroup> {
        let mut groups: Vec<CollectibleGroup> = Vec::new();
        for item in self.collectibles() {
            match groups
                .iter_mut()
                .find(|group| group.category == item.category)
            {
                Some(group) => group.items.push(item),
                None => groups.push(CollectibleGroup {
                    category: item.category,
                    label: item.category_label,
                    color: item.category_color,
                    items: vec![item],
                }),
            }
        }
        groups
    }

    /// `collectibleLevels`: every collectible's saved level, in save order.
    pub fn collectible_levels(&self) -> [u32; COLLECTIBLES] {
        std::array::from_fn(|index| self.collectible_level(index))
    }

    /// One collectible at its saved level.
    pub fn collectible(&self, index: usize) -> Option<Collectible> {
        let row = data().collectible(index)?;
        let level = self.collectible_level(index);
        let bonus = self.collectible_bonus(index, self.ext.legend_pts);
        let category = row.category.as_str();
        Some(Collectible {
            index,
            name: row.name.as_str(),
            category,
            category_label: data().category_label(category).unwrap_or(category),
            category_color: data().category_color(category).unwrap_or("#888"),
            level,
            enchant_levels: level.saturating_sub(1),
            per_level: row.per_level,
            bonus,
            effect: collectible_effect(index, bonus),
            per_level_effect: collectible_effect(index, row.per_level),
        })
    }

    /// `enchantPriority`: found collectibles sorted by what the next enchant
    /// level adds, then by the fewest enchant levels.
    pub fn enchant_priority(&self) -> Vec<EnchantPriority> {
        let mut rows: Vec<EnchantPriority> = (0..COLLECTIBLES)
            .filter_map(|index| {
                let item = self.collectible(index)?;
                if item.level < 1 {
                    return None;
                }
                Some(EnchantPriority {
                    index,
                    name: item.name,
                    category: item.category,
                    category_label: item.category_label,
                    category_color: item.category_color,
                    level: item.level,
                    enchant_levels: item.enchant_levels,
                    bonus: item.bonus,
                    marginal: item.per_level * (1.0 + self.ext.legend_pts / 100.0),
                })
            })
            .collect();
        rows.sort_by(|left, right| {
            right
                .marginal
                .total_cmp(&left.marginal)
                .then(left.enchant_levels.cmp(&right.enchant_levels))
        });
        rows
    }
}

/// The name of a collectible by save index.
pub fn collectible_name(index: usize) -> &'static str {
    data().collectible_name(index)
}

/// `_collectibleEffect`: the upstream effect text at `bonus`, filling its
/// `{%` or `}x` placeholder.
pub fn collectible_effect(index: usize, bonus: f64) -> String {
    let Some(row) = data().collectible(index) else {
        return String::new();
    };
    let effect = row.effect.as_str();
    if effect.contains("Future cavern") {
        return effect.to_owned();
    }
    if effect.contains("}x") {
        return effect.replacen("}x", &format!("{:.2}x", 1.0 + bonus / 100.0), 1);
    }
    effect.replacen("{%", &format!("{bonus:.1}%"), 1)
}
