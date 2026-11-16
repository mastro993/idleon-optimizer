# Test fixtures

`data.json` is a complete synthetic IdleOn account save: 823 top-level keys, 11 characters, the
research rows (`Research`, `Lv0_*`, `Exp0_*`, …) and 32 statue rows per character in the raw
decoded shape the game writes.

It exists for testing and debugging only: do not reverse-engineer game logic from this file or
tune behaviour to match it.

Used by most crates' tests, which import it through `idleon_import::import`
(`crates/import/tests/fixture.rs` is the reference). To exercise the app by
hand, open the header's Import dialog and paste the file's contents, or drop the file on it.

Treat it as frozen. Add small purpose-built saves to `crates/import/tests/save.rs` for edge cases
instead of editing this one; regenerating it means updating the fixture test on purpose.
