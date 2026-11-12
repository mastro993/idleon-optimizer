//! Helpers the Clamworks parity tests share.

/// Upstream figures are doubles; compare to the last few bits they share.
#[track_caller]
pub fn close(actual: f64, expected: f64) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected}"
    );
}
