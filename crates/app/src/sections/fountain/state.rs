//! The Fountain section's visible state: its tab, the water the upgrade table
//! lists, the simulated desired currency and the upgrade the cost curve plots.
//!
//! The save supplies every number; these are only the section's selections, so
//! they live in the project document like every other section's.

use idleon_core::Fountain;
use idleon_core::fountain::{self, WATERS};
use serde::{Deserialize, Serialize};

/// The fountain tabs this build implements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    /// Summary, income, bonuses, stacks and the level plan.
    #[default]
    Overview,
    /// Currency values, per-water multipliers and the desired simulator.
    Currency,
    /// Every upgrade, the water switch and the cost curve.
    Upgrades,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Overview, Tab::Currency, Tab::Upgrades];

    pub fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Currency => "currency",
            Tab::Upgrades => "upgrades",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Currency => "Currency",
            Tab::Upgrades => "Upgrades",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The section's selections, kept across tab switches and dashboard visits.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub tab: Tab,
    /// The water the upgrade table lists.
    pub water: usize,
    /// The desired currency the currency tab simulates; `None` reads the save's.
    pub desired: Option<usize>,
    /// The upgrade the cost curve plots.
    pub upgrade: Option<usize>,
}

impl State {
    /// Keep the selections the current cavern can still resolve.
    pub fn adopt(&mut self, fountain: Option<&Fountain>) {
        let Some(fountain) = fountain else {
            return;
        };
        self.water = self.water.min(WATERS - 1);
        self.desired = self.desired.filter(|currency| {
            *currency < fountain::CURRENCIES && fountain.currency_unlocked(*currency)
        });
        self.upgrade = self
            .upgrade
            .filter(|upgrade| fountain::upgrade(self.water, *upgrade).is_some());
    }
}
