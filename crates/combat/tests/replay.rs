//! The replay codec round trip, frame packing and the Killroy handoff payload.

use idleon_combat::replay::{self, Batch, Encoder, Store};
use serde_json::{Value, json};

/// A short run: monsters move, take damage, die and drop a field, and one
/// leaves the roster.
fn snapshots() -> Vec<Value> {
    (0..10)
        .map(|frame| {
            let t = f64::from(frame) * 100.0;
            let mut monsters = vec![
                json!({ "uid": "m1", "kind": "mushG", "x": 10.5 + f64::from(frame), "y": 20.0, "hp": 100.0 - f64::from(frame) * 10.0, "alive": true }),
                json!({ "uid": "m2", "kind": "frogG", "x": 300.0, "y": 40.5, "hp": 50.0, "alive": frame < 6 }),
            ];
            if frame < 4 {
                monsters.push(json!({ "uid": "m3", "kind": "beanG", "x": 5.0, "y": 5.0, "hp": 1.0, "alive": true }));
            }
            if frame % 3 == 0 {
                monsters[0]["label"] = json!("hit");
            }
            json!({ "t": t, "monsters": monsters })
        })
        .map(integral)
        .collect()
}

/// The store writes a whole number back as an integer, so the expected
/// snapshots do too.
fn integral(value: Value) -> Value {
    match value {
        Value::Number(number) => match number.as_f64() {
            Some(float) if float.fract() == 0.0 => json!(float as i64),
            _ => Value::Number(number),
        },
        Value::Array(values) => values.into_iter().map(integral).collect(),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, integral(value)))
                .collect(),
        ),
        value => value,
    }
}

#[test]
fn every_encoded_frame_decodes_back_to_its_snapshot() {
    let snapshots = snapshots();
    let mut encoder = Encoder::new(4);
    let mut store = Store::new();
    for chunk in snapshots.chunks(3) {
        let frames = chunk
            .iter()
            .map(|snapshot| encoder.push(snapshot))
            .collect();
        let keys = encoder.new_keys();
        store.append(Batch {
            keys_from: Some(keys.from),
            keys: keys.keys,
            frames,
        });
    }
    assert_eq!(store.len(), snapshots.len());
    assert_eq!(encoder.count(), snapshots.len());
    // Forward, backward and repeated reads all decode from the nearest keyframe.
    for (index, expected) in snapshots.iter().enumerate() {
        assert_eq!(store.get(index).as_ref(), Some(expected), "frame {index}");
        assert_eq!(
            store.time_ms(index),
            expected["t"].as_f64(),
            "frame {index} time"
        );
    }
    for (index, expected) in snapshots.iter().enumerate().rev() {
        assert_eq!(
            store.get(index).as_ref(),
            Some(expected),
            "frame {index} backwards"
        );
    }
    assert_eq!(store.get(snapshots.len()), None);
}

#[test]
fn pack_frames_concatenates_with_their_lengths() {
    let frames = vec![vec![1, 2, 3], Vec::new(), vec![4, 5]];
    let (packed, lens) = replay::pack_frames(&frames);
    assert_eq!(packed, vec![1, 2, 3, 4, 5]);
    assert_eq!(lens, vec![3, 0, 2]);
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
