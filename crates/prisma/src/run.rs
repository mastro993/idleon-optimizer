//! The Prisma search: one variant per comparable bubble, streamed with progress
//! and cancellable.
//!
//! Ported from `prismaChanges`, `prismaOutcome` and the row loop in
//! `js/stats/prisma.js`. Every candidate is the saved state with one bubble's
//! Prisma flag toggled; the measured scope is every purchased bubble's bonus
//! (an intermediate) plus the downstream outcome families this build models.
//! Today that is one: `statue-retention`, the Statue EXP the STARTUE EXP bubble
//! buys (`statueLevelingInputs` in `js/stats/systems/common/statue-leveling.js`).
//!
//! Every other effect keeps reporting the bonus as an intermediate only.
//! [`Coverage`] names the native systems upstream's audit
//! (`js/stats/data/prisma-source-coverage.js`) found reading each effect and
//! whether this build computes a downstream metric for it, so a bubble bonus is
//! never presented as a stat, skill or damage gain it is not.

use super::data::Consumer;
use super::plan::{Plan, Row};
use idleon_common::{Control, Progress};

/// The metric id a bubble's processed bonus carries.
fn bonus_id(row: &Row) -> String {
    format!("bubble-bonus:{}", row.id)
}

/// The metric name a bubble's processed bonus carries.
fn bonus_name(row: &Row) -> String {
    format!("Bubble bonus · {}", row.name)
}

/// One downstream outcome family the build models for a bubble effect.
struct Model {
    /// The effect key the bubble carries.
    effect: &'static str,
    /// The upstream metric id.
    id: &'static str,
    /// The upstream metric name.
    name: &'static str,
    /// The metric's value from the effect's processed bonus.
    outcome: fn(f64) -> f64,
}

/// The downstream outcomes this build models, in display order.
///
/// Upstream's evaluator covers thousands of metrics; the port measures the one
/// whose whole formula is already here. `statue-retention` is
/// `statueLevelingInputs(...).retentionPct`, which reads only the STARTUE EXP
/// bubble: the bubble keeps half the EXP a statue level consumes, so the metric
/// is `(1 - max(.5, 1 - pct/100)) × 100`.
const MODELS: [Model; 1] = [Model {
    effect: "StatueStartEXP",
    id: "statue-retention",
    name: "Statue EXP retained on level-up",
    outcome: |pct| idleon_statue::Leveling::new(pct, 0.0, 0, 0, Vec::new(), false).retention_pct,
}];

/// One evaluated metric: a value, or why it has none.
#[derive(Clone, Debug, PartialEq)]
pub struct Metric {
    pub id: String,
    pub name: String,
    pub value: Option<f64>,
    /// Processed bonuses and other inputs are intermediates, never outcomes.
    pub intermediate: bool,
    pub lower_is_better: bool,
    /// Why the value is missing, when it is.
    pub reason: String,
}

impl Metric {
    fn available(id: String, name: String, value: f64, intermediate: bool) -> Self {
        Self {
            id,
            name,
            value: Some(value),
            intermediate,
            lower_is_better: false,
            reason: String::new(),
        }
    }

    fn with_value(&self, value: f64) -> Self {
        Self {
            value: Some(value),
            ..self.clone()
        }
    }
}

/// One metric's change between two evaluated scopes.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub id: String,
    pub name: String,
    pub before: f64,
    pub after: f64,
    pub delta: f64,
    /// `delta / |before| * 100`; `None` when the starting value is zero.
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

/// Which state the save is in for one candidate's bubble.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    /// The bubble is not Prisma-ed: the row previews adding it.
    Added,
    /// The bubble is already Prisma-ed: the row measures the benefit it keeps.
    Retained,
}

impl Comparison {
    /// The upstream comparison id.
    pub fn id(self) -> &'static str {
        match self {
            Comparison::Added => "added",
            Comparison::Retained => "retained",
        }
    }

    /// The row's direction, in the upstream detail wording.
    pub fn label(self) -> &'static str {
        match self {
            Comparison::Added => "Potential gain",
            Comparison::Retained => "Benefit retained",
        }
    }
}

/// One candidate's comparison against the saved state.
#[derive(Clone, Debug, PartialEq)]
pub struct RowResult {
    pub row: Row,
    /// Whether the candidate's Prisma is already in the save.
    pub comparison: Comparison,
    /// The same scope without this candidate's Prisma.
    pub before: Vec<Metric>,
    /// The same scope with it: the state the row measures.
    pub after: Vec<Metric>,
    /// The downstream outcomes that moved.
    pub changes: Vec<Change>,
    /// The processed bubble bonuses that moved.
    pub intermediates: Vec<Change>,
}

impl RowResult {
    /// A metric's value in the measured state, changed or not, as
    /// `prismaOutcome` reads it.
    pub fn outcome(&self, metric: &str) -> Option<&Metric> {
        self.after.iter().find(|entry| entry.id == metric)
    }

    /// The change a metric carries in this result, downstream or intermediate,
    /// when it moved.
    pub fn change(&self, metric: &str) -> Option<&Change> {
        self.changes
            .iter()
            .chain(&self.intermediates)
            .find(|change| change.id == metric)
    }

    /// The metric's comparison for display: its change when it moved, else the
    /// measured value with a zero delta, as `prismaOutcome` returns it.
    pub fn view(&self, metric: &str) -> Option<Change> {
        if let Some(change) = self.change(metric) {
            return Some(change.clone());
        }
        let entry = self.outcome(metric)?;
        let value = entry.value?;
        Some(Change {
            id: entry.id.clone(),
            name: entry.name.clone(),
            before: value,
            after: value,
            delta: 0.0,
            percent: (value != 0.0).then_some(0.0),
            intermediate: entry.intermediate,
            lower_is_better: entry.lower_is_better,
        })
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
                .chain(&self.intermediates)
                .map(|change| change.name.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        );
        haystack.to_lowercase().contains(&query)
    }
}

/// One bubble effect, its bubbles, the native systems that read it and whether
/// this build computes a downstream metric for it.
#[derive(Clone, Debug, PartialEq)]
pub struct CoverageEntry {
    pub effect: String,
    /// The bubbles that carry the effect.
    pub bubbles: Vec<String>,
    /// The direct native references upstream's audit traced for the effect.
    pub consumers: Vec<Consumer>,
    /// The downstream metric ids this build computes for the effect.
    pub metrics: Vec<String>,
}

impl CoverageEntry {
    /// Whether this build models a downstream outcome for the effect.
    pub fn modelled(&self) -> bool {
        !self.metrics.is_empty()
    }
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
        self.effects.iter().filter(|entry| entry.modelled()).count()
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

    /// The metric ids the filters offer: the downstream outcomes first, then
    /// the processed bonuses.
    pub fn metrics(&self) -> Vec<&Metric> {
        self.baseline.iter().collect()
    }
}

/// The purchased bubbles, which are the ones with a value to compare.
fn purchased(plan: &Plan) -> impl Iterator<Item = &Row> {
    plan.rows
        .iter()
        .filter(|row| row.level.is_some_and(|level| level > 0.0))
}

/// A bubble's bonus in the saved state: the curve at its level, times the
/// Prisma multiplier when the save marks it.
fn saved_value(row: &Row, plan: &Plan) -> f64 {
    if row.prisma {
        row.with_prisma(plan.multiplier)
    } else {
        row.bonus
    }
}

fn bonus_metric(row: &Row, value: f64) -> Metric {
    Metric::available(bonus_id(row), bonus_name(row), value, true)
}

fn outcome_metric(model: &Model, value: f64) -> Metric {
    Metric::available(
        model.id.to_string(),
        model.name.to_string(),
        (model.outcome)(value),
        false,
    )
}

/// The saved scope: the modelled downstream outcomes, then every purchased
/// bubble's processed bonus.
fn baseline(plan: &Plan) -> Vec<Metric> {
    let mut metrics: Vec<Metric> = MODELS
        .iter()
        .filter_map(|model| {
            let row = purchased(plan).find(|row| row.effect == model.effect)?;
            Some(outcome_metric(model, saved_value(row, plan)))
        })
        .collect();
    for row in purchased(plan) {
        metrics.push(bonus_metric(row, saved_value(row, plan)));
    }
    metrics
}

/// The saved scope with one candidate's bubble set to `value`, and every
/// outcome that reads the bubble recomputed from it.
fn variant(candidate: &Row, baseline: &[Metric], value: f64) -> Vec<Metric> {
    baseline
        .iter()
        .map(|metric| {
            if metric.id == bonus_id(candidate) {
                return metric.with_value(value);
            }
            if let Some(model) = MODELS
                .iter()
                .find(|model| model.id == metric.id && model.effect == candidate.effect)
            {
                return metric.with_value((model.outcome)(value));
            }
            metric.clone()
        })
        .collect()
}

/// The differences between two evaluated scopes.
fn compare(plan: &Plan, candidate: &Row, baseline: &[Metric]) -> RowResult {
    // A ready bubble is previewed with Prisma; a retained one measures the
    // benefit the save keeps. Either way `before` is the state without this
    // bubble's Prisma and `after` the state with it, so a change is always the
    // benefit adding Prisma (or keeping it) buys.
    let before = variant(candidate, baseline, candidate.bonus);
    let after = variant(candidate, baseline, candidate.with_prisma(plan.multiplier));
    let mut changes = Vec::new();
    let mut intermediates = Vec::new();
    for (old, new) in before.iter().zip(&after) {
        let (Some(old_value), Some(new_value)) = (old.value, new.value) else {
            continue;
        };
        if old_value == new_value {
            continue;
        }
        let delta = new_value - old_value;
        let change = Change {
            id: old.id.clone(),
            name: old.name.clone(),
            before: old_value,
            after: new_value,
            delta,
            percent: (old_value != 0.0).then(|| delta / old_value.abs() * 100.0),
            intermediate: old.intermediate,
            lower_is_better: old.lower_is_better,
        };
        if old.intermediate {
            intermediates.push(change);
        } else {
            changes.push(change);
        }
    }
    RowResult {
        row: candidate.clone(),
        comparison: if candidate.prisma {
            Comparison::Retained
        } else {
            Comparison::Added
        },
        before,
        after,
        changes,
        intermediates,
    }
}

/// Which effects the cauldron's purchased bubbles carry, the native systems
/// that read each, and whether this build models a downstream outcome.
fn coverage(plan: &Plan) -> Coverage {
    let mut effects: Vec<CoverageEntry> = Vec::new();
    for row in purchased(plan) {
        match effects.iter_mut().find(|entry| entry.effect == row.effect) {
            Some(entry) => entry.bubbles.push(row.name.clone()),
            None => effects.push(CoverageEntry {
                effect: row.effect.clone(),
                bubbles: vec![row.name.clone()],
                consumers: super::data::consumers(&row.effect).cloned().collect(),
                metrics: MODELS
                    .iter()
                    .filter(|model| model.effect == row.effect)
                    .map(|model| model.id.to_string())
                    .collect(),
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
    let start = if total == 0 {
        Progress {
            fraction: 1.0,
            message: "No bubble in this cauldron can be compared.".into(),
        }
    } else {
        Progress {
            fraction: 0.0,
            message: format!("Comparing 0 / {total} eligible bubbles…"),
        }
    };
    if report(start) == Control::Stop {
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
    use crate::plan;

    fn tiny() -> Plan {
        let save = idleon_save::Save::new(
            serde_json::from_str(include_str!("../tests/prisma-save.json"))
                .expect("the tiny save parses"),
        );
        plan(&save, 0, 0).expect("the tiny save plans")
    }

    /// A save where the STARTUE EXP bubble is the tested outcome: one character,
    /// the bubble at a known level, and Prisma either absent or already present.
    fn outcomes(prismad: bool) -> Plan {
        let prisma = if prismad { "c2," } else { "" };
        let mut options = vec![serde_json::json!(0); 385];
        options[383] = serde_json::json!(120);
        options[384] = serde_json::json!(prisma);
        let save = idleon_save::Save::new(serde_json::json!({
            "Lv0_0": "[100]",
            "CharacterClass_0": 1,
            "CurrentMap_0": 5,
            // DropRate at 10 and STARTUE EXP at 25: two comparable bubbles.
            "CauldronInfo": [[], [], [], [0, 10, 25]],
            "OptLacc": serde_json::Value::Array(options),
        }));
        plan(&save, 0, 3).expect("the outcome save plans")
    }

    fn statue(outcome: &Outcome) -> &RowResult {
        outcome
            .results
            .iter()
            .find(|result| result.row.id == "3:2")
            .expect("the STARTUE EXP candidate")
    }

    fn finish(plan: &Plan) -> Outcome {
        let mut report = |_progress: Progress| Control::Continue;
        run(plan, &mut report).expect("the run finishes")
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
    fn every_row_measures_adding_prisma_even_when_the_save_keeps_it() {
        let outcome = finish(&tiny());
        let ready = outcome
            .results
            .iter()
            .find(|result| result.row.id == "0:2")
            .expect("the ready candidate");
        assert_eq!(ready.comparison, Comparison::Added);
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
        assert_eq!(already.comparison, Comparison::Retained);
        let change = already.change("bubble-bonus:0:0").expect("the bonus moved");
        assert_eq!(
            (change.before, change.after, change.delta),
            (10.0, 20.0, 10.0),
            "a retained bubble shows the benefit the save keeps"
        );
        assert_eq!(change.percent, Some(100.0));
        assert!(ready.changes.is_empty(), "the bonus is an intermediate");
        assert_eq!(ready.intermediates.len(), 1);
    }

    #[test]
    fn the_statue_retention_outcome_moves_with_the_startue_bubble() {
        let outcome = finish(&outcomes(false));
        let result = statue(&outcome);
        assert_eq!(result.row.effect, "StatueStartEXP");
        assert_eq!(result.changes.len(), 1);
        let change = result
            .change("statue-retention")
            .expect("the downstream outcome moved");
        // decay(25, 60, 25) = 7.3529, so half the EXP level costs is retained.
        assert!((change.before - 7.352_941_176_470_588).abs() <= 1e-9);
        assert!((change.after - 14.705_882_352_941_176).abs() <= 1e-9);
        assert!((change.delta - change.before).abs() <= 1e-9);
        assert!(
            change
                .percent
                .is_some_and(|percent| (percent - 100.0).abs() <= 1e-9)
        );
        assert_eq!(change.name, "Statue EXP retained on level-up");
        assert!(!change.intermediate);
        assert_eq!(result.intermediates.len(), 1);
        assert_eq!(result.intermediates[0].id, "bubble-bonus:3:2");

        let retained = finish(&outcomes(true));
        let change = statue(&retained)
            .change("statue-retention")
            .expect("the retained outcome");
        assert_eq!(statue(&retained).comparison, Comparison::Retained);
        assert!((change.before - 7.352_941_176_470_588).abs() <= 1e-9);
        assert!((change.after - 14.705_882_352_941_176).abs() <= 1e-9);
    }

    #[test]
    fn the_ranking_sorts_by_the_selected_metric() {
        let outcome = finish(&tiny());
        let ranked: Vec<&str> = outcome
            .rank("bubble-bonus:0:2")
            .iter()
            .map(|result| result.row.id.as_str())
            .collect();
        assert_eq!(
            ranked,
            ["0:2", "0:0"],
            "the row that moves the metric leads"
        );
        let ranked: Vec<&str> = outcome
            .rank("bubble-bonus:0:0")
            .iter()
            .map(|result| result.row.id.as_str())
            .collect();
        assert_eq!(
            ranked,
            ["0:0", "0:2"],
            "the unchanged row scores zero behind the moved metric"
        );

        let outcome = finish(&outcomes(false));
        let order: Vec<&str> = outcome
            .rank("statue-retention")
            .iter()
            .map(|result| result.row.id.as_str())
            .collect();
        assert_eq!(
            order,
            ["3:2", "3:1"],
            "the measured outcome outranks the unchanged row"
        );
    }

    #[test]
    fn a_selected_metric_shows_a_zero_delta_when_it_did_not_move() {
        let outcome = finish(&outcomes(false));
        let result = statue(&outcome);
        let view = result
            .view("bubble-bonus:3:1")
            .expect("an unchanged metric still has a view");
        assert_eq!(view.delta, 0.0);
        assert_eq!(view.percent, Some(0.0));
        assert_eq!(view.before, view.after);
        assert!(view.intermediate);
        assert!(result.view("nope").is_none());
    }

    #[test]
    fn the_search_matches_names_effects_and_changed_metrics() {
        let outcome = finish(&tiny());
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

        let outcome = finish(&outcomes(false));
        let ids = |query: &str| -> Vec<String> {
            outcome
                .search(query)
                .iter()
                .map(|result| result.row.id.clone())
                .collect()
        };
        assert_eq!(ids("startue"), ["3:2"]);
        assert_eq!(ids("retained"), ["3:2"]);
        assert!(ids("statue retention").is_empty(), "names match as typed");
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
    fn every_effect_lists_its_native_consumers_and_whether_a_model_reads_it() {
        let outcome = finish(&tiny());
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
        assert!(!strength.modelled());
        assert!(
            strength
                .consumers
                .iter()
                .any(|consumer| consumer.method == "_customBlock_TotalStats"),
            "upstream traces TotalStats reading TotalSTR"
        );

        let outcome = finish(&outcomes(false));
        let coverage = &outcome.coverage;
        assert_eq!(coverage.total(), 2);
        assert_eq!(coverage.modelled(), 1);
        assert_eq!(coverage.missing(), 1);
        let statue = coverage
            .effects
            .iter()
            .find(|entry| entry.effect == "StatueStartEXP")
            .expect("the statue effect");
        assert!(statue.modelled());
        assert_eq!(statue.metrics, ["statue-retention"]);
        assert!(
            statue
                .consumers
                .iter()
                .any(|consumer| consumer.method == "_event_StatueBars"),
            "upstream traces the statue bar event reading the effect"
        );
    }
}
