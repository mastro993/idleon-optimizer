//! Formula edges the fixtures do not reach: packed unit rows, the rank walk,
//! world unlocks, the clearing states and the range plan's two dead ends.

use super::*;
use serde_json::json;

/// A save carrying only the two Royal Guardian rows.
fn save(g: Value, maps: Value) -> Map<String, Value> {
    let mut save = Map::new();
    save.insert("RoyalG".into(), Value::String(g.to_string()));
    save.insert("RoyalMaps".into(), Value::String(maps.to_string()));
    save
}

/// `RoyalG` with every armory level at zero and every other row empty.
fn blank_rows() -> Vec<Value> {
    let mut g = vec![json!([]); 24];
    g[row::ARMORY] = json!(vec![0.0; 83]);
    g
}

/// A save whose armory opens `unlocked` worlds: every world from the second
/// needs one of the seven unlock upgrades at level 1.
fn unlocked_worlds(unlocked: usize) -> RoyalGuardian {
    let mut armory = vec![0.0; 83];
    for world in 1..unlocked {
        // Armory 2..8 unlock worlds 2..8; one level each opens one world.
        armory[1 + world] = 1.0;
    }
    let mut g = vec![json!([]); 24];
    g[row::ARMORY] = json!(armory);
    RoyalGuardian::from_save(&save(json!(g), json!([])), None).expect("the rows read")
}

#[test]
fn a_packed_unit_row_decodes_one_digit_per_slot() {
    // Each digit is the unit type plus two, so `3 2 2 1…` is one type-1 unit,
    // two type-0 units, then locked slots.
    assert_eq!(
        RoyalGuardian::packed_units(322_111_111.0),
        vec![(0, 1), (1, 0), (2, 0)]
    );
    // Only 2 to 5 are unit types: 0 is empty, 1 is locked and 6 to 9 are not
    // unit digits at all.
    assert_eq!(
        RoyalGuardian::packed_units(555_555_555.0),
        (0..9).map(|slot| (slot, 3)).collect::<Vec<_>>()
    );
    assert!(RoyalGuardian::packed_units(999_999_999.0).is_empty());
    assert!(RoyalGuardian::packed_units(0.0).is_empty());
}

#[test]
fn a_rank_bar_walks_its_own_curve() {
    // Bar 0 is (10 + 5 × rank) × 1.3^rank: 10, 19.5, 33.8…
    assert_eq!(RoyalGuardian::exp_formula(0.0, 0), 10.0);
    assert!((RoyalGuardian::exp_formula(1.0, 0) - 19.5).abs() < 1e-9);
    // Bar 2 is (50 + 50 × rank) × 1.8^rank and bar 4 is 1e5 × 10^rank.
    assert!((RoyalGuardian::exp_formula(2.0, 2) - 150.0 * 3.24).abs() < 1e-9);
    assert_eq!(RoyalGuardian::exp_formula(3.0, 4), 1e8);
    let g = blank_rows();
    let mut maps = vec![json!([])];
    // 25 saved kills on bar 0: past rank 1's 10 and rank 2's 19.5, short of 33.8.
    maps[0] = json!([0, 0, 0, 25.0]);
    let royal =
        RoyalGuardian::from_save(&save(json!(g), json!(maps)), None).expect("the rows read");
    assert_eq!(royal.rank(0, 0), 2.0);
    assert_eq!(royal.rank(0, 1), 0.0);
}

#[test]
fn the_armory_opens_one_world_per_unlock_level() {
    for unlocked in 1..=WORLDS {
        let royal = unlocked_worlds(unlocked);
        assert_eq!(royal.worlds_unlocked(), unlocked, "{unlocked} worlds");
    }
}

#[test]
fn a_clearing_outpost_reports_its_raw_values_and_states() {
    let royal = clearing_outpost(true);
    assert!(royal.outpost(1).eligible);
    // Map 1 is a W1 outpost; a one-cell row accrues kills against map 1.
    let clear = royal.clear(1);
    assert_eq!(clear.state, ClearState::Clearing);
    assert!(clear.active);
    assert_eq!(clear.assignments, 1.0);
    assert_eq!(clear.progress, 100.0);
    assert_eq!(clear.requirement, royal.kill_requirement(1));
    assert_eq!(clear.remaining, clear.requirement - 100.0);
    assert!(clear.eta_hours.expect("an ETA").is_finite());
    // A row that is not there at all does not accrue.
    let missing = royal.clear(3);
    assert_eq!(missing.state, ClearState::NonAccruing);
    assert_eq!(
        missing.reason.as_deref(),
        Some("RoyalMaps row is missing"),
        "the empty row names itself"
    );
}

#[test]
fn an_unassigned_outpost_waits_for_militia_and_a_secured_one_does_not() {
    let royal = clearing_outpost(false);
    let waiting = royal.clear(1);
    assert_eq!(waiting.state, ClearState::WaitingMilitia);
    assert_eq!(waiting.reason.as_deref(), Some("no militia assignment"));

    let mut g = blank_rows();
    let mut kind = vec![0.0; 15];
    kind[0] = 4.0;
    let mut target = vec![0.0; 15];
    target[0] = 1.0;
    g[row::MILITIA] = json!(kind);
    g[row::MILITIA + 1] = json!(target);
    let requirement = data::map(1).expect("map 1").kill_requirement;
    let royal = RoyalGuardian::from_save(&save(json!(g), json!([[], [requirement]])), None)
        .expect("the rows read");
    let secured = royal.clear(1);
    assert_eq!(secured.state, ClearState::Secured);
    assert_eq!(secured.remaining, 0.0);
}

#[test]
fn the_range_plan_names_the_level_that_reaches_the_node() {
    let royal = built_outpost(0.0);
    let target = royal
        .next_range_target(1)
        .expect("a W1 outpost has an unreachable node");
    assert!(
        target.distance > target.reach,
        "the target is the nearest node past the reach"
    );
    let plan = &target.plan;
    assert!(plan.possible, "{plan:?}");
    let levels = plan.levels_needed.expect("a level count");
    assert!(levels > 0.0);
    assert_eq!(plan.points(), levels * 2.0);
    let reach = royal.range_at_logistics(1, plan.target_level.expect("a target level"));
    assert!(reach >= plan.required_range, "the level reaches the node");
    // A node the outpost already reaches plans nothing.
    let nearby = royal.logistics_plan(1, 10.0);
    assert!(nearby.possible);
    assert_eq!(nearby.levels_needed, Some(0.0));
    // A node beyond the maximum Advanced Logistics reach is not reachable.
    let far = royal.logistics_plan(1, 5_000.0);
    assert!(!far.possible);
    assert_eq!(far.reason, Some("beyond maximum Advanced Logistics range"));
}

#[test]
fn a_support_outpost_cannot_link_resources() {
    let royal = built_outpost(1.0);
    let target = royal.next_range_target(1).expect("a node exists");
    assert!(!target.plan.possible);
    assert_eq!(
        target.plan.reason,
        Some("Support outposts cannot connect to resources")
    );
    // A support outpost reaches nothing, even a node inside its range.
    assert!(!royal.resource_reachable(1, 0));
}

#[test]
fn the_militia_rate_multiplies_the_talent_and_the_cross_system_factors() {
    let royal = RoyalGuardian::from_save(&save(json!(vec![json!([]); 24]), json!([])), None)
        .expect("the rows read");
    // No Royal Guardian character: upstream reads the talent at one, and the
    // clearing factors are zero.
    assert_eq!(royal.talent_multi(), 1.0);
    assert_eq!(royal.xtra_clear_killz(), 1.0);
    assert_eq!(royal.unit_spec(4), 4000.0);

    // The talent's curve is `1 + x1 × lv / (lv + x2)`: 1 + 5 × 300 / 700.
    let g = blank_rows();
    let mut save = save(json!(g), json!([]));
    save.insert("CharacterClass_0".into(), json!(16));
    save.insert(
        "SL_0".into(),
        Value::String(json!({"231": 300}).to_string()),
    );
    let royal = RoyalGuardian::from_save(&save, None).expect("the rows read");
    assert!((royal.talent_multi() - (1.0 + 5.0 * 300.0 / 700.0)).abs() < 1e-9);
    assert_eq!(royal.missing_sources().len(), 2, "Spelunk and Research");
}

#[test]
fn a_save_without_royal_rows_has_no_state() {
    let empty = Map::new();
    assert!(RoyalGuardian::from_save(&empty, None).is_none());
}

/// A built W1 outpost of `kind` on map 1, with every armory level at zero.
fn built_outpost(kind: f64) -> RoyalGuardian {
    let g = blank_rows();
    let mut maps = vec![json!([]), json!([])];
    // A built outpost row: three upgrade cells, five rank bars, the links, the
    // type, the packed units and the glorified flag.
    maps[1] = json!([0, 0, 0, 0, 0, 0, 0, 0, -1, -1, kind, 211_111_111, 0]);
    RoyalGuardian::from_save(&save(json!(g), json!(maps)), None).expect("the rows read")
}

/// A W1 outpost with map 1 clearing, and map 3's row absent.
fn clearing_outpost(assign_militia: bool) -> RoyalGuardian {
    let mut g = blank_rows();
    let mut kind = vec![0.0; 15];
    let mut target = vec![0.0; 15];
    if assign_militia {
        kind[0] = 4.0;
        target[0] = 1.0;
    }
    g[row::MILITIA] = json!(kind);
    g[row::MILITIA + 1] = json!(target);
    let mut maps = vec![json!([]); 4];
    maps[1] = json!([100.0]);
    RoyalGuardian::from_save(&save(json!(g), json!(maps)), None).expect("the rows read")
}

// ----- RG-02: connections, resource nodes, currencies and the drain -----

#[test]
fn a_saved_endpoint_is_a_resource_a_map_or_nothing() {
    // A resource index sits below the offset, a map above it, and the game
    // writes `-1` for an empty slot.
    assert_eq!(
        RoyalGuardian::parse_endpoint(Some(7.0)),
        Endpoint::Resource(7)
    );
    assert_eq!(
        RoyalGuardian::parse_endpoint(Some(1003.0)),
        Endpoint::Map(3)
    );
    assert_eq!(RoyalGuardian::parse_endpoint(Some(-1.0)), Endpoint::Empty);
    assert_eq!(RoyalGuardian::parse_endpoint(None), Endpoint::Empty);
    assert_eq!(
        RoyalGuardian::parse_endpoint(Some(f64::NAN)),
        Endpoint::Empty
    );
    assert_eq!(Endpoint::Map(3).encode(), 1003);
    assert_eq!(Endpoint::Resource(7).encode(), 7);
    assert_eq!(Endpoint::Empty.encode(), -1);
}

#[test]
fn a_support_outpost_links_its_target_and_collects_from_it() {
    // Map 1 stays a Normal outpost; map 2 is built as a Support outpost whose
    // first slot links map 1 and whose second slot is the legacy empty one.
    let g = blank_rows();
    let mut maps = vec![json!([]); 4];
    maps[1] = json!([0, 0, 0, 0, 0, 0, 0, 0, -1, -1, 0, 211_111_111, 0]);
    maps[2] = json!([0, 0, 0, 0, 0, 0, 0, 0, 1001, -1, 1, 211_111_111, 0]);
    let royal =
        RoyalGuardian::from_save(&save(json!(g), json!(maps)), None).expect("the rows read");

    let connections = royal.outpost_connections(2);
    assert_eq!(connections[0].endpoint, Endpoint::Map(1));
    assert_eq!(connections[1].endpoint, Endpoint::Empty);
    assert!(royal.outpost_supports(2));
    assert_eq!(royal.support_count(1), 1, "map 2 links map 1");
    assert_eq!(royal.support_links(1)[0].slot, 0);
    assert!(!royal.support_links(1)[0].legacy);
    assert_eq!(royal.support_count(3), 0, "nothing links map 3");
    // A Normal outpost has no support targets of its own.
    assert!(royal.support_targets(1).is_empty());
    // A Support outpost cannot link a resource node, so the node reports no
    // reachable outpost at all.
    assert!(!royal.resource_reachable(2, 0));
    assert!(
        royal
            .reachable_outposts(0)
            .iter()
            .all(|outpost| outpost.map != 2)
    );
}

#[test]
fn a_node_capacity_scales_with_its_grade_and_world_tier() {
    // `RoyalResources[0]` is 1M at grade zero, and the tier multiplies by five
    // once per twenty rows of the catalogue.
    assert_eq!(
        RoyalGuardian::resource_capacity_at_grade(0, 0.0),
        5.0 * 1_000_000.0
    );
    assert_eq!(
        RoyalGuardian::resource_capacity_at_grade(0, 2.0),
        5.0 * 1_000_000.0 * 1.5 * 1.5
    );
    // A grade is floored before it scales, and a negative one reads as zero.
    assert_eq!(
        RoyalGuardian::resource_capacity_at_grade(0, 2.9),
        RoyalGuardian::resource_capacity_at_grade(0, 2.0)
    );
    assert_eq!(
        RoyalGuardian::resource_capacity_at_grade(0, -4.0),
        RoyalGuardian::resource_capacity_at_grade(0, 0.0)
    );
    // Row 20 opens the second tier.
    assert!(
        RoyalGuardian::resource_capacity_at_grade(20, 0.0) > 1.0,
        "the second tier carries its own base"
    );
}

#[test]
fn a_drained_node_reads_full_and_a_collection_window_caps_at_what_is_left() {
    let mut g = blank_rows();
    let mut grades = vec![0.0; 80];
    grades[0] = 1.0;
    g[row::GRADES] = json!(grades);
    // Resource 0 is drained; resource 1 has collected a third of its capacity.
    let third = RoyalGuardian::resource_capacity_at_grade(1, 0.0) / 3.0;
    let mut progress = vec![0.0; 80];
    progress[0] = -1.0;
    progress[1] = third;
    g[row::RESOURCES] = json!(progress);
    let royal = RoyalGuardian::from_save(&save(json!(g), json!([])), None).expect("the rows read");

    let drained = royal.resource_node(0);
    assert!(drained.drained);
    assert_eq!(drained.collected, drained.capacity);
    assert_eq!(drained.remaining, 0.0);
    assert_eq!(royal.resource_collected_within(0, 1_000.0, 10.0), 0.0);

    let partial = royal.resource_node(1);
    assert!(!partial.drained);
    assert!((partial.collected - third).abs() < 1e-6);
    assert!((partial.remaining - (partial.capacity - third)).abs() < 1e-6);
    let window = royal.resource_window(1, 1_000.0, 10.0);
    assert_eq!(window.collected, 10_000.0);
    assert!(!window.emptied);
    // A rate that covers the rest empties the node.
    let ample = royal.resource_window(1, 1e9, 10.0);
    assert!(ample.emptied);
    assert!(!ample.already_empty);
    // Nothing drains a node at a zero rate.
    assert!(royal.resource_drain_hours(1, 0.0).is_infinite());
    assert_eq!(royal.resource_drain_hours(0, 0.0), 0.0);
}

#[test]
fn the_next_cycle_needs_replenish_and_grades_only_where_it_is_unlocked() {
    let mut g = vec![json!([]); 24];
    let mut armory = vec![0.0; 83];
    g[row::ARMORY] = json!(armory.clone());
    let royal = RoyalGuardian::from_save(&save(json!(g.clone()), json!([])), None).expect("rows");
    let cycle = royal.resource_cycle(0, 100.0);
    assert!(!cycle.replenish_unlocked);
    assert!(!cycle.next_cycle_available);
    assert_eq!(cycle.next_capacity, None);
    assert_eq!(cycle.next_rate, None);
    // A quarter of the rate is the grade multiplier at grade one, so the
    // ungraded rate is the rest.
    assert!((cycle.full_rate - 100.0).abs() < 1e-9);

    armory[70] = 1.0;
    g[row::ARMORY] = json!(armory.clone());
    let royal = RoyalGuardian::from_save(&save(json!(g.clone()), json!([])), None).expect("rows");
    let cycle = royal.resource_cycle(0, 100.0);
    assert!(cycle.replenish_unlocked);
    assert!(cycle.next_cycle_available);
    // Replenish alone refills at the same grade.
    assert_eq!(cycle.next_grade, cycle.grade);
    assert_eq!(cycle.next_capacity, Some(cycle.capacity));

    armory[0] = 1.0;
    g[row::ARMORY] = json!(armory);
    let royal = RoyalGuardian::from_save(&save(json!(g), json!([])), None).expect("rows");
    let cycle = royal.resource_cycle(0, 100.0);
    assert!(cycle.grade_increase_unlocked);
    assert_eq!(cycle.next_grade, cycle.grade + 1.0);
    assert!(
        cycle.next_capacity.expect("a capacity") > cycle.capacity,
        "the refill is bigger at the higher grade"
    );
}

#[test]
fn a_save_without_the_reset_countdown_has_no_projection() {
    let g = blank_rows();
    let royal = RoyalGuardian::from_save(&save(json!(g), json!([])), None).expect("the rows read");
    let reset = royal.reset_timing();
    assert!(!reset.available);
    assert_eq!(reset.hours_remaining, None);
    assert_eq!(reset.missing, vec!["TimeAway.ShopRestock".to_owned()]);
    let allocation = royal.allocation_to_reset();
    assert!(!allocation.reset_projection_available);
    assert_eq!(allocation.collectable_before_reset, None);
    assert!(
        allocation
            .missing
            .contains(&"TimeAway.ShopRestock".to_owned())
    );

    // The countdown itself is seconds, floored at zero.
    let mut save = save(json!(vec![json!([]); 24]), json!([]));
    save.insert(
        "TimeAway".into(),
        Value::String(json!({"ShopRestock": 7200.0}).to_string()),
    );
    let royal = RoyalGuardian::from_save(&save, None).expect("the rows read");
    let reset = royal.reset_timing();
    assert!(reset.available);
    assert_eq!(reset.hours_remaining, Some(2.0));
    assert!(!reset.due);
}

#[test]
fn the_banked_time_banks_only_with_armory_30_and_reads_in_hours() {
    let mut g = vec![json!([]); 24];
    let mut armory = vec![0.0; 83];
    let mut banked = vec![0.0; 20];
    banked[0] = 9_000.0;
    g[row::ARMORY] = json!(armory.clone());
    g[row::BANKED] = json!(banked);
    let royal = RoyalGuardian::from_save(&save(json!(g.clone()), json!([])), None).expect("rows");
    assert_eq!(royal.banked_hours(), 2.5);
    assert!(!royal.banks_time(), "Armory 30 is not bought");

    armory[30] = 1.0;
    g[row::ARMORY] = json!(armory);
    let royal = RoyalGuardian::from_save(&save(json!(g), json!([])), None).expect("rows");
    assert!(royal.banks_time());
}

#[test]
fn a_currency_balance_reads_its_slot_and_a_negative_slot_reads_zero() {
    let mut g = blank_rows();
    let mut currencies = vec![0.0; 80];
    currencies[5] = 1_234.0;
    g[row::CURRENCIES] = json!(currencies);
    let royal = RoyalGuardian::from_save(&save(json!(g), json!([])), None).expect("the rows read");
    assert_eq!(royal.currency_balance(5), 1_234.0);
    assert_eq!(royal.currency_balance(6), 0.0);
    assert_eq!(royal.currency_balance(-1), 0.0);
}

#[test]
fn a_stream_travels_from_its_outpost_to_its_node() {
    // A built Normal outpost on map 1 links resource 7, which is a W1 node.
    let g = blank_rows();
    let mut maps = vec![json!([]); 4];
    maps[1] = json!([0, 0, 0, 0, 0, 0, 0, 0, 7, -1, 0, 211_111_111, 0]);
    let royal =
        RoyalGuardian::from_save(&save(json!(g), json!(maps)), None).expect("the rows read");
    let (streams, invalid) = royal.resource_streams();
    assert!(invalid.is_empty(), "{invalid:?}");
    assert_eq!(streams.len(), 1);
    let stream = &streams[0];
    assert_eq!((stream.map, stream.slot, stream.resource), (1, 0, 7));
    assert!(!stream.savage);
    assert_eq!(stream.currency_rate, stream.rate, "a Normal outpost pays");
    assert_eq!(stream.currency_slot, resource_currency(7));
    // The node's allocation groups that one stream.
    let allocation = royal.resource_allocation_with(&royal.collection_inputs(), 24.0);
    let detail = allocation
        .details
        .iter()
        .find(|detail| detail.resource == 7)
        .expect("the linked node reports");
    assert_eq!(detail.streams.len(), 1);
    assert_eq!(detail.rate, stream.rate);
}

#[test]
fn the_two_fixed_first_level_prices_hold_until_their_level() {
    let g = blank_rows();
    let royal = RoyalGuardian::from_save(&save(json!(g), json!([])), None).expect("the rows read");
    // Shelf 46 at its first two levels is a flat 2, and shelf 58 at level 0 is
    // a flat 3: the source hard-codes both.
    let cheap = royal
        .armory_unlock_order()
        .iter()
        .position(|index| *index == 46.0)
        .expect("shelf 46 is in the order");
    let cost = royal.armory_upgrade_cost_at(cheap, 2.0, 0.0);
    assert_eq!(cost.value, 2.0);
    assert_eq!(cost.factors.len(), 1, "the special case has one factor");
    // At level 3 the product takes over.
    assert!(royal.armory_upgrade_cost_at(cheap, 3.0, 0.0).value > 2.0);
    let late = royal
        .armory_unlock_order()
        .iter()
        .position(|index| *index == 58.0)
        .expect("shelf 58 is in the order");
    assert_eq!(royal.armory_upgrade_cost_at(late, 0.0, 0.0).value, 3.0);
    assert!(royal.armory_upgrade_cost_at(late, 1.0, 0.0).value > 3.0);
}

#[test]
fn a_cost_plan_stops_at_the_level_cap() {
    let mut g = blank_rows();
    // Shelf 0 has one level; a level-1 save has nothing left to plan.
    let mut armory = vec![0.0; 83];
    armory[0] = 1.0;
    g[row::ARMORY] = json!(armory);
    let royal = RoyalGuardian::from_save(&save(json!(g), json!([])), None).expect("the rows read");
    let order = royal
        .armory_unlock_order()
        .iter()
        .position(|index| *index == 0.0)
        .expect("shelf 0 is in the order");
    let plan = royal.armory_cost_plan(order, 10).expect("shelf 0 prices");
    assert!(plan.rows.is_empty(), "a maxed shelf has no next level");
    assert_eq!(plan.covered_full(), 0);
}

#[test]
fn an_orblet_level_price_grows_and_rounds_down() {
    let g = blank_rows();
    let royal = RoyalGuardian::from_save(&save(json!(g), json!([])), None).expect("the rows read");
    // Orblet 0 costs 2 at level 0 and 1 + 2 × 50 = 101 at level 1.
    assert_eq!(royal.orblet_cost(0), 2.0);
    assert!(!royal.orblet_unlocked(1), "orblet 0 is not bought yet");
    let mut g = blank_rows();
    g[row::ORBLET] = json!(vec![1.0]);
    let royal = RoyalGuardian::from_save(&save(json!(g), json!([])), None).expect("the rows read");
    assert_eq!(royal.orblet_cost(0), 101.0);
    assert!(royal.orblet_unlocked(1));
}

#[test]
fn the_forecast_clamps_its_horizon_and_folds_a_collect_into_a_window() {
    let g = blank_rows();
    let royal = RoyalGuardian::from_save(&save(json!(g), json!([])), None).expect("the rows read");
    // A horizon past the maximum reads at the maximum, and a missing one at
    // the default.
    assert_eq!(
        royal
            .forecast(&ForecastOptions {
                horizon_hours: 10_000.0,
                ..ForecastOptions::default()
            })
            .horizon_hours,
        HORIZON.1
    );
    assert_eq!(
        royal.forecast(&ForecastOptions::default()).horizon_hours,
        HORIZON.2
    );
    // A collect inside a menu-open window is covered by the window, so it is
    // reported as ignored instead of committing twice.
    let forecast = royal.forecast(&ForecastOptions {
        horizon_hours: 24.0,
        collects: vec![5.0, 12.0],
        windows: vec![(10.0, 14.0)],
    });
    assert_eq!(forecast.ignored_collects, vec![12.0]);
    assert_eq!(
        forecast.collects.len(),
        1,
        "only the collect outside the window commits"
    );
}

#[test]
fn the_orblet_count_names_the_inventories_it_could_not_read() {
    let g = blank_rows();
    let mut save = save(json!(g), json!([]));
    // One character, a chest and its own inventory, all carrying one Orblet.
    save.insert("Lv0_0".into(), json!([1.0]));
    save.insert("ChestOrder".into(), json!(["Orblet", "Blank"]));
    save.insert("ChestQuantity".into(), json!([2.0, 1.0]));
    save.insert("InventoryOrder_0".into(), json!(["Orblet"]));
    save.insert("ItemQTY_0".into(), json!([3.0]));
    let royal = RoyalGuardian::from_save(&save, None).expect("the rows read");
    let owned = royal.orblet_owned();
    assert_eq!(owned.value, 5.0);
    assert!(!owned.partial, "{:?}", owned.missing);

    // Without the chest the count still reads the character, and says what is
    // missing instead of calling the roster empty.
    let mut save = save;
    save.remove("ChestOrder");
    save.remove("ChestQuantity");
    let royal = RoyalGuardian::from_save(&save, None).expect("the rows read");
    let owned = royal.orblet_owned();
    assert_eq!(owned.value, 3.0);
    assert!(owned.partial);
    assert_eq!(owned.missing, vec!["storage inventory".to_owned()]);
}

#[test]
fn a_collect_inside_a_merged_window_is_still_ignored() {
    let g = blank_rows();
    let royal = RoyalGuardian::from_save(&save(json!(g), json!([])), None).expect("the rows read");
    // Two overlapping windows merge, so a collect between them is covered.
    let forecast = royal.forecast(&ForecastOptions {
        horizon_hours: 24.0,
        collects: vec![12.5],
        windows: vec![(10.0, 12.0), (11.0, 14.0)],
    });
    assert_eq!(forecast.ignored_collects, vec![12.5]);
    assert!(forecast.collects.is_empty());
}
