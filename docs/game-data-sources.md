# Game data sources

Where to find Legends of Idleon formulas and data values when adding or updating a calculator.
Vetted 2026-11-16; re-check licenses and freshness before relying on a row.

## Sources

| Source | Gives | Use it to |
| --- | --- | --- |
| [Web client `N.js`](https://www.legendsofidleon.com/ytGl5oc/N.js) | Ground truth: every formula (`_customBlock_*` functions) and data table (`CustomLists` and friends), W1–W7. Minified Haxe output, ~26 MB. | Read the rule, then write it in our own Rust. Proprietary: keep it out of the repo. |
| [idleon.wiki `Module:*/data.json`](https://idleon.wiki/api.php?action=query&list=allpages&apnamespace=828&aplimit=500&format=json) | Bot-generated JSON from the client: curve parameters (`decay x1/x2`, `add`), costs, some expression strings. | Look up and cross-check values. CC BY-NC-SA: never copy the files in. |
| [idleon.wiki `Module:Logic/sandbox`](https://idleon.wiki/wiki/Module:Logic/sandbox/data.json) | Decompiled client rules as pseudo-code with call graphs (e.g. the whole `ExpMulti` chain). Sandbox, partial coverage. | Understand how bonuses stack. Read only. |
| [idleon.wiki `Module:Changelog/<ver>`](https://idleon.wiki/wiki/Module:Changelog/1.25/data.json?action=raw) | Per-build before/after values. | Spot which tables a patch changed. |
| [Arkh-BR/idleon-leaderboards-tracker](https://github.com/Arkh-BR/idleon-leaderboards-tracker) | TypeScript readings of ~85 `_customBlock_*` functions citing `N.js` offsets, golden tests, and an `N.js` change-detection pipeline. | MIT on `main` since #85 (2026-11-10): code may be reused with attribution after checking it against `N.js`. Its `generated/` and `njs-snapshot/` content is the game's, not MIT. |
| [Morta1/IdleonToolbox](https://github.com/Morta1/IdleonToolbox) | The broadest open calculator; comments name the game function behind each formula (`// game: SushiStuff(...)`); `website-data/*.json` values. | GPL-3.0: read to learn the facts, then write our own code. |
| [idleon-efficiency `docs/game-snippets`](https://github.com/Sludging/idleon-efficiency/tree/main/docs/game-snippets) | ~156 client functions, one folder per game version, so formula changes show as diffs. | See when and how a formula changed. Read only (no license). |
| [Sludging/IdleonWikiBot3.0](https://github.com/Sludging/IdleonWikiBot3.0) | Static tables as JSON in `exported/repo/**`; maps tables to `N.js` sections. | Find which client table holds a value. Read only (no license). |
| [Steam News, app 1476970](https://api.steampowered.com/ISteamNews/GetNewsForApp/v2/?appid=1476970&count=20&format=json) | Official patch notes; no formulas. | Know a patch landed and which systems it touched. |

## Adding or checking a formula

1. Download the current `N.js` into a gitignored scratch folder (e.g. `target/game/`).
2. Find the rule in `N.js` by its `_customBlock_<Name>` function or table name; the wiki Logic
   sandbox, Toolbox comments and the efficiency snippets help locate it.
3. Write it in Rust in our own words; put the values the calculator needs in its crate's
   `data/*.json` in our own schema.
4. Cross-check the result against at least one independent reading (wiki data, Toolbox).
   Evaluating the client's own function in a Node `vm` over the scratch `N.js` gives exact
   expected values for a test.

## Taking game art

The web client keeps its images in `lib/default.pak` beside `N.js`: a `lime-asset-pack` header,
then one gzip stream per file. `N.js` carries the pack's manifest (search for `"assets":"aoy8:`), a
Haxe-serialized list giving each file's `id` (`assets/data/<Name>.png` for the named sprites),
`position` and `length` in the pack. Download both into the gitignored scratch folder, gunzip
the entries you need and copy only those images under `assets/game/`, each with a manifest entry
(`path`, `bytes`, `sha256`). The images are the game's art, credited in
`assets/game/README.md`; the pack itself never gets committed.

## Staying current

- Poll `N.js` with a HEAD request: its `ETag` and `Last-Modified` change on every patch. Re-read the
  URL from the game's `index.html` if it 404s, since the `ytGl5oc` segment can change.
- Pin each extraction to that `ETag`/`Last-Modified`; version labels differ between sites
  (wiki "1.25", Toolbox "2.3.561").
- Diff table names and string keys between builds, not code: the minifier renames identifiers on
  every build.

## Licensing

- Facts (numbers, formula maths) rewritten in our own code are fine to use from any source above.
- Code, JSON layouts and verbatim tables are someone's expression: copy only from MIT sources
  (Arkh after #85), with attribution.
- `N.js` and other client code never get committed.
- Skip lywood/activeon-calculator (AGPL with an explicit text-and-data-mining opt-out),
  DigitalTQ (terms forbid automated access), injector tools, and anything that touches live
  accounts or cloud saves.
- Open question for the owner: LavaFlame2 publishes no ToS; the Steam Subscriber Agreement limits
  reverse engineering and EU database rights may cover bulk table extraction. Take only the
  values each calculator needs.
