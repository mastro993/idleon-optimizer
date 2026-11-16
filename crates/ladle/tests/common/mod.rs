//! Helpers the ladle tests share.

/// Equal to the last few bits two doubles share.
#[track_caller]
pub fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}
