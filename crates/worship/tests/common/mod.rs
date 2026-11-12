//! Account inputs shared by the Worship TD tests.

#![allow(dead_code)]

use idleon_worship::Inputs;

/// Trial levels for a fully unlocked account, so every type can be placed.
pub fn unlocked() -> Inputs {
    Inputs {
        tower_levels: [50; 9],
        start_pts: 100_000.0,
        totem_hp: 50.0,
        ..Inputs::default()
    }
}

/// An unlocked account with plenty of points, so placement is not what limits
/// a test.
pub fn rich() -> Inputs {
    Inputs {
        start_pts: 1_000_000_000_000.0,
        ..unlocked()
    }
}
