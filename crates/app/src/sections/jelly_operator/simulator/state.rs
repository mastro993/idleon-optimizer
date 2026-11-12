//! The sandbox board and its knobs: everything the simulator tab edits.

use idleon_core::jelly::board::{self, Layout};
use idleon_core::jelly::operation::{RevivePolicy, RoidTiming};
use idleon_core::jelly::{Jelly, OperationOptions, boss_time, data, fever_mode_name};
use std::collections::BTreeSet;

/// The last obstruction the page offers (`Math.min(71, …)`).
pub const MAX_STAGE: usize = 71;

/// The simulator's own board and knobs. It starts from the save and never
/// writes back to it.
#[derive(Clone, Debug)]
pub struct Simulator {
    pub layout: Layout,
    pub obstruction: usize,
    /// The board sections the sandbox pretends are purchased.
    pub plots: BTreeSet<usize>,
    pub fever: u32,
    /// Stronkroid's activation second; a negative value disables it.
    pub roid_timing: f64,
    pub revive: RevivePolicy,
    /// The palette pick; `None` is remove mode.
    pub selected: Option<usize>,
    pub seed: u32,
    pub message: String,
    pub message_bad: bool,
}

impl Default for Simulator {
    fn default() -> Self {
        Self {
            layout: Layout::new(),
            obstruction: 0,
            plots: BTreeSet::new(),
            fever: 0,
            roid_timing: -1.0,
            revive: RevivePolicy::default(),
            selected: Some(0),
            seed: 1,
            message: String::new(),
            message_bad: false,
        }
    }
}

impl Simulator {
    /// The upstream `makeSimulatorState`: the save's board, stage, plots and
    /// Fever, with Stronkroid off and remove mode not selected.
    pub fn new(jelly: &Jelly) -> Self {
        let modes = fever_modes(jelly);
        let mut simulator = Self {
            layout: jelly.layout.clone(),
            obstruction: jelly.progress.obstruction.min(MAX_STAGE),
            plots: jelly.plots.iter().copied().collect(),
            fever: if modes > 0 {
                jelly.progress.fever.min(modes - 1)
            } else {
                0
            },
            ..Self::default()
        };
        simulator.message = "Sandbox initialized from the current save.".to_string();
        simulator
    }

    /// The save with the sandbox's stage and board sections applied.
    pub fn sandbox(&self, jelly: &Jelly) -> Jelly {
        let mut sandbox = jelly.clone();
        sandbox.progress.obstruction = self.obstruction;
        sandbox.plots = self.plots.iter().copied().collect();
        sandbox
    }

    /// The inputs a run started now would use, so a later edit marks it stale.
    pub fn inputs(&self, jelly: Option<&Jelly>) -> RunInputs {
        RunInputs {
            jelly: jelly.cloned(),
            layout: self.layout.clone(),
            obstruction: self.obstruction,
            plots: self.plots.iter().copied().collect(),
            fever: self.fever,
            seed: self.seed,
            roid: self.roid(),
            revive: self.revive,
        }
    }

    /// The one-attempt options the sandbox runs with.
    pub fn options(&self) -> OperationOptions {
        OperationOptions {
            obstruction: Some(self.obstruction),
            fever: Some(self.fever),
            seed: self.seed,
            roid_timing: self.roid(),
            revive_policy: self.revive,
            max_critical_seconds: None,
        }
    }

    fn roid(&self) -> RoidTiming {
        if self.roid_timing < 0.0 {
            RoidTiming::Never
        } else {
            RoidTiming::At(self.roid_timing)
        }
    }

    /// Re-place every cell against the sandbox's unlocked slots, as
    /// `normalizeSimulatorLayout` does, and say how many no longer fit.
    pub fn normalize(&mut self, jelly: &Jelly, message: &str) {
        let sandbox = self.sandbox(jelly);
        let (layout, removed) = board::normalize(&sandbox, &self.layout);
        self.layout = layout;
        self.message = if removed > 0 {
            format!(
                "{message} Removed {removed} cell{} that no longer fit unlocked slots.",
                if removed == 1 { "" } else { "s" }
            )
        } else {
            message.to_string()
        };
        self.message_bad = false;
    }

    /// The stage select.
    pub fn set_stage(&mut self, jelly: &Jelly, stage: usize) {
        self.obstruction = stage.min(MAX_STAGE);
        let message = format!("Selected stage {}.", self.obstruction + 1);
        self.normalize(jelly, &message);
    }

    /// The Fever select, limited to the Feverizer's unlocked effects.
    pub fn set_fever(&mut self, jelly: &Jelly, fever: u32) {
        let modes = fever_modes(jelly);
        self.fever = if modes > 0 { fever.min(modes - 1) } else { 0 };
        self.message = format!("Selected {} Fever.", fever_mode_name(self.fever));
        self.message_bad = false;
    }

    /// Stronkroid's activation second; `-1` disables it.
    pub fn set_roid(&mut self, seconds: f64) {
        self.roid_timing = if seconds.is_finite() {
            seconds.clamp(-1.0, boss_time(self.obstruction)).round()
        } else {
            -1.0
        };
        self.message = if self.roid_timing < 0.0 {
            "Stronkroid disabled.".to_string()
        } else {
            format!("Stronkroid will activate at {} seconds.", self.roid_timing)
        };
        self.message_bad = false;
    }

    /// The automatic revive policy select.
    pub fn set_revive(&mut self, revive: RevivePolicy) {
        self.revive = revive;
        self.message = format!("Revival strategy set to {}.", revive_label(revive));
        self.message_bad = false;
    }

    /// The seed one attempt is drawn from; the same seed is the same attempt.
    pub fn set_seed(&mut self, seed: f64) {
        self.seed = if seed.is_finite() {
            seed.clamp(0.0, f64::from(u32::MAX)) as u32
        } else {
            1
        };
        self.message = format!("Seed {}.", self.seed);
        self.message_bad = false;
    }

    /// Pick the palette cell the next board click places.
    pub fn select(&mut self, cell_type: usize) {
        self.selected = Some(cell_type);
        self.message = format!(
            "{} selected. Click a slot to place it.",
            cell_name(cell_type)
        );
        self.message_bad = false;
    }

    /// Remove mode: the next board click removes the cell under the pointer.
    pub fn remove_mode(&mut self) {
        self.selected = None;
        self.message = "Remove mode selected. Click any occupied footprint.".to_string();
        self.message_bad = false;
    }

    /// Empty the sandbox board.
    pub fn clear_board(&mut self) {
        self.layout.clear();
        self.message = "Removed every cell from the sandbox board.".to_string();
        self.message_bad = false;
    }

    /// Put the sandbox back to the save's board, stage, sections and Fever.
    pub fn reset(&mut self, jelly: &Jelly) {
        *self = Self::new(jelly);
        self.message = "Sandbox reset from the current save.".to_string();
    }

    /// Restore the save's purchased board sections.
    pub fn use_save_sections(&mut self, jelly: &Jelly) {
        self.plots = jelly.plots.iter().copied().collect();
        self.normalize(jelly, "Restored board sections from the current save.");
    }

    /// Pretend every board section is purchased.
    pub fn unlock_all_sections(&mut self, jelly: &Jelly) {
        self.plots = data().plots.iter().map(|plot| plot.plot_id).collect();
        self.normalize(jelly, "Unlocked every board section for this sandbox.");
    }

    /// Lock every purchased board section.
    pub fn lock_sections(&mut self, jelly: &Jelly) {
        self.plots.clear();
        self.normalize(jelly, "Locked every purchased board section.");
    }

    /// Apply the whole board-section selection the toggle group reports.
    pub fn set_plots(&mut self, jelly: &Jelly, ids: &[usize]) {
        self.plots = ids.iter().copied().collect();
        let message = format!("Board sections: {} unlocked.", self.plots.len());
        self.normalize(jelly, &message);
    }

    /// Place the picked cell, or remove the cell under the pointer in remove
    /// mode. A placement over an occupied footprint replaces what it covers,
    /// like `jellyPlaceCell`.
    pub fn click(&mut self, jelly: &Jelly, slot: usize) {
        let Some(cell_type) = self.selected else {
            let Some((anchor, _)) = board::occupancy(&self.layout).get(&slot).copied() else {
                self.message = "There is no cell at that slot to remove.".to_string();
                self.message_bad = true;
                return;
            };
            self.layout = board::remove(&self.layout, anchor);
            self.message = format!("Removed the cell at slot {anchor}.");
            self.message_bad = false;
            return;
        };
        let sandbox = self.sandbox(jelly);
        match board::place(&sandbox, &self.layout, slot, cell_type) {
            Ok(next) => {
                self.layout = next;
                self.message = format!("Placed {} at anchor slot {slot}.", cell_name(cell_type));
                self.message_bad = false;
            }
            Err(reason) => {
                self.message = format!(
                    "Cannot place {} at slot {slot}: {reason}.",
                    cell_name(cell_type)
                );
                self.message_bad = true;
            }
        }
    }
}

/// The inputs a stored simulator result came from.
#[derive(Clone, Debug, PartialEq)]
pub struct RunInputs {
    pub jelly: Option<Jelly>,
    pub layout: Layout,
    pub obstruction: usize,
    pub plots: Vec<usize>,
    pub fever: u32,
    pub seed: u32,
    pub roid: RoidTiming,
    pub revive: RevivePolicy,
}

/// `jellyUpgradeLevel(S, 16)`, capped at the six Fever effects.
pub(super) fn fever_modes(jelly: &Jelly) -> u32 {
    (jelly.upgrade_level(16) as u32).min(6)
}

fn cell_name(cell_type: usize) -> String {
    data().cell_name(cell_type).to_string()
}

pub(super) fn revive_label(revive: RevivePolicy) -> &'static str {
    match revive {
        RevivePolicy::None => "Do not revive",
        RevivePolicy::ImmunoidFirst => "Immunoid first",
        RevivePolicy::AnchorFirst => "Cell anchors first",
        RevivePolicy::DpsFirst => "Highest-DPS anchors first",
        RevivePolicy::SoakFirst => "Soak first",
    }
}
