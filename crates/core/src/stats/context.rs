//! The five calculation-context selectors and the save defaults they start from.

use super::maps::{MapOption, build_options};
use super::snapshot::Snapshot;
use serde::{Deserialize, Serialize};

/// The skill the context resolves skill-scoped descriptors against.
///
/// The labels are the upstream `skill-sel` option texts, which descriptors read
/// as the `skillType` layer; the ids are this app's project-file names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Skill {
    #[default]
    Mining,
    Choppin,
    Fishing,
    Catching,
    Trapping,
    Worship,
    Smithing,
    Cooking,
    Laboratory,
    Divinity,
    Spelunking,
    Breeding,
}

impl Skill {
    /// Every option in the upstream select's order.
    pub const ALL: [Skill; 12] = [
        Skill::Mining,
        Skill::Choppin,
        Skill::Fishing,
        Skill::Catching,
        Skill::Trapping,
        Skill::Worship,
        Skill::Smithing,
        Skill::Cooking,
        Skill::Laboratory,
        Skill::Divinity,
        Skill::Spelunking,
        Skill::Breeding,
    ];

    /// The project-file id.
    pub fn id(self) -> &'static str {
        match self {
            Skill::Mining => "mining",
            Skill::Choppin => "choppin",
            Skill::Fishing => "fishing",
            Skill::Catching => "catching",
            Skill::Trapping => "trapping",
            Skill::Worship => "worship",
            Skill::Smithing => "smithing",
            Skill::Cooking => "cooking",
            Skill::Laboratory => "laboratory",
            Skill::Divinity => "divinity",
            Skill::Spelunking => "spelunking",
            Skill::Breeding => "breeding",
        }
    }

    /// The upstream option text, which descriptors read as `skillType`.
    pub fn label(self) -> &'static str {
        match self {
            Skill::Mining => "Mining",
            Skill::Choppin => "Choppin",
            Skill::Fishing => "Fishing",
            Skill::Catching => "Catching",
            Skill::Trapping => "Trapping",
            Skill::Worship => "Worship",
            Skill::Smithing => "Smithing",
            Skill::Cooking => "Cooking",
            Skill::Laboratory => "Laboratory",
            Skill::Divinity => "Divinity",
            Skill::Spelunking => "Spelunking",
            Skill::Breeding => "Breeding",
        }
    }

    /// The skill an id names, or `None` for an id this build does not know.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|skill| skill.id() == id)
    }
}

/// Which maps the map selector lists: upstream's `#map-visibility`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MapVisibility {
    /// Only maps the character has proven access to.
    #[default]
    Accessible,
    /// Accessible maps plus the special and advanced scenes the character knows.
    Advanced,
    /// Every valid map, including ones the character has not unlocked.
    All,
}

impl MapVisibility {
    pub const ALL: [MapVisibility; 3] = [
        MapVisibility::Accessible,
        MapVisibility::Advanced,
        MapVisibility::All,
    ];

    pub fn id(self) -> &'static str {
        match self {
            MapVisibility::Accessible => "accessible",
            MapVisibility::Advanced => "advanced",
            MapVisibility::All => "all",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MapVisibility::Accessible => "Accessible",
            MapVisibility::Advanced => "Accessible + Special",
            MapVisibility::All => "All Valid Maps",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.id() == id)
    }
}

/// Whether the character is building: upstream's `#construction-mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConstructionMode {
    /// Construction bonuses apply.
    #[default]
    Active,
    /// Construction bonuses do not apply.
    Inactive,
}

impl ConstructionMode {
    pub const ALL: [ConstructionMode; 2] = [ConstructionMode::Active, ConstructionMode::Inactive];

    pub fn id(self) -> &'static str {
        match self {
            ConstructionMode::Active => "active",
            ConstructionMode::Inactive => "inactive",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ConstructionMode::Active => "Active",
            ConstructionMode::Inactive => "Inactive",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.id() == id)
    }

    /// The `isActive` flag descriptors read.
    pub fn is_active(self) -> bool {
        self == ConstructionMode::Active
    }
}

/// The context every breakdown on the page is resolved against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Context {
    /// The character, as its save row index.
    pub character: usize,
    /// The map, as its game index.
    pub map: usize,
    pub map_visibility: MapVisibility,
    pub skill: Skill,
    pub construction: ConstructionMode,
}

impl Context {
    /// The context a freshly imported save opens on: character one, its current
    /// map, and the selectors' own defaults (upstream `onLoad`).
    pub fn from_snapshot(snapshot: &Snapshot) -> Self {
        Self {
            character: 0,
            map: snapshot.current_map(0).unwrap_or(0),
            ..Self::default()
        }
    }

    /// The options the map selector lists for this context.
    pub fn map_options(&self, snapshot: &Snapshot) -> Vec<MapOption> {
        build_options(self.character, snapshot, self.map_visibility)
    }

    /// Point the context at `character`, following upstream `switchChar`: the
    /// map becomes that character's current map, then the options clamp it.
    pub fn select_character(&mut self, character: usize, snapshot: &Snapshot) {
        self.character = character;
        if let Some(map) = snapshot.current_map(character) {
            self.map = map;
        }
        self.clamp_map(snapshot);
    }

    /// Keep the selected map inside the options the save and list allow.
    ///
    /// Returns the map that was dropped, when the selection had to move: the
    /// caller names it in a warning. Upstream `populateMapSel` keeps the
    /// preferred map, else the character's current map, else the first option.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_selector_id_round_trips() {
        for skill in Skill::ALL {
            assert_eq!(Skill::from_id(skill.id()), Some(skill));
        }
        for level in MapVisibility::ALL {
            assert_eq!(MapVisibility::from_id(level.id()), Some(level));
        }
        for mode in ConstructionMode::ALL {
            assert_eq!(ConstructionMode::from_id(mode.id()), Some(mode));
        }
        assert_eq!(Skill::from_id("frogs"), None);
        assert_eq!(Skill::ALL.len(), 12);
    }

    #[test]
    fn the_skill_labels_are_the_upstream_option_texts() {
        let labels: Vec<&str> = Skill::ALL.iter().map(|skill| skill.label()).collect();
        assert_eq!(
            labels,
            [
                "Mining",
                "Choppin",
                "Fishing",
                "Catching",
                "Trapping",
                "Worship",
                "Smithing",
                "Cooking",
                "Laboratory",
                "Divinity",
                "Spelunking",
                "Breeding",
            ]
        );
    }

    #[test]
    fn a_context_round_trips_through_json() {
        let context = Context {
            character: 3,
            map: 120,
            map_visibility: MapVisibility::All,
            skill: Skill::Spelunking,
            construction: ConstructionMode::Inactive,
        };
        let text = serde_json::to_string(&context).expect("the context serializes");
        assert!(text.contains("\"mapVisibility\":\"all\""), "{text}");
        assert!(text.contains("\"construction\":"));
        let back: Context = serde_json::from_str(&text).expect("the context reads back");
        assert_eq!(back, context);
        let partial: Context =
            serde_json::from_str(r#"{ "map": 120 }"#).expect("a partial config keeps defaults");
        assert_eq!(partial.map, 120);
        assert_eq!(partial.skill, Skill::Mining);
        assert_eq!(partial.construction, ConstructionMode::Active);
    }
}
