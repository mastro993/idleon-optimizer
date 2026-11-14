//! The encounter run against the upstream engine's own outcomes.
//!
//! `fixtures/combat-encounter.json` and `crates/combat/data/encounters.json` are
//! written together by `script/combat-encounter-oracle.mjs` from one upstream
//! checkout, so a case that disagrees here is a port bug, not a stale
//! expectation. The fixture runs the upstream stand-in and idle players at
//! neutral monster tuning with crystal, giant and plunder spawns off.

use idleon_combat::drops::{self, Inputs};
use idleon_combat::encounter::{self, Options, Outcome, Player};
use idleon_common::{Control, Progress};
use serde_json::Value as Json;

fn fixture() -> Json {
    serde_json::from_str(include_str!("../../../fixtures/combat-encounter.json"))
        .expect("fixtures/combat-encounter.json is valid")
}

fn run(options: Options) -> Outcome {
    encounter::validate(&options).expect("the fixture case is runnable");
    encounter::run(options, &mut |_| Control::Continue)
        .expect("the run starts")
        .expect("the run reaches its end")
}

fn case_options(case: &Json) -> Options {
    let player = match case["player"].as_str().expect("a player name") {
        "idle" => Player::Idle,
        "standIn" => Player::StandIn {
            kill_every_ms: case["killEveryMs"].as_u64().expect("a kill interval"),
        },
        other => panic!("unknown player {other}"),
    };
    Options::basic(
        case["map"].as_u64().expect("a map") as usize,
        player,
        case["seconds"].as_u64().expect("a duration") * 1000,
    )
}

#[test]
fn every_fixture_case_matches_the_upstream_run() {
    for case in fixture()["cases"].as_array().expect("cases") {
        let expected = &case["expected"];
        let options = case_options(case);
        let label = format!("map {} {}", options.map, options.player.id());
        let outcome = run(options);

        assert_eq!(
            outcome.simulated_ms,
            expected["simulatedMs"].as_u64().expect("simulatedMs"),
            "{label}: simulated time"
        );
        assert_eq!(
            outcome.steps,
            expected["steps"].as_u64().expect("steps"),
            "{label}: steps"
        );
        assert_eq!(
            outcome.frames,
            expected["frames"].as_u64().expect("frames"),
            "{label}: frames"
        );
        assert_eq!(
            outcome.kills,
            expected["kills"].as_u64().expect("kills"),
            "{label}: kills"
        );
        assert!(
            (outcome.damage - expected["damage"].as_f64().expect("damage")).abs() < 1e-6,
            "{label}: damage {} vs {}",
            outcome.damage,
            expected["damage"]
        );

        let expected_types = expected["killsByType"].as_object().expect("killsByType");
        assert_eq!(
            outcome.kills_by_type.len(),
            expected_types.len(),
            "{label}: killed monster types {:?}",
            outcome.kills_by_type
        );
        for (kind, count) in expected_types {
            assert_eq!(
                outcome.kills_by_type.get(kind),
                Some(&count.as_u64().expect("a kill count")),
                "{label}: kills of {kind}"
            );
        }

        let expected_respawn = expected["respawn"].as_array().expect("respawn");
        assert_eq!(
            outcome.respawn.len(),
            expected_respawn.len(),
            "{label}: roster size"
        );
        for (index, (value, expected)) in outcome.respawn.iter().zip(expected_respawn).enumerate() {
            // `null` is a monster the run never killed, so upstream has no
            // counter for it; the port's counter is still at its ready value.
            let Some(expected) = expected.as_f64() else {
                assert!(*value <= 0.0, "{label}: monster {index} never died");
                continue;
            };
            assert!(
                (value - expected).abs() < 1e-9,
                "{label}: monster {index} respawn {value} vs {expected}"
            );
        }
    }
}

#[test]
fn the_stand_in_ticks_are_kills_plus_idle_ticks() {
    let case = &fixture()["cases"][0];
    let options = case_options(case);
    let player = options.player;
    let outcome = run(options);
    let Player::StandIn { kill_every_ms } = player else {
        panic!("the first case uses the stand-in")
    };
    assert_eq!(
        outcome.kill_ticks(),
        outcome.simulated_ms / kill_every_ms,
        "every kill tick either kills a monster or finds none"
    );
    assert!(
        outcome.kill_interval_ms().is_some(),
        "a run that killed something reports its interval"
    );
}

#[test]
fn the_idle_player_kills_nothing() {
    let case = fixture()["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["player"] == "idle")
        .cloned()
        .expect("an idle case");
    let outcome = run(case_options(&case));
    assert_eq!(outcome.kills, 0);
    assert_eq!(outcome.damage, 0.0);
    assert!(outcome.kills_by_type.is_empty());
    assert_eq!(outcome.idle_ticks, 0, "the idle player has no kill tick");
}

#[test]
fn a_cancelled_run_reports_nothing() {
    let options = Options::basic(1, Player::StandIn { kill_every_ms: 500 }, 600_000);
    let mut reports = 0;
    let outcome = encounter::run(options, &mut |progress: Progress| {
        reports += 1;
        assert!((0.0..=1.0).contains(&progress.fraction));
        if progress.fraction >= 0.25 {
            Control::Stop
        } else {
            Control::Continue
        }
    })
    .expect("the run starts");
    assert!(outcome.is_none(), "a cancelled run stores nothing");
    assert!(reports >= 2, "the run reported progress before stopping");
}

#[test]
fn every_supported_map_runs_and_is_not_excluded() {
    assert!(
        encounter::maps().len() > 100,
        "the extractor keeps the maps"
    );
    for map in encounter::maps() {
        assert!(!map.mobs.is_empty(), "map {} spawns monsters", map.index);
        assert!(
            encounter::excluded(map.index).is_none(),
            "map {} is both supported and excluded",
            map.index
        );
        let options = Options::basic(
            map.index,
            Player::StandIn {
                kill_every_ms: 1_500,
            },
            1_000,
        );
        assert!(
            encounter::validate(&options).is_ok(),
            "map {} validates",
            map.index
        );
    }
}

#[test]
fn an_excluded_map_reports_why() {
    let options = Options::basic(216, Player::Idle, 1_000);
    let error = encounter::validate(&options).expect_err("map 216 is not supported");
    assert!(
        error.contains("cavern"),
        "the reason names the gap: {error}"
    );
    assert!(encounter::run(options, &mut |_| Control::Continue).is_err());
}

#[test]
fn the_unsupported_list_names_the_unported_halves() {
    let unsupported = encounter::unsupported().join(" ").to_lowercase();
    for part in [
        "auto-mode player ai",
        "reward engine",
        "crystals, giants",
        "account monster tuning",
        "killroy",
        "the replay records",
    ] {
        assert!(
            unsupported.contains(part),
            "the list names {part}: {unsupported}"
        );
    }
}

#[test]
fn the_stand_in_interval_must_land_on_a_step() {
    let options = Options::basic(
        1,
        Player::StandIn {
            kill_every_ms: 1_505,
        },
        1_000,
    );
    assert!(encounter::validate(&options).is_err());
}

#[test]
fn the_timeline_ends_on_the_runs_own_numbers() {
    let options = Options::basic(1, Player::StandIn { kill_every_ms: 700 }, 45_000);
    let outcome = run(options);
    assert_eq!(outcome.timeline[0].t_ms, encounter::TIMELINE_MS);
    for pair in outcome.timeline.windows(2) {
        assert!(
            pair[1].kills >= pair[0].kills,
            "kills only grow: {:?}",
            outcome.timeline
        );
        assert!(pair[1].t_ms > pair[0].t_ms, "samples advance in time");
    }
    let last = outcome.timeline.last().expect("a run samples its end");
    assert_eq!(
        last.kills, outcome.kills,
        "the chart ends on the run's kills"
    );
    assert_eq!(last.t_ms, outcome.simulated_ms, "and on its simulated time");
}

#[test]
fn an_idle_run_draws_no_drops() {
    let options = Options::basic(1, Player::Idle, 60_000);
    let outcome = run(options);
    assert_eq!(outcome.kills, 0);
    assert!(outcome.drops.cards.is_empty());
    assert_eq!(outcome.drops.fragments_earned, 0.0);
    assert_eq!(outcome.drops.obols_unpicked, 0);
    assert!(outcome.timeline.iter().all(|sample| sample.kills == 0));
}

#[test]
fn the_fixture_accounts_drops_are_reproducible() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    let imported = idleon_import::import(&text).expect("the fixture imports");
    let snapshot = drops::snapshot(&imported.save);
    assert!(snapshot.auto_trash, "the fixture has Silver Jellyfish");
    assert!(snapshot.fragments_known);
    assert!(
        snapshot.cards.values().any(|count| *count > 0.0),
        "the fixture owns cards"
    );
    let options = Options {
        map: 1,
        player: Player::StandIn { kill_every_ms: 200 },
        duration_ms: 600_000,
        seed: 7,
        drops: snapshot,
        // The fixture's own stats, so the rolls land like the account's would.
        drop_inputs: Inputs {
            drop_rate: 4_950.574_185_831_907,
            card_chance: 17.875_638_048_625_472,
            talent29: 0.0,
        },
        progress: idleon_combat::progress::Config::default(),
        grants: idleon_combat::progress::Grants::default(),
        replay: Some(idleon_combat::replay::snapshot_ms()),
    };
    let outcome = run(options.clone());
    assert!(outcome.kills > 0);
    assert!(!outcome.drops.cards.is_empty(), "the run finds cards");
    let again = run(options);
    assert_eq!(outcome.drops, again.drops, "a seed reproduces the rolls");
}

#[test]
fn a_run_advances_the_saved_furnace_and_its_progress() {
    // One ore, one oil and one bar with a one-second interval and four bars'
    // worth of ore: the run's own 250 ms clock produces while it replays.
    let mut forge = idleon_combat::progress::Forge {
        available: true,
        order: vec!["Blank".to_string(); 48],
        qty: vec![0.0; 48],
        progress: vec![0.0; 16],
        slots: 16.0,
        cap: 1e6,
        speed: 1.0,
        bars: 1.0,
        oil_use: 1.0,
        ..idleon_combat::progress::Forge::default()
    };
    forge.order[0] = "Copper".into();
    forge.order[2] = "CopperBar".into();
    forge.qty[0] = 8.0;
    let mut options = Options::basic(1, Player::StandIn { kill_every_ms: 500 }, 60_000);
    options.progress = idleon_combat::progress::Config {
        forge,
        coral: idleon_combat::progress::Coral {
            unlocked: true,
            kills: 10.0,
        },
        ..idleon_combat::progress::Config::default()
    };
    options.grants = idleon_combat::progress::Grants {
        kill_credit: 2.5,
        ..idleon_combat::progress::Grants::default()
    };
    let outcome = run(options.clone());
    assert!(outcome.progress.forge_available);
    let forge = outcome.progress.forge.as_ref().expect("the furnace state");
    let produced: f64 = outcome.progress.bars.values().sum();
    assert!(produced >= 1.0, "the run smelts: {produced}");
    assert!(forge.qty[0] < 8.0, "the run spends ore: {}", forge.qty[0]);
    let credited: u64 = outcome
        .kills_by_type
        .iter()
        .filter(|(kind, _)| idleon_combat::drops::rolls(kind))
        .map(|(_, kills)| *kills)
        .sum();
    assert_eq!(outcome.progress.coral_kills, credited as f64 * 2.5);
    let again = run(options);
    assert_eq!(
        outcome.progress, again.progress,
        "a seed reproduces the furnace"
    );
}

#[test]
fn a_recorded_run_keeps_frames_on_its_own_clock() {
    use idleon_combat::replay;

    let mut options = Options::basic(1, Player::StandIn { kill_every_ms: 500 }, 120_000);
    options.replay = Some(replay::snapshot_ms());
    let outcome = run(options);
    let replay = outcome.replay.as_ref().expect("the run records a replay");
    assert_eq!(replay.interval_ms(), replay::snapshot_ms());
    assert_eq!(replay.time_ms(0), Some(0.0), "the replay starts at t=0");
    assert_eq!(
        replay.last_ms(),
        Some(outcome.simulated_ms as f64),
        "the replay ends on the run's clock"
    );
    assert_eq!(
        replay.len() as u64,
        outcome.simulated_ms / replay::snapshot_ms() + 1,
        "one frame per interval, plus both ends"
    );
    let first = replay.frame(0).expect("the first frame decodes");
    let monsters = first["monsters"].as_array().expect("the roster");
    let map = encounter::map(1).expect("map 1");
    assert_eq!(monsters.len(), map.mobs.len(), "the whole roster is drawn");
    // Every monster sits on its spawn platform: the x is inside the node and
    // the y above it, because this run never moves them.
    for monster in monsters {
        let node = &map.art.nodes[monster["node"].as_u64().expect("a node") as usize];
        let x = monster["x"].as_f64().expect("an x");
        let y = monster["y"].as_f64().expect("a y");
        assert!(
            x >= node.x && x <= node.length,
            "monster x {x} is off node {}..{}",
            node.x,
            node.length
        );
        assert!(y <= node.y, "monster y {y} is below its platform");
    }
    let last = replay
        .frame(replay.len() - 1)
        .expect("the last frame decodes");
    assert_ne!(
        first["monsters"], last["monsters"],
        "the run's kills and respawns reach the frames"
    );
    assert!(replay.bytes() > 0, "the frames are encoded");
    assert!(
        run(Options::basic(1, Player::Idle, 1_000)).replay.is_none(),
        "a run that asks for no replay keeps no frames"
    );
    // The idle player changes nothing but the clock and the respawn counters.
    let mut idle = Options::basic(1, Player::Idle, 1_000);
    idle.replay = Some(replay::snapshot_ms());
    let idle = run(idle);
    let replay = idle.replay.as_ref().expect("the idle run records");
    let state = |frame: &serde_json::Value| {
        frame["monsters"]
            .as_array()
            .expect("the roster")
            .iter()
            .map(|monster| {
                (
                    monster["uid"].clone(),
                    monster["hp"].clone(),
                    monster["alive"].clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    let first = replay.frame(0).expect("the first frame");
    let last = replay.frame(replay.len() - 1).expect("the last frame");
    assert_eq!(state(&first), state(&last), "nothing took damage or died");
}

#[test]
fn a_long_run_records_at_a_coarser_interval() {
    use idleon_combat::replay;
    assert_eq!(replay::interval_for(120_000), 100, "a short run at 100 ms");
    assert_eq!(replay::interval_for(3_600_000), 400, "an hour at 400 ms");
    assert_eq!(
        replay::interval_for(86_400_000),
        8_700,
        "a day fits the frame cap"
    );
    assert!(
        86_400_000 / replay::interval_for(86_400_000) <= replay::MAX_FRAMES,
        "the interval keeps the run under the cap"
    );
}
