//! The section registry: every page the suite can open, ported from the upstream
//! `index.html` cards and `js/ui/site-nav.js` navigation.
//!
//! One entry per section carries its id, title, group, icon, one-line description,
//! spec link, status and the factory that renders it. The dashboard, the router,
//! the keyboard shortcuts and the MCP harness all read this table, so adding a
//! section means one entry here plus one module under `sections/`.
//!
//! The five pages upstream hides from its index (`stats-all`, `damage`, `drop-rate`,
//! `coin-multi`, `pickle`) are registered without a dashboard card: the router and
//! the harness can still open them by id.

pub mod clamworks;
pub mod combat_sim;
pub mod coral_god_rank;
pub mod endgame;
pub mod farming;
pub mod fishing;
pub mod jar;
pub mod killroy;
pub mod ladle;
pub mod monument;
pub mod pickle;
pub mod research;
pub mod sailing;
pub mod shadow;
pub mod stats_all;
pub mod statue;
pub mod worship_td;

use crate::state::AppState;
use crate::ui::section;
use gpui::{AnyElement, App, Context, Window};

/// A section's own renderer: the module that owns its upstream tab strip and content.
pub type View = fn(&mut AppState, &mut Window, &mut Context<AppState>) -> AnyElement;

/// A section's keyboard shortcuts, registered once at startup.
pub type Keys = fn(&mut App);

/// How much of a section exists in this build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Built: the entry's view is the section module's own renderer.
    Ready,
    /// Specced and tracked by a milestone, not built: the shared placeholder renders.
    Planned,
}

impl Status {
    /// The word the status bar shows.
    pub fn label(self) -> &'static str {
        match self {
            Status::Ready => "Ready",
            Status::Planned => "Planned",
        }
    }
}

/// A world badge shared by the cards of one group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    General,
    W2,
    W3,
    W4,
    W5,
    W6,
    W7,
    /// The hidden upstream tool pages, which have no dashboard card.
    Tools,
}

impl Group {
    /// Every group in dashboard order.
    pub const ALL: [Group; 8] = [
        Group::General,
        Group::W2,
        Group::W3,
        Group::W4,
        Group::W5,
        Group::W6,
        Group::W7,
        Group::Tools,
    ];

    /// Stable id used for group headings and grid element ids.
    pub fn id(self) -> &'static str {
        match self {
            Group::General => "general",
            Group::W2 => "w2",
            Group::W3 => "w3",
            Group::W4 => "w4",
            Group::W5 => "w5",
            Group::W6 => "w6",
            Group::W7 => "w7",
            Group::Tools => "tools",
        }
    }

    /// The heading above the group's cards.
    pub fn label(self) -> &'static str {
        match self {
            Group::General => "General",
            Group::W2 => "World 2",
            Group::W3 => "World 3",
            Group::W4 => "World 4",
            Group::W5 => "World 5",
            Group::W6 => "World 6",
            Group::W7 => "World 7",
            Group::Tools => "Tools",
        }
    }

    /// The badge on each card, matching the upstream classes.
    pub fn badge(self) -> &'static str {
        match self {
            Group::General => "General",
            Group::W2 => "W2",
            Group::W3 => "W3",
            Group::W4 => "W4",
            Group::W5 => "W5",
            Group::W6 => "W6",
            Group::W7 => "W7",
            Group::Tools => "Hidden",
        }
    }
}

/// One section in the registry: a dashboard card, or one of the hidden tools.
#[derive(Clone, Copy, Debug)]
pub struct Section {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    pub group: Group,
    pub description: &'static str,
    /// The generated specification, relative to the repository root.
    pub spec: &'static str,
    pub status: Status,
    /// Whether the dashboard shows a card; the hidden tools open by id only.
    pub dashboard: bool,
    /// The section module's renderer, or the shared planned placeholder.
    pub view: View,
    /// The section's keyboard shortcuts, or a no-op for a planned section.
    pub keys: Keys,
}

impl PartialEq for Section {
    /// Sections compare by id, their stable identity across the registry.
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Section {
    /// The badge the card and the section header show.
    pub fn badge(self) -> &'static str {
        self.group.badge()
    }

    /// Whether the card matches a search query by title, description or world.
    pub fn matches(self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || [
                self.title,
                self.description,
                self.group.label(),
                self.group.badge(),
            ]
            .iter()
            .any(|field| field.to_lowercase().contains(&query))
    }
}

/// Every section, grouped like `js/ui/site-nav.js` and ordered by group.
pub const SECTIONS: [Section; 30] = [
    Section {
        id: "combat-sim",
        title: "Combat Simulator",
        icon: "⚔️",
        group: Group::General,
        description: "Active combat, AFK gains, loadout comparisons, and replays",
        spec: "docs/sections/combat-sim.md",
        status: Status::Ready,
        dashboard: true,
        view: combat_sim::view,
        keys: no_keys,
    },
    Section {
        id: "statue",
        title: "Statue Calculator",
        icon: "🗿",
        group: Group::General,
        description: "Statues needed per level, STARTUE EXP bubble, time estimates",
        spec: "docs/sections/statue.md",
        status: Status::Ready,
        dashboard: true,
        view: statue::view,
        keys: no_keys,
    },
    Section {
        id: "shadow",
        title: "Am I shadow banned?",
        icon: "?",
        group: Group::General,
        description: "Sign in or load a save for a simple yes or no",
        spec: "docs/sections/shadow.md",
        status: Status::Ready,
        dashboard: true,
        view: shadow::view,
        keys: no_keys,
    },
    Section {
        id: "endgame",
        title: "Are you endgame?",
        icon: "?",
        group: Group::General,
        description: "Find out if you have reached endgame",
        spec: "docs/sections/endgame.md",
        status: Status::Ready,
        dashboard: true,
        view: endgame::view,
        keys: no_keys,
    },
    Section {
        id: "stats-all",
        title: "All Stats Dashboard",
        icon: "📊",
        group: Group::General,
        description: "Map-aware character and account stats with breakdowns and a formula explorer",
        spec: "docs/sections/stats-all.md",
        status: Status::Ready,
        dashboard: false,
        view: stats_all::view,
        keys: no_keys,
    },
    Section {
        id: "fishing",
        title: "Fishing Calculator",
        icon: "🎣",
        group: Group::W2,
        description: "AFK catches, fish split, real vs panel EXP, best lure + line per pool",
        spec: "docs/sections/fishing.md",
        status: Status::Ready,
        dashboard: true,
        view: fishing::view,
        keys: no_keys,
    },
    Section {
        id: "prisma",
        title: "Prisma Calculator",
        icon: "🔮",
        group: Group::W2,
        description: "Compare each bubble's Prisma impact on damage, stats, skills and account totals",
        spec: "docs/sections/prisma.md",
        status: Status::Planned,
        dashboard: true,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "killroy",
        title: "Killroy Calculator",
        icon: "💀",
        group: Group::W2,
        description: "Weekly encounters, monster and class records, kills, skulls and +1 upgrade comparisons",
        spec: "docs/sections/killroy.md",
        status: Status::Ready,
        dashboard: true,
        view: killroy::view,
        keys: no_keys,
    },
    Section {
        id: "cog",
        title: "Cog Board Optimizer",
        icon: "⚙️",
        group: Group::W3,
        description: "Optimize cog placement for construction bonuses",
        spec: "docs/sections/cog.md",
        status: Status::Planned,
        dashboard: true,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "worship-td",
        title: "Worship Tower Defense Simulator",
        icon: "🏰",
        group: Group::W3,
        description: "Tower layouts, wave simulation, strategy search, and replays",
        spec: "docs/sections/worship-td.md",
        status: Status::Ready,
        dashboard: true,
        view: worship_td::view,
        keys: no_keys,
    },
    Section {
        id: "ladle",
        title: "Ladle Optimizer",
        icon: "🥄",
        group: Group::W4,
        description: "Ladles/day per character, Cooking EXP balloons, meal costs, ladle + NMLB leveling plan",
        spec: "docs/sections/ladle.md",
        status: Status::Ready,
        dashboard: true,
        view: ladle::view,
        keys: no_keys,
    },
    Section {
        id: "cooking-mastery",
        title: "Cooking Mastery",
        icon: "🍳",
        group: Group::W4,
        description: "Mastery EXP/hr optimizer — which upgrade to level next",
        spec: "docs/sections/cooking-mastery.md",
        status: Status::Planned,
        dashboard: true,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "fountain",
        title: "Fountain Calculator",
        icon: "⛲",
        group: Group::W5,
        description: "Currency values, earn rates, upgrades, marble bonuses",
        spec: "docs/sections/fountain.md",
        status: Status::Planned,
        dashboard: true,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "jar",
        title: "Jar Calculator",
        icon: "🏺",
        group: Group::W5,
        description: "Production rates, rupie value, chances, collectibles",
        spec: "docs/sections/jar.md",
        status: Status::Ready,
        dashboard: true,
        view: jar::view,
        keys: no_keys,
    },
    Section {
        id: "sailing",
        title: "Sailing Calculator",
        icon: "⛵",
        group: Group::W5,
        description: "Fleet routes, chest tiers, loot value, artifact odds",
        spec: "docs/sections/sailing.md",
        status: Status::Ready,
        dashboard: true,
        view: sailing::view,
        keys: no_keys,
    },
    Section {
        id: "cglunko",
        title: "Cglunko Calculator",
        icon: "💎",
        group: Group::W5,
        description: "Cglunko upgrades, optimizer, stone focus, drop rate breakdown",
        spec: "docs/sections/cglunko.md",
        status: Status::Planned,
        dashboard: true,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "coral-god-rank",
        title: "Coral God Rank Planner",
        icon: "🐠",
        group: Group::W5,
        description: "Coral Kid upgrades, god-rank offering planning, and objective optimization",
        spec: "docs/sections/coral-god-rank.md",
        status: Status::Ready,
        dashboard: true,
        view: coral_god_rank::view,
        keys: no_keys,
    },
    Section {
        id: "monument",
        title: "Monument Calculator",
        icon: "🏛️",
        group: Group::W5,
        description: "Bravery, Justice, Wisdom — reward multipliers, permanent bonuses, time scaling",
        spec: "docs/sections/monument.md",
        status: Status::Ready,
        dashboard: true,
        view: monument::view,
        keys: no_keys,
    },
    Section {
        id: "opal",
        title: "Opal Optimizer",
        icon: "💎",
        group: Group::W5,
        description: "Villager EXP rates, opal distribution optimizer, all bonus terms",
        spec: "docs/sections/opal.md",
        status: Status::Planned,
        dashboard: true,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "sneaking",
        title: "Sneaking Optimizer",
        icon: "🥷",
        group: Group::W6,
        description: "Stealth, detection, charm equips, ninja kit bonuses",
        spec: "docs/sections/sneaking.md",
        status: Status::Planned,
        dashboard: true,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "farming",
        title: "Farming Planner",
        icon: "🌱",
        group: Group::W6,
        description: "Full plot grid, Land Rank allocation, evolution, overgrowth, beans, markets, and stickers",
        spec: "docs/sections/farming.md",
        status: Status::Ready,
        dashboard: true,
        view: farming::view,
        keys: no_keys,
    },
    Section {
        id: "clamworks",
        title: "Clamworks Calculator",
        icon: "🐚",
        group: Group::W7,
        description: "Pearl upgrades, optimizer, promotion tracker",
        spec: "docs/sections/clamworks.md",
        status: Status::Ready,
        dashboard: true,
        view: clamworks::view,
        keys: no_keys,
    },
    Section {
        id: "royal-guardian",
        title: "Royal Guardian Calculator",
        icon: "👑",
        group: Group::W7,
        description: "Outposts, resources and currencies, Armory, statues, and map quick views",
        spec: "docs/sections/royal-guardian.md",
        status: Status::Planned,
        dashboard: true,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "spelunking",
        title: "Spelunking Simulator & Optimizer",
        icon: "⛏️",
        group: Group::W7,
        description: "Experimental Normal and Manic expeditions, Auto speeds, manual strategies, parties and elixirs",
        spec: "docs/sections/spelunking.md",
        status: Status::Planned,
        dashboard: true,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "jelly-operator",
        title: "Jelly Operator Optimizer",
        icon: "🧫",
        group: Group::W7,
        description: "Cell layouts, obstruction clears, Bloodcells, upgrades, and account bonuses",
        spec: "docs/sections/jelly-operator.md",
        status: Status::Planned,
        dashboard: true,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "research",
        title: "Research Optimizer",
        icon: "🔬",
        group: Group::W7,
        description: "Simulate research grid strategies and EXP allocation",
        spec: "docs/sections/research.md",
        status: Status::Ready,
        dashboard: true,
        view: research::view,
        keys: research::bind_keys,
    },
    Section {
        id: "damage",
        title: "Damage Breakdown",
        icon: "💥",
        group: Group::Tools,
        description: "Min and max damage, mastery and the damage layer formula tree",
        spec: "docs/sections/damage.md",
        status: Status::Planned,
        dashboard: false,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "drop-rate",
        title: "Drop Rate Calculator",
        icon: "🎲",
        group: Group::Tools,
        description: "Dungeon runtime, luck and drop-rate inputs with the formula breakdown",
        spec: "docs/sections/drop-rate.md",
        status: Status::Planned,
        dashboard: false,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "coin-multi",
        title: "Coin Multiplier Calculator",
        icon: "🪙",
        group: Group::Tools,
        description: "Map, target, HP, damage and overkill inputs with the coin formula breakdown",
        spec: "docs/sections/coin-multi.md",
        status: Status::Planned,
        dashboard: false,
        view: PLACEHOLDER,
        keys: no_keys,
    },
    Section {
        id: "pickle",
        title: "BoneJoePickle Calculator",
        icon: "🥒",
        group: Group::Tools,
        description: "Hidden DPS and time-to-kill calculator",
        spec: "docs/sections/pickle.md",
        status: Status::Ready,
        dashboard: false,
        view: pickle::view,
        keys: no_keys,
    },
];

/// The sections registered under `id`, if any.
pub fn find(id: &str) -> Option<&'static Section> {
    SECTIONS.iter().find(|section| section.id == id)
}

/// The sections the dashboard shows, in card order.
pub fn cards() -> impl Iterator<Item = &'static Section> {
    SECTIONS.iter().filter(|section| section.dashboard)
}

/// The dashboard's card count.
pub fn card_count() -> usize {
    cards().count()
}

/// The registry as JSON, so the harness can enumerate sections without
/// hardcoding ids that the table already owns.
pub fn manifest() -> String {
    let sections: Vec<serde_json::Value> = SECTIONS
        .iter()
        .map(|section| {
            serde_json::json!({
                "id": section.id,
                "title": section.title,
                "group": section.group.id(),
                "badge": section.badge(),
                "status": section.status.label(),
                "dashboard": section.dashboard,
                "spec": section.spec,
            })
        })
        .collect();
    serde_json::to_string(&sections).expect("the section manifest is plain JSON")
}

/// The shared renderer for every planned section.
const PLACEHOLDER: View = section::planned;

/// A planned section registers no shortcuts of its own.
fn no_keys(_: &mut App) {}

#[cfg(test)]
mod tests;
