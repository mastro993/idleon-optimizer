//! What the window does when the prisma section is edited or run.

use crate::sections::prisma::RunInputs as PrismaRunInputs;
use crate::state::AppState;
use gpui::{App, Context};
use idleon_prisma::Outcome as PrismaOutcome;
use idleon_ui::runner::TaskState;

impl AppState {
    /// The inputs a Prisma run started now would use.
    pub(crate) fn prisma_inputs(&self) -> PrismaRunInputs {
        PrismaRunInputs {
            save_revision: self.save_revision,
            character: self.prisma.character,
            cauldron: self.prisma.cauldron.index(),
            afk_seconds: if crate::sections::prisma::valid_hours(self.prisma.afk_hours) {
                (self.prisma.afk_hours * 3600.0).round() as u64
            } else {
                0
            },
        }
    }

    /// The Prisma run: running, progress, result and staleness.
    pub(crate) fn prisma_task(&self, cx: &App) -> TaskState<PrismaOutcome> {
        self.prisma_run.read(cx).state(&self.prisma_inputs())
    }

    /// Select the character the plan reads its Prisma state for.
    pub(crate) fn set_prisma_character(&mut self, character: usize, cx: &mut Context<Self>) {
        self.prisma.character = character;
        self.prisma.adopt(self.save.as_ref());
        cx.notify();
    }

    /// Switch cauldron, which rebuilds the plan and its eligible rows.
    pub(crate) fn set_prisma_cauldron(
        &mut self,
        cauldron: idleon_prisma::Cauldron,
        cx: &mut Context<Self>,
    ) {
        self.prisma.cauldron = cauldron;
        // Metric ids carry the cauldron, so a new cauldron cannot keep the old one.
        self.prisma.metric = None;
        self.prisma.adopt(self.save.as_ref());
        cx.notify();
    }

    /// Set the AFK claim the run captures.
    pub(crate) fn set_prisma_hours(&mut self, hours: f64, cx: &mut Context<Self>) {
        if hours.is_finite() {
            self.prisma.afk_hours = hours.clamp(
                crate::sections::prisma::MIN_HOURS,
                crate::sections::prisma::MAX_HOURS,
            );
        }
        cx.notify();
    }

    /// Select the metric the rows rank by; empty is the bubble order.
    pub(crate) fn set_prisma_metric(&mut self, metric: &str, cx: &mut Context<Self>) {
        self.prisma.metric = (!metric.is_empty()).then(|| metric.to_string());
        cx.notify();
    }

    /// Run the bubble comparison off the UI thread through the shared task runner.
    pub(crate) fn run_prisma(&mut self, cx: &mut Context<Self>) {
        if !crate::sections::prisma::valid_hours(self.prisma.afk_hours) {
            self.set_status(
                "Enter AFK claim hours above 2 minutes and at most 55,555.",
                cx,
            );
            return;
        }
        let Some(plan) = self.prisma.plan.clone() else {
            self.set_status("Import a save to compare Prisma bubbles.", cx);
            return;
        };
        let inputs = self.prisma_inputs();
        self.start_run(
            self.prisma_run.clone(),
            "Comparing Prisma bubbles…",
            inputs,
            move |_inputs, report| idleon_prisma::run(&plan, report),
            cx,
        );
    }

    /// Ask a running Prisma comparison to stop; the previous result stays.
    pub(crate) fn cancel_prisma(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(self.prisma_run.clone(), "Cancelling the Prisma run…", cx);
    }
}
