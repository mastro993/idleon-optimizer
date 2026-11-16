//! Observation Unlocks on the committed account fixture.

use idleon_common::Control;
use idleon_research::model::Effort;
use idleon_research::unlock::{self, Filter};

fn fixture_save() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

#[test]
fn the_filters_split_the_usable_rows() {
    let imported = idleon_import::import(&fixture_save()).expect("the fixture imports");
    let outcome = unlock::analyze(
        &imported.simulation,
        &imported.context,
        Effort::Balanced,
        &mut |_| Control::Continue,
    )
    .expect("the analysis finishes");
    assert!(
        outcome.missing.is_empty(),
        "the fixture carries every roll field: {:?}",
        outcome.missing
    );
    // Usable rows come first, each group by score, best first.
    assert!(outcome.rows.windows(2).all(|pair| {
        let (left, right) = (&pair[0], &pair[1]);
        (left.usable && !right.usable)
            || (left.usable == right.usable
                && left.score.unwrap_or(0.0) >= right.score.unwrap_or(0.0))
    }));
    let all = outcome.filtered(Filter::All).len();
    let usable = outcome.filtered(Filter::Usable).len();
    let waiting = outcome.filtered(Filter::Waiting).len();
    assert_eq!(all, outcome.rows.len());
    assert_eq!(usable + waiting, all);
    assert_eq!(usable, outcome.rows.iter().filter(|row| row.usable).count());
    // Every priority the table shows is drawn from the best score.
    let best = outcome.best_score();
    assert!(best > 0.0, "the fixture ranks at least one observation");
    assert!(
        outcome
            .rows
            .iter()
            .any(|row| row.priority(best) == unlock::Priority::Best)
    );
}

/// Without the save's roll fields the estimate reports them instead of
/// assuming a full day of rolls and a fresh streak.
#[test]
fn missing_roll_data_is_reported() {
    let imported = idleon_import::import(&fixture_save()).expect("the fixture imports");
    let mut context = imported.context.clone();
    context.observation_rolls_remaining = None;
    context.observation_failed_rolls = None;
    let outcome = unlock::analyze(
        &imported.simulation,
        &context,
        Effort::Balanced,
        &mut |_| Control::Continue,
    )
    .expect("the analysis finishes");
    assert_eq!(outcome.missing.len(), 2, "{:?}", outcome.missing);
    assert!(outcome.missing[0].starts_with("Research[7][2]: "));
    assert!(outcome.missing[1].starts_with("OptLacc[514]: "));
    assert!(
        outcome.rows.iter().all(|row| row.chance_today.is_none()
            && row.expected_days.is_none()
            && row.score.is_none()),
        "no row guesses a probability without the roll data"
    );
    assert_eq!(outcome.best_score(), 0.0);
    assert!(
        outcome
            .rows
            .iter()
            .all(|row| row.priority(0.0) != unlock::Priority::Best)
    );
}

/// The run stops cleanly when the task runner asks it to.
#[test]
fn a_cancelled_run_stores_nothing() {
    let imported = idleon_import::import(&fixture_save()).expect("the fixture imports");
    let stopped = unlock::analyze(
        &imported.simulation,
        &imported.context,
        Effort::Quick,
        &mut |_| Control::Stop,
    );
    assert!(stopped.is_none());
}
