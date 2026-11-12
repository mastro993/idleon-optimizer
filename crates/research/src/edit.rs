//! Manual edits: the simulation fields a hand edit can change, and the undo
//! trail over them.
//!
//! The editor writes grid levels, observation state, magnifiers and shapes. One
//! [`Snapshot`] holds that slice and nothing else, so undo is a swap rather than
//! a command log: every edit, from a click on a square to a whole optimizer
//! result, is one step.

use crate::model::{GridState, Mag, Observations, Placement, Simulation};

/// How many edits the trail keeps before the oldest falls off.
pub const UNDO_LIMIT: usize = 64;

/// The simulation fields a manual edit can change.
///
/// `saved_overlay` travels with the shapes: the save's own overlay wins over
/// `placements` until a shape is edited, so undo has to restore which of the two
/// the rate reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub grid: GridState,
    pub observations: Observations,
    pub mags: Vec<Mag>,
    pub placements: Vec<Placement>,
    pub saved_overlay: Option<Vec<i32>>,
}

impl Snapshot {
    /// The editable slice of `sim`.
    pub fn of(sim: &Simulation) -> Self {
        Self {
            grid: sim.grid.clone(),
            observations: sim.observations.clone(),
            mags: sim.mags.clone(),
            placements: sim.placements.clone(),
            saved_overlay: sim.saved_overlay.clone(),
        }
    }

    /// Write this slice back over `sim`, leaving the save-derived fields alone.
    pub fn apply(&self, sim: &mut Simulation) {
        sim.grid = self.grid.clone();
        sim.observations = self.observations.clone();
        sim.mags = self.mags.clone();
        sim.placements = self.placements.clone();
        sim.saved_overlay = self.saved_overlay.clone();
    }
}

/// An undo/redo trail of [`Snapshot`]s, oldest first.
#[derive(Clone, Debug, Default)]
pub struct History {
    past: Vec<Snapshot>,
    future: Vec<Snapshot>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Keep `before` as the state an undo returns to, and drop the redo trail:
    /// a new edit makes the edits that were undone unreachable again.
    pub fn record(&mut self, before: Snapshot) {
        self.past.push(before);
        if self.past.len() > UNDO_LIMIT {
            self.past.remove(0);
        }
        self.future.clear();
    }

    /// The state before the last edit, keeping `current` for redo.
    pub fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let previous = self.past.pop()?;
        self.future.push(current);
        Some(previous)
    }

    /// The state the last undone edit produced, keeping `current` for undo.
    pub fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let next = self.future.pop()?;
        self.past.push(current);
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Forget both trails, as a new save or document does.
    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Mag, MagKind, Placement};

    fn snapshot(levels: u32, shapes: usize) -> Snapshot {
        let mut grid = GridState::empty();
        grid.set(50, levels);
        Snapshot {
            grid,
            observations: Observations::default(),
            mags: vec![Mag::new(MagKind::Regular, Some(0)); shapes],
            placements: vec![
                Placement {
                    shape: 0,
                    x: 0.0,
                    y: 0.0,
                    rot: 0.0,
                };
                shapes
            ],
            saved_overlay: None,
        }
    }

    #[test]
    fn undo_returns_the_recorded_snapshot_and_redo_returns_it_again() {
        let mut history = History::new();
        history.record(snapshot(1, 0));
        assert!(history.can_undo() && !history.can_redo());

        let undone = history.undo(snapshot(2, 1)).expect("one edit was recorded");
        assert_eq!(undone, snapshot(1, 0));
        assert!(!history.can_undo() && history.can_redo());

        let redone = history
            .redo(undone.clone())
            .expect("the undo can be redone");
        assert_eq!(redone, snapshot(2, 1));
        assert!(history.can_undo() && !history.can_redo());
    }

    #[test]
    fn a_new_edit_drops_the_redo_trail() {
        let mut history = History::new();
        history.record(snapshot(1, 0));
        history.undo(snapshot(2, 0));
        history.record(snapshot(3, 0));
        assert!(!history.can_redo());
        assert_eq!(history.undo(snapshot(4, 0)), Some(snapshot(3, 0)));
    }

    #[test]
    fn the_trail_keeps_the_newest_edits() {
        let mut history = History::new();
        for level in 1..=(UNDO_LIMIT as u32 + 5) {
            history.record(snapshot(level, 0));
        }
        let mut undone = 0;
        while history.undo(snapshot(0, 0)).is_some() {
            undone += 1;
        }
        assert_eq!(undone, UNDO_LIMIT);
    }

    #[test]
    fn a_snapshot_round_trips_through_the_simulation() {
        let mut sim = Simulation {
            research_level: 7,
            ..Simulation::default()
        };
        sim.placements.push(Placement {
            shape: 2,
            x: 30.0,
            y: 60.0,
            rot: 15.0,
        });
        sim.saved_overlay = Some(vec![1; 240]);
        let before = Snapshot::of(&sim);

        sim.grid.set(4, 2);
        sim.mags.push(Mag::new(MagKind::Monocle, None));
        sim.placements.clear();
        before.apply(&mut sim);

        assert_eq!(Snapshot::of(&sim), before);
        assert_eq!(sim.research_level, 7, "save-derived fields stay put");
    }
}
