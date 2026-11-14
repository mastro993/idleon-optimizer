//! The replay codec against the upstream module's own bytes and frames.
//!
//! `fixtures/combat-replay.json` is written by `script/combat-replay-oracle.mjs`:
//! it pushes a scripted snapshot series through upstream's `createReplayEncoder`
//! and reads it back through `createReplayStore`. The encoder cases here push the
//! same snapshots and must produce the same bytes, frame for frame; the store
//! cases decode upstream's own frames and must materialize the same snapshots.
//! The Killroy handoff cases are upstream's `_validate` verdicts.

use base64::Engine as _;
use idleon_combat::replay::{self, Batch, Encoder, Store};
use serde_json::Value as Json;

fn fixture() -> Json {
    serde_json::from_str(include_str!("../../../fixtures/combat-replay.json"))
        .expect("fixtures/combat-replay.json is valid")
}

fn decode(text: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .expect("the fixture frames are base64")
}

#[test]
fn the_encoder_writes_the_same_bytes_as_upstream() {
    let fixture = fixture();
    let codec = &fixture["codec"];
    let snapshots = fixture["snapshots"].as_array().expect("a snapshot series");
    let keyframe_every = codec["keyframeEvery"]
        .as_u64()
        .expect("a keyframe interval") as usize;
    let batch_size = codec["batch"].as_u64().expect("a batch size") as usize;
    let mut encoder = Encoder::new(keyframe_every);
    let mut frames: Vec<Vec<u8>> = Vec::new();
    let want_frames: Vec<&Json> = codec["frames"]
        .as_array()
        .expect("encoded frames")
        .iter()
        .collect();
    let want_batches: Vec<&Json> = codec["batches"]
        .as_array()
        .expect("dictionary batches")
        .iter()
        .collect();
    let mut batch = 0;
    for (seen_frames, snapshot) in snapshots.iter().enumerate() {
        let bytes = encoder.push(snapshot);
        assert_eq!(
            decode(want_frames[seen_frames].as_str().expect("a base64 frame")),
            bytes,
            "frame {seen_frames} differs from the upstream encoder"
        );
        frames.push(bytes);
        if frames.len().is_multiple_of(batch_size) {
            let keys = encoder.new_keys();
            let want = &want_batches[batch];
            assert_eq!(
                keys.from,
                want["keysFrom"].as_u64().unwrap_or(0) as usize,
                "batch {batch}: dictionary offset"
            );
            let want_keys: Vec<&str> = want["keys"]
                .as_array()
                .expect("batch keys")
                .iter()
                .map(|key| key.as_str().expect("a key"))
                .collect();
            assert_eq!(keys.keys, want_keys, "batch {batch}: dictionary entries");
            batch += 1;
        }
    }
    if batch < want_batches.len() {
        let keys = encoder.new_keys();
        let want = &want_batches[batch];
        assert_eq!(keys.from, want["keysFrom"].as_u64().unwrap_or(0) as usize);
        let want_keys: Vec<&str> = want["keys"]
            .as_array()
            .expect("batch keys")
            .iter()
            .map(|key| key.as_str().expect("a key"))
            .collect();
        assert_eq!(keys.keys, want_keys);
    }
    assert_eq!(frames.len(), want_frames.len(), "frame count");
    assert_eq!(
        encoder.bytes(),
        codec["bytes"].as_u64().expect("the encoded size") as usize,
        "encoded size"
    );
    let dictionary: Vec<&str> = codec["dictionary"]
        .as_array()
        .expect("the dictionary")
        .iter()
        .map(|key| key.as_str().expect("a key"))
        .collect();
    assert_eq!(encoder.keys(), dictionary, "dictionary order");
}

#[test]
fn the_store_materializes_upstreams_frames() {
    let fixture = fixture();
    let codec = &fixture["codec"];
    let want: Vec<&Json> = codec["decoded"]
        .as_array()
        .expect("decoded frames")
        .iter()
        .collect();
    let frames: Vec<Vec<u8>> = codec["frames"]
        .as_array()
        .expect("encoded frames")
        .iter()
        .map(|frame| decode(frame.as_str().expect("a base64 frame")))
        .collect();
    let mut store = Store::new();
    let mut frame = 0;
    for batch in codec["batches"].as_array().expect("dictionary batches") {
        let keys: Vec<String> = batch["keys"]
            .as_array()
            .expect("batch keys")
            .iter()
            .map(|key| key.as_str().expect("a key").to_string())
            .collect();
        let count = batch["frames"].as_array().expect("batch frames").len();
        let batch_frames = frames[frame..frame + count].to_vec();
        store.append(Batch {
            keys_from: batch["keysFrom"].as_u64().map(|from| from as usize),
            keys,
            frames: batch_frames,
        });
        frame += count;
    }
    assert_eq!(store.len(), want.len(), "the store holds every frame");
    // Forward, backward and repeated reads all decode from the nearest keyframe.
    for (index, expected) in want.iter().enumerate() {
        let got = store.get(index).expect("a decoded frame");
        assert_eq!(&got, *expected, "frame {index}");
    }
    for (index, expected) in want.iter().enumerate().rev() {
        let got = store.get(index).expect("a decoded frame");
        assert_eq!(&got, *expected, "frame {index} backwards");
    }
    assert_eq!(
        store.bytes(),
        codec["bytes"].as_u64().expect("the encoded size") as usize
    );
    // The frame times come from the frame heads, without decoding the deltas.
    for (index, frame) in want.iter().enumerate() {
        assert_eq!(
            store.time_ms(index).map(|time| time.round() as i64),
            frame["t"].as_f64().map(|time| time.round() as i64),
            "frame {index} time"
        );
    }
}

#[test]
fn pack_frames_concatenates_with_their_lengths() {
    let frames = vec![vec![1, 2, 3], Vec::new(), vec![4, 5]];
    let (packed, lens) = replay::pack_frames(&frames);
    assert_eq!(packed, vec![1, 2, 3, 4, 5]);
    assert_eq!(lens, vec![3, 0, 2]);
}

#[test]
fn the_killroy_handoff_matches_upstreams_validation() {
    let fixture = fixture();
    let killroy = &fixture["killroy"];
    assert!(killroy["readBack"].as_bool().expect("a read-back verdict"));
    for case in killroy["cases"].as_array().expect("the Killroy cases") {
        let note = case["note"].as_str().expect("a note");
        let payload = &case["payload"];
        let verdict = replay::read_killroy_replay(payload);
        match (case["error"].as_str(), verdict) {
            (None, Ok(replay)) => {
                assert_eq!(replay.char_idx, 0, "{note}: character");
                assert_eq!(replay.label, "Room practice", "{note}: label");
                assert_eq!(replay.seconds, 300.0, "{note}: duration");
                assert_eq!(replay.seed, 20_260_101, "{note}: seed");
                assert!(replay.keep_buffs, "{note}: buffs");
                assert!(!replay.unlimited_inventory, "{note}: inventory");
            }
            (None, Err(error)) => panic!("{note}: upstream accepts this payload, got {error}"),
            (Some(want), Ok(_)) => panic!("{note}: upstream rejects this payload with {want}"),
            (Some(want), Err(error)) => assert_eq!(error, want, "{note}: error message"),
        }
    }
    let captured = &killroy["captured"];
    let replay = replay::read_killroy_replay(captured).expect("the captured payload is replayable");
    let room = replay.map_idx;
    assert!(
        replay::killroy_maps().contains(&room),
        "the captured map is a Killroy room"
    );
    assert!(
        replay::killroy_maps().len() > 1,
        "the extracted room map list is not empty"
    );
}

#[test]
fn a_captured_replay_round_trips_through_its_payload() {
    let raw =
        serde_json::json!({ "Lv0_0": "[1,2]", "CharacterClass_0": "[0,0]", "charNames": ["A"] });
    let replay = replay::KillroyReplay::capture(
        raw,
        0,
        replay::killroy_maps()[0] as usize,
        60.0,
        7,
        "Room practice",
        false,
        true,
        serde_json::json!({}),
        serde_json::json!({}),
    );
    replay.validate().expect("a captured replay is valid");
    let payload = serde_json::to_value(&replay).expect("a payload");
    assert_eq!(
        replay::read_killroy_replay(&payload).expect("read back"),
        replay
    );
    assert_eq!(
        replay::read_killroy_replay(&payload).expect("read back again"),
        replay,
        "a payload reads back to the same replay"
    );
}
