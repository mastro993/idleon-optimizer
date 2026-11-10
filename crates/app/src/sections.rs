//! The dashboard's section catalog, ported from the upstream `index.html`.
//!
//! The cards keep the upstream titles, icons, descriptions and world badges.
//! Their order follows the upstream site navigation, which is the only place
//! the pages are grouped by world.

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
}

impl Group {
    /// Every group in dashboard order.
    pub const ALL: [Group; 7] = [
        Group::General,
        Group::W2,
        Group::W3,
        Group::W4,
        Group::W5,
        Group::W6,
        Group::W7,
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
        }
    }
}

/// One card on the landing dashboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Section {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    pub group: Group,
    pub description: &'static str,
}

impl Section {
    /// The badge the card shows.
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

/// The 25 cards in upstream `index.html`, grouped like `js/ui/site-nav.js`.
pub const SECTIONS: [Section; 25] = [
    Section {
        id: "combat-sim",
        title: "Combat Simulator",
        icon: "⚔️",
        group: Group::General,
        description: "Active combat, AFK gains, loadout comparisons, and replays",
    },
    Section {
        id: "statue",
        title: "Statue Calculator",
        icon: "🗿",
        group: Group::General,
        description: "Statues needed per level, STARTUE EXP bubble, time estimates",
    },
    Section {
        id: "shadow",
        title: "Am I shadow banned?",
        icon: "?",
        group: Group::General,
        description: "Sign in or load a save for a simple yes or no",
    },
    Section {
        id: "endgame",
        title: "Are you endgame?",
        icon: "?",
        group: Group::General,
        description: "Find out if you have reached endgame",
    },
    Section {
        id: "fishing",
        title: "Fishing Calculator",
        icon: "🎣",
        group: Group::W2,
        description: "AFK catches, fish split, real vs panel EXP, best lure + line per pool",
    },
    Section {
        id: "prisma",
        title: "Prisma Calculator",
        icon: "🔮",
        group: Group::W2,
        description: "Compare each bubble's Prisma impact on damage, stats, skills and account totals",
    },
    Section {
        id: "killroy",
        title: "Killroy Calculator",
        icon: "💀",
        group: Group::W2,
        description: "Weekly encounters, monster and class records, kills, skulls and +1 upgrade comparisons",
    },
    Section {
        id: "cog",
        title: "Cog Board Optimizer",
        icon: "⚙️",
        group: Group::W3,
        description: "Optimize cog placement for construction bonuses",
    },
    Section {
        id: "worship-td",
        title: "Worship Tower Defense Simulator",
        icon: "🏰",
        group: Group::W3,
        description: "Tower layouts, wave simulation, strategy search, and replays",
    },
    Section {
        id: "ladle",
        title: "Ladle Optimizer",
        icon: "🥄",
        group: Group::W4,
        description: "Ladles/day per character, Cooking EXP balloons, meal costs, ladle + NMLB leveling plan",
    },
    Section {
        id: "cooking-mastery",
        title: "Cooking Mastery",
        icon: "🍳",
        group: Group::W4,
        description: "Mastery EXP/hr optimizer — which upgrade to level next",
    },
    Section {
        id: "fountain",
        title: "Fountain Calculator",
        icon: "⛲",
        group: Group::W5,
        description: "Currency values, earn rates, upgrades, marble bonuses",
    },
    Section {
        id: "jar",
        title: "Jar Calculator",
        icon: "🏺",
        group: Group::W5,
        description: "Production rates, rupie value, chances, collectibles",
    },
    Section {
        id: "sailing",
        title: "Sailing Calculator",
        icon: "⛵",
        group: Group::W5,
        description: "Fleet routes, chest tiers, loot value, artifact odds",
    },
    Section {
        id: "cglunko",
        title: "Cglunko Calculator",
        icon: "💎",
        group: Group::W5,
        description: "Cglunko upgrades, optimizer, stone focus, drop rate breakdown",
    },
    Section {
        id: "coral-god-rank",
        title: "Coral God Rank Planner",
        icon: "🐠",
        group: Group::W5,
        description: "Coral Kid upgrades, god-rank offering planning, and objective optimization",
    },
    Section {
        id: "monument",
        title: "Monument Calculator",
        icon: "🏛️",
        group: Group::W5,
        description: "Bravery, Justice, Wisdom — reward multipliers, permanent bonuses, time scaling",
    },
    Section {
        id: "opal",
        title: "Opal Optimizer",
        icon: "💎",
        group: Group::W5,
        description: "Villager EXP rates, opal distribution optimizer, all bonus terms",
    },
    Section {
        id: "sneaking",
        title: "Sneaking Optimizer",
        icon: "🥷",
        group: Group::W6,
        description: "Stealth, detection, charm equips, ninja kit bonuses",
    },
    Section {
        id: "farming",
        title: "Farming Planner",
        icon: "🌱",
        group: Group::W6,
        description: "Full plot grid, Land Rank allocation, evolution, overgrowth, beans, markets, and stickers",
    },
    Section {
        id: "clamworks",
        title: "Clamworks Calculator",
        icon: "🐚",
        group: Group::W7,
        description: "Pearl upgrades, optimizer, promotion tracker",
    },
    Section {
        id: "royal-guardian",
        title: "Royal Guardian Calculator",
        icon: "👑",
        group: Group::W7,
        description: "Outposts, resources and currencies, Armory, statues, and map quick views",
    },
    Section {
        id: "spelunking",
        title: "Spelunking Simulator & Optimizer",
        icon: "⛏️",
        group: Group::W7,
        description: "Experimental Normal and Manic expeditions, Auto speeds, manual strategies, parties and elixirs",
    },
    Section {
        id: "jelly-operator",
        title: "Jelly Operator Optimizer",
        icon: "🧫",
        group: Group::W7,
        description: "Cell layouts, obstruction clears, Bloodcells, upgrades, and account bonuses",
    },
    Section {
        id: "research",
        title: "Research Optimizer",
        icon: "🔬",
        group: Group::W7,
        description: "Simulate research grid strategies and EXP allocation",
    },
];

/// The section registered under `id`, if any.
pub fn find(id: &str) -> Option<&'static Section> {
    SECTIONS.iter().find(|section| section.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_every_upstream_card_once_and_grouped() {
        assert_eq!(SECTIONS.len(), 25);
        let mut ids: Vec<&str> = SECTIONS.iter().map(|section| section.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(
            ids.len(),
            SECTIONS.len(),
            "card ids must be stable and unique"
        );
        assert!(
            SECTIONS
                .windows(2)
                .all(|pair| group_index(pair[0].group) <= group_index(pair[1].group)),
            "cards must stay grouped"
        );
        let counts: Vec<usize> = Group::ALL
            .iter()
            .map(|group| {
                SECTIONS
                    .iter()
                    .filter(|section| section.group == *group)
                    .count()
            })
            .collect();
        assert_eq!(counts, [4, 3, 2, 2, 7, 2, 5]);
        for section in &SECTIONS {
            assert!(!section.title.is_empty());
            assert!(!section.icon.is_empty());
            assert!(!section.description.is_empty());
        }
    }

    #[test]
    fn search_reads_title_description_and_world() {
        let research = find("research").expect("research card");
        assert!(research.matches("research"));
        assert!(research.matches("EXP allocation"));
        assert!(research.matches("w7"));
        assert!(research.matches("world 7"));
        assert!(!research.matches("fishing"));

        let combat = find("combat-sim").expect("combat card");
        assert!(combat.matches("general"));
        assert!(combat.matches("replays"));
    }

    fn group_index(group: Group) -> usize {
        Group::ALL
            .into_iter()
            .position(|candidate| candidate == group)
            .expect("a listed group")
    }
}
