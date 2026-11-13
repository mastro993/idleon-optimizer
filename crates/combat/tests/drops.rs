//! The card and obol drop rules against the upstream module's own output.
//!
//! `fixtures/combat-drops.json` is written by `script/combat-drops-oracle.mjs`:
//! it calls `dropOdds`, `lootDropOddsReset`, `generateMonsterDrops` and
//! `tryPickup` from `js/stats/systems/common/combat-sim-rewards.js` on the same
//! filtered rows the port reads, with a fresh seeded stream. Every case here
//! replays one of those calls, so a rule cannot drift from its source.

use idleon_combat::drops::{self, Inputs, Pickup, Row, Snapshot, State};
use idleon_common::Rng;
use serde_json::Value;
use std::collections::BTreeMap;

fn fixture() -> Value {
    let path = format!(
        "{}/../../fixtures/combat-drops.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("the drops fixture is committed"))
        .expect("fixture JSON")
}

fn cases<'a>(value: &'a Value, key: &str) -> &'a Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("the fixture carries {key}"))
}

fn rows(value: &Value) -> Vec<Row> {
    value
        .as_array()
        .expect("rows are an array")
        .iter()
        .map(|row| Row {
            name: row["name"].as_str().expect("a row name").to_string(),
            chance: row["chance"].as_f64().expect("a row chance"),
            qty: row["qty"].as_f64().expect("a row quantity"),
        })
        .collect()
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.0)
}

/// The fixture account's saved card counts, with a case's overrides applied.
fn cards0(account: &Value, overrides: Option<&Value>) -> BTreeMap<String, f64> {
    let mut cards: BTreeMap<String, f64> = account["cards0"]
        .as_object()
        .expect("the account carries Cards0")
        .iter()
        .map(|(key, value)| (key.clone(), number(value)))
        .collect();
    if let Some(overrides) = overrides.and_then(Value::as_object) {
        for (key, value) in overrides {
            cards.insert(key.clone(), number(value));
        }
    }
    cards
}

fn account_inputs(account: &Value) -> Inputs {
    Inputs {
        drop_rate: number(&account["dropRate"]),
        card_chance: number(&account["cardChanceMulti"]),
        talent29: number(&account["talent29"]),
    }
}

/// The snapshot the cases share: the fixture account's card table and obol task.
fn account_snapshot(account: &Value) -> Snapshot {
    Snapshot {
        cards: cards0(account, None),
        own: account["obolInvOwn"]
            .as_array()
            .map(|rows| rows.iter().map(number).collect())
            .unwrap_or_default(),
        fragments: number(&account["obolFragmentBalance"]),
        fragments_known: true,
        auto_trash: account["obolAutoTrash"].as_bool().unwrap_or(false),
        auto_trash_rate: number(&account["obolAutoTrashRate"]),
        tasks_obol: number(&account["tasksObol"]),
        ..Snapshot::default()
    }
}

#[test]
fn the_snapshot_reads_the_fixture_account_like_upstream() {
    let fixture = fixture();
    let account = &fixture["account"];
    let path = format!("{}/../../fixtures/data.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).expect("the fixture save is committed");
    let imported = idleon_import::import(&text).expect("the fixture save imports");
    let snapshot = drops::snapshot(&imported.save);
    assert_eq!(
        account["obolAutoTrash"].as_bool().unwrap_or(false),
        snapshot.auto_trash
    );
    assert_eq!(
        number(&account["obolAutoTrashRate"]),
        snapshot.auto_trash_rate
    );
    assert_eq!(number(&account["tasksObol"]), snapshot.tasks_obol);
    assert_eq!(number(&account["obolFragmentBalance"]), snapshot.fragments);
    assert!(snapshot.fragments_known);
    assert_eq!(
        account["obolInvOwn"]
            .as_array()
            .map(|rows| rows.iter().map(number).collect::<Vec<f64>>())
            .unwrap_or_default(),
        snapshot.own
    );
    assert_eq!(cards0(account, None), snapshot.cards);
    assert!(
        snapshot.pages.iter().any(|page| !page.is_empty()),
        "the fixture has obol pages"
    );
}

#[test]
fn the_drop_odds_match_upstream() {
    let fixture = fixture();
    let mut rng = Rng::new(11);
    for case in cases(&fixture, "oddsCases") {
        let e = number(&case["e"]);
        let p = number(&case["p"]);
        let dr = number(&case["dr"]);
        assert_eq!(
            number(&case["calls"]) as u64,
            rng.calls(),
            "case {case} draws before its roll"
        );
        let got = drops::drop_odds(&mut rng, e, p, dr);
        assert_eq!(number(&case["result"]), got, "case e={e} p={p} dr={dr}");
    }
}

#[test]
fn the_card_odds_reset_matches_upstream() {
    let fixture = fixture();
    let account = &fixture["account"];
    for case in cases(&fixture, "resetCases") {
        let snapshot = Snapshot {
            cards: cards0(account, Some(&case["cards0"])),
            ..account_snapshot(account)
        };
        let state = State::new(snapshot, account_inputs(account));
        for (kind, want) in case["monsters"]
            .as_object()
            .expect("the case carries monsters")
        {
            assert_eq!(
                rows(want),
                state.rows(kind),
                "{kind} rows after the reset for {case}"
            );
        }
        for (name, want) in case["tables"].as_object().expect("the case carries tables") {
            assert_eq!(
                rows(want),
                state.table(name),
                "{name} rows after the reset for {case}"
            );
        }
    }
}

#[test]
fn the_drop_rolls_match_upstream() {
    let fixture = fixture();
    let account = &fixture["account"];
    for case in cases(&fixture, "generateCases") {
        let kind = case["type"].as_str().expect("a monster type");
        let mut state = State::new(account_snapshot(account), account_inputs(account));
        let mut rng = Rng::new(number(&case["seed"]) as u64);
        for (index, want) in case["rolls"]
            .as_array()
            .expect("the case carries rolls")
            .iter()
            .enumerate()
        {
            let got: Vec<(String, f64)> = state.roll(&mut rng, kind);
            let want: Vec<(String, f64)> = want
                .as_array()
                .expect("rolls are arrays")
                .iter()
                .map(|row| {
                    (
                        row[0].as_str().expect("a roll name").to_string(),
                        number(&row[1]),
                    )
                })
                .collect();
            assert_eq!(want, got, "{kind} roll {index}");
        }
    }
}

#[test]
fn the_obol_and_card_pickups_match_upstream() {
    let fixture = fixture();
    let account = &fixture["account"];
    for case in cases(&fixture, "pickupCases") {
        let options = &case["options"];
        let order = options["order"]
            .as_array()
            .map(|pages| {
                pages
                    .iter()
                    .map(|page| {
                        page.as_object()
                            .map(|slots| {
                                slots
                                    .iter()
                                    .filter_map(|(slot, item)| {
                                        Some((slot.parse().ok()?, item.as_str()?.to_string()))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let snapshot = Snapshot {
            cards: cards0(account, Some(&options["cards0"])),
            pages: order,
            own: options["own"]
                .as_array()
                .map(|rows| rows.iter().map(number).collect())
                .unwrap_or_default(),
            fragments: number(&options["balance"]),
            fragments_known: true,
            auto_trash: options["autoTrash"].as_bool().unwrap_or(false),
            auto_trash_rate: number(&options["rate"]),
            tasks_obol: number(&account["tasksObol"]),
        };
        let mut state = State::new(snapshot, account_inputs(account));
        let mut rng = Rng::new(5);
        let out = &case["out"];
        for (entry, want) in case["entries"]
            .as_array()
            .expect("the case carries entries")
            .iter()
            .zip(out["picked"].as_array().expect("picked flags"))
        {
            let name = entry[0].as_str().expect("a drop name");
            let amount = number(&entry[1]);
            let got = state.take(&mut rng, name, amount);
            assert_eq!(
                want.as_bool().unwrap_or(false),
                !matches!(got, Pickup::Unpicked),
                "{name} pickup in {case}"
            );
        }
        let summary = state.summary();
        assert_eq!(
            number(&out["fragments"]),
            summary.fragments_earned,
            "{case}"
        );
        assert_eq!(
            number(&out["autoTrashed"]) as u64,
            summary.fragments_auto_trashed,
            "{case}"
        );
        assert_eq!(number(&out["balance"]), summary.fragments_balance, "{case}");
        let want_cards: BTreeMap<String, f64> = out["cards"]
            .as_object()
            .expect("the case carries cards")
            .iter()
            .map(|(key, value)| (key.clone(), number(value)))
            .collect();
        assert_eq!(want_cards, summary.cards, "{case}");
        assert_eq!(
            number(&out["calls"]) as u64,
            rng.calls(),
            "the case draws its own stream calls"
        );
        assert_eq!(rows(&out["cardRows"]), state.rows("mushG"), "{case}");
        let want_pages: Vec<BTreeMap<usize, String>> = out["pages"]
            .as_array()
            .expect("the case carries pages")
            .iter()
            .map(|page| {
                page.as_object()
                    .map(|slots| {
                        slots
                            .iter()
                            .filter_map(|(slot, item)| {
                                Some((slot.parse().ok()?, item.as_str()?.to_string()))
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(want_pages, state.pages(), "{case}");
    }
}
