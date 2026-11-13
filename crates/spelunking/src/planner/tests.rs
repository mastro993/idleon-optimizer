use super::*;
use crate::{AmberPolicy, fixture_inputs};

fn base(input: &Inputs) -> Config {
    normalize(input, &Requested::default()).expect("the fixture's defaults run")
}

#[test]
fn the_clear_mode_follows_the_policy() {
    let input = fixture_inputs();
    let mut config = base(&input);
    config.policy = Policy::Auto;
    config.full_clear = false;
    assert_eq!(clear_mode(&config), Clear::First);
    config.full_clear = true;
    assert_eq!(clear_mode(&config), Clear::Full);
    config.policy = Policy::ClearFifth;
    assert_eq!(clear_mode(&config), Clear::Fifth);
    config.policy = Policy::Clear;
    assert_eq!(clear_mode(&config), Clear::Full);
    config.policy = Policy::AmberFarm;
    assert_eq!(clear_mode(&config), Clear::Full);
    config.policy = Policy::Efficient;
    assert_eq!(clear_mode(&config), Clear::First);
}

#[test]
fn the_policy_menu_respects_the_mode_and_clears() {
    let input = fixture_inputs();
    let config = base(&input);
    let scope = normalize_scope(
        &input,
        &config,
        &ScopeRequest {
            modes: Some(vec![Mode::Manual]),
            clears: Some(vec![Clear::Full]),
            ..ScopeRequest::default()
        },
    )
    .expect("the scope is valid");
    assert_eq!(
        policies(&scope, &config),
        vec![Policy::Clear, Policy::AmberFarm]
            .into_iter()
            .filter(|policy| *policy != Policy::AmberFarm)
            .collect::<Vec<_>>()
    );
    let scope = normalize_scope(
        &input,
        &config,
        &ScopeRequest {
            modes: Some(vec![Mode::Auto]),
            clears: Some(vec![Clear::First, Clear::Full]),
            ..ScopeRequest::default()
        },
    )
    .expect("an Auto-only scope is valid");
    assert_eq!(policies(&scope, &config), vec![Policy::Auto]);
    let error = normalize_scope(
        &input,
        &config,
        &ScopeRequest {
            modes: Some(vec![Mode::Auto]),
            clears: Some(vec![Clear::Fifth]),
            ..ScopeRequest::default()
        },
    )
    .expect_err("every-fifth-depth clearing needs manual play");
    assert_eq!(
        error.to_string(),
        "Every-fifth-depth clearing requires manual play. Allow Manual or another descent choice."
    );
}

#[test]
fn the_party_and_belt_enumerators_match_the_upstream_order() {
    let ordered = parties(&[0, 1, 2], 1, 2, false);
    assert_eq!(ordered.len(), 9);
    assert_eq!(ordered[0], vec![0]);
    assert_eq!(ordered[1], vec![0, 1]);
    let unordered = parties(&[0, 1, 2], 1, 2, true);
    assert_eq!(unordered.len(), 6);
    assert_eq!(unordered[0], vec![0]);
    let mut belts = Vec::new();
    belts_into(
        &[0, 1],
        5,
        1,
        2,
        &mut Vec::new(),
        &mut BTreeMap::new(),
        &mut belts,
    );
    assert_eq!(belts.len(), 20);
    assert!(
        belts
            .iter()
            .all(|belt| belt.iter().filter(|id| **id >= 0).count() == 2)
    );
    let mut memories = Vec::new();
    memories_into(&[0, 1, 2], 2, 0, &mut Vec::new(), &mut memories);
    assert_eq!(memories.len(), 7);
}

#[test]
fn the_configuration_count_sums_every_dimension() {
    let input = fixture_inputs();
    let config = base(&input);
    let scope = normalize_scope(
        &input,
        &config,
        &ScopeRequest {
            enabled: Some(Enabled {
                party: false,
                elixirs: false,
                memorized: false,
                entry: false,
                ..Enabled::default()
            }),
            modes: Some(vec![Mode::Auto, Mode::Manual]),
            clears: Some(vec![Clear::First, Clear::Full, Clear::Fifth]),
            speeds: Some((0..=input.speed_max).collect()),
            ..ScopeRequest::default()
        },
    )
    .expect("the scope is valid");
    // Auto: 5 speeds × 2 clears. Manual: 4 policies × 6 tool choices. Entry fixed.
    assert_eq!(configuration_count(&input, &config, &scope), 34);
}

#[test]
fn the_plan_text_names_the_amber_phases() {
    let input = fixture_inputs();
    let mut config = base(&input);
    config.policy = Policy::AmberFarm;
    config.full_clear = true;
    config.amber = Some(AmberPolicy {
        dump_depth: 12,
        finishers: 2,
        reserve_pct: 80,
        harvest_depths: 4,
        rope_reserve_pct: 40,
    });
    let text = plan_text(&input, &config);
    assert!(text.contains("D12"));
    assert!(text.contains("80% stamina"));
    assert!(text.contains("+4 floors"));
    assert!(text.contains("40% finishing stamina"));
}
