# Repo notes

## Source of truth

Game formulas, save decoding and optimizer behaviour are ported from the upstream
[`Corgan/idleon-research-optimizer`](https://github.com/Corgan/idleon-research-optimizer) JS. When
changing maths or save handling, read the upstream implementation first (clone it and point
`script/extract-research-data.mjs` or `script/extract-monument-data.mjs` at the checkout); do not
infer rules from the test fixture. Where a section has an oracle script (`script/killroy-oracle.mjs`,
`script/monument-oracle.mjs`, `script/sailing-oracle.mjs`, …), regenerate its fixture from the same
checkout so the port is compared against the upstream calculator's own output.

## Crate layout

One crate per calculator under `crates/`, layered (see `docs/plan.md` §7):
`common`/`save` → `research`, domain crates → `account` → `stats` and the crates that read it →
`import` → `app`. A lower crate never depends on a higher one. When a domain test needs a fully
imported save, put it in that crate's `tests/` and add `idleon-import` as a dev-dependency; unit
tests under `src/` build a raw `idleon_save::Save` instead. Each crate owns its `data/*.json`
tables and oracle fixtures.

## Test data

`fixtures/data.json` is a full synthetic account save. Use it whenever a change touches save import
or needs a realistic run to debug:

- import it in tests through `idleon_import::import` (`crates/import/tests/fixture.rs`);
- paste it into the Import dialog (the Import button in the shared header), or type it through the
  MCP bridge, to debug the app with real account state.

It is test input only: never tune production logic to its quirks. Add small purpose-built saves in
`crates/import/tests/save.rs` for edge cases and leave the fixture frozen; if it is regenerated,
update `tests/fixture.rs` deliberately.

## Checks

```console
cargo test
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
node script/mcp-drive.mjs        # own window + private bridge endpoint
```

`.github/workflows/ci.yml` runs the `fmt`, `clippy` and `cargo test` gates on every push to `main`
and on every pull request; the MCP harnesses still run locally only.

Harness details and per-lane isolation live in [`docs/agent-harness.md`](docs/agent-harness.md).
