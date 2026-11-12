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

use idleon_save::Save;
use idleon_stats::{self as stats, Category, MapVisibility, Snapshot};
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

    /// Every map the selector lists, in the shared map order.
    pub fn map_options(&self, snapshot: &Snapshot) -> Vec<stats::MapOption> {
        stats::build_options(self.character, snapshot, MapVisibility::All)
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
        let options = self.map_options(snapshot);
        let first = options.first()?;
        if options.iter().any(|option| option.class.map == self.map) {
            return None;
        }
        let dropped = self.map;
        self.map = snapshot
            .current_map(self.character)
            .filter(|current| options.iter().any(|option| option.class.map == *current))
            .unwrap_or(first.class.map);
        Some(dropped)
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
pub struct Summary {
    /// Whether the three layers were resolved.
    pub available: bool,
    /// Whether the imported JSON lacked metadata the upstream descriptor flags.
    pub partial: bool,
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
            partial: false,
            reason: String::new(),
            min: max * mastery,
            max,
            mastery,
        }
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
            partial: false,
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
/// reason everywhere else, rather than showing a number it cannot justify.
pub fn evaluate(_save: &Save, context: Context) -> Summary {
    if is_dungeon(context.map) {
        return Summary::unavailable(DUNGEON);
    }
    Summary::unavailable(STATS_ENGINE)
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
