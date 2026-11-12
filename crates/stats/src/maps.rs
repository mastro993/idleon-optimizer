//! Which maps a character may pick, and how each one is classified.
//!
//! Ported from the upstream `js/stats/systems/common/map-access.js` and the
//! `stats-all.html` map selector. The classification decides the selector
//! label, the category tab a breakdown lands in (SA-02), and the context note's
//! warning; the access walk decides which maps a character has proven.

use super::access::{access, is_visited};
use super::context::MapVisibility;
use super::data::{data, map_count};
use super::snapshot::Snapshot;

/// A map's `MapDispName` entry, or `None` where upstream leaves it blank.
///
/// The drop-rate selector lists only named maps; the All Stats list classifies
/// blank ones as internal instead.
pub fn display_name(map: usize) -> Option<&'static str> {
    data()
        .maps
        .get(map)
        .map(|row| row.display_name.as_str())
        .filter(|name| !name.is_empty())
}

/// Maps whose representative target is not the whole map's content.
const TOWN_MAPS: [usize; 7] = [0, 50, 100, 150, 200, 250, 300];
const BOSS_MAPS: [usize; 6] = [29, 66, 114, 165, 214, 266];
const FEATURE_MAPS: [usize; 6] = [41, 42, 73, 120, 166, 216];
const RUNTIME_MAPS: [usize; 2] = [37, 69];
const INTERNAL_MAPS: [usize; 13] = [3, 4, 5, 20, 21, 22, 23, 25, 34, 35, 56, 313, 314];

/// Where a map sits in the upstream categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Map,
    Internal,
    Town,
    Dungeon,
    Colosseum,
    Boss,
    Runtime,
    Feature,
    Combat,
    Skill,
}

impl Category {
    pub fn id(self) -> &'static str {
        match self {
            Category::Map => "map",
            Category::Internal => "internal",
            Category::Town => "town",
            Category::Dungeon => "dungeon",
            Category::Colosseum => "colosseum",
            Category::Boss => "boss",
            Category::Runtime => "runtime",
            Category::Feature => "feature",
            Category::Combat => "combat",
            Category::Skill => "skill",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Map => "Map",
            Category::Internal => "Internal",
            Category::Town => "Town",
            Category::Dungeon => "Dungeon",
            Category::Colosseum => "Colosseum",
            Category::Boss => "Boss",
            Category::Runtime => "Runtime",
            Category::Feature => "Feature",
            Category::Combat => "Combat",
            Category::Skill => "Skill",
        }
    }
}

/// How the map list treats a map's category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassVisibility {
    /// Listed when the character has access.
    Default,
    /// Listed behind "Accessible + Special", when the character knows the map.
    Advanced,
    /// Never listed by the selector.
    Hidden,
}

/// One map's classification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapClass {
    pub map: usize,
    /// The display name without its underscores.
    pub name: String,
    pub internal_name: String,
    pub target: String,
    pub target_name: String,
    pub side_targets: Vec<String>,
    pub afk_type: String,
    pub category: Category,
    pub visibility: ClassVisibility,
    pub world: u32,
    pub warning: String,
}

/// One row of the map selector: the map, how the character knows it, and
/// whether it is the one it stands on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapOption {
    pub class: MapClass,
    pub current: bool,
    pub visited: bool,
    pub accessible: bool,
    /// Neither accessible, visited nor current: listed but not proven.
    pub locked: bool,
    /// The upstream reason string, e.g. `Cleared portal from Spore Meadows`.
    pub evidence: String,
}

impl MapOption {
    /// The selector label, as upstream `_mapLabel` builds it.
    pub fn label(&self, map_bonus: Option<&[f64]>) -> String {
        let mut label = self.class.name.clone();
        let mob = clean_name(&self.class.target_name);
        if !mob.is_empty() && mob != "Nothing" {
            label.push_str(" - ");
            label.push_str(&mob);
        }
        if let Some(bonus) = map_bonus {
            let multi = arcane_multi(bonus.first().copied().unwrap_or(0.0));
            if multi > 0.0 {
                label.push_str(&format!(" ({:.2}x)", 1.0 + multi / 100.0));
            }
        }
        if self.current {
            label.push_str(" (current)");
        } else if self.locked {
            label.push_str(" (not proven accessible)");
        }
        if self.class.visibility == ClassVisibility::Advanced {
            label.push_str(&format!(" [{}]", self.class.category.id()));
        }
        label
    }
}

/// The account's arcane map bonus, as the map label's multiplier.
///
/// Upstream `_arcaneMulti` in `stats-all.html`: a log-scaled reward for the
/// lifetime kills on a map, with a flat first-thousand term.
pub fn arcane_multi(kills: f64) -> f64 {
    if kills < 1.0 || !kills.is_finite() {
        return 0.0;
    }
    let lg = game_log(kills);
    let lg2 = kills.max(1.0).log2();
    (2.0 * (lg - 3.5).max(0.0) + (lg2 - 12.0).max(0.0)) * (lg / 2.5)
        + (kills / 1000.0).min(2.0)
        + (5.0 * (lg - 5.0)).max(0.0)
}

/// The upstream `_getLOG`: a base-10 log pinned at one and rounded the same way.
/// The divisor is upstream's own literal, not `LN_10`, so the values match.
#[allow(clippy::approx_constant)]
fn game_log(value: f64) -> f64 {
    value.max(1.0).ln() / 2.30259
}

/// One map's classification, ported from upstream `classifyMap`.
pub fn classify(map: usize) -> MapClass {
    let Some(row) = data().maps.get(map) else {
        return unknown_map(map);
    };
    let side_targets: Vec<String> = row
        .side_targets
        .iter()
        .filter(|target| !target.is_empty() && *target != "_" && *target != "Z")
        .cloned()
        .collect();
    let mut class = MapClass {
        map,
        name: if row.display_name.is_empty() {
            format!("Map {map}")
        } else {
            clean_name(&row.display_name)
        },
        internal_name: row.internal_name.clone(),
        target: row.afk_target.clone(),
        target_name: row.target_name.clone(),
        side_targets,
        afk_type: row.afk_type.clone(),
        category: Category::Map,
        visibility: ClassVisibility::Default,
        world: world_for_map(map),
        warning: String::new(),
    };
    if INTERNAL_MAPS.contains(&map) || is_internal_name(&row.display_name, &row.internal_name) {
        class.category = Category::Internal;
        class.visibility = ClassVisibility::Hidden;
        class.warning = "Internal or non-gameplay scene.".into();
    } else if TOWN_MAPS.contains(&map) {
        class.category = Category::Town;
    } else if row.dungeon {
        class.category = Category::Dungeon;
        class.visibility = ClassVisibility::Advanced;
        class.warning = "Dungeon results require live run-state inputs.".into();
    } else if row.colosseum {
        class.category = Category::Colosseum;
        class.visibility = ClassVisibility::Advanced;
        class.warning = "Colosseum enemy state is runtime-only.".into();
    } else if BOSS_MAPS.contains(&map) {
        class.category = Category::Boss;
        class.visibility = ClassVisibility::Advanced;
        class.warning = "Boss maps do not use their representative AFK target as boss HP.".into();
    } else if RUNTIME_MAPS.contains(&map) {
        class.category = Category::Runtime;
        class.visibility = ClassVisibility::Advanced;
        class.warning = "This scene does not provide a normal AFK map context.".into();
    } else if FEATURE_MAPS.contains(&map) {
        class.category = Category::Feature;
        class.visibility = ClassVisibility::Advanced;
    } else if row.afk_type == "FIGHTING" {
        class.category = Category::Combat;
    } else if (!row.afk_target.is_empty() && row.afk_target != "Nothing" && row.afk_target != "Z")
        || !class.side_targets.is_empty()
    {
        class.category = Category::Skill;
    }
    class
}

/// A map index the data does not carry: upstream classifies it as internal.
fn unknown_map(map: usize) -> MapClass {
    MapClass {
        map,
        name: format!("Map {map}"),
        internal_name: String::new(),
        target: String::new(),
        target_name: String::new(),
        side_targets: Vec::new(),
        afk_type: String::new(),
        category: Category::Internal,
        visibility: ClassVisibility::Hidden,
        world: world_for_map(map),
        warning: "Internal or non-gameplay scene.".into(),
    }
}

/// Which maps a character may pick, in the selector's order.
///
/// Upstream `buildCharacterMapOptions`: the current map first, then world and
/// index. The list level decides which categories appear at all.
pub fn build_options(
    character: usize,
    snapshot: &Snapshot,
    visibility: MapVisibility,
) -> Vec<MapOption> {
    let access = access(character, snapshot);
    let current_map = snapshot.current_map(character);
    let mut candidates: Vec<usize> = (0..map_count()).collect();
    if let Some(current) = current_map
        && !candidates.contains(&current)
    {
        candidates.push(current);
    }
    let mut options = Vec::new();
    for map in candidates {
        let class = classify(map);
        let current = current_map == Some(map);
        let visited = is_visited(snapshot, character, map);
        let accessible = access.accessible.contains(&map);
        let world_town_map = (class.world as usize).saturating_sub(1) * 50;
        let world_town =
            TOWN_MAPS.contains(&world_town_map) && access.accessible.contains(&world_town_map);
        // A hidden map the character is standing on is still listed, as upstream
        // does: the selector must name the map the context is on.
        let include = current
            || match class.visibility {
                ClassVisibility::Hidden => false,
                ClassVisibility::Default => match visibility {
                    MapVisibility::All => true,
                    _ => accessible || visited,
                },
                ClassVisibility::Advanced => match visibility {
                    MapVisibility::Accessible => false,
                    MapVisibility::Advanced => accessible || visited || world_town,
                    MapVisibility::All => true,
                },
            };
        if !include {
            continue;
        }
        options.push(MapOption {
            locked: !accessible && !visited && !current,
            class,
            current,
            visited,
            accessible,
            evidence: access.reasons.get(&map).cloned().unwrap_or_default(),
        });
    }
    options.sort_by(|first, second| {
        second
            .current
            .cmp(&first.current)
            .then(first.class.world.cmp(&second.class.world))
            .then(first.class.map.cmp(&second.class.map))
    });
    options
}

/// Upstream `_isInternalName`: an unnamed or placeholder scene.
fn is_internal_name(display: &str, internal: &str) -> bool {
    let display = display.to_lowercase();
    let internal = internal.to_lowercase();
    display.is_empty()
        || matches!(
            display.as_str(),
            "z" | "playerselect" | "filler" | "unused" | "fillername"
        )
        || internal == "z"
        || internal == "playerselect"
        || internal == "nothinglol"
}

/// Upstream `_worldForMap`.
fn world_for_map(map: usize) -> u32 {
    map as u32 / 50 + 1
}

/// Upstream `_cleanName`: display names carry underscores.
fn clean_name(name: &str) -> String {
    name.replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_classification_matches_the_upstream_categories() {
        let expected = [
            (0, Category::Town, ClassVisibility::Default),
            (1, Category::Combat, ClassVisibility::Default),
            (3, Category::Internal, ClassVisibility::Hidden),
            (29, Category::Boss, ClassVisibility::Advanced),
            (36, Category::Colosseum, ClassVisibility::Advanced),
            (37, Category::Runtime, ClassVisibility::Advanced),
            (39, Category::Dungeon, ClassVisibility::Advanced),
            (41, Category::Feature, ClassVisibility::Advanced),
            (50, Category::Town, ClassVisibility::Default),
            (120, Category::Feature, ClassVisibility::Advanced),
            (200, Category::Town, ClassVisibility::Default),
            (299, Category::Internal, ClassVisibility::Hidden),
            (313, Category::Internal, ClassVisibility::Hidden),
        ];
        for (map, category, visibility) in expected {
            let class = classify(map);
            assert_eq!(class.category, category, "map {map}: {class:?}");
            assert_eq!(class.visibility, visibility, "map {map}: {class:?}");
        }
        assert_eq!(classify(0).name, "Blunder Hills");
        assert_eq!(classify(1).target_name, "Green_Mushroom");
        assert_eq!(classify(1).side_targets, ["OakTree"]);
        assert_eq!(classify(50).world, 2);
    }

    #[test]
    fn a_map_outside_the_data_is_internal() {
        let class = classify(5_000);
        assert_eq!(class.category, Category::Internal);
        assert_eq!(class.visibility, ClassVisibility::Hidden);
        assert_eq!(class.name, "Map 5000");
    }

    #[test]
    fn the_arcane_multiplier_matches_the_upstream_js() {
        // Values evaluated from `_arcaneMulti` in stats-all.html.
        let cases = [
            (0.0, 0.0),
            (0.5, 0.0),
            (1.0, 0.001),
            (1_000.0, 1.0),
            (1_000_000.0, 38.035_573_119_211_79),
        ];
        for (kills, expected) in cases {
            assert!(
                (arcane_multi(kills) - expected).abs() < 1e-9,
                "{kills}: {} vs {expected}",
                arcane_multi(kills)
            );
        }
        assert!(arcane_multi(1_000_000_000.0) > 100.0);
    }
}
