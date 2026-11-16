//! The board section planner on the committed save: every row's board is legal
//! for the sections it buys, the beam width changes the search, and one input
//! gives one plan.

mod common;

use common::{bought_sections, save};
use idleon_jelly::board;
use idleon_jelly::planner::{Plan, PlanOptions, Quality};

/// A fast plan for one section.
fn single() -> PlanOptions {
    PlanOptions {
        tokens: Some(1),
        quality: Quality::Fast,
        seed: 5,
        ..PlanOptions::default()
    }
}

/// A fast three-section plan at one beam width.
fn beam(beam_width: usize) -> PlanOptions {
    PlanOptions {
        tokens: Some(3),
        allow_hypothetical_tokens: true,
        quality: Quality::Fast,
        seed: 5,
        beam_width,
        ..PlanOptions::default()
    }
}

/// The planner's own rules: every row buys unpurchased sections and fills a
/// legal board for the save those sections make.
fn assert_plan_rules(jelly: &idleon_jelly::Jelly, plan: &Plan, name: &str) {
    assert!(
        plan.tokens <= plan.requested_tokens,
        "{name}: more sections planned than the request asked for"
    );
    assert!(
        plan.tokens <= idleon_jelly::data().plots.len(),
        "{name}: more sections planned than the board has"
    );
    for rows in [&plan.plots, &plan.sequences] {
        for row in rows.iter() {
            assert!(
                row.plot_ids.iter().all(|plot| !jelly.plots.contains(plot)),
                "{name}: a row buys a section the save already owns"
            );
            for slot in &row.new_slots {
                assert!(
                    !board::unlocked_slots(jelly)[*slot],
                    "{name}: a new slot was already unlocked"
                );
            }
            assert_eq!(
                row.new_usable_slots,
                row.new_slots.len(),
                "{name}: the new slot count matches the slots"
            );
            let mut expanded = jelly.clone();
            expanded.plots.extend(row.plot_ids.iter().copied());
            board::validate(&expanded, &row.layout)
                .unwrap_or_else(|reason| panic!("{name}: the row board is illegal: {reason}"));
        }
    }
    if plan.tokens == 0 {
        assert!(
            plan.sequences.is_empty(),
            "{name}: a zero-token plan ranks no sequence"
        );
        assert_eq!(
            plan.final_layout, plan.baseline_layout,
            "{name}: no recommendation keeps the baseline"
        );
    }
}

#[test]
#[ignore = "long simulation: CI runs it on main only"]
fn a_narrow_beam_searches_different_sequences() {
    let save = bought_sections();
    let narrow = Plan::run(&save, beam(2));
    let wide = Plan::run(&save, beam(8));
    assert_eq!(narrow.beam_width, 2);
    assert_eq!(wide.beam_width, 8);
    assert_plan_rules(&save, &narrow, "narrow");
    assert_plan_rules(&save, &wide, "wide");
    let ids = |plan: &Plan| {
        plan.sequences
            .iter()
            .map(|row| row.plot_ids.clone())
            .collect::<Vec<_>>()
    };
    assert_ne!(
        ids(&narrow),
        ids(&wide),
        "the beam width must change which section combinations the search keeps"
    );
}

#[test]
#[ignore = "long simulation: CI runs it on main only"]
fn the_plan_is_reproducible() {
    let jelly = bought_sections();
    let first = Plan::run(&jelly, beam(2));
    let second = Plan::run(&jelly, beam(2));
    assert_eq!(first, second, "one input must give one plan");
    let mut reseeded = beam(2);
    reseeded.seed += 1;
    let reseeded = Plan::run(&jelly, reseeded);
    assert_ne!(
        first, reseeded,
        "the seed moves the shared-seed trials the plan reports"
    );
}

#[test]
fn a_zero_token_plan_keeps_the_saved_board() {
    let jelly = bought_sections();
    let plan = Plan::run(
        &jelly,
        PlanOptions {
            tokens: Some(0),
            ..single()
        },
    );
    assert_eq!(plan.tokens, 0);
    assert_plan_rules(&jelly, &plan, "zero");
}

#[test]
fn a_stopped_plan_reports_nothing() {
    let jelly = save();
    let stopped = Plan::run_with(&jelly, single(), |_| false);
    assert!(stopped.is_none(), "the first report stops the plan");
}
