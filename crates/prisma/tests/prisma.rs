//! The Prisma plan, search and coverage on purpose-built and synthetic saves.
//!
//! The tiny save is purpose-built for the status ladder and the ranking; the
//! outcomes save puts the STARTUE EXP bubble in a cauldron with no noisy
//! neighbours; the committed synthetic account is the realistic run.

use idleon_common::{Control, Progress};
use idleon_import::import;
use idleon_prisma::{self as prisma, Comparison, Outcome, Plan};
use idleon_save::Save;

fn save(path: &str) -> Save {
    let text = std::fs::read_to_string(path).expect("the save is committed");
    Save::new(serde_json::from_str(&text).expect("the save is valid JSON"))
}

fn fixture() -> Save {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    import(&text).expect("the fixture imports").save
}

fn tiny() -> Save {
    save(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/prisma-save.json"
    ))
}

fn outcomes() -> Save {
    save(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/prisma-outcomes-save.json"
    ))
}

fn run(plan: &Plan) -> Outcome {
    let mut report = |_progress: Progress| Control::Continue;
    prisma::run(plan, &mut report).expect("the run finishes")
}

#[test]
fn the_tiny_save_plans_the_power_cauldron() {
    let plan = prisma::plan(&tiny(), 0, 0).expect("the tiny save plans");
    assert_eq!(plan.comparable(), 2);
    assert_eq!(plan.rows.len(), 34);
    assert_eq!(plan.cauldron_label(), "Power");
}

#[test]
fn the_outcomes_save_has_two_comparable_rows() {
    let plan = prisma::plan(&outcomes(), 0, 3).expect("the outcomes save plans");
    assert_eq!(plan.comparable(), 2);
}

#[test]
fn the_statue_retention_outcome_ranks_the_startue_bubble_first() {
    let plan = prisma::plan(&fixture(), 0, 3).expect("the fixture plans");
    let outcome = run(&plan);
    let ranked: Vec<&str> = outcome
        .rank("statue-retention")
        .iter()
        .map(|result| result.row.id.as_str())
        .collect();
    assert_eq!(
        ranked.first(),
        Some(&"3:2"),
        "STARTUE EXP is the only bubble a modelled outcome reads"
    );
    assert!(
        ranked.len() > 20,
        "the rest of the cauldron keeps a zero score"
    );
    let statue = outcome
        .results
        .iter()
        .find(|result| result.row.id == "3:2")
        .expect("the STARTUE EXP row");
    assert_eq!(statue.comparison, Comparison::Retained);
    let change = statue
        .change("statue-retention")
        .expect("the retained retention benefit");
    assert!((change.before - 24.925_280_199_252_796).abs() <= 1e-9);
    assert!((change.after - 50.0).abs() <= 1e-9);
    assert!((change.delta - 25.074_719_800_747_204).abs() <= 1e-9);
    assert!(
        change
            .percent
            .is_some_and(|percent| (percent - 100.599_550_337_247_1).abs() <= 1e-9)
    );
    // Every other row still shows the outcome, with a zero delta.
    for result in &outcome.results {
        let view = result
            .view("statue-retention")
            .expect("every row shows the outcome");
        if result.row.id != "3:2" {
            assert_eq!(view.delta, 0.0, "{}: unchanged outcome", result.row.id);
        }
    }
}

#[test]
fn the_fixture_search_reports_every_comparable_bubble_and_its_coverage() {
    let plan = prisma::plan(&fixture(), 0, 0).expect("the fixture plans");
    let outcome = run(&plan);
    assert_eq!(outcome.results.len(), plan.comparable());
    assert!(
        outcome.results.len() > 20,
        "the fixture has most bubbles owned"
    );
    assert!(outcome.baseline.len() > 20);
    for result in &outcome.results {
        assert!(
            !result.intermediates.is_empty(),
            "{}: a comparison changes its bubble bonus",
            result.row.id
        );
        assert_eq!(result.intermediates.len(), 1);
        assert!(
            result.changes.is_empty(),
            "{}: Power has no modelled downstream outcome",
            result.row.id
        );
        assert!(result.matches(&result.row.name));
        assert!(result.matches(&result.row.effect));
    }
    assert_eq!(outcome.coverage.modelled(), 0);
    assert_eq!(outcome.coverage.missing(), outcome.coverage.total());
    assert!(
        outcome.coverage.total() >= 29,
        "the fixture carries many effects"
    );
    for entry in &outcome.coverage.effects {
        if let Some(consumer) = entry.consumers.first() {
            assert!(!consumer.method.is_empty(), "{}", entry.effect);
        }
    }

    let plan = prisma::plan(&fixture(), 0, 3).expect("the fixture plans");
    let outcome = run(&plan);
    assert_eq!(outcome.coverage.modelled(), 1);
    let statue = outcome
        .coverage
        .effects
        .iter()
        .find(|entry| entry.effect == "StatueStartEXP")
        .expect("the STARTUE EXP effect");
    assert!(statue.modelled());
    assert_eq!(statue.metrics, ["statue-retention"]);
    assert_eq!(statue.bubbles, ["Startue Exp"]);
    assert!(
        statue
            .consumers
            .iter()
            .any(|consumer| consumer.method == "_event_StatueBars")
    );
    let unmodelled = outcome
        .coverage
        .effects
        .iter()
        .find(|entry| !entry.modelled())
        .expect("a missing downstream system");
    assert!(unmodelled.metrics.is_empty());
}
