//! The step-by-step replay of a finished run's swap list.
//!
//! The optimizer returns one [`SwapStep`] per slot it changes. The section shows
//! that list as a walkthrough: the board starts at the run's input, Next applies
//! the next step in order, Previous rebuilds the board at the prior step and
//! Done commits the optimized board. Rebuilding from the start on every move is
//! what makes Previous exact; the board is 96 slots, so the cost is irrelevant.

use super::optimize::{SwapStep, apply_swaps};
use super::{Board, Cog};

/// A swap list being replayed one step at a time.
#[derive(Clone, Debug)]
pub struct SwapWalkthrough {
    start: Board,
    target: Board,
    shelf_start: Vec<Option<Cog>>,
    steps: Vec<SwapStep>,
    applied: usize,
    board: Board,
    shelf: Vec<Option<Cog>>,
}

impl SwapWalkthrough {
    /// A walkthrough from `start` to `target`, taking shelf cogs from
    /// `shelf_slots` exactly as the optimizer's swap list describes.
    pub fn new(
        start: &Board,
        target: &Board,
        shelf_slots: &[Option<Cog>],
        steps: &[SwapStep],
    ) -> Self {
        let mut walkthrough = Self {
            start: start.clone(),
            target: target.clone(),
            shelf_start: shelf_slots.to_vec(),
            steps: steps.to_vec(),
            applied: 0,
            board: start.clone(),
            shelf: shelf_slots.to_vec(),
        };
        walkthrough.replay();
        walkthrough
    }

    /// The swap list, in application order.
    pub fn steps(&self) -> &[SwapStep] {
        &self.steps
    }

    /// How many steps are applied.
    pub fn applied(&self) -> usize {
        self.applied
    }

    /// How many steps the run produced.
    pub fn total(&self) -> usize {
        self.steps.len()
    }

    /// Whether every step is applied.
    pub fn is_complete(&self) -> bool {
        self.applied >= self.steps.len()
    }

    /// The board with `applied` steps on it.
    pub fn board(&self) -> &Board {
        &self.board
    }

    /// The step Next would apply, or `None` once every step is applied.
    pub fn current(&self) -> Option<&SwapStep> {
        self.steps.get(self.applied)
    }

    /// Apply the next step. Returns whether the walkthrough moved.
    pub fn next_step(&mut self) -> bool {
        if self.is_complete() {
            return false;
        }
        self.applied += 1;
        self.replay();
        true
    }

    /// Undo the last applied step. Returns whether the walkthrough moved.
    pub fn previous(&mut self) -> bool {
        if self.applied == 0 {
            return false;
        }
        self.applied -= 1;
        self.replay();
        true
    }

    /// Apply every step and return the optimized board, the board Done keeps.
    pub fn finish(&mut self) -> Board {
        self.applied = self.steps.len();
        self.replay();
        self.target.clone()
    }

    /// Rebuild the board and shelf at the current cursor from the start state,
    /// so a step forward and a step back land on exactly the same board.
    fn replay(&mut self) {
        self.board = self.start.clone();
        self.shelf = self.shelf_start.clone();
        apply_swaps(
            &mut self.board,
            &self.target,
            &mut self.shelf,
            &self.steps,
            self.applied,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::empty_board;

    fn cog(name: &str, slot: usize) -> Cog {
        Cog::named(name, slot)
    }

    /// Two steps: a board↔board move, then a shelf move that sends the board's
    /// player cog to the shelf.
    fn sample() -> (Board, Board, Vec<Option<Cog>>, Vec<SwapStep>) {
        let mut start = empty_board();
        start[0] = Some(cog("Player_Ana", 0));
        start[1] = Some(cog("Cog3A00", 1));
        start[2] = Some(cog("Cog3B01", 2));

        let mut target = empty_board();
        target[0] = Some(cog("Cog3A00", 1));
        target[1] = Some(cog("CogCry2", 108));
        target[2] = Some(cog("Cog3B01", 2));

        let shelf = vec![Some(cog("CogCry2", 108)), None, None];
        let steps = vec![
            SwapStep {
                slot: 0,
                from: "Player_Ana".into(),
                to: "Cog3A00".into(),
                src_slot: Some(1),
                col: 0,
                row: 0,
            },
            SwapStep {
                slot: 1,
                from: "Cog3A00".into(),
                to: "CogCry2".into(),
                src_slot: None,
                col: 1,
                row: 0,
            },
        ];
        (start, target, shelf, steps)
    }

    #[test]
    fn next_applies_the_steps_in_order() {
        let (start, target, shelf, steps) = sample();
        let mut walkthrough = SwapWalkthrough::new(&start, &target, &shelf, &steps);
        assert_eq!(walkthrough.board(), &start);
        assert_eq!(
            walkthrough.current().map(|step| step.to.as_str()),
            Some("Cog3A00")
        );

        assert!(walkthrough.next_step());
        let mut expected = start.clone();
        let mut expected_shelf = shelf.clone();
        apply_swaps(&mut expected, &target, &mut expected_shelf, &steps, 1);
        assert_eq!(walkthrough.applied(), 1);
        assert_eq!(walkthrough.board(), &expected);
        assert_eq!(
            walkthrough.current().map(|step| step.to.as_str()),
            Some("CogCry2")
        );

        assert!(walkthrough.next_step());
        assert_eq!(walkthrough.board(), &target);
        assert!(walkthrough.is_complete());
        assert!(walkthrough.current().is_none());
        assert!(!walkthrough.next_step());
    }

    #[test]
    fn previous_restores_each_prior_board() {
        let (start, target, shelf, steps) = sample();
        let mut walkthrough = SwapWalkthrough::new(&start, &target, &shelf, &steps);
        assert!(!walkthrough.previous());

        assert!(walkthrough.next_step());
        let after_first = walkthrough.board().clone();
        assert!(walkthrough.next_step());
        assert_eq!(walkthrough.board(), &target);

        assert!(walkthrough.previous());
        assert_eq!(walkthrough.board(), &after_first);
        assert_eq!(walkthrough.applied(), 1);
        assert!(walkthrough.previous());
        assert_eq!(walkthrough.board(), &start);
        assert_eq!(walkthrough.applied(), 0);
        assert!(!walkthrough.previous());
    }

    #[test]
    fn finish_commits_the_optimized_board_from_any_step() {
        let (start, target, shelf, steps) = sample();
        let mut walkthrough = SwapWalkthrough::new(&start, &target, &shelf, &steps);
        assert!(walkthrough.next_step());
        assert_eq!(walkthrough.finish(), target);
        assert_eq!(walkthrough.board(), &target);
        assert!(walkthrough.is_complete());
        assert_eq!(walkthrough.applied(), walkthrough.total());
    }

    #[test]
    fn the_shelf_follows_the_steps() {
        let (start, target, shelf, steps) = sample();
        let mut walkthrough = SwapWalkthrough::new(&start, &target, &shelf, &steps);
        assert!(walkthrough.next_step());
        assert!(
            walkthrough
                .shelf
                .iter()
                .flatten()
                .any(|cog| cog.name == "CogCry2"),
            "the shelf cog is still there until the second step"
        );
        assert!(walkthrough.next_step());
        assert!(
            !walkthrough
                .shelf
                .iter()
                .flatten()
                .any(|cog| cog.name == "CogCry2"),
            "the shelf cog moved to the board"
        );
        assert!(
            walkthrough
                .shelf
                .iter()
                .flatten()
                .any(|cog| cog.name == "Player_Ana"),
            "the displaced player cog went back to the shelf"
        );
        assert!(walkthrough.previous());
        assert!(
            walkthrough
                .shelf
                .iter()
                .flatten()
                .any(|cog| cog.name == "CogCry2"),
            "Previous restores the shelf too"
        );
    }
}
