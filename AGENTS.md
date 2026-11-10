# Repo notes

## Source of truth

Game formulas, save decoding and optimizer behaviour are ported from the upstream
[`Corgan/idleon-research-optimizer`](https://github.com/Corgan/idleon-research-optimizer) JS. When
changing maths or save handling, read the upstream implementation first (clone it and point
`script/extract-research-data.mjs` at the checkout); do not infer rules from the test fixture.

## Test data

`fixtures/data.json` is a full synthetic account save. Use it whenever a change touches save import
or needs a realistic run to debug:

- import it in tests through `idleon_core::save::import` (`crates/core/tests/fixture.rs`);
- paste it into the Rates pane's Save import panel, or type it through the MCP bridge, to debug the
  app with real account state.

It is test input only: never tune production logic to its quirks. Add small purpose-built saves in
`crates/core/tests/save.rs` for edge cases and leave the fixture frozen; if it is regenerated,
update `tests/fixture.rs` deliberately.

## Checks

```console
cargo test
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
node script/mcp-drive.mjs        # own window + private bridge endpoint
```

Harness details and per-lane isolation live in [`docs/agent-harness.md`](docs/agent-harness.md).
