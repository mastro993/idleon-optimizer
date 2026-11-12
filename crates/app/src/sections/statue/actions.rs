//! What the window does when the statue section is edited or run.

use crate::sections::statue::Sort as StatueSort;
use crate::state::AppState;
use gpui::Context;
use idleon_statue::Estimate as StatueEstimate;

impl AppState {
    /// Pick a character for the statue table; characters without saved rows
    /// cannot be selected, so the table always has a valid subject.
    pub(crate) fn set_statue_character(&mut self, character: usize, cx: &mut Context<Self>) {
        if self.statue.save.has_character(character) {
            self.statue.select_character(character, cx);
            cx.notify();
        }
    }

    pub(crate) fn set_statue_sort(&mut self, sort: StatueSort, cx: &mut Context<Self>) {
        self.statue.sort = sort;
        cx.notify();
    }

    /// Switch the statue table between guaranteed and average deposit estimates.
    pub(crate) fn set_statue_estimate(&mut self, estimate: StatueEstimate, cx: &mut Context<Self>) {
        self.statue.estimate = estimate;
        cx.notify();
    }

    /// Set one statue's target level. A hand edit ends the batch undo step and
    /// the status that described it.
    pub(crate) fn set_statue_target(&mut self, statue: usize, target: u32, cx: &mut Context<Self>) {
        self.statue.set_target(statue, target);
        self.statue.batch_status = None;
        cx.notify();
    }

    /// Set every statue's target to the level in the batch entry, or show why
    /// the plan left it alone.
    pub(crate) fn set_statue_targets_all(&mut self, cx: &mut Context<Self>) {
        let text = self.statue.batch_input.read(cx).text().to_string();
        match self
            .statue
            .plan
            .set_all(&self.statue.save, self.statue.character, &text)
        {
            Ok(level) => {
                self.statue.batch_error = None;
                self.statue.batch_status = Some(format!(
                    "All {} targets set to Lv {level}.",
                    idleon_statue::count()
                ));
            }
            Err(error) => self.statue.batch_error = Some(error.message()),
        }
        cx.notify();
    }

    /// Next level for all: clear the entered targets, so every row follows its
    /// saved level plus one.
    pub(crate) fn set_statue_next_level(&mut self, cx: &mut Context<Self>) {
        self.statue.plan.next_level();
        self.statue.batch_error = None;
        self.statue.batch_status = Some("Targets follow each saved level plus one.".to_string());
        cx.notify();
    }

    /// Restore the targets the last statue batch action replaced.
    pub(crate) fn undo_statue_batch(&mut self, cx: &mut Context<Self>) {
        if let Some(label) = self.statue.plan.undo() {
            self.statue.batch_error = None;
            self.statue.batch_status = Some(format!("Undid {label}."));
            cx.notify();
        }
    }

    /// Pick the statue the farming breakdown describes.
    pub(crate) fn set_statue_breakdown(&mut self, statue: usize, cx: &mut Context<Self>) {
        if statue < idleon_statue::count() {
            self.statue.breakdown_statue = statue;
            cx.notify();
        }
    }
}
