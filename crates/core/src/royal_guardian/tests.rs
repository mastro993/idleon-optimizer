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
    // 3 is a Worker (2) and 4 a Trader (2)… the digits are the type id plus two,
    // so `3 2 2 1…` is a Worker, then Traders, then locked slots.
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
    let mut g = vec![json!([]); 24];
    g[row::ARMORY] = json!(vec![0.0; 83]);
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

    let mut g = vec![json!([]); 24];
    g[row::ARMORY] = json!(vec![0.0; 83]);
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
    // No Royal Guardian character: the talent falls back to 1, the factors to 0.
    assert_eq!(royal.talent_multi(), 0.0);
    assert_eq!(royal.xtra_clear_killz(), 1.0);
    assert_eq!(royal.unit_spec(4), 4000.0);

    // The talent's curve is `1 + x1 × lv / (lv + x2)`: 1 + 5 × 300 / 700.
    let mut g = vec![json!([]); 24];
    g[row::ARMORY] = json!(vec![0.0; 83]);
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
    let mut g = vec![json!([]); 24];
    g[row::ARMORY] = json!(vec![0.0; 83]);
    let mut maps = vec![json!([]), json!([])];
    // A built outpost row: three upgrade cells, five rank bars, the links, the
    // type, the packed units and the glorified flag.
    maps[1] = json!([0, 0, 0, 0, 0, 0, 0, 0, -1, -1, kind, 211_111_111, 0]);
    RoyalGuardian::from_save(&save(json!(g), json!(maps)), None).expect("the rows read")
}

/// A W1 outpost with map 1 clearing, and map 3's row absent.
fn clearing_outpost(assign_militia: bool) -> RoyalGuardian {
    let mut g = vec![json!([]); 24];
    g[row::ARMORY] = json!(vec![0.0; 83]);
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
