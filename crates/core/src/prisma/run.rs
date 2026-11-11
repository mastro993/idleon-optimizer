//! The Prisma search: one variant per comparable bubble, streamed with progress
//! and cancellable.
//!
//! Ported from `prismaChanges`, `prismaOutcome` and the row loop in
//! `js/stats/prisma.js`. Every candidate is the saved state with one bubble's
//! Prisma flag toggled; the metrics are the cauldron's processed bubble
//! bonuses, so a change is exactly what Prisma does to that bonus.
//!
//! The downstream outcome families (damage, stats, skills and account totals)
//! arrive with PR-02: this module reports which effects still have no model in
//! [`Coverage`], so the section never presents a bonus as a downstream gain.

use super::plan::{Plan, Row};
use crate::model::{Control, Progress};

/// The metric id a bubble's processed bonus carries.
fn bonus_id(row: &Row) -> String {
    format!("bubble-bonus:{}", row.id)
}

/// The metric label a bubble's processed bonus carries.
fn bonus_name(row: &Row) -> String {
    format!("Bubble bonus · {}", row.name)
}

/// One evaluated metric: a value, or why it has none.
#[derive(Clone, Debug, PartialEq)]
pub struct Metric {
    pub id: String,
    pub name: String,
    pub value: Option<f64>,
    /// Processed bonuses are intermediates, never downstream outcomes.
    pub intermediate: bool,
    pub lower_is_better: bool,
    /// Why the value is missing, when it is.
    pub reason: String,
}

impl Metric {
    fn available(id: String, name: String, value: f64) -> Self {
        Self {
            id,
            name,
            value: Some(value),
            intermediate: true,
            lower_is_better: false,
            reason: String::new(),
        }
    }
}

/// One metric's change between the saved state and one candidate's variant.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub id: String,
    pub name: String,
    pub before: f64,
    pub after: f64,
    pub delta: f64,
    /// `delta / |before| * 100`; `None` when the saved value is zero.
    pub percent: Option<f64>,
    pub intermediate: bool,
    pub lower_is_better: bool,
}

impl Change {
    /// The score the ranking sorts by, as upstream ranks a metric: the relative
    /// change, else the absolute one, negated when lower is better.
    pub fn score(&self) -> f64 {
        let key = self.percent.unwrap_or(self.delta);
        if self.lower_is_better { -key } else { key }
    }
}

/// One candidate's comparison against the saved state.
#[derive(Clone, Debug, PartialEq)]
pub struct RowResult {
    pub row: Row,
    /// The saved scope: every purchased bubble in the cauldron.
    pub before: Vec<Metric>,
    /// The same scope with this candidate's Prisma flag toggled.
    pub after: Vec<Metric>,
    /// The metrics that moved. Empty when Prisma changes nothing measurable.
    pub changes: Vec<Change>,
}

impl RowResult {
    /// A metric's value in this variant, changed or not, as `prismaOutcome` reads it.
    pub fn outcome(&self, metric: &str) -> Option<&Metric> {
        self.after.iter().find(|entry| entry.id == metric)
    }

    /// The change a metric carries in this result, when it moved.
    pub fn change(&self, metric: &str) -> Option<&Change> {
        self.changes.iter().find(|change| change.id == metric)
    }

    /// What the ranking sort uses for one metric, matching `prismaOutcome`: a
    /// moved metric scores by its change, an unchanged one scores zero.
    fn score(&self, metric: &str) -> Option<f64> {
        match self.change(metric) {
            Some(change) => Some(change.score()),
            None => self.outcome(metric).map(|_| 0.0),
        }
    }

    /// Whether the row's own labels or any changed metric name matches `query`.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }
        let haystack = format!(
            "{} {} {} {}",
            self.row.name,
            self.row.effect,
            self.row.reason,
            self.changes
                .iter()
                .map(|change| change.name.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        );
        haystack.to_lowercase().contains(&query)
    }
}

/// One bubble effect and whether a downstream model reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct CoverageEntry {
    pub effect: String,
    /// The bubbles that carry the effect.
    pub bubbles: Vec<String>,
    /// Whether this build models a downstream outcome for the effect.
    pub modelled: bool,
}

/// Which of the cauldron's effects have a downstream model in this build.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Coverage {
    pub effects: Vec<CoverageEntry>,
}

impl Coverage {
    pub fn total(&self) -> usize {
        self.effects.len()
    }

    pub fn modelled(&self) -> usize {
        self.effects.iter().filter(|entry| entry.modelled).count()
    }

    pub fn missing(&self) -> usize {
        self.total() - self.modelled()
    }
}

/// One completed search: the saved scope and every candidate's comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub cauldron: usize,
    pub multiplier: f64,
    /// The saved scope every candidate is compared against.
    pub baseline: Vec<Metric>,
    pub results: Vec<RowResult>,
    pub coverage: Coverage,
}

impl Outcome {
    /// The candidates matching `query`, in plan order.
    pub fn search(&self, query: &str) -> Vec<&RowResult> {
        self.results
            .iter()
            .filter(|result| result.matches(query))
            .collect()
    }

    /// The candidates ranked by one metric, best first, ties in plan order.
    pub fn rank(&self, metric: &str) -> Vec<&RowResult> {
        let mut ranked: Vec<&RowResult> = self.results.iter().collect();
        ranked.sort_by(|a, b| {
            match (a.score(metric), b.score(metric)) {
                (Some(left), Some(right)) => right
                    .partial_cmp(&left)
                    .unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then(a.row.index.cmp(&b.row.index))
        });
        ranked
    }

    /// The default order: the plan's own bubble order.
    pub fn bubbles(&self) -> Vec<&RowResult> {
        self.results.iter().collect()
    }

    /// The metric ids the filters offer, in plan order.
    pub fn metrics(&self) -> Vec<&Metric> {
        self.baseline.iter().collect()
    }
}

/// The saved scope: every purchased bubble in the one cauldron.
fn baseline(plan: &Plan) -> Vec<Metric> {
    plan.rows
        .iter()
        .filter(|row| row.level.is_some_and(|level| level > 0.0))
        .map(|row| {
            let value = if row.prisma {
                row.with_prisma(plan.multiplier)
            } else {
                row.bonus
            };
            Metric::available(bonus_id(row), bonus_name(row), value)
        })
        .collect()
}

/// The candidate's scope: the saved scope with its own Prisma flag toggled.
fn variant(plan: &Plan, candidate: &Row, baseline: &[Metric]) -> Vec<Metric> {
    baseline
        .iter()
        .map(|metric| {
            if metric.id != bonus_id(candidate) {
                return metric.clone();
            }
            let value = if candidate.prisma {
                candidate.bonus
            } else {
                candidate.with_prisma(plan.multiplier)
            };
            Metric::available(metric.id.clone(), metric.name.clone(), value)
        })
        .collect()
}

/// The differences between the saved scope and one candidate's variant.
fn compare(plan: &Plan, candidate: &Row, baseline: &[Metric]) -> RowResult {
    let after = variant(plan, candidate, baseline);
    let changes = baseline
        .iter()
        .filter_map(|before| {
            let now = after.iter().find(|metric| metric.id == before.id)?;
            let (Some(old), Some(new)) = (before.value, now.value) else {
                return None;
            };
            if old == new {
                return None;
            }
            let delta = new - old;
            Some(Change {
                id: before.id.clone(),
                name: before.name.clone(),
                before: old,
                after: new,
                delta,
                percent: (old != 0.0).then(|| delta / old.abs() * 100.0),
                intermediate: before.intermediate,
                lower_is_better: before.lower_is_better,
            })
        })
        .collect();
    RowResult {
        row: candidate.clone(),
        before: baseline.to_vec(),
        after,
        changes,
    }
}

/// Which effects the cauldron's purchased bubbles carry, and whether a model reads them.
fn coverage(plan: &Plan) -> Coverage {
    let mut effects: Vec<CoverageEntry> = Vec::new();
    for row in plan
        .rows
        .iter()
        .filter(|row| row.level.is_some_and(|level| level > 0.0))
    {
        match effects.iter_mut().find(|entry| entry.effect == row.effect) {
            Some(entry) => entry.bubbles.push(row.name.clone()),
            None => effects.push(CoverageEntry {
                effect: row.effect.clone(),
                bubbles: vec![row.name.clone()],
                // No downstream outcome family is ported yet (PR-02).
                modelled: false,
            }),
        }
    }
    Coverage { effects }
}

/// Run the search, reporting progress after every candidate.
///
/// Returns `None` as soon as `report` asks to stop, so the caller keeps the
/// result it already showed.
pub fn run(plan: &Plan, report: &mut dyn FnMut(Progress) -> Control) -> Option<Outcome> {
    let baseline = baseline(plan);
    let candidates: Vec<&Row> = plan.rows.iter().filter(|row| row.comparable).collect();
    let total = candidates.len();
    if total == 0 {
        let progress = Progress {
            fraction: 1.0,
            message: "No bubble in this cauldron can be compared.".into(),
        };
        return match report(progress) {
            Control::Continue => Some(Outcome {
                cauldron: plan.cauldron,
                multiplier: plan.multiplier,
                baseline,
                results: Vec::new(),
                coverage: coverage(plan),
            }),
            Control::Stop => None,
        };
    }
    if report(Progress {
        fraction: 0.0,
        message: format!("Comparing 0 / {total} eligible bubbles…"),
    }) == Control::Stop
    {
        return None;
    }
    let mut results = Vec::with_capacity(total);
    for (done, candidate) in candidates.iter().enumerate() {
        results.push(compare(plan, candidate, &baseline));
        let progress = Progress {
            fraction: (done + 1) as f32 / total as f32,
            message: format!("Comparing {} / {total} eligible bubbles…", done + 1),
        };
        if report(progress) == Control::Stop {
            return None;
        }
    }
    Some(Outcome {
        cauldron: plan.cauldron,
        multiplier: plan.multiplier,
        baseline,
        results,
        coverage: coverage(plan),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prisma::plan;

    fn tiny() -> Plan {
        let save = crate::save::Save::new(
            serde_json::from_str(include_str!("../../tests/prisma-save.json"))
                .expect("the tiny save parses"),
        );
        plan(&save, 0, 0).expect("the tiny save plans")
    }

    fn finish() -> Outcome {
        let mut report = |_progress: Progress| Control::Continue;
        run(&tiny(), &mut report).expect("the run finishes")
    }

    #[test]
    fn a_run_compares_every_comparable_bubble_and_reports_progress() {
        let mut reports = Vec::new();
        let mut report = |progress: Progress| {
            reports.push(progress);
            Control::Continue
        };
        let outcome = run(&tiny(), &mut report).expect("the run finishes");
        assert_eq!(outcome.results.len(), 2);
        assert_eq!(outcome.baseline.len(), 3, "three purchased bubbles");
        assert_eq!(reports.first().map(|progress| progress.fraction), Some(0.0));
        assert_eq!(reports.last().map(|progress| progress.fraction), Some(1.0));
        assert!(
            reports
                .last()
                .is_some_and(|progress| progress.message.contains("2 / 2")),
            "the last report names both candidates"
        );
    }

    #[test]
    fn a_ready_bubble_gains_the_prisma_multiplier_and_an_already_bubble_loses_it() {
        let outcome = finish();
        let ready = outcome
            .results
            .iter()
            .find(|result| result.row.id == "0:2")
            .expect("the ready candidate");
        let change = ready.change("bubble-bonus:0:2").expect("the bonus moved");
        assert_eq!(
            (change.before, change.after, change.delta),
            (10.0, 20.0, 10.0)
        );
        assert_eq!(change.percent, Some(100.0));
        let already = outcome
            .results
            .iter()
            .find(|result| result.row.id == "0:0")
            .expect("the already candidate");
        let change = already.change("bubble-bonus:0:0").expect("the bonus moved");
        assert_eq!(
            (change.before, change.after, change.delta),
            (20.0, 10.0, -10.0)
        );
        assert_eq!(change.percent, Some(-50.0));
    }

    #[test]
    fn the_ranking_sorts_by_the_selected_metric() {
        let outcome = finish();
        let ranked: Vec<&str> = outcome
            .rank("bubble-bonus:0:2")
            .iter()
            .map(|result| result.row.id.as_str())
            .collect();
        assert_eq!(ranked, ["0:2", "0:0"], "the bubble that gains 100% leads");
        let ranked: Vec<&str> = outcome
            .rank("bubble-bonus:0:0")
            .iter()
            .map(|result| result.row.id.as_str())
            .collect();
        assert_eq!(
            ranked,
            ["0:2", "0:0"],
            "the unchanged row scores zero and stays ahead of the -50% row"
        );
    }

    #[test]
    fn the_search_matches_names_effects_and_changed_metrics() {
        let outcome = finish();
        let ids = |query: &str| -> Vec<String> {
            outcome
                .search(query)
                .iter()
                .map(|result| result.row.id.clone())
                .collect()
        };
        assert_eq!(ids("hearty"), ["0:2"]);
        assert_eq!(ids("MinEff"), ["0:2"]);
        assert_eq!(ids("bubble bonus"), ["0:0", "0:2"]);
        assert_eq!(ids("nope"), Vec::<String>::new());
        assert_eq!(ids("  "), ["0:0", "0:2"]);
    }

    #[test]
    fn cancelling_stops_the_run_and_returns_nothing() {
        let mut seen = 0;
        let mut report = |_progress: Progress| {
            seen += 1;
            Control::Stop
        };
        let outcome = run(&tiny(), &mut report);
        assert!(outcome.is_none(), "a cancelled run has no result");
        assert_eq!(seen, 1, "the first report stops the run");
    }

    #[test]
    fn every_effect_lists_whether_a_downstream_model_reads_it() {
        let outcome = finish();
        let coverage = &outcome.coverage;
        assert_eq!(coverage.total(), 3);
        assert_eq!(coverage.modelled(), 0);
        assert_eq!(coverage.missing(), 3);
        let strength = coverage
            .effects
            .iter()
            .find(|entry| entry.effect == "TotalSTR")
            .expect("the strength effect");
        assert_eq!(strength.bubbles, ["Roid Ragin"]);
        assert!(!strength.modelled);
    }
}
