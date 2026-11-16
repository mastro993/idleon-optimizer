//! The Kitchens & Jellyfish plan on the committed fixture save.

use idleon_ladle::kitchen;

#[test]
fn the_committed_fixture_plans_a_reset_horizon() {
    // The full synthetic account: ten kitchens, 74 discovered meals and the
    // Menagerie grant. The plan must stay finite, never lower a level and come
    // out the same twice.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    let imported = idleon_import::import(&text).expect("the fixture imports");
    let ladle = imported
        .ladle
        .expect("the fixture carries a character roster");
    let reset = ladle.kitchen.expect("the fixture carries Cooking rows");
    let inputs = kitchen::Inputs {
        days: 7,
        hours_to_reset: ladle.reset_hours.unwrap_or(24.0),
        ladles_per_day: 150.0,
        start_ladles: ladle.ladles.value,
        ladle_multi: 1.5,
        max_level: 150.0,
        base_speed: 1_000.0,
        policy: kitchen::Policy::Share,
        income_fraction: 0.5,
        ..kitchen::Inputs::default()
    };
    let plan = kitchen::plan(&reset, &inputs).expect("the fixture plans");
    assert_eq!(plan.days, 7);
    assert_eq!(plan.timeline.len(), 7);
    for day in &plan.timeline {
        let layout = day
            .phases
            .iter()
            .find_map(|phase| phase.layout.as_ref())
            .expect("every reset cooks a batch");
        assert_eq!(layout.kitchens.len(), 10);
        assert!(day.instructions().contains("At reset:"));
        assert!(day.jelly_hours.is_finite());
    }
    assert!(
        plan.levels
            .iter()
            .zip(&plan.start_levels)
            .all(|(end, start)| end >= start)
    );
    assert!(plan.gained.iter().all(|gained| gained.is_finite()));
    assert_eq!(
        plan,
        kitchen::plan(&reset, &inputs).expect("the fixture plans again")
    );
}
