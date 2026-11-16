//! The optimizer search invariants: reproducibility, cancelling, the scope
//! counts, the plan text and the Amber belt check.

use idleon_common::Control;
use idleon_import::import;
use idleon_spelunking::{
    Account, AmberSearchOptions, Clear, Enabled, Inputs, Objective, Requested, Save, ScopeRequest,
    SearchOptions, amber_search, plan_text, result_text, search, search_space,
};

/// The fixture save's inputs under a neutral account.
fn inputs() -> Inputs {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("the fixture is committed");
    let imported = import(&text).expect("the fixture imports");
    let save = Save::from_parts(&imported.save).expect("the fixture carries Spelunk rows");
    save.inputs(&Account::default())
}

/// A search over two elixir ids only, every other dimension fixed.
fn belt_options() -> SearchOptions {
    let enabled = Enabled {
        party: false,
        modes: false,
        clears: false,
        speeds: false,
        memorized: false,
        tools: false,
        entry: false,
        ..Enabled::default()
    };
    let scope = ScopeRequest {
        enabled: Some(enabled),
        elixirs: Some(vec![0, 1]),
        clears: Some(vec![Clear::First]),
        ..ScopeRequest::default()
    };
    SearchOptions {
        objective: Objective::Depth,
        screening: 1,
        trials: 2,
        seed: 1,
        scope,
    }
}

#[test]
fn the_search_is_reproducible_and_cancellable() {
    let inputs = inputs();
    let baseline = Requested::default();
    let first = search(&inputs, &baseline, &belt_options(), &mut |_| {
        Control::Continue
    })
    .expect("the search runs")
    .expect("the search is not cancelled");
    let second = search(&inputs, &baseline, &belt_options(), &mut |_| {
        Control::Continue
    })
    .expect("the search runs")
    .expect("the search is not cancelled");
    assert_eq!(first.rows, second.rows);
    assert_eq!(first.winner, second.winner);
    let cancelled = search(&inputs, &baseline, &belt_options(), &mut |_| Control::Stop)
        .expect("a cancelled search is not an error");
    assert!(cancelled.is_none());
}

#[test]
fn a_belt_scope_enumerates_the_count_it_promises() {
    let inputs = inputs();
    let result = search(&inputs, &Requested::default(), &belt_options(), &mut |_| {
        Control::Continue
    })
    .expect("the belt search runs")
    .expect("the belt search is not cancelled");
    // Two elixir ids fill the five physical slots under the duplicate limit: 2^5.
    assert_eq!(result.configurations, 32);
    // The saved belt is outside the pool, so the baseline is the one reference
    // measured on top of the exhaustive count.
    assert_eq!(result.screened, 33);
    assert!(!result.rows.is_empty());
}

#[test]
fn the_plan_text_names_the_recommendation() {
    let inputs = inputs();
    let result = search(&inputs, &Requested::default(), &belt_options(), &mut |_| {
        Control::Continue
    })
    .expect("the search runs")
    .expect("the search is not cancelled");
    let winner = result.winner.as_ref().expect("a winner");
    let plan = plan_text(&inputs, &winner.summary.config);
    assert!(plan.contains("Pebble Cove"));
    assert!(plan.contains("Character 1"));
    assert!(plan.contains("belt"));
    let text = result_text(&inputs, &result);
    assert!(text.contains("Best measured"));
    assert!(text.contains(&plan));
}

#[test]
fn the_amber_search_needs_an_orange_elixir() {
    let inputs = inputs();
    let requested = Requested {
        elixirs: Some(vec![-1; 5]),
        ..Requested::default()
    };
    let error = amber_search(
        &inputs,
        &requested,
        &AmberSearchOptions::default(),
        &mut |_| Control::Continue,
    )
    .expect_err("the belt has no orange");
    assert_eq!(
        error.to_string(),
        "Amber farming needs at least one equipped orange elixir. Apply the Amber belt preset or choose a belt."
    );
}

#[test]
fn a_large_party_scope_counts_every_combination() {
    let inputs = inputs();
    let scope = ScopeRequest {
        enabled: Some(Enabled::default()),
        party_min: Some(1),
        party_max: Some(2),
        characters: Some(vec![0, 1, 2]),
        ..ScopeRequest::default()
    };
    let (count, _) = search_space(&inputs, &Requested::default(), &scope).expect("a valid scope");
    // 3 ordered singles + 6 ordered pairs, every other dimension at its default.
    assert!(count > 9, "the count includes the unfixed dimensions");
}
