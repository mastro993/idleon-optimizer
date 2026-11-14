//! The Den attempt against the upstream module's own output.
//!
//! `fixtures/combat-den.json` is written by `script/combat-den-oracle.mjs`: it
//! drives upstream's `combat-sim-special.js` Den functions on a fabricated sim
//! — `initSpecial`'s start, the scene's frame, the hounds' 100 ms timer and the
//! kill branch — and records the snapshots every ten seconds. Every case here
//! replays one of those attempts, so a wave, fear or score formula cannot drift
//! from its source.

use idleon_combat::den::{self, Config};
use serde_json::Value;

fn fixture() -> Value {
    let path = format!(
        "{}/../../fixtures/combat-den.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("the den fixture is committed"))
        .expect("fixture JSON")
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.0)
}

fn config(value: &Value) -> Config {
    let mut amps = [0.0; 8];
    for (index, level) in value["amps"]
        .as_array()
        .expect("the case carries amplifiers")
        .iter()
        .enumerate()
        .take(8)
    {
        amps[index] = number(level);
    }
    Config {
        amps,
        b_upg23: number(&value["bUpg23"]),
        bolaia_level: number(&value["bolaiaLevel"]),
        best_score: number(&value["bestScore"]),
        opal_level: number(&value["opalLevel"]),
        hound_hp: number(&value["houndHp"]),
    }
}

fn check_snapshot(want: &Value, got: &idleon_combat::den::Snapshot, note: &str) {
    assert_eq!(number(&want["active"]), got.active, "{note}: active");
    assert_eq!(number(&want["fear"]), got.fear, "{note}: fear");
    assert_eq!(number(&want["fearMax"]), got.fear_max, "{note}: fear max");
    assert_eq!(number(&want["score"]), got.score, "{note}: score");
    assert_eq!(number(&want["kills"]), got.kills, "{note}: kills");
    assert_eq!(number(&want["wave"]), got.wave, "{note}: wave");
    assert_eq!(number(&want["tokens"]), got.tokens, "{note}: tokens");
    assert_eq!(
        number(&want["ampMulti"]),
        got.amp_multi,
        "{note}: amp multi"
    );
    assert_eq!(number(&want["chain"]), got.chain, "{note}: chain");
    assert_eq!(number(&want["runs"]), got.runs, "{note}: runs");
    assert_eq!(number(&want["opals"]), got.opals, "{note}: opals");
}

#[test]
fn the_amplifier_formulas_match_upstream() {
    let fixture = fixture();
    for case in fixture["formulaCases"].as_array().expect("formula cases") {
        let mut amps = [0.0; 8];
        for (index, level) in case["amps"].as_array().expect("amps").iter().enumerate() {
            amps[index] = number(level);
        }
        for (kind, want) in case["debuff"]
            .as_array()
            .expect("debuff")
            .iter()
            .enumerate()
        {
            assert_eq!(
                number(want),
                den::amp_debuff(&amps, kind),
                "ampDebuff({kind}) for {amps:?}"
            );
        }
        for entry in case["multi"].as_array().expect("multi") {
            let b_upg = number(&entry["bUpg"]);
            assert_eq!(
                number(&entry["value"]),
                den::amp_multi(&amps, b_upg),
                "ampMulti with B_UPG {b_upg} for {amps:?}"
            );
        }
    }
    for case in fixture["opalCases"].as_array().expect("opal cases") {
        assert_eq!(
            number(&case["requirement"]),
            den::next_opal_score(number(&case["level"])),
            "next opal at level {}",
            case["level"]
        );
    }
}

#[test]
fn the_projected_attempt_matches_upstream() {
    let fixture = fixture();
    for case in fixture["cases"].as_array().expect("cases") {
        let note = case["note"].as_str().unwrap_or("");
        let config = config(&case["config"]);
        let kill_every = case["killEveryMs"].as_u64().expect("kill interval");
        let seconds = case["seconds"].as_u64().expect("seconds");
        let seed = case["seed"].as_u64().expect("seed");
        let outcome = den::project(&config, kill_every, seconds * 1000, seed);
        let expected = &case["expected"];
        for mark in expected["marks"].as_array().expect("marks") {
            let at = mark["tMs"].as_u64().expect("a mark");
            let partial = den::project(&config, kill_every, at, seed);
            check_snapshot(mark, &partial.snapshot, &format!("{note} at {at} ms"));
        }
        let want = &expected["den"];
        check_snapshot(want, &outcome.snapshot, note);
        let totals = &expected["totals"];
        assert_eq!(number(&totals["runs"]), outcome.totals.runs, "{note}");
        assert_eq!(
            number(&totals["houndsSpawned"]),
            outcome.totals.hounds_spawned,
            "{note}"
        );
        assert_eq!(
            number(&totals["houndsKilled"]),
            outcome.totals.hounds_killed,
            "{note}"
        );
        assert_eq!(number(&totals["opals"]), outcome.totals.opals, "{note}");
        assert_eq!(
            number(&totals["fearGained"]),
            outcome.totals.fear_gained,
            "{note}"
        );
        assert_eq!(
            number(&totals["playerKills"]),
            outcome.totals.player_kills,
            "{note}"
        );
        // Every wave is on the run's own clock, in order, and the count at each
        // ten-second mark is the one the module's snapshot reports there.
        assert_eq!(
            outcome.waves.len() as f64,
            outcome.snapshot.wave,
            "{note}: one entry per wave"
        );
        let mut previous = 0;
        for (index, wave) in outcome.waves.iter().enumerate() {
            assert_eq!(wave.wave as usize, index + 1, "{note}: wave numbers");
            assert!(
                wave.t_ms < debug_duration(case),
                "{note}: wave {} after the run",
                wave.wave
            );
            assert!(wave.t_ms >= previous, "{note}: waves out of order");
            previous = wave.t_ms;
        }
        for mark in expected["marks"].as_array().expect("marks") {
            let at = mark["tMs"].as_u64().expect("a mark");
            let started = outcome.waves.iter().filter(|wave| wave.t_ms < at).count() as f64;
            assert_eq!(
                started,
                number(&mark["wave"]),
                "{note}: waves started by {at} ms"
            );
        }
    }
}

fn debug_duration(case: &serde_json::Value) -> u64 {
    case["seconds"].as_u64().unwrap_or(0) * 1000
}

#[test]
fn an_attempt_reports_the_upstream_status_words() {
    let fixture = fixture();
    let case = &fixture["cases"][0];
    let config = config(&case["config"]);
    let kill_every = case["killEveryMs"].as_u64().expect("kill interval");
    let seed = case["seed"].as_u64().expect("seed");
    let running = den::project(&config, kill_every, 30_000, seed);
    assert_eq!(running.status(), "In progress");
    let second = &fixture["cases"][1];
    let ended = den::project(
        &config,
        second["killEveryMs"].as_u64().unwrap(),
        60_000,
        second["seed"].as_u64().unwrap(),
    );
    assert_eq!(ended.status(), "Ending");
    let settled = den::project(
        &config,
        second["killEveryMs"].as_u64().unwrap(),
        70_000,
        second["seed"].as_u64().unwrap(),
    );
    assert_eq!(settled.status(), "Ended");
    let untouched = den::project(&Config::default(), 1000, 0, 1);
    assert_eq!(untouched.totals.runs, 1.0);
}
