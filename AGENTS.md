# Repo notes

## Game data

Game formulas, save decoding and optimizer behaviour follow the game's own rules; each crate keeps
the values it needs in `crates/*/data/*.json`. Take rules from the game, never from the test
fixture.

Before adding or changing a formula or data table, read
[`docs/game-data-sources.md`](docs/game-data-sources.md): where each rule lives (the game client's
`N.js` is the ground truth; the Idleon wiki and open tools help find and cross-check it), how to
track game patches, and what each source's license lets us copy.

## Crate layout

One crate per calculator under `crates/`, layered (see `docs/plan.md` §7):
`common`/`save` → `research`, domain crates → `account` → `stats` and the crates that read it →
`import` → `app`. A lower crate never depends on a higher one. When a domain test needs a fully
imported save, put it in that crate's `tests/` and add `idleon-import` as a dev-dependency; unit
tests under `src/` build a raw `idleon_save::Save` instead. Each crate owns its `data/*.json`
tables.

## Test data

`fixtures/data.json` is a full synthetic account save. Use it whenever a change touches save import
or needs a realistic run to debug:

- import it in tests through `idleon_import::import` (`crates/import/tests/fixture.rs`);
- launch the app with it to debug real account state:
  `idleon-optimizer --import fixtures/data.json` (or `IDLEON_OPTIMIZER_IMPORT=<path>`, which a
  harness-spawned window inherits) applies it as the Import dialog would. The bridge refuses typed
  text over 64 KiB, so do not paste the fixture through it.

It is test input only: never tune production logic to its quirks. Add small purpose-built saves in
`crates/import/tests/save.rs` for edge cases and leave the fixture frozen; if it is regenerated,
update `tests/fixture.rs` deliberately.

## Checks

```console
git submodule update --init --recursive   # once per clone or worktree; cargo fails without it
cargo test
cargo test -- --include-ignored  # also the long simulations, when touching them
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
node script/mcp-drive.mjs        # own window + private bridge endpoint
```

`.github/workflows/ci.yml` runs the `fmt`, `clippy` and `cargo test` gates when a push to `main` or
a pull request changes Rust code or a file a test reads (`docs/roadmap.json`, `docs/sections/**`,
`docs/ui-kit/forms.md`, `assets/game/manifest.json`, the `vendor/gpui-mcp` pin). Every pull request reports the three
required checks: when nothing Rust-relevant changed (docs, assets, scripts) their steps are skipped
and the checks pass in seconds, so such a PR can still merge. The long simulations
(`#[ignore = "long simulation: …"]`) run on `main` only. The MCP harnesses still run locally only.

Harness details and per-lane isolation live in [`docs/agent-harness.md`](docs/agent-harness.md).

Releases and nightly betas are cut by `release.yml` and `nightly.yml`, which build through
`build.yml`; see [`docs/release.md`](docs/release.md). Never edit the workspace version by hand
and never dispatch a release unless asked: `release.yml` owns the version bump.
