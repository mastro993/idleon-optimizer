# Test fixtures

`data.json` is a complete synthetic IdleOn account save: 823 top-level keys, 11 characters and the
research rows (`Research`, `Lv0_*`, `Exp0_*`, …) in the raw decoded shape the game writes.

It exists for testing and debugging only. The game formulas, save decoding and optimizer behaviour
come from the upstream [`Corgan/idleon-research-optimizer`](https://github.com/Corgan/idleon-research-optimizer)
repo; do not reverse-engineer logic from this file or tune behaviour to match it.

Used by `crates/core/tests/fixture.rs`. To exercise the app by hand, open the header's Import dialog
and paste the file's contents, or drop the file on it.

Treat it as frozen. Add small purpose-built saves to `crates/core/tests/save.rs` for edge cases
instead of editing this one; regenerating it means updating the fixture test on purpose.
