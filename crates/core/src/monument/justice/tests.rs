//! Worksheet behaviour: the effect table, undoable edits and the advisor's own
//! explanation.
//!
//! Formula parity is in `crates/core/tests/monument.rs`, which compares the port
//! against the upstream calculator's output for every case.

use super::*;

fn fresh() -> State {
    State {
        round: 0,
        health: 5,
        coins: 61,
        popularity: 10,
        dismissals: 4,
        chests: 0,
        effects: BTreeMap::new(),
    }
}

fn worksheet() -> Worksheet {
    let mut worksheet = Worksheet::default();
    worksheet.reset(fresh());
    worksheet
}

#[test]
fn every_effect_carries_its_upstream_key() {
    assert_eq!(Effect::ALL.len(), EFFECT_ROWS.len());
    for effect in Effect::ALL {
        assert_eq!(
            serde_json::to_value(effect).expect("the effect serialises"),
            serde_json::Value::String(effect.key().to_string()),
            "{effect:?} serialises as its key"
        );
        assert!(!effect.name().is_empty() && !effect.help().is_empty());
    }
    assert!(Effect::PermaPop.flag());
    assert!(!Effect::CryptoInvest.flag());
}

#[test]
fn the_case_table_names_its_visitors() {
    let cases = cases();
    assert_eq!(cases.len(), 90);
    assert_eq!(cases[0].visitor, 0);
    assert_eq!(cases[0].name, "Little Timmy");
    assert!(!cases[0].text.contains('_'), "the text reads as words");
    assert_eq!(cases[15].visitor, 4);
    assert_eq!(cases[15].name, "The Harbinger");
}

#[test]
fn a_case_fills_its_coin_and_popularity_range_from_the_state() {
    let text = case_text(&fresh(), 0).expect("the case exists").text;
    assert!(text.contains('1'), "the coin range is filled: {text}");
    assert!(!text.contains('{') && !text.contains('}'));
    assert_eq!(
        case_text(&fresh(), 999),
        Err(Error::UnknownCase),
        "an id with no row is rejected"
    );
}

#[test]
fn the_effects_editor_is_undoable() {
    let mut worksheet = worksheet();
    assert!(!worksheet.can_undo(), "a fresh worksheet has no trail");
    assert!(!worksheet.undo());

    let mut checked = worksheet.state.clone();
    checked.effects.insert(Effect::PermaPop, 1);
    worksheet.edit(checked.clone());
    assert_eq!(worksheet.state, checked);
    assert_eq!(delayed_effects(&worksheet.state).len(), 1);

    // A second edit (the value beside the box) is its own step.
    let mut raised = checked.clone();
    raised.effects.insert(Effect::CryptoInvest, 4);
    worksheet.edit(raised.clone());
    assert_eq!(worksheet.state, raised);

    assert!(worksheet.undo());
    assert_eq!(worksheet.state, checked, "the value edit came undone");
    assert!(worksheet.undo());
    assert_eq!(worksheet.state, fresh(), "the toggle came undone");
    assert!(delayed_effects(&worksheet.state).is_empty());
}

#[test]
fn a_recorded_response_and_its_next_case_are_each_undoable() {
    let mut worksheet = worksheet();
    let report = responses(&worksheet.state, 0).expect("the first case is valid");
    let approve = report
        .responses
        .iter()
        .find(|response| response.action == Action::Approve)
        .expect("the first case can be approved");
    let outcome = approve.outcomes[0].state.clone();
    worksheet.record(outcome.clone());
    assert!(worksheet.awaiting);
    assert_ne!(worksheet.state, fresh());

    assert!(worksheet.undo());
    assert!(!worksheet.awaiting, "the recorded response came undone");
    assert_eq!(worksheet.state, fresh());

    worksheet.record(outcome);
    assert_eq!(
        worksheet.open_next(),
        Ok(Vec::new()),
        "the first case settles nothing"
    );
    assert!(!worksheet.awaiting);
    assert_ne!(worksheet.state.round, fresh().round);
    assert!(worksheet.undo(), "the opened case came undone");
    assert!(worksheet.awaiting, "undo restores the recorded response");
}

#[test]
fn selecting_a_case_is_not_an_undoable_step() {
    let mut worksheet = worksheet();
    worksheet.select(42);
    assert_eq!(worksheet.case_id, 42);
    assert!(!worksheet.undo(), "selection alone leaves no trail");
    worksheet.select(999);
    assert_eq!(worksheet.case_id, 42, "an id with no row is ignored");
}

#[test]
fn the_trail_keeps_the_thirty_newest_steps() {
    let mut worksheet = worksheet();
    for health in 1..=40 {
        let mut next = worksheet.state.clone();
        next.health = health;
        worksheet.edit(next);
    }
    assert_eq!(worksheet.history.len(), HISTORY);
    while worksheet.undo() {}
    assert_eq!(worksheet.state.health, 10, "the oldest kept step is 10");
}

#[test]
fn reset_starts_over_without_a_trail() {
    let mut worksheet = worksheet();
    worksheet.select(7);
    let mut next = worksheet.state.clone();
    next.coins = 5;
    worksheet.edit(next);
    worksheet.reset(fresh());
    assert_eq!(worksheet.state, fresh());
    assert_eq!(worksheet.case_id, 0);
    assert!(!worksheet.awaiting);
    assert!(!worksheet.undo(), "the trail starts empty again");
}

#[test]
fn the_advisor_explains_its_choice() {
    let report = responses(&fresh(), 0).expect("the first case is valid");
    assert_eq!(report.row.id, 0);
    assert!(!report.ambiguous);
    assert!(report.heuristic.contains("Survival-first heuristic"));
    let recommended = report.recommended.expect("a legal response exists");
    let chosen = report
        .responses
        .iter()
        .find(|response| response.action == recommended)
        .expect("the suggestion is one of the responses");
    assert!(chosen.blocked.is_none());
    assert!(chosen.survival > 0.0);
    // Dismissal is legal with a dismissal left and blocked without one.
    let dismiss = |state: &State| {
        let report = responses(state, 0).expect("the first case is valid");
        report
            .responses
            .into_iter()
            .find(|response| response.action == Action::Dismiss)
            .expect("dismiss is always reported")
    };
    assert!(dismiss(&fresh()).blocked.is_none());
    let spent = State {
        dismissals: 0,
        ..fresh()
    };
    let blocked = dismiss(&spent);
    assert_eq!(blocked.blocked.as_deref(), Some("No dismissals remain."));
    assert!(blocked.outcomes.is_empty());
}

#[test]
fn a_guaranteed_death_has_no_suggestion() {
    let dead = State {
        health: 0,
        ..fresh()
    };
    let report = responses(&dead, 0).expect("the ended story still reports");
    assert_eq!(report.recommended, None);
    for response in &report.responses {
        assert_eq!(response.blocked.as_deref(), Some("The story has ended."));
    }
    assert_eq!(open_case(&dead, 0), Err(Error::Ended));
}

#[test]
fn an_ambiguous_case_labels_its_outcomes_by_what_changed() {
    let report = responses(&fresh(), 44).expect("the gift case is valid");
    assert!(report.ambiguous, "several rows share the visitor's wording");
    for response in &report.responses {
        for outcome in &response.outcomes {
            assert!(
                outcome.label.contains("coins")
                    || outcome.label.contains("popularity")
                    || outcome.label.contains("chest")
                    || outcome.label.contains("dismissal")
                    || outcome.label.contains("health")
                    || outcome.label == "No resource change",
                "the label names the change: {}",
                outcome.label
            );
        }
    }
}

#[test]
fn opening_a_case_returns_the_settlements_it_made() {
    let state = State {
        health: 5,
        coins: 100,
        effects: BTreeMap::from([(Effect::StudentLoanInvest, 20)]),
        ..fresh()
    };
    let opened = open_case(&state, 2).expect("Big Biz's case opens");
    assert_eq!(opened.state.coins, 140);
    assert_eq!(opened.notes, ["Student loan repaid."]);
    assert!(
        !opened
            .state
            .effects
            .contains_key(&Effect::StudentLoanInvest)
    );
}
