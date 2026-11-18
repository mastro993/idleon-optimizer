//! GPUI building blocks every calculator section shares, independent of the
//! window state: the background task runner, its progress chrome, the formula
//! tree widget, the number formats, the display composites and form controls
//! GPUI Kit lacks, and the kit table delegate.

pub mod format;
pub mod formula;
pub mod run_chrome;
pub mod runner;
pub mod widgets;
