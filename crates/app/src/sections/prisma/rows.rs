//! The rows the impact and coverage tables draw, formatted once per stored run,
//! metric, filter and width instead of on every frame.
//!
//! Ranking by a metric sorts the run's rows and the filter walks every bubble's
//! names and changes; both happen in [`Derived::sync`], which keeps the key it
//! last built from, so a frame that changes none of them only compares.

use idleon_prisma::{Change, Consumer, Outcome, RowResult, Status};
use idleon_ui::format;
use idleon_ui::widgets::{Choice, Tone};

/// One bubble's row in the impact table, ready to draw.
pub(super) struct Row {
    /// `<cauldron>-<index>`, the suffix of every cell's element id.
    pub(super) key: String,
    pub(super) name: String,
    pub(super) effect: String,
    /// The bubble cell's accessible label.
    pub(super) spoken: String,
    pub(super) level: String,
    pub(super) status: &'static str,
    pub(super) status_tone: Tone,
    pub(super) bonus: String,
    pub(super) bonus_delta: Option<String>,
    pub(super) bonus_spoken: String,
    pub(super) change: String,
    pub(super) change_name: Option<String>,
    pub(super) change_tone: Tone,
    pub(super) change_spoken: String,
}

/// One effect's row in the coverage table.
pub(super) struct CoverageRow {
    pub(super) effect: String,
    pub(super) bubbles: String,
    pub(super) consumers: String,
    pub(super) modelled: bool,
    pub(super) models: String,
    pub(super) spoken: String,
}

/// What the run's figures read, kept per stored run.
#[derive(Default)]
pub(super) struct Counts {
    pub(super) compared: usize,
    pub(super) changed: usize,
    pub(super) modelled: usize,
    pub(super) total: usize,
    pub(super) missing: usize,
}

/// The cached rows, choices and counts.
#[derive(Default)]
pub(crate) struct Derived {
    rows_key: Option<(u64, Option<String>, String, bool)>,
    run_key: Option<u64>,
    pub(super) rows: Vec<Row>,
    pub(super) coverage: Vec<CoverageRow>,
    /// `Bubble order`, then every metric the run measured.
    pub(super) metrics: Vec<Choice<Option<String>>>,
    pub(super) counts: Counts,
}

impl Derived {
    /// Bring the rows in line with the stored run `generation`, the ranking
    /// metric, the filter and the width.
    pub(super) fn sync(
        &mut self,
        outcome: Option<&Outcome>,
        generation: u64,
        metric: Option<&str>,
        query: &str,
        compact: bool,
    ) {
        // No stored result is its own generation, so a cleared run empties
        // the tables.
        let generation = outcome.map_or(0, |_| generation + 1);
        if self.run_key != Some(generation) {
            self.run_key = Some(generation);
            self.metrics = metric_choices(outcome);
            self.coverage = outcome.map(coverage_rows).unwrap_or_default();
            self.counts = outcome.map(counts).unwrap_or_default();
        }
        let key = (
            generation,
            metric.map(str::to_string),
            query.to_string(),
            compact,
        );
        if self.rows_key.as_ref() == Some(&key) {
            return;
        }
        self.rows_key = Some(key);
        let Some(outcome) = outcome else {
            self.rows = Vec::new();
            return;
        };
        let ranked = match metric {
            None => outcome.bubbles(),
            Some(metric) => outcome.rank(metric),
        };
        self.rows = ranked
            .into_iter()
            .filter(|result| result.matches(query))
            .map(|result| row(result, metric, compact))
            .collect();
    }
}

fn metric_choices(outcome: Option<&Outcome>) -> Vec<Choice<Option<String>>> {
    let mut choices = vec![Choice::new(None, "Bubble order")];
    if let Some(outcome) = outcome {
        choices.extend(
            outcome
                .metrics()
                .iter()
                .map(|metric| Choice::new(Some(metric.id.clone()), metric.name.clone())),
        );
    }
    choices
}

fn counts(outcome: &Outcome) -> Counts {
    Counts {
        compared: outcome.results.len(),
        changed: outcome
            .results
            .iter()
            .filter(|result| !result.changes.is_empty())
            .count(),
        modelled: outcome.coverage.modelled(),
        total: outcome.coverage.total(),
        missing: outcome.coverage.missing(),
    }
}

fn row(result: &RowResult, metric: Option<&str>, compact: bool) -> Row {
    let level = match result.row.level {
        Some(level) => format::number(level),
        None => "unavailable".into(),
    };
    let status = result.row.status;
    let status_tone = match status {
        Status::Ready => Tone::Success,
        Status::Already => Tone::Info,
        Status::Ineligible => Tone::Neutral,
        Status::Unavailable | Status::Unpurchased => Tone::Warning,
    };

    // The bubble's own processed bonus: the value without this bubble's
    // Prisma and the value with it, plus the delta the flip buys.
    let bonus_metric = format!("bubble-bonus:{}", result.row.id);
    let bonus_change = result
        .intermediates
        .first()
        .cloned()
        .or_else(|| result.view(&bonus_metric));
    let bonus = match &bonus_change {
        Some(change) => format!(
            "{} → {}",
            format::number(change.before),
            format::number(change.after)
        ),
        None => "unavailable".into(),
    };
    let bonus_delta = bonus_change.map(|change| change_text(&change, true));
    let bonus_spoken = match &bonus_delta {
        Some(delta) => format!("{bonus} ({delta})"),
        None => bonus.clone(),
    };

    // The downstream delta: the selected metric when one ranks the rows, else
    // the first measured change the build found.
    let change = match metric {
        Some(metric) => result.view(metric),
        None => result.changes.first().cloned(),
    };
    let change_tone = match &change {
        Some(change) if change.delta > 0.0 => Tone::Success,
        Some(change) if change.delta < 0.0 => Tone::Warning,
        _ => Tone::Neutral,
    };
    let (change_label, change_name) = match &change {
        Some(change) => (
            change_text(change, compact),
            metric.is_none().then(|| change.name.clone()),
        ),
        None => ("No measured change".to_string(), None),
    };
    let change_spoken = match &change_name {
        Some(name) => format!("{change_label} · {name}"),
        None => change_label.clone(),
    };

    Row {
        key: format!("{}-{}", result.row.cauldron, result.row.index),
        name: result.row.name.clone(),
        effect: result.row.effect.clone(),
        spoken: format!(
            "{} · {} · {}",
            result.row.name,
            result.row.effect,
            result.comparison.label()
        ),
        level,
        status: status.label(),
        status_tone,
        bonus,
        bonus_delta,
        bonus_spoken,
        change: change_label,
        change_name,
        change_tone,
        change_spoken,
    }
}

fn change_text(change: &Change, compact: bool) -> String {
    let percent = change
        .percent
        .map(|percent| format!(" ({percent:+.2}%)"))
        .unwrap_or_default();
    if compact {
        format!("{}{percent}", format::number(change.delta))
    } else {
        format!(
            "{}{percent} · {} → {}",
            format::number(change.delta),
            format::number(change.before),
            format::number(change.after)
        )
    }
}

/// Every effect in the cauldron, the native systems that read it and the
/// downstream models in this build that read it too.
fn coverage_rows(outcome: &Outcome) -> Vec<CoverageRow> {
    outcome
        .coverage
        .effects
        .iter()
        .map(|entry| {
            let models = if entry.modelled() {
                entry
                    .metrics
                    .iter()
                    .map(|id| {
                        outcome
                            .metrics()
                            .iter()
                            .find(|metric| metric.id == *id)
                            .map(|metric| metric.name.clone())
                            .unwrap_or_else(|| id.clone())
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                String::new()
            };
            let spoken = if entry.modelled() {
                format!("Modelled · {models}")
            } else {
                "Missing".to_string()
            };
            CoverageRow {
                effect: entry.effect.clone(),
                bubbles: entry.bubbles.join(", "),
                consumers: consumer_summary(&entry.consumers),
                modelled: entry.modelled(),
                models,
                spoken,
            }
        })
        .collect()
}

/// The native reference a consumer row names, trimmed the way the detail list
/// prints it.
fn consumer_text(consumer: &Consumer) -> String {
    let method = consumer
        .method
        .replace("_customBlock_", "")
        .replace("_customEvent_", "")
        .replace("_event_", "");
    match &consumer.mode {
        Some(mode) => format!("{method} · {}", mode.label()),
        None => method,
    }
}

/// The first few consumers, with the rest counted.
fn consumer_summary(consumers: &[Consumer]) -> String {
    const SHOWN: usize = 3;
    if consumers.is_empty() {
        return "No direct native reference".into();
    }
    let mut parts: Vec<String> = consumers.iter().take(SHOWN).map(consumer_text).collect();
    if consumers.len() > SHOWN {
        parts.push(format!("+{} more", consumers.len() - SHOWN));
    }
    parts.join(", ")
}
