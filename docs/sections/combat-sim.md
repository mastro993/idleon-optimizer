# Combat Simulator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M2`
- Section key: `combat-sim`
- Group: `general`
- Status: `partial`
- Depends on: `foundation`

Implement the active combat simulator: loadout editor, mechanics overrides, component terms, encounter simulation, comparisons, Killroy, obols, cards, kills-over-time and replays.

## Current build

- CS-01 ships the loadout editor: the character picker, the 16 armor and 7 tool slots with their saved `EMm` upgrade deltas, and the talent table read from `SL_<n>`/`SM_<n>`.
- Item pickers read the `crates/combat/data/combat.json` catalog (871 wearable items) and filter by the game's armor and tool slot rules.
- Every edit recomputes the gear totals the header shows: weapon power, defence, the class main stat and the summed talent points, so the summary always matches the editor. It is the simulator's input preview; the encounter run reports monster damage and kill outcomes, while character DPS waits for the auto-mode player.
- Missing or invalid equipment is flagged: an item key outside the catalog, an item in the wrong slot family, a level or class requirement the character misses, and a saved talent level above its saved maximum.
- Named loadouts are presets of item keys and talent levels. They live in the project JSON under `sections.combat-sim` and survive an export/import round trip; applying one replaces the working loadout like equipping each slot by hand.
- CS-02 ships the mechanics overrides: the catalog of 68 mechanics in 11 groups lives in `crates/combat/data/combat-overrides.json`, and `crates/combat/src/overrides.rs` keeps one rule per mechanic id, normalizes it and resolves it against a run's saved inputs (`off` writes the neutral value, `set` the typed value, `scale` saved × value, the flags with their own keys, the per-monster object, and `raw:<input>` ids for any other input).
- The Mechanics tab lists the catalog with a search field, a group filter and an only-changed filter; each row picks Saved, Off, On, Set or Scale × and a value, the active rules show as removable chips above the tabs, and Reset all puts every input back to the save. The overrides live in `sections.combat-sim` under `toggles`, so a project export carries them.
- CS-03 ships the encounter run: `crates/combat/src/encounter.rs` replays the game's fixed 10 ms step, its render-frame batch and its timed-task loop over one map's spawn roster, then the spawn, one-second respawn tick, draw-time respawn and kill handlers. Two players run without the auto-mode AI: the fixed kill interval stand-in and the idle player.
- `crates/combat/data/encounters.json` holds every map whose scene the run supports (124 spawn rosters with their base monster HP and respawn time).
- The Encounter tab picks a map, the stand-in or idle player, the kill interval and the 1-minute to 24-hour duration, then runs through the shared task runner: progress, Cancel, the cancelled notice and the stale marker are the shared ones, and a cancelled run keeps the previous result. The result card reports kills, damage, damage per second, kills per hour, the mean kill interval, and the kills by monster.
- The mechanics the run cannot model are listed under the options instead of being dropped: the auto-mode player AI (so no character DPS or survival yet), the reward engine's EXP, coins, equipment and inventory halves (a full obol page without the auto-trash sends the drop to a bag slot), crystal/giant/Plunder spawns, the account monster tuning, The Den, the Rift, caverns, Clam Work and dungeons, and replays.
- CS-04 ships the comparison: `crates/combat/src/compare.rs` plans one row per character, loadout and map, runs the same encounter on every row, reuses a map's outcome because the stand-in run reads no loadout, and gives every metric its paired delta against the first row.
- The Compare tab picks characters, their imported save, working edits and named loadouts, and the maps to run. The result table shows the run's kills per hour, damage per second and mean kill interval beside the setup's weapon power, defence, main stat and talent points, each with its delta against the baseline row; Export copies the comparison document as JSON.
- Neutral this build: the auto-mode player AI and its component-term trace, the saved values Scale multiplies, the project-to-run wiring of the mechanics overrides beyond the drop multipliers, and the Den's other caverns and its run's map roster.
- CS-05 ships the results: `crates/combat/src/drops.rs` implements the card and obol half of the reward engine (drop odds, monster drop rolls, the drop-odds reset and the card and obol pickups), and the encounter run rolls a kill's rows on the seed's mulberry32 stream after booking it. `crates/combat/data/drops.json` holds the card, obol and drop-table rows of every supported map's monsters, the obol pages, the card descriptions and the drop-odds reset splices.
- The run's drop inputs are the Mechanics tab's Drop rate, Card drop chance multiplier and Rares Everywhere! bonus (`off` is neutral, `set` applies, and the save-derived values `scale` multiplies are not wired yet), plus the saved obol task. The run reads the saved `Cards0`, `ObolInvOr`/`ObolInvOwn`, fragment balance, Silver Jellyfish unlock and auto-trash rate, so cards and fragments continue from the imported account.
- The results are tabs over the one stored run: Summary (kills, damage, rate, interval), Killroy, Obols, Kills, Cards and Timeline. Killroy practice re-runs the room's native timer at the kill rate this run measured, through `idleon_killroy::trial`; the tab reports kills, skulls, talent points, credits, pearls and spawned monsters. A tab with nothing to show says why, and the Seed field makes every roll reproducible.
- The kills timeline samples the run's cumulative kills every ten simulated seconds and ends on the run's own kills and simulated time; the Timeline tab draws it with the shared line chart, so the chart and the summary can never disagree.
- CS-06 ships the progress and smelting results: `crates/combat/src/progress.rs` implements the forge (stats, production, refills, grants and the 250 ms forge tick), the banked pet-time batch, the Scurge, Coral Kid and Omega Arcade kill credits and the progress summary. The run advances the saved furnace on its own clock, banks each kill's credits and stores the bars it produced; the Smelting tab lists them, the Progress tab reports the grants, the Scurge/Coral/Omega notes and every notice the run raises.
- `crates/combat/data/progress.json` holds the forge item rows, the Spelunky Scurge column and the pet names.
- `crates/stats/src/combat.rs` decodes the combat progress inputs from the save: the `ForgeLV`/`ForgeItemOrder`/`ForgeItemQty`/`ForgeIntProg` furnace, its `ChestOrder` storage, the `Lab` refill gate, the gem slots and the cap and speed terms through the account decoder (cosmo, arcade, `ForgeCap` stamp, bribes, vault, achievement, Rift rank, the `Godshard` card and the Menagerie boost).
- The two terms this build cannot decode are named instead of guessed: the star-sign 57b term of the shiny-pet speed and the per-kill forge, pet and alchemy grants (talents 280, 362 and 490) need the talent and star-sign formula tables, so they read neutral and the Progress tab prints both warnings under the cards.
- CS-06 also ships The Den as its own projection: `crates/combat/src/den.rs` implements the amplifier debuff and multiplier, the next opal score requirement, the Den's start state, its wave, fear-token and end rules, the hounds' 100 ms timer, the cave setup, hound spawns and hound kills. The Den tab replays the cavern's hound waves at the encounter run's kill interval, seed and duration, and reports the score, kills, waves, fear, Skull chain and opals. The projection is the attempt only: the map's other monsters and the player's movement are not part of it, which the tab says.
- The comparison export now carries each row's progress: the forge, pet and alchemy seconds, the Scurge EXP, the Coral kills, the Omega balls and every produced bar by display name.
- CS-07 ships the replays: `crates/combat/src/replay.rs` implements a delta codec — the encoder that stores only what changed since the previous frame, a full state every keyframe interval, 0.1 px quantization for positions, tenth deltas, interned dictionary entries and uid lists with template reuse — plus frame packing and the store that decodes a frame off the nearest keyframe. `crates/combat/data/replay.json` holds the Killroy room maps and the codec's defaults.
- Every encounter run records its own frames — 100 ms apart, coarsened in 100 ms steps so a day keeps at most 10,000: the roster's monsters with their type, node, actor x and y, HP, alive flag and respawn counter. The spawn's x within a platform is a draw this run does not model, so a node's spawns take an even share of its span. The frames are encoded as the run goes and stored on its result.
- The Replay results tab draws that run: the map's background and platform art, every monster's animation sheet on its spawn node, the stand-in on the scene's player node, HP bars, and a labelled placeholder for any file the asset manifest cannot resolve. It has Play/Pause, speeds from 0.5× to 300×, a scrubber over the frames and, when the run has a Den projection, the wave boundaries to step between.
- A Killroy practice captures as a version-1 replay document: the imported save, the character, the map, duration and seed, validated on open (a character the save carries, a Killroy room, 1 s to a day, a 32-bit seed). Opening it restores those options and re-simulates the run. It lives in the session, not the project.
- The replay never draws what the run does not have: the player's attacks, projectiles, damage numbers and the scene's decorations are named on the tab instead of faked.

## Known gaps

- `CS-G1` No character DPS or survival — blocked by `combat:auto-player` (absent)

## UI

- Loadout editor and mechanics overrides
- Component terms and simulator options
- Encounter simulation workspace
- Results tabs: comparison, Killroy, obols, kills by monster, cards, kills over time, The Den, combat-driven progress, smelting

## Inputs

- Character save data
- Loadout and equipment
- Mechanics overrides
- Encounter and map selection
- Simulator options

## Outputs

- DPS/TTK and kill outcomes
- Comparison tables
- Killroy and card progress
- Replay data
- Warnings for unsupported mechanics

## Assets

assets/combat-sim (maps, mobs, player, bullets, items, UI); 38 MB of committed artwork.

## Feature issues

### CS-01 — Loadout editor

Edit character, equipment, talents and loadout presets with save-backed defaults.

Depends on: FND-01 Dashboard card grid, FND-03 Shared save JSON import, FND-08 Shared layout, theme and responsive rules, FND-09 Empty, loading, error and stale states

Acceptance:

- Loadout changes update the simulation immediately.
- Invalid or missing items are flagged.
- Presets save in project JSON.

### CS-02 — Mechanics overrides

Override combat mechanics and component terms with searchable groups and reset-to-save behavior.

Depends on: CS-01 Loadout editor

Acceptance:

- Every mechanic override is present.
- Overrides are visible as active chips.
- Reset restores save values.

### CS-03 — Encounter simulator

Run active combat against the selected encounter with its options and progress reporting.

Depends on: CS-02 Mechanics overrides, FND-12 Background task runner with progress and cancel

Acceptance:

- Runs can be cancelled.
- Unsupported mechanics are listed, not silently ignored.

### CS-04 — Character, loadout and map comparison

Side-by-side comparison across characters, loadouts and maps.

Depends on: CS-03 Encounter simulator

Acceptance:

- Comparison selects two or more setups.
- Deltas are shown per metric.
- Export copies the comparison as JSON.

### CS-05 — Killroy, obols, cards and kills

Killroy encounter results, obol fragments, kills by monster, card drops and kills-over-time chart.

Depends on: CS-03 Encounter simulator, FND-11 Shared formula tree and breakdown component

Acceptance:

- Each result tab renders from the same run.
- Charts match the simulated timeline.
- Empty runs show a clear state.

### CS-06 — The Den, progress and smelting

Combat-driven progress, The Den outcomes and smelting output panels.

Depends on: CS-03 Encounter simulator

Acceptance:

- Missing inputs are warned.
- Values feed the comparison export.

### CS-07 — Replays and combat assets

Play, scrub and speed-control combat replays using the game's sprites and maps.

Depends on: CS-03 Encounter simulator, FND-06 Game asset pipeline

Acceptance:

- Replay loads from a run or Killroy replay.
- Scrub, speed and wave controls work.
- Assets resolve through the manifest with placeholders.

## Verification

- The loadout readers decode the equipment and talents of the committed synthetic save.
- The item catalog covers every equipped item in that save and every slot has picker options.
- Presets round-trip through the project JSON and refuse another character's loadout.
- The section harness imports a save, edits an item and a talent, flags an unknown item and saves a preset.
- Every override mode resolves against a run's saved inputs.
- The section harness sets a mechanic off, sees its chip and resets every override.
- The encounter run reports kills, damage, kills by monster and respawn counters for the stand-in and idle players.
- A cancelled encounter run stores nothing, and the section harness runs a map from the Encounter tab and reads the result.
- Every comparison cell carries its delta against the baseline row, and the section harness compares two characters and copies the comparison JSON.
- The card and obol odds, reset and pickups follow the seeded stream, so a seed reproduces the drops.
- The kills timeline ends on the run's own kills and simulated time, and an idle run draws no drops.
- The section harness runs a map and reads the Summary, Killroy, Obols, Kills, Cards and Timeline tabs from one result, and sees each tab's empty state before the first run.
- Formula tests cover the forge grants, ticks, refills, gem slots and unavailable state, the pet batch and the Scurge, Coral and Omega credits.
- The save-derived progress config decodes from the committed synthetic save, and the missing star-sign and talent-grant terms are returned as warnings.
- Formula tests cover the Den's amplifiers, multiplier and opal requirement, and a scripted attempt's wave, fear, score, chain and opal totals.
- The section harness runs a map and reads the Den, Progress and Smelting tabs from one result; a run with the saved furnace lists its bars, and an unavailable furnace shows its warning.
- The replay encoder round-trips: the store materializes the encoded frames back into the same snapshots, frame for frame.
- The Killroy handoff's capture accepts valid documents and rejects invalid ones, and a captured replay round-trips through its payload.
- A recorded run's frames start at 0, end on the run's own clock and follow its kills and respawns; a long run coarsens its interval to stay under the frame cap.
- Every map's background, platform and monster sheet in `encounters.json` resolves through the asset manifest, so the replay's references cannot drift from the committed artwork.
- The section harness plays, scrubs, steps between waves and captures/opens a Killroy replay from the Replay tab, and captures the section screenshot there.
