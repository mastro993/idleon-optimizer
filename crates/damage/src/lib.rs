//! The damage calculator's context and summary.
//!
//! Ported from the final combine in `js/stats/defs/damage.js` and the context
//! wiring in `damage-calc.html`: the summary is the base damage, percentage
//! multiplier and multiplicative-bonus layers multiplied together, the minimum
//! is that maximum times the mastery ratio, and the "character sheet" card is
//! the same pair. The three layers themselves come from the upstream shared
//! stat pipeline (`js/stats/systems/common/stats.js` and the talent, stamp,
//! bubble, card and equipment systems it reads), which this build does not
//! decode; `crates/stats/src/breakdowns` names the same gap for its
//! `damage` row. This module keeps the arithmetic, the dungeon rule and the
//! missing-input state separate so the provider can land without changing the
//! section.
//!
//! Upstream hides the summary on a dungeon map, because a dungeon's current-run
//! damage and mastery are not stored in the save JSON. The same maps are
//! [`is_dungeon`], so the page answers with the upstream reason instead of a
//! number.
//!
//! The descriptor also flags the saved metadata an import can lack: the `it.json`
//! companion roster and the server's active-vote row. The loader decides both
//! (`js/save/loader.js`), the descriptor reports the pair as `partial` with one
//! reason, and [`Metadata`] ports that rule so the page names a partial result
//! instead of a complete one.

use idleon_save::{Roster, Save};
use idleon_stats::{self as stats, Category, Snapshot};
use serde::{Deserialize, Serialize};

/// Why a summary cannot be computed without the shared stat pipeline.
pub const STATS_ENGINE: &str = "The shared stat pipeline (talents, stamps, bubbles, cards and equipment) is not ported yet, so the damage layers cannot be resolved.";

/// Why upstream hides the summary on a dungeon map.
pub const DUNGEON: &str =
    "Unavailable: current-run Dungeon DMG and Mastery are not stored in save JSON.";

/// The calculation context the damage page resolves against.
///
/// Upstream's `damage-calc.html` carries exactly two selectors, a character and
/// a map, and lists every valid map rather than the character's accessible
/// subset. The context round-trips through the project and session files.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Context {
    /// The character, as its save row index.
    pub character: usize,
    /// The map, as its game index.
    pub map: usize,
}

impl Context {
    /// The context a freshly imported save opens on: character one and its
    /// current map, exactly like upstream `onLoad`.
    pub fn from_snapshot(snapshot: &Snapshot) -> Self {
        Self {
            character: 0,
            map: snapshot.current_map(0).unwrap_or(0),
        }
    }

    /// Every map the selector lists, as upstream `populateMapSel` builds it.
    pub fn map_rows(&self, snapshot: &Snapshot) -> Vec<MapRow> {
        // Upstream `onSaveLoaded` reads `Number(currentMapData[0]) || 0`.
        let current = snapshot.current_map(self.character).unwrap_or(0);
        let mut rows: Vec<MapRow> = (0..stats::map_count())
            .filter(|map| stats::display_name(*map).is_some())
            .map(|map| MapRow {
                map,
                label: map_label(map, current),
                dungeon: is_dungeon(map),
                current: current == map,
            })
            .collect();
        // A map the character stands on that the table leaves unnamed is still
        // listed, first, as upstream `populateMapSel` inserts it.
        if !rows.iter().any(|row| row.current) {
            rows.insert(
                0,
                MapRow {
                    map: current,
                    label: format!("{} (current)", map_name(current)),
                    dungeon: is_dungeon(current),
                    current: true,
                },
            );
        }
        rows
    }

    /// Point the context at `character`; the map becomes that character's
    /// current map, then the map list clamps it, as upstream `switchChar` does.
    pub fn select_character(&mut self, character: usize, snapshot: &Snapshot) {
        self.character = character;
        if let Some(map) = snapshot.current_map(character) {
            self.map = map;
        }
        self.clamp_map(snapshot);
    }

    /// Keep the selected map inside the list the save allows.
    ///
    /// Returns the dropped map when the selection had to move, so the caller
    /// can name it in a warning, like the shared stats context does.
    pub fn clamp_map(&mut self, snapshot: &Snapshot) -> Option<usize> {
        let rows = self.map_rows(snapshot);
        if rows.iter().any(|row| row.map == self.map) {
            return None;
        }
        let dropped = self.map;
        self.map = snapshot
            .current_map(self.character)
            .or_else(|| rows.first().map(|row| row.map))?;
        Some(dropped)
    }
}

/// One row of the damage page's map selector.
///
/// Upstream `populateMapSel` lists every named `MapDispName` entry in index
/// order, marks the dungeon and the character's current map in the label, and
/// inserts the current map first when the table leaves it unnamed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapRow {
    /// The map's game index.
    pub map: usize,
    /// The option text: the display name, then `(dungeon)` and `(current)`.
    pub label: String,
    /// Whether the map is a dungeon, where the page hides its summary.
    pub dungeon: bool,
    /// Whether the character stands on this map.
    pub current: bool,
}

/// Upstream `_mapName`: the display name with its underscores replaced.
fn map_name(map: usize) -> String {
    stats::display_name(map)
        .map(|name| name.replace('_', " "))
        .unwrap_or_else(|| format!("Map {map}"))
}

/// The selector label for a map, as `populateMapSel` builds it.
fn map_label(map: usize, current: usize) -> String {
    let mut label = map_name(map);
    if is_dungeon(map) {
        label.push_str(" (dungeon)");
    }
    if current == map {
        label.push_str(" (current)");
    }
    label
}

/// The saved metadata the summary reads, ported from `js/save/loader.js`.
///
/// The loader sets `companionDataAvailable` from the `it.json` wrapper's
/// `companion.l` list and `activeVoteDataAvailable` from
/// `serverVars.voteCategories[0]`; both default to unavailable (`js/state.js`),
/// so a plain `save.json` reads as missing both.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    /// Whether the import carried the companion roster.
    pub companions: bool,
    /// Whether it carried the server's active-vote row.
    pub vote: bool,
}

impl Metadata {
    /// Both fields present, for a summary built without a save.
    pub const COMPLETE: Metadata = Metadata {
        companions: true,
        vote: true,
    };

    /// Read the pair from an import, wrapped or flat.
    pub fn read(save: &Save) -> Self {
        Self {
            companions: Roster::read(save.root().get("companion")).available,
            vote: save
                .get("serverVars")
                .and_then(|vars| vars.get("voteCategories").cloned())
                .and_then(|categories| categories.get(0).cloned())
                .and_then(|vote| js_number(&vote))
                .is_some_and(f64::is_finite),
        }
    }

    /// Whether the import carried everything the summary reads.
    pub fn complete(&self) -> bool {
        self.companions && self.vote
    }

    /// The descriptor's `missingMetadata` list, in its order.
    pub fn missing(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if !self.companions {
            missing.push("companion ownership");
        }
        if !self.vote {
            missing.push("current server vote");
        }
        missing
    }

    /// The descriptor's `reason`, empty when nothing is missing.
    pub fn reason(&self) -> String {
        let missing = self.missing();
        if missing.is_empty() {
            return String::new();
        }
        format!(
            "Partial total: the imported JSON does not include {} metadata.",
            missing.join(" or ")
        )
    }
}

/// JavaScript's `Number(value)` for the saved vote row: numbers and numeric
/// strings read as numbers, and anything else is not a value.
fn js_number(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        serde_json::Value::Bool(flag) => Some(if *flag { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// The three damage layers and the mastery ratio they combine with.
///
/// Upstream's `damage` descriptor returns `Max Damage`, `Min Damage` and
/// `Mastery` children; the layers are its `Base Damage (with softcap)`,
/// `Percentage Damage Multiplier` and `Multiplicative Damage Bonuses` children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Layers {
    /// DDL0: base damage after its softcaps.
    pub base: f64,
    /// DDL1: percentage damage multiplier after its softcaps.
    pub percent: f64,
    /// DDL2: multiplicative bonuses after their softcap chain.
    pub multiplier: f64,
    /// The minimum/maximum ratio, capped at 0.8 upstream.
    pub mastery: f64,
}

/// A computed summary, or the reason the page cannot compute one.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Whether the three layers were resolved.
    pub available: bool,
    /// The saved metadata the descriptor flags; a missing field is partial.
    pub metadata: Metadata,
    /// The upstream reason a summary is unavailable, or the partial note.
    pub reason: String,
    /// The minimum damage; zero while unavailable.
    pub min: f64,
    /// The maximum damage; zero while unavailable.
    pub max: f64,
    /// The mastery ratio, capped exactly as upstream caps it.
    pub mastery: f64,
}

impl Summary {
    /// The summary the three layers produce.
    ///
    /// Upstream computes `MaxDmg = DDL[0] * DDL[1] * DDL[2]` and
    /// `MinDmg = MaxDmg * mastery`, with mastery already capped at 0.8 by the
    /// descriptor. The cap is applied again here so a hand-entered layer cannot
    /// produce a minimum above the maximum.
    pub fn from_layers(layers: Layers) -> Self {
        let mastery = layers.mastery.clamp(0.0, 0.8);
        let max = layers.base * layers.percent * layers.multiplier;
        let max = if max.is_finite() { max } else { 0.0 };
        Self {
            available: true,
            metadata: Metadata::COMPLETE,
            reason: String::new(),
            min: max * mastery,
            max,
            mastery,
        }
    }

    /// Attach the saved metadata the descriptor reads, so a summary built from
    /// complete metadata is not marked partial.
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Whether the import lacked metadata, as the descriptor reports it.
    pub fn partial(&self) -> bool {
        !self.metadata.complete()
    }

    /// The descriptor's partial reason, or an empty string when complete.
    pub fn partial_reason(&self) -> String {
        self.metadata.reason()
    }

    /// The "Character Sheet Range" card: upstream formats the same pair as a
    /// range and notes that a partial result is not what the sheet shows.
    pub fn game_range(&self) -> Option<(f64, f64)> {
        self.available.then_some((self.min, self.max))
    }

    /// The mastery bar's fraction: `min(100, mastery / 0.8 * 100)` upstream.
    pub fn mastery_fill(&self) -> f64 {
        (self.mastery / 0.8 * 100.0).clamp(0.0, 100.0)
    }

    /// A summary that could not be resolved, with the upstream reason.
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            metadata: Metadata::COMPLETE,
            reason: reason.into(),
            ..Self::default()
        }
    }
}

/// Whether a map is a dungeon, where upstream hides the whole summary.
pub fn is_dungeon(map: usize) -> bool {
    stats::classify(map).category == Category::Dungeon
}

/// Resolve the summary for a save and context.
///
/// The shared stat pipeline is the missing provider. Until it lands this
/// reports the upstream dungeon reason on a dungeon map and the missing-pipeline
/// reason everywhere else, rather than showing a number it cannot justify. The
/// metadata flags are read either way, so the page can name a partial import.
pub fn evaluate(save: &Save, context: Context) -> Summary {
    let metadata = Metadata::read(save);
    let reason = if is_dungeon(context.map) {
        DUNGEON
    } else {
        STATS_ENGINE
    };
    Summary::unavailable(reason).with_metadata(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Character one's upstream layers from `fixtures/damage.json`, written by
    /// `script/damage-oracle.mjs` from `damageDebug()` in a checkout of
    /// `Corgan/idleon-research-optimizer`. The arithmetic is the only part this
    /// module owns until the shared stat pipeline lands.
    const FIXTURE: Layers = Layers {
        base: 24_314_521.292_961_78,
        percent: 33_196_799.975_627_754,
        multiplier: 19_132_764_507_064_010.0,
        mastery: 0.8,
    };

    #[test]
    fn the_summary_is_the_three_layers_and_the_mastery_ratio() {
        let summary = Summary::from_layers(FIXTURE);
        assert!(summary.available);
        assert_eq!(summary.max, 1.544_328_446_783_761e31);
        assert_eq!(summary.min, 1.235_462_757_427_009e31);
        assert_eq!(summary.mastery, 0.8);
        assert_eq!(summary.game_range(), Some((summary.min, summary.max)));
        assert_eq!(summary.mastery_fill(), 100.0);
    }

    #[test]
    fn mastery_is_capped_at_the_upstream_eight_tenths() {
        let summary = Summary::from_layers(Layers {
            base: 100.0,
            percent: 2.0,
            multiplier: 3.0,
            mastery: 1.5,
        });
        assert_eq!(summary.max, 600.0);
        assert_eq!(summary.min, 480.0);
        assert_eq!(summary.mastery, 0.8);
        assert_eq!(summary.mastery_fill(), 100.0);
    }

    #[test]
    fn the_dungeon_maps_are_the_upstream_set() {
        for map in [39, 40, 70, 71, 118, 119] {
            assert!(is_dungeon(map), "map {map}");
        }
        for map in [0, 1, 50, 200, 306] {
            assert!(!is_dungeon(map), "map {map}");
        }
    }

    /// The flat save the loader sees when the import carries no wrapper.
    fn flat_save() -> Save {
        Save::new(serde_json::json!({
            "charNames": ["Ana"],
            "Lv0_0": "[1]",
            "CharacterClass_0": 1,
            "CurrentMap_0": 0,
        }))
    }

    #[test]
    fn missing_metadata_reads_as_partial_with_the_descriptor_reason() {
        let summary = evaluate(&flat_save(), Context::default());
        assert!(!summary.metadata.companions);
        assert!(!summary.metadata.vote);
        assert!(summary.partial());
        assert_eq!(
            summary.partial_reason(),
            "Partial total: the imported JSON does not include companion ownership or current server vote metadata."
        );
    }

    #[test]
    fn a_wrapped_save_with_the_roster_and_the_vote_is_complete() {
        let wrapped = Save::new(serde_json::json!({
            "data": { "charNames": ["Ana"], "Lv0_0": "[1]", "CurrentMap_0": 0 },
            "companion": { "l": [] },
            "serverVars": { "voteCategories": [2] },
        }));
        let summary = evaluate(&wrapped, Context::default());
        assert!(summary.metadata.complete());
        assert!(!summary.partial());
        assert_eq!(summary.partial_reason(), "");
    }

    #[test]
    fn each_missing_field_is_named_on_its_own() {
        let no_vote = Save::new(serde_json::json!({
            "data": { "charNames": ["Ana"], "Lv0_0": "[1]" },
            "companion": { "l": ["12,Prometheus,0,0,1"] },
            "serverVars": { "voteCategories": [] },
        }));
        let summary = evaluate(&no_vote, Context::default());
        assert!(summary.metadata.companions);
        assert!(!summary.metadata.vote);
        assert!(summary.partial_reason().contains("current server vote"));
        assert!(!summary.partial_reason().contains("companion ownership"));
    }

    #[test]
    fn the_map_selector_lists_every_named_map_in_index_order() {
        let save = flat_save();
        let snapshot = Snapshot::from_save(&save);
        let rows = Context::default().map_rows(&snapshot);
        assert!(rows.len() > 300, "{} rows", rows.len());
        assert!(rows.windows(2).all(|pair| pair[0].map < pair[1].map));
        let blunder = &rows[0];
        assert_eq!(blunder.map, 0);
        assert_eq!(blunder.label, "Blunder Hills (current)");
        assert!(blunder.current);
        assert!(!blunder.dungeon);
        let dungeon = rows
            .iter()
            .find(|row| row.dungeon)
            .expect("the table carries dungeons");
        assert!(dungeon.label.ends_with(" (dungeon)"), "{}", dungeon.label);
    }

    #[test]
    fn an_unnamed_current_map_is_inserted_first() {
        let save = Save::new(serde_json::json!({
            "charNames": ["Ana"],
            "Lv0_0": "[1]",
            "CurrentMap_0": 5_000,
        }));
        let snapshot = Snapshot::from_save(&save);
        let mut context = Context {
            character: 0,
            map: 5_000,
        };
        let rows = context.map_rows(&snapshot);
        assert_eq!(rows[0].map, 5_000);
        assert_eq!(rows[0].label, "Map 5000 (current)");
        assert!(rows[0].current);
        assert_eq!(context.clamp_map(&snapshot), None);
    }

    #[test]
    fn a_context_round_trips_through_json() {
        let context = Context {
            character: 3,
            map: 120,
        };
        let text = serde_json::to_string(&context).expect("the context serializes");
        assert!(text.contains("\"character\":3"), "{text}");
        assert!(text.contains("\"map\":120"), "{text}");
        let back: Context = serde_json::from_str(&text).expect("the context reads back");
        assert_eq!(back, context);
        let partial: Context =
            serde_json::from_str(r#"{ "map": 120 }"#).expect("a partial config keeps defaults");
        assert_eq!(partial.map, 120);
        assert_eq!(partial.character, 0);
    }
}
