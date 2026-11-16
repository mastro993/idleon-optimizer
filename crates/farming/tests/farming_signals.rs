//! The Farming Planner's signals, thresholds and curves on `fixtures/data.json`.

use idleon_farming::Farm;
use idleon_import::import;

/// The fixture save, imported the way a user's save is.
fn imported() -> Farm {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let save = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    import(&save).expect("the fixture save imports").farming
}

#[test]
fn the_gmo_thresholds_feed_the_crop_counts() {
    assert_eq!(
        idleon_farming::data().gmo_thresholds,
        [200.0, 1_000.0, 2_500.0, 10_000.0, 100_000.0]
    );
    let farm = imported();
    assert_eq!(
        farm.crop_threshold_counts(),
        farm.snapshot().crop_counts,
        "the counts are the ones the multipliers read"
    );
}

#[test]
fn the_chain_reaches_the_target_it_names() {
    let farm = imported();
    let snapshot = farm.snapshot();
    let (target, chain) = idleon_farming::data()
        .seeds
        .iter()
        .filter_map(|family| {
            let chain = farm.evolution_chain(family.end, &snapshot)?;
            Some((family.end, chain))
        })
        .max_by_key(|(_, chain)| chain.step_count())
        .expect("a chain");
    assert!(
        chain.step_count() >= 20,
        "the longest chain has 20 steps or more"
    );
    // Every depth from the deepest plot to the target is one step, in order,
    // so the chain is complete and its steps are the crops to grow in turn.
    let family_start = idleon_farming::data().seeds[chain.family].start;
    let first = chain.steps.first().expect("a first step").predecessor_depth;
    for (offset, step) in chain.steps.iter().enumerate() {
        assert_eq!(step.predecessor_depth, first + offset as u32);
        assert_eq!(
            step.target_crop,
            family_start + i64::from(step.predecessor_depth) + 1
        );
    }
    assert_eq!(chain.steps.last().unwrap().target_crop, target);
    let hardest = chain.hardest_steps(20);
    assert_eq!(hardest.len(), 20);
    assert!(
        hardest
            .windows(2)
            .all(|pair| pair[0].plan.expected_seconds >= pair[1].plan.expected_seconds),
        "the slowest stages come first"
    );
}

#[test]
fn the_overgrowth_advice_follows_each_plots_own_target() {
    let farm = imported();
    let snapshot = farm.snapshot();
    for (index, curve) in farm.overgrowth_curves(&snapshot).into_iter().enumerate() {
        let best = curve.best.target;
        let best_row = curve.rows[best as usize];
        let what = format!("plot {} advice", index + 1);
        match curve.advice() {
            idleon_farming::OvergrowthAdvice::Harvest => assert!(
                curve.current_og >= best,
                "{what}: harvest only at or past the target"
            ),
            idleon_farming::OvergrowthAdvice::Wait(seconds) => {
                assert!(curve.current_og < best, "{what}: waits below the target");
                assert_eq!(seconds, best_row.eta_from_current, "{what}: wait time");
            }
        }
    }
}
