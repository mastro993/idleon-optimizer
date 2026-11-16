# Section activity is pushed from runner events, keyed by the section that owns the runner

The sidebar shows each section's runs (running with progress, finished, failed). The 41 runners
live in scattered `AppState` fields and section `State`s, and every one is already created through
`report_run`, which subscribes to its events. So `RunStatus` gains the owning section's id, the same
subscription records each event into one `Activity` table on `AppState`, and the sidebar only reads
that table. `TaskEvent::Finished` carries how the run ended (stored, cancelled, failed) so a panic
is told apart from a cancel.

Considered: a per-section `activity` function in the registry that reads the section's runners
each frame (41 hand-written lists to keep in step with the fields, and "finished while away" still
needs an event); a gpui global registry of type-erased runner probes (hidden state, and the
finished/failed marks still need the events).
