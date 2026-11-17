# Section-owned state

A section owns its state, its import step and its project config in its own module. `AppState`
holds one field per section and calls the section's hooks one line at a time; it never spells out
what a section does with a save or a config.

This document is the contract, the checklist for moving the remaining sections onto it, and the
result of the pilot that proved it (the Opal Optimizer, `crates/app/src/sections/opal/state.rs`).

## Why

Before the pilot, everything a section owned besides its view lived in shared switchboards:

| Where | What each section added |
|---|---|
| `AppState` (`state/mod.rs`) | loose fields (`opal`, `opal_selected`, `opal_tab`, …), runners, formula trees, and a `report_run` closure in an 849-line constructor |
| `load_imported` (`state/documents.rs`) | its import step, with its own verb (`load`, `adopt`, field writes, `discard_run`) |
| `project.rs` | its config struct, a `configs` insert, an `enum Config` variant, a `decode` arm, an `assign` arm |

So parallel work on two sections conflicted in these files every time, and a section's own
load/persist logic lived where its author was least likely to look.

## 1. The state holder

One struct per section, named `State` in the section module, owning everything the section owns:
the decoded domain value, selections, tab, inputs, runners, formula trees and the subscriptions
that watch its runners. It is built by `new`, like `statue::State::new`:

```rust
pub(crate) struct State {
    /// The villagers and account bonuses, when the save has them.
    pub(crate) save: Option<Opal>,
    pub(crate) selected: Option<usize>,
    pub(crate) tab: Tab,
    pub(crate) target: Option<usize>,
    pub(crate) total: f64,
    pub(crate) manual: [f64; VILLAGERS],
    pub(crate) run: Entity<TaskRunner<Plan, PlanInput>>,
    pub(crate) breakdown: Entity<FormulaTree>,
    _subscriptions: Vec<Subscription>,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self { … }
}
```

`AppState` keeps one field and one constructor line:

```rust
    /// The Opal Optimizer's villagers, selections, plan run and formula tree.
    pub(crate) opal: OpalState,
    …
            opal: OpalState::new(window, cx),
```

The section state is a plain field, not an `Entity`. Its methods take `cx: &mut Context<AppState>`,
so an edit is `state.opal.set_tab(tab, cx)`: the borrow of `state.opal` and the `cx` are disjoint,
and `cx.notify()` redraws the window exactly as the old `AppState` method did.

## 2. The lifecycle hooks

```rust
impl State {
    /// Everything `load_imported` did for the section, including discarding
    /// its runs (`discard_run(&self.run, cx)` when it has one to drop).
    pub(crate) fn on_import(&mut self, opal: Option<Opal>);

    /// What `configs` writes for the section.
    pub(crate) fn config(&self) -> Config;

    /// What `assign` did for the section; runs after `on_import`.
    pub(crate) fn apply_config(&mut self, config: Config, cx: &mut Context<AppState>);
}
```

- `on_import` takes **its slice** of `Imported` by value (`imported.opal`), not `&Imported`:
  `load_imported` moves each domain value out of `Imported`, and borrowing the whole would force a
  clone per section. A section whose import reads shared state (the research `Simulation`, the raw
  `Save`, the character count) takes those as extra arguments; it never reads another section's
  fields. It takes `cx: &mut App` only when it discards a run.
- `config`/`apply_config` are **typed**: `Config` is the serde struct that used to live in
  `project.rs`, moved verbatim into the section (same id, same fields, same `serde` attributes, so
  the JSON is byte-identical). A section whose apply step warns about values the save no longer has
  adds `warnings: &mut Vec<String>` to `apply_config`; opal never warns, so it does not take it.
- **Run completion**: the section's `new` calls `report_run` itself and keeps the `Subscription` in
  its own `_subscriptions`. The completion closure still receives `&mut AppState` (it writes the
  shared status line), but it lives in the section. `RunStatus.section` names the section's registry
  id, under which every runner event is recorded in `AppState::activity` for the sidebar's marks.
  `report_run`, `RunStatus` and `ProgressLine` are `pub(crate)` for this.

```rust
        let mut subscriptions = Vec::new();
        let run = report_run::<Plan, PlanInput>(
            cx,
            &mut subscriptions,
            RunStatus { section: "opal", progress: ProgressLine::Message("Optimizing the opal distribution…"), … },
            |_, plan| Some(format!("Opal plan ready: {} opals across the villagers.", …)),
        );
```

Starting, cancelling and copying a run stay `impl AppState` methods in the section's `actions.rs`
(`run_opal_plan`, …): they go through `start_run`/`cancel_run`/`set_status`, which own the shared
status line. They read only `self.opal`.

## 3. How the shared code calls the hooks

Two options were weighed:

- **(a) a `SectionState` trait** that `AppState` iterates. It does not fit: each section's
  `on_import` takes a different, moved slice of `Imported`; `decode` must run during the import
  preview with no `AppState` at all, so a trait's `apply_config` would have to take a `Value` and
  give up the typed, strict/lenient preview; and iterating a heterogeneous list needs `dyn` over
  types that hold `Entity`s of different generics. The trait would buy a loop and cost all three.
- **(b) plain methods, called explicitly** — chosen, and proven by the pilot:

```rust
// state/documents.rs, load_imported — at the position the old lines held
        self.opal.on_import(imported.opal);

// project.rs, configs
    insert("opal", value(&state.opal.config()));

// project.rs, assign
        Config::Opal(config) => state.opal.apply_config(config, cx),
```

`decode` and `enum Config` keep one line each per section (`"opal" => decode!(Opal, OpalConfig)` and
`Opal(OpalConfig)`), because decoding runs before the window is touched and must not need it. They
are a typed shim now: the payload type is the section's own `Config`.

## 4. What does not move

- The route, the shared header, the import/export dialogs and the session.
- The decoded `Save`, `save_revision` and `save_loaded`/`save_partial`.
- Cross-section inputs: the research `Simulation` and `SimContext` (All Stats, Damage, Minehead and
  Sushi read them), the character count. They stay on `AppState` and are passed into the hooks
  that need them.
- The shared status line and the run helpers (`start_run`, `cancel_run`, `discard_run`).
- `decode` and `enum Config` (one line each, see 3).

## 5. Migration order and checklist

One section per PR, between feature work. Order by fewest loose fields and no import-order
coupling first, research last:

1. **Already one struct, add the hooks**: `statue`, `pickle`, `coin-multi`, `drop-rate`,
   `coral-god-rank` (+ `coral_run`), `prisma` (+ `prisma_run`), `cog` (+ two runners).
2. **A few loose fields**: `cglunko`, `ladle`, `killroy`, `combat-sim`, `sneaking`,
   `cooking-mastery`, `jar`, `fishing`.
3. **Many fields or runners**: `clamworks`, `fountain`, `spelunking`, `jelly-operator`,
   `royal-guardian`, `worship-td`, `farming`, `sailing`, `monument`.
4. **Reads the research `Simulation`/`Save` during import**: `stats-all`, `damage`.
5. **Last**: `research` (≈23 loose fields, the optimizer runners, Minehead and Sushi).

Per section:

1. Check nothing outside the section reads its fields:
   `rg -n '\.<id>(_[a-z_]+)?\b' crates/app/src --glob '!**/sections/<id>/**' --glob '!**/state/**' --glob '!**/project.rs'`.
   A reader elsewhere is a finding: route it through a method or a shared input first.
2. Create or extend `sections/<id>/state.rs`: move the `AppState` fields, the formula trees and the
   `report_run` calls into `State` and `State::new`; replace them with one `AppState` field and one
   constructor line.
3. Move the section's setters from `impl AppState` onto `State` (taking
   `cx: &mut Context<AppState>`); keep run/cancel/copy on `AppState` in `actions.rs`.
4. Move the section's `load_imported` lines into `on_import`, called at the same position.
5. Move its config struct from `project.rs` into the section as `Config`, unchanged; add
   `config`/`apply_config`; make its `configs` insert and `assign` arm one line each.
6. Add a `project.rs` test that decodes the pre-migration JSON shape by hand and writes it back
   byte for byte (see `a_session_written_before_the_opal_section_owned_its_state_still_restores`).
7. Run `cargo test -p idleon-optimizer`, clippy, fmt, and `node script/mcp-sections.mjs --only <id>`.

## 6. What "one registry entry plus one module" honestly means

After migration a new section touches, outside its own module:

- `sections/mod.rs`: the `pub(crate) mod` line and the registry entry;
- `state/mod.rs`: one field and one constructor line;
- `state/documents.rs`: one `on_import` line in `load_imported`, when it reads the save;
- `project.rs`: one line each in `configs`, `enum Config`, `decode` and `assign` (plus `CONFIG_IDS`
  and `config_value` in its tests), or one `STATELESS` entry;
- `script/mcp-sections.mjs`: its harness interaction.

The sentence `docs/plan.md` §3 now carries:

> Adding a section is one registry entry plus one module under `sections/` that owns its state,
> import step and config (see [`docs/section-state.md`](section-state.md)); the shared core gains
> one `AppState` field and one line each in `load_imported`, `configs`, `Config`, `decode` and
> `assign`, and the harness one interaction.

## 7. Risks

- **Import order.** `load_imported` is order-sensitive today: `stats-all`, `damage`, Minehead and
  Sushi read `self.simulation`/`self.context`/`self.save` after the research step wrote them;
  `fishing`, `farming` and `sailing` read the value they just stored; `clamworks`' claim and
  `combat` read the character count. Each `on_import` call stays at the position its lines held,
  and shared inputs are passed in, never read off another section. Opal reads nothing shared.
- **Config after import.** `project::apply` calls `load_imported` before `assign`, so
  `apply_config` validates against the new save (opal drops a villager the save has not unlocked).
  Hooks must keep that order.
- **Session compatibility.** The config struct moves verbatim, so a session written by an earlier
  build restores unchanged. Plan 016's tests guard the whole path:
  `an_export_writes_every_config_id_and_reads_back_into_the_same_configs` (every id, export →
  decode → assign → export is stable), `every_default_config_decodes_from_its_own_serialisation`
  and `every_built_section_is_a_config_id_or_a_page_the_save_alone_fills`; each migrated section
  adds its own pre-migration shape test.
- **Shared status line.** Completion closures still write `AppState::status`; a section never
  reaches into another section through them.

## Pilot result

The Opal Optimizer moved first. Of the three candidates (`opal`, `clamworks`, `cglunko`) none has a
reader outside its section; opal had six loose fields, one runner, one formula tree, a config and an
import step, and its import reads nothing shared. The section harness
(`node script/mcp-sections.mjs --only opal`) passes.

| Measure | Before | After |
|---|---|---|
| `AppState` fields | 151 | 144 (eight opal fields → one) |
| `AppState::new` lines | 849 | 826 |
| `report_run` calls in `state/mod.rs` | 35 | 34 |
| `load_imported` lines | 136 | 127 (eleven opal lines → one) |
| `configs` / `assign` / `decode` lines | 201 / 425 / 39 | 192 / 410 / 39 |
| opal lines in `project.rs` (config struct, `configs` insert, `assign` arm) | 40 | 2 |

Lines changed outside the section: `state/mod.rs` −44/+20 (15 of the added are the headless test
window, moved here from `project.rs` tests so section tests can build one), `state/documents.rs`
−11/+1, `project.rs` −57/+30 (26 of the added are the pre-migration shape test),
`state/subscriptions.rs` three visibility changes. Under the section: `state.rs` +249 (29 of them
the import test), `actions.rs` −107 net, view files only renamed accessors.

Harder than assumed:

- `decode` cannot move: the import preview decodes before any window state exists, so the typed
  `Config` variant and `decode` arm stay as one-line shims. A trait taking `Value` would have lost
  the strict/lenient preview.
- `report_run` was `pub(super)`; the section needs it `pub(crate)` and its own subscription list.
- Run start/cancel/copy cannot move off `AppState` without passing the status line around; they
  stay `AppState` methods in the section's `actions.rs`, which is where they already were.
- The section's import step had no unit test; plan 016's headless window made one possible
  (`an_import_seeds_the_section_from_the_save_and_drops_the_old_selections`).

**Recommendation: proceed** with the remaining sections as designed, one PR each, in the order of
topic 5.
