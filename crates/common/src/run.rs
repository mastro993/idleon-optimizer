//! How a long-running optimizer reports progress and is told to stop.

/// Progress reported back to the UI while a run is in flight.
#[derive(Clone, Debug)]
pub struct Progress {
    pub fraction: f32,
    pub message: String,
}

/// What a progress reporter asks the run to do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Keep going.
    Continue,
    /// Stop the run and leave the previous result in place.
    Stop,
}
