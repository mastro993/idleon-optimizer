//! The Drop Rate Calculator's maths: the context map list, the live dungeon
//! inputs and the total.
//!
//! Ported from `drop-rate-calc.html`, the `drop-rate` descriptor in
//! `js/stats/defs/drop-rate.js` and the dungeon system in
//! `js/stats/systems/w2/dungeon.js`. On a dungeon map the descriptor ignores
//! every saved term and reads only the two values the active run shows:
//!
//! ```text
//! total = 1 + (Dungeon Total LUK + Dungeon Drop Rate stat) / 100
//! ```
//!
//! Those two values are not stored in save JSON, so a dungeon map without them
//! reports the upstream reason instead of a number. Every other map's total is
//! the descriptor's whole pool chain (talents, equipment, stamps, cards,
//! shrines, prayers, companions, …), which is the account stats engine the All
//! Stats breakdowns build; until that engine lands this module reports that
//! total as [`Total::Unsupported`] rather than guessing it.
//!
//! The map list is `populateMapSel` from `drop-rate-calc.html`: Town first, then
//! every named map, each marked when it is a dungeon and when it is the
//! character's current map. Unlike the All Stats Dashboard it is not filtered by
//! what the character has unlocked.

use idleon_stats::{self as stats, Snapshot};
use serde::{Deserialize, Serialize};

/// The maps upstream treats as dungeons (`DUNGEON_MAPS` in `dungeon.js`).
pub const DUNGEON_MAPS: [usize; 6] = [39, 40, 70, 71, 118, 119];

/// Whether the map runs the live dungeon drop-rate formula.
pub fn is_dungeon_map(map: usize) -> bool {
    DUNGEON_MAPS.contains(&map)
}

/// The upstream reason a dungeon map cannot answer without its live values.
pub const NEEDS_RUNTIME: &str = "Dungeon Drop Rate needs live dungeon Total LUK and Dungeon Drop Rate \
                                 stat values, which are not stored in save JSON.";

/// Why a non-dungeon total is not computed yet: the descriptor's pool chain is
/// the same shared stat pipeline the All Stats breakdowns name as their gap.
pub const UNSUPPORTED: &str = "The pool chain needs the shared stat pipeline (talents, stamps, \
                               bubbles, cards and equipment), which this build does not decode; \
                               dungeon maps read the live run inputs and are exact.";

/// The two values the active dungeon run shows, as the fields hold them.
///
/// `None` is a blank field: upstream reads a blank as `NaN`, which is what makes
/// a dungeon map report [`NEEDS_RUNTIME`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Runtime {
    /// The run's Total LUK.
    pub luk: Option<f64>,
    /// The run's Drop Rate stat.
    pub drop_rate: Option<f64>,
}

impl Runtime {
    /// Read the two field drafts: a blank stays blank, and text that is not a
    /// number is refused with the field's own message.
    pub fn read(luk: &str, drop_rate: &str) -> Result<Self, String> {
        Ok(Self {
            luk: number("Dungeon Total LUK", luk)?,
            drop_rate: number("Dungeon Drop Rate Stat", drop_rate)?,
        })
    }

    /// The first entry the fields refuse, as the message the card shows.
    ///
    /// Upstream declares both inputs `min="0" step="1"`; a value outside that
    /// range cannot be a live run value.
    pub fn invalid(self) -> Option<String> {
        let checks = [
            ("Dungeon Total LUK", self.luk),
            ("Dungeon Drop Rate Stat", self.drop_rate),
        ];
        checks.into_iter().find_map(|(label, value)| {
            let value = value?;
            if !value.is_finite() {
                Some(format!("{label} must be a number."))
            } else if value < 0.0 {
                Some(format!("{label} must be 0 or more."))
            } else {
                None
            }
        })
    }
}

/// What the total card shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Total {
    /// The exact upstream total.
    Value(f64),
    /// A dungeon map whose live run values are blank.
    NeedsRuntime,
    /// A map whose total needs the account stats engine.
    Unsupported,
    /// An entry outside the field's range, with the reason to show.
    Invalid(String),
}

impl Total {
    /// The multiplier, when this build computed one.
    pub fn value(&self) -> Option<f64> {
        match self {
            Total::Value(value) => Some(*value),
            _ => None,
        }
    }

    /// The reason the card shows under `Unavailable`.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Total::Value(_) => None,
            Total::NeedsRuntime => Some(NEEDS_RUNTIME),
            Total::Unsupported => Some(UNSUPPORTED),
            Total::Invalid(reason) => Some(reason),
        }
    }
}

/// The total drop-rate multiplier for a map, exactly as upstream resolves it.
pub fn total(map: usize, runtime: Runtime) -> Total {
    if !is_dungeon_map(map) {
        return Total::Unsupported;
    }
    if let Some(reason) = runtime.invalid() {
        return Total::Invalid(reason);
    }
    let (Some(luk), Some(drop_rate)) = (runtime.luk, runtime.drop_rate) else {
        return Total::NeedsRuntime;
    };
    // `computeDungeonDropRate(totalLuk, dungeonDropRarity)`.
    Total::Value(1.0 + (luk + drop_rate) / 100.0)
}

/// One field's text as the number it holds.
fn number(label: &str, text: &str) -> Result<Option<f64>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    text.parse::<f64>()
        .map(Some)
        .map_err(|_| format!("{label} must be a number."))
}

/// The character and map the page resolves against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Selection {
    /// The character, as its save row index.
    pub character: usize,
    /// The map, as its game index.
    pub map: usize,
}

impl Selection {
    /// A freshly imported save opens on character one and its current map,
    /// exactly like upstream `onSaveLoaded`.
    pub fn from_snapshot(snapshot: &Snapshot) -> Self {
        Self {
            character: 0,
            map: snapshot.current_map(0).unwrap_or(0),
        }
    }

    /// Switch character; the map follows that character's current map, as
    /// upstream `switchChar` does.
    pub fn select_character(&mut self, character: usize, snapshot: &Snapshot) {
        if character >= snapshot.characters() {
            return;
        }
        self.character = character;
        if let Some(map) = snapshot.current_map(character) {
            self.map = map;
        }
    }

    /// Switch map; only a map the selector lists is accepted.
    pub fn select_map(&mut self, map: usize, snapshot: &Snapshot) {
        if maps(Some(snapshot), self.character)
            .iter()
            .any(|row| row.map == map)
        {
            self.map = map;
        }
    }
}

/// One row of the map selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapRow {
    /// The game's map index.
    pub map: usize,
    /// The `_mapLabel` text, including the dungeon and current markers.
    pub label: String,
    pub dungeon: bool,
    /// The selected character's current map, marked `(current)`.
    pub current: bool,
}

/// The map selector's rows for a character, in upstream `populateMapSel` order.
///
/// Town comes first even when no save is loaded, exactly like the upstream
/// select's default option.
pub fn maps(snapshot: Option<&Snapshot>, character: usize) -> Vec<MapRow> {
    let town = MapRow {
        map: 0,
        label: "Town".into(),
        dungeon: false,
        current: false,
    };
    let Some(snapshot) = snapshot else {
        // Before a save loads upstream's select holds only its Town default.
        return vec![town];
    };
    let current = snapshot.current_map(character);
    let mut rows = vec![MapRow {
        current: current == Some(0),
        ..town
    }];
    let push = |map: usize, rows: &mut Vec<MapRow>| {
        let class = stats::classify(map);
        let mut label = class.name.clone();
        let mob = class.target_name.replace('_', " ");
        if !mob.is_empty() && mob != "Nothing" {
            label.push_str(&format!(" - {mob}"));
        }
        if let Some(kills) = snapshot.map_bonus(map).and_then(|bonus| bonus.first()) {
            let multi = stats::arcane_multi(*kills);
            if multi > 0.0 {
                label.push_str(&format!(" ({:.2}x)", 1.0 + multi / 100.0));
            }
        }
        let dungeon = is_dungeon_map(map);
        let current = current == Some(map);
        if dungeon {
            label.push_str(" (dungeon)");
        }
        if current {
            label.push_str(" (current)");
        }
        rows.push(MapRow {
            map,
            label,
            dungeon,
            current,
        });
    };
    for map in 1..stats::map_count() {
        if stats::display_name(map).is_some() {
            push(map, &mut rows);
        }
    }
    // Upstream adds the current map as its own second entry when the named list
    // does not carry it.
    if let Some(map) = current
        && !rows.iter().any(|row| row.map == map)
    {
        let index = rows.len().min(1);
        rows.insert(
            index,
            MapRow {
                map,
                label: format!("Map {map} (current)"),
                dungeon: is_dungeon_map(map),
                current: true,
            },
        );
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_save::Save;
    use serde_json::json;

    fn save() -> Save {
        Save::new(json!({
            "charNames": ["Ana", "Bo"],
            "Lv0_0": "[1]",
            "Lv0_1": "[1]",
            "CharacterClass_0": 1,
            "CharacterClass_1": 7,
            "CurrentMap_0": 39,
            "CurrentMap_1": 0,
        }))
    }

    #[test]
    fn the_upstream_dungeon_maps_are_the_only_ones() {
        assert_eq!(DUNGEON_MAPS, [39, 40, 70, 71, 118, 119]);
        for map in DUNGEON_MAPS {
            assert!(is_dungeon_map(map), "{map} is a dungeon");
        }
        assert!(!is_dungeon_map(0));
        assert!(!is_dungeon_map(38));
    }

    #[test]
    fn the_dungeon_total_is_one_plus_the_percent() {
        let runtime = Runtime {
            luk: Some(1200.0),
            drop_rate: Some(35.0),
        };
        assert_eq!(total(39, runtime), Total::Value(13.35));
        assert_eq!(
            total(
                118,
                Runtime {
                    luk: Some(12_345.5),
                    drop_rate: Some(678.25)
                }
            ),
            Total::Value(131.2375)
        );
        assert_eq!(
            total(
                70,
                Runtime {
                    luk: Some(0.0),
                    drop_rate: Some(0.0)
                }
            ),
            Total::Value(1.0)
        );
    }

    #[test]
    fn a_blank_live_value_holds_the_total_back() {
        for runtime in [
            Runtime::default(),
            Runtime {
                luk: Some(500.0),
                drop_rate: None,
            },
            Runtime {
                luk: None,
                drop_rate: Some(50.0),
            },
        ] {
            for map in DUNGEON_MAPS {
                let outcome = total(map, runtime);
                assert_eq!(outcome, Total::NeedsRuntime, "{runtime:?} on {map}");
                assert_eq!(outcome.reason(), Some(NEEDS_RUNTIME));
            }
        }
    }

    #[test]
    fn a_map_outside_the_dungeons_reports_the_missing_engine() {
        let runtime = Runtime {
            luk: Some(1200.0),
            drop_rate: Some(35.0),
        };
        for map in [0, 1, 38, 41, 216] {
            let outcome = total(map, runtime);
            assert_eq!(outcome, Total::Unsupported, "map {map}");
            assert_eq!(outcome.value(), None);
            assert_eq!(outcome.reason(), Some(UNSUPPORTED));
        }
    }

    #[test]
    fn an_entry_outside_the_range_is_refused() {
        assert_eq!(
            Runtime {
                luk: Some(-1.0),
                drop_rate: Some(10.0)
            }
            .invalid()
            .as_deref(),
            Some("Dungeon Total LUK must be 0 or more.")
        );
        assert_eq!(
            Runtime {
                luk: Some(1.0),
                drop_rate: Some(f64::INFINITY)
            }
            .invalid()
            .as_deref(),
            Some("Dungeon Drop Rate Stat must be a number.")
        );
        assert_eq!(
            total(
                39,
                Runtime {
                    luk: Some(-1.0),
                    drop_rate: Some(10.0)
                }
            ),
            Total::Invalid("Dungeon Total LUK must be 0 or more.".into())
        );
        assert_eq!(
            Runtime {
                luk: Some(0.0),
                drop_rate: Some(0.0)
            }
            .invalid(),
            None
        );
    }

    #[test]
    fn the_map_list_starts_with_town_and_carries_dungeon_markers() {
        let rows = maps(None, 0);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].map, 0);
        assert_eq!(rows[0].label, "Town");
        assert!(rows.iter().all(|row| !row.dungeon));

        let save = save();
        let snapshot = Snapshot::from_save(&save);
        let rows = maps(Some(&snapshot), 0);
        assert_eq!(rows[0].label, "Town");
        let dungeon = rows
            .iter()
            .find(|row| row.map == 39)
            .expect("a dungeon row");
        assert!(dungeon.dungeon);
        assert!(dungeon.current, "character one stands on map 39");
        assert!(
            dungeon.label.ends_with("(dungeon) (current)"),
            "{}",
            dungeon.label
        );
        let town = rows.iter().find(|row| row.map == 0).expect("town");
        assert!(!town.current);
        let other = rows.iter().find(|row| row.map == 1).expect("a named map");
        assert!(!other.dungeon);
        assert!(!other.current);
        // Every named map is listed once, in index order after Town.
        let mut indices: Vec<usize> = rows.iter().map(|row| row.map).collect();
        let sorted = indices.windows(2).all(|pair| pair[0] < pair[1]);
        assert!(sorted, "{indices:?}");
        indices.sort_unstable();
        indices.dedup();
        assert_eq!(indices.len(), rows.len(), "no map is listed twice");
    }

    #[test]
    fn the_current_map_is_marked_for_its_own_character() {
        let save = save();
        let snapshot = Snapshot::from_save(&save);
        let rows = maps(Some(&snapshot), 1);
        let town = rows.iter().find(|row| row.map == 0).expect("town");
        assert!(town.current, "character two stands in town");
        let dungeon = rows.iter().find(|row| row.map == 39).expect("a dungeon");
        assert!(!dungeon.current);
        assert!(!dungeon.label.contains("(current)"));
    }

    #[test]
    fn the_field_drafts_read_as_numbers_or_blanks() {
        assert_eq!(Runtime::read("", " "), Ok(Runtime::default()));
        assert_eq!(
            Runtime::read(" 1200 ", "35.5"),
            Ok(Runtime {
                luk: Some(1200.0),
                drop_rate: Some(35.5)
            })
        );
        assert_eq!(
            Runtime::read("twelve", "35"),
            Err("Dungeon Total LUK must be a number.".into())
        );
        assert_eq!(
            Runtime::read("1", "1.2.3"),
            Err("Dungeon Drop Rate Stat must be a number.".into())
        );
        // A draft too long to be a number parses as infinity, which the range
        // check then refuses instead of reading as a blank.
        let huge = Runtime::read("1".repeat(400).as_str(), "1")
            .expect("a long digit string is still a number");
        assert!(huge.invalid().is_some(), "{huge:?}");
    }

    #[test]
    fn a_selection_opens_on_the_save_and_follows_the_character() {
        let save = save();
        let snapshot = Snapshot::from_save(&save);
        let mut selection = Selection::from_snapshot(&snapshot);
        assert_eq!(
            selection,
            Selection {
                character: 0,
                map: 39
            }
        );
        selection.select_character(1, &snapshot);
        assert_eq!(
            selection,
            Selection {
                character: 1,
                map: 0
            }
        );
        selection.select_map(39, &snapshot);
        assert_eq!(selection.map, 39);
        selection.select_character(snapshot.characters(), &snapshot);
        assert_eq!(
            selection.character, 1,
            "a character the save lacks is ignored"
        );
        selection.select_map(6_000, &snapshot);
        assert_eq!(selection.map, 39, "a map the selector lacks is ignored");
    }
}
