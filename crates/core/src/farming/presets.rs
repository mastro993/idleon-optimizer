//! The preset goals side by side: the upstream "Preset Comparison" table.
//!
//! `renderRank` in `farming-calc.html` ends with a table that runs
//! `optimizeLandRank` once for every goal in the page's `OBJECTIVES` list from
//! the selected baseline, so the player sees what each goal would spend and
//! gain before committing to one. The seven goals are the built-in presets; a
//! custom preset, which the app keeps in the project document, saves one goal
//! together with its baseline and crop target.

use super::{Allocation, Farm, Mode, Objective};
use crate::model::{Control, Progress};

/// Every built-in preset planned from one baseline.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub mode: Mode,
    pub target_crop: Option<u32>,
    /// One row per goal, in the upstream `OBJECTIVES` order.
    pub rows: Vec<Allocation>,
}

impl Farm {
    /// `optimizeLandRank` without a reporter: one goal's plan.
    pub fn plan(&self, objective: Objective, mode: Mode, target_crop: Option<u32>) -> Allocation {
        self.allocation(objective, mode, target_crop, &mut |_progress| {
            Control::Continue
        })
        .expect("an allocation that cannot be cancelled returns a plan")
    }

    /// The preset comparison: every goal planned from `mode` and `target_crop`.
    ///
    /// Returns `None` when the reporter stopped the run, so a cancelled
    /// comparison stores nothing.
    pub fn comparison(
        &self,
        mode: Mode,
        target_crop: Option<u32>,
        report: &mut dyn FnMut(Progress) -> Control,
    ) -> Option<Comparison> {
        let goals = Objective::ALL.len() as f32;
        let mut rows = Vec::with_capacity(Objective::ALL.len());
        for (index, objective) in Objective::ALL.into_iter().enumerate() {
            let label = objective.label();
            let row = self.allocation(objective, mode, target_crop, &mut |progress| {
                let fraction = (index as f32 + progress.fraction.clamp(0.0, 1.0)) / goals;
                report(Progress {
                    fraction,
                    message: format!("{label} — {}", progress.message),
                })
            })?;
            rows.push(row);
        }
        report(Progress {
            fraction: 1.0,
            message: "Compared every preset goal.".into(),
        });
        Some(Comparison {
            mode,
            target_crop,
            rows,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::farming::PLOTS;

    /// A farm with points to spend, so the comparison has something to move.
    fn farm() -> Farm {
        let mut farm = Farm {
            rank_ranks: vec![4; PLOTS],
            ..Farm::default()
        };
        farm.plots[0].seed_family = 0;
        farm.plots[1].seed_family = 1;
        farm
    }

    fn no_report(_progress: Progress) -> Control {
        Control::Continue
    }

    #[test]
    fn the_comparison_plans_every_goal_in_order() {
        let farm = farm();
        let comparison = farm
            .comparison(Mode::Reset, Some(240), &mut no_report)
            .expect("an uncancelled comparison returns every goal");
        assert_eq!(comparison.mode, Mode::Reset);
        assert_eq!(comparison.target_crop, Some(240));
        assert_eq!(
            comparison
                .rows
                .iter()
                .map(|row| row.objective)
                .collect::<Vec<_>>(),
            Objective::ALL.to_vec()
        );
        for row in &comparison.rows {
            let planned = farm.plan(row.objective, Mode::Reset, Some(240));
            assert_eq!(
                row,
                &planned,
                "{}: the row is that goal's plan",
                row.objective.id()
            );
        }
    }

    #[test]
    fn a_comparison_reports_every_goal_and_ends_complete() {
        let farm = farm();
        let mut fractions: Vec<f32> = Vec::new();
        let comparison = farm
            .comparison(Mode::Reset, Some(1), &mut |progress| {
                fractions.push(progress.fraction);
                Control::Continue
            })
            .expect("the comparison completes");
        assert_eq!(comparison.rows.len(), Objective::ALL.len());
        assert!(!fractions.is_empty());
        assert!(fractions.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(fractions.last().copied(), Some(1.0));
        assert!(
            fractions
                .iter()
                .all(|fraction| (0.0..=1.0).contains(fraction))
        );
    }

    #[test]
    fn cancelling_the_comparison_stores_nothing() {
        let farm = farm();
        let mut reports = 0;
        let outcome = farm.comparison(Mode::Reset, Some(1), &mut |_progress| {
            reports += 1;
            Control::Stop
        });
        assert!(outcome.is_none());
        assert_eq!(reports, 1, "the run stops at its first report");
    }

    #[test]
    fn every_upstream_goal_is_a_built_in_preset() {
        let ids: Vec<&str> = Objective::ALL.iter().map(|goal| goal.id()).collect();
        assert_eq!(
            ids,
            [
                "rankXp",
                "farmingExp",
                "evolution",
                "overgrowth",
                "cropValue",
                "beans",
                "growth",
            ]
        );
        for goal in Objective::ALL {
            assert_eq!(Objective::from_id(goal.id()), Some(goal));
        }
    }
}
