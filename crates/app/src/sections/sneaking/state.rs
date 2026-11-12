//! The sneaking section's own state: the login context and the slot the
//! inventory is aimed at.
//!
//! The workspace mirrors upstream's loadout panel, where the character selector
//! and the current selection survive a re-render. The loadout itself comes from
//! the save and is edited in memory; this state is what a project document keeps
//! so the workspace opens where it was left.

use crate::state::AppState;
use gpui::Context;
use idleon_core::sneaking::{Loadout, SlotKind};
use serde::{Deserialize, Serialize};

/// The two workspaces the section shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Workspace {
    #[default]
    Loadout,
    Breakdowns,
}

impl Workspace {
    pub const ALL: [Workspace; 2] = [Workspace::Loadout, Workspace::Breakdowns];

    pub fn id(self) -> &'static str {
        match self {
            Workspace::Loadout => "loadout",
            Workspace::Breakdowns => "breakdowns",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Workspace::Loadout => "Loadout",
            Workspace::Breakdowns => "Breakdowns",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|entry| entry.id() == id)
    }
}

/// The formula tabs inside the breakdowns workspace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FormulaTab {
    #[default]
    Stealth,
    Jade,
    Exp,
    Door,
    Runtime,
}

impl FormulaTab {
    pub const ALL: [FormulaTab; 5] = [
        FormulaTab::Stealth,
        FormulaTab::Jade,
        FormulaTab::Exp,
        FormulaTab::Door,
        FormulaTab::Runtime,
    ];

    pub fn id(self) -> &'static str {
        match self {
            FormulaTab::Stealth => "stealth",
            FormulaTab::Jade => "jade",
            FormulaTab::Exp => "exp",
            FormulaTab::Door => "door",
            FormulaTab::Runtime => "runtime",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FormulaTab::Stealth => "Stealth",
            FormulaTab::Jade => "Jade",
            FormulaTab::Exp => "EXP",
            FormulaTab::Door => "Door",
            FormulaTab::Runtime => "Runtime",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|entry| entry.id() == id)
    }
}

/// The sneak workspace's selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    /// The login context, or `None` for the first twin.
    pub login: Option<usize>,
    /// The twin whose slot the inventory is aimed at.
    pub twin: Option<usize>,
    /// The slot index, `0..4`: hat, weapon, charm one, charm two.
    pub slot: Option<usize>,
    /// The workspace the section opens.
    pub workspace: Workspace,
    /// The formula tab inside the breakdowns workspace.
    pub formula_tab: FormulaTab,
    /// The twin the breakdown trees read.
    pub breakdown_twin: Option<usize>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            login: None,
            twin: None,
            slot: None,
            workspace: Workspace::Loadout,
            formula_tab: FormulaTab::Stealth,
            breakdown_twin: None,
        }
    }
}

impl State {
    /// The login context, clamped to the roster.
    pub fn login(&self, players: usize) -> usize {
        self.login
            .filter(|index| *index < players.max(1))
            .unwrap_or(0)
    }

    /// The slot the inventory edits, clamped to the roster.
    pub fn target(&self, players: usize) -> Option<(usize, usize)> {
        let twin = self.twin.filter(|index| *index < players.max(1))?;
        Some((twin, self.slot.unwrap_or(2).min(3)))
    }

    /// Aim the inventory at one slot.
    pub fn select(&mut self, twin: usize, slot: usize) {
        self.twin = Some(twin);
        self.slot = Some(slot.min(3));
    }

    /// The twin the breakdown trees read, clamped to the roster.
    pub fn breakdown_twin(&self, players: usize) -> usize {
        self.breakdown_twin
            .filter(|index| *index < players.max(1))
            .unwrap_or_else(|| self.login(players))
    }
}

impl AppState {
    /// Point the loadout workspace at a login context.
    pub fn sneak_login(&mut self, index: usize, cx: &mut Context<Self>) {
        let players = self.sneaking.as_ref().map_or(0, Loadout::players);
        self.sneaking_state.login = (index < players).then_some(index);
        cx.notify();
    }

    /// Aim the inventory at one loadout slot.
    pub fn sneak_select(&mut self, twin: usize, slot: usize, cx: &mut Context<Self>) {
        self.sneaking_state.select(twin, slot);
        cx.notify();
    }

    /// Swap an inventory item into the selected slot.
    pub fn sneak_equip(&mut self, row: usize, cx: &mut Context<Self>) {
        let Some(loadout) = self.sneaking.as_mut() else {
            return;
        };
        let Some((twin, slot)) = self.sneaking_state.target(loadout.players()) else {
            self.status = "Sneaking: pick a loadout slot first.".into();
            cx.notify();
            return;
        };
        let Some(slot) = SlotKind::of(slot) else {
            return;
        };
        match loadout.equip(twin, slot, row) {
            Ok(()) => {
                let name = loadout
                    .twin(twin)
                    .map_or_else(String::new, |entry| entry.name.clone());
                let item = loadout.twin(twin).map(|entry| entry.slot(slot).name());
                self.status = format!(
                    "Sneaking: {} into {}'s {}.",
                    item.unwrap_or_default(),
                    name,
                    slot.label()
                );
            }
            Err(error) => self.status = format!("Sneaking: {error}."),
        }
        cx.notify();
    }

    /// Move the selected slot's item back to the inventory.
    pub fn sneak_unequip(&mut self, cx: &mut Context<Self>) {
        let Some(loadout) = self.sneaking.as_mut() else {
            return;
        };
        let Some((twin, slot)) = self.sneaking_state.target(loadout.players()) else {
            return;
        };
        let Some(slot) = SlotKind::of(slot) else {
            return;
        };
        match loadout.unequip(twin, slot) {
            Ok(()) => self.status = format!("Sneaking: {} emptied.", slot.label()),
            Err(error) => self.status = format!("Sneaking: {error}."),
        }
        cx.notify();
    }

    /// Switch the section's workspace.
    pub fn sneak_workspace(&mut self, workspace: Workspace, cx: &mut Context<Self>) {
        self.sneaking_state.workspace = workspace;
        cx.notify();
    }

    /// Switch the breakdown formula tab.
    pub fn sneak_formula_tab(&mut self, tab: FormulaTab, cx: &mut Context<Self>) {
        self.sneaking_state.formula_tab = tab;
        cx.notify();
    }

    /// Point the breakdown trees at one twin.
    pub fn sneak_breakdown_twin(&mut self, index: usize, cx: &mut Context<Self>) {
        let players = self.sneaking.as_ref().map_or(0, Loadout::players);
        self.sneaking_state.breakdown_twin = (index < players).then_some(index);
        cx.notify();
    }

    /// Put every edited slot back to the save.
    pub fn sneak_reset(&mut self, cx: &mut Context<Self>) {
        if let Some(loadout) = self.sneaking.as_mut() {
            loadout.reset();
            self.status = "Sneaking: the loadout is back to the save.".into();
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_selection_clamps_to_the_roster() {
        let mut state = State::default();
        assert_eq!(state.login(3), 0);
        assert_eq!(state.target(3), None);
        state.select(1, 3);
        assert_eq!(state.target(3), Some((1, 3)));
        state.login = Some(9);
        assert_eq!(state.login(3), 0);
        state.twin = Some(9);
        assert_eq!(state.target(3), None);
        state.twin = None;
        assert_eq!(state.target(3), None);
    }

    #[test]
    fn the_state_round_trips_through_json() {
        let state = State {
            login: Some(2),
            twin: Some(1),
            slot: Some(3),
            ..State::default()
        };
        let text = serde_json::to_string(&state).expect("the state serializes");
        let back: State = serde_json::from_str(&text).expect("the state reads back");
        assert_eq!(back, state);
        let partial: State = serde_json::from_str("{}").expect("an empty config is the default");
        assert_eq!(partial, State::default());
    }
}
