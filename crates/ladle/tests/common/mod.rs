//! Helpers the ladle parity tests share.

/// Upstream figures are doubles; compare to the last few bits they share.
#[track_caller]
pub fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}
