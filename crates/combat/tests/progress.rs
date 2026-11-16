//! The combat progress and smelting rules.

use idleon_combat::progress::{Config, Grant, State};
use idleon_common::Rng;

#[test]
fn a_run_with_no_forge_inputs_reports_its_notices() {
    let mut state = State::new(Config::default());
    let mut rng = Rng::new(1);
    state.grant(&mut rng, Grant::Forge, 30.0);
    state.grant(&mut rng, Grant::Pets, 12.0);
    let summary = state.summary();
    assert_eq!(summary.forge_seconds, 30.0);
    assert_eq!(summary.pet_pending_seconds, 12.0);
    assert!(!summary.forge_available);
    assert!(summary.forge.is_none());
    let keys: Vec<&str> = summary.notices.iter().map(|notice| notice.key).collect();
    assert_eq!(keys, ["forgeState", "petState", "petNonShiny"]);
}
