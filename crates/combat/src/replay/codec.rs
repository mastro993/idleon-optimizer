//! The delta encoder: one frame per snapshot, holding only what changed.

use super::ordered::Ordered;
use super::{
    KIND_L, KIND_O, KIND_V, QUANTIZED, SAFE_INTEGER, SMALL, SMALL_MAX, js_round, number, quantize,
    same,
};
use serde_json::{Map, Value};
use std::collections::HashMap;

/// `Number.isSafeInteger` without a negative zero.
pub(super) fn safe_integer(value: f64) -> bool {
    is_safe_integer(value)
}

/// `Number.isSafeInteger` without a negative zero.
fn is_safe_integer(value: f64) -> bool {
    value.is_finite()
        && value.fract() == 0.0
        && value.abs() <= SAFE_INTEGER
        && !(value == 0.0 && value.is_sign_negative())
}

// ---------------------------------------------------------------------------
// Byte cursor and writer
// ---------------------------------------------------------------------------

/// A growable little-endian byte writer.
#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn reset(&mut self) {
        self.bytes.clear();
    }

    fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    /// Unsigned varint, exact up to 2^53 like upstream's `uv`.
    fn uv(&mut self, mut value: u64) {
        while value >= 128 {
            self.byte((value % 128) as u8 | 128);
            value /= 128;
        }
        self.byte(value as u8);
    }

    /// Zigzag varint, so small negative deltas stay one byte.
    fn zz(&mut self, value: i64) {
        let zigzag = if value >= 0 {
            (value as u64) * 2
        } else {
            (-value as u64) * 2 - 1
        };
        self.uv(zigzag);
    }

    fn f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
}

// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------

/// The previous frame's stored shape for one top-level key.
#[derive(Clone, Debug)]
enum Prev {
    Obj(Map<String, Value>),
    List(Ordered<Map<String, Value>>),
    Value(String),
}

/// A dictionary batch: the entries added since the last call.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keys {
    pub from: usize,
    pub keys: Vec<String>,
}

/// The delta encoder: push snapshots in order, get one frame per snapshot.
pub struct Encoder {
    keyframe_every: usize,
    keys: Vec<String>,
    index: HashMap<String, usize>,
    sent: usize,
    prev: Ordered<Prev>,
    count: usize,
    bytes: usize,
    writer: Writer,
}

impl Encoder {
    /// `createReplayEncoder`: at least one frame between keyframes, defaulting
    /// to the extracted interval.
    pub fn new(keyframe_every: usize) -> Self {
        let keyframe_every = if keyframe_every == 0 {
            super::keyframe_every()
        } else {
            keyframe_every
        };
        Self {
            keyframe_every: keyframe_every.max(1),
            keys: Vec::new(),
            index: HashMap::new(),
            sent: 0,
            prev: Ordered::new(),
            count: 0,
            bytes: 0,
            writer: Writer::default(),
        }
    }

    /// The whole dictionary, in the order keys were first seen.
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// Frames pushed so far.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Bytes the frames have taken.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Dictionary entries added since the previous call (`newKeys`).
    pub fn new_keys(&mut self) -> Keys {
        let from = self.sent;
        self.sent = self.keys.len();
        Keys {
            from,
            keys: self.keys[from..].to_vec(),
        }
    }

    fn key_index(&mut self, key: &str) -> usize {
        if let Some(index) = self.index.get(key) {
            return *index;
        }
        let index = self.keys.len();
        self.keys.push(key.to_string());
        self.index.insert(key.to_string(), index);
        index
    }

    /// `push`: one snapshot in, one delta-encoded frame out.
    pub fn push(&mut self, snapshot: &Value) -> Vec<u8> {
        let snapshot = snapshot.as_object().cloned().unwrap_or_default();
        let keyframe = self.count.is_multiple_of(self.keyframe_every);
        if keyframe {
            self.prev.clear();
        }
        self.writer.reset();
        self.writer.byte(if keyframe { 1 } else { 0 });
        let time = snapshot
            .get("t")
            .and_then(Value::as_f64)
            .filter(|time| *time != 0.0)
            .unwrap_or(0.0);
        self.write_value(&number(time));

        // Work out which top-level keys changed before writing the count.
        let mut parts = Vec::new();
        let mut next: Ordered<Prev> = Ordered::new();
        for (key, value) in &snapshot {
            if key == "t" {
                continue;
            }
            let kind = kind_of(value);
            let previous = self
                .prev
                .get(key)
                .filter(|previous| kind_of_prev(previous) == kind);
            match kind {
                KIND_O => {
                    let old = match previous {
                        Some(Prev::Obj(old)) => Some(old),
                        _ => None,
                    };
                    if let Some(old) = old
                        && !changed(old, value)
                    {
                        next.insert(
                            key.clone(),
                            previous.cloned().unwrap_or(Prev::Value(String::new())),
                        );
                        continue;
                    }
                    parts.push((key.clone(), kind, value.clone()));
                }
                KIND_L => {
                    let old = match previous {
                        Some(Prev::List(old)) => Some(old),
                        _ => None,
                    };
                    let dirty = list_dirty(old, value);
                    if !dirty {
                        next.insert(
                            key.clone(),
                            previous.cloned().unwrap_or(Prev::Value(String::new())),
                        );
                        continue;
                    }
                    parts.push((key.clone(), kind, value.clone()));
                }
                _ => {
                    let serialized = serde_json::to_string(value).unwrap_or_default();
                    if matches!(previous, Some(Prev::Value(old)) if *old == serialized) {
                        next.insert(
                            key.clone(),
                            previous.cloned().unwrap_or(Prev::Value(String::new())),
                        );
                        continue;
                    }
                    parts.push((key.clone(), kind, value.clone()));
                }
            }
        }

        self.writer.uv(parts.len() as u64);
        for (key, kind, value) in parts {
            let index = self.key_index(&key);
            self.writer.uv(index as u64);
            self.writer.byte(kind);
            let previous = self.prev.get(&key).cloned();
            match kind {
                KIND_O => {
                    let old = match previous {
                        Some(Prev::Obj(old)) => Some(old),
                        _ => None,
                    };
                    let delta = self.obj_delta(old.as_ref(), &value, false);
                    next.insert(key, Prev::Obj(delta));
                }
                KIND_L => {
                    let old = match previous {
                        Some(Prev::List(old)) => Some(old),
                        _ => None,
                    };
                    let list = self.list_delta(old.as_ref(), &value);
                    next.insert(key, Prev::List(list));
                }
                _ => {
                    self.write_value(&value);
                    let serialized = serde_json::to_string(&value).unwrap_or_default();
                    next.insert(key, Prev::Value(serialized));
                }
            }
        }
        self.prev = next;
        self.count += 1;
        self.bytes += self.writer.bytes.len();
        self.writer.bytes.clone()
    }

    fn write_value(&mut self, value: &Value) {
        match value {
            Value::Null => self.writer.byte(0),
            Value::Bool(false) => self.writer.byte(1),
            Value::Bool(true) => self.writer.byte(2),
            Value::Number(entry) => match entry.as_f64() {
                Some(raw) if raw.is_finite() && safe_integer(raw) => {
                    if (0.0..=SMALL_MAX).contains(&raw) {
                        self.writer.byte(SMALL + raw as u8);
                    } else if raw >= 0.0 {
                        self.writer.byte(3);
                        self.writer.uv(raw as u64);
                    } else {
                        self.writer.byte(4);
                        self.writer.uv((-raw) as u64);
                    }
                }
                Some(raw) if raw.is_finite() => {
                    self.writer.byte(5);
                    self.writer.f64(raw);
                }
                // JSON cannot carry an infinity or a NaN; upstream writes null.
                _ => self.writer.byte(0),
            },
            Value::String(text) => {
                let index = self.key_index(text);
                self.writer.byte(6);
                self.writer.uv(index as u64);
            }
            Value::Array(items) => {
                self.writer.byte(7);
                self.writer.uv(items.len() as u64);
                for item in items {
                    self.write_value(item);
                }
            }
            Value::Object(fields) => {
                self.writer.byte(8);
                self.writer.uv(fields.len() as u64);
                for (field, value) in fields {
                    let index = self.key_index(field);
                    self.writer.uv(index as u64);
                    self.write_value(value);
                }
            }
        }
    }

    /// `objDelta`: the changed fields of `value` against `old`, plus deletes.
    fn obj_delta(
        &mut self,
        old: Option<&Map<String, Value>>,
        value: &Value,
        skip_uid: bool,
    ) -> Map<String, Value> {
        let object = value.as_object().cloned().unwrap_or_default();
        let mut next = Map::new();
        let mut changed_fields: Vec<String> = Vec::new();
        for (field, raw) in &object {
            if skip_uid && field == "uid" {
                next.insert(field.clone(), raw.clone());
                continue;
            }
            let quantized = quantize(field, raw);
            match old.and_then(|old| old.get(field)) {
                Some(previous) if same(previous, &quantized) => {}
                _ => changed_fields.push(field.clone()),
            }
            next.insert(field.clone(), quantized);
        }
        let mut gone: Vec<String> = Vec::new();
        if let Some(old) = old {
            for field in old.keys() {
                if !object.contains_key(field) && !(skip_uid && field == "uid") {
                    gone.push(field.clone());
                }
            }
        }
        self.writer.uv((changed_fields.len() + gone.len()) as u64);
        for field in &changed_fields {
            let index = self.key_index(field);
            self.writer.uv(index as u64);
            let value = next.get(field).cloned().unwrap_or(Value::Null);
            let previous = old.and_then(|old| old.get(field));
            self.field_value(field, &value, previous);
        }
        for field in &gone {
            let index = self.key_index(field);
            self.writer.uv(index as u64);
            self.writer.byte(12);
        }
        next
    }

    /// `fieldVal`: quantized numbers as absolute or differential tenths.
    fn field_value(&mut self, field: &str, value: &Value, old: Option<&Value>) {
        if QUANTIZED.contains(&field)
            && let Value::Number(entry) = value
            && let Some(raw) = entry.as_f64()
            && raw.is_finite()
        {
            let tenths = js_round(raw * 10.0) as i64;
            match old.and_then(Value::as_f64).filter(|old| old.is_finite()) {
                Some(old) => {
                    self.writer.byte(10);
                    self.writer.zz(tenths - js_round(old * 10.0) as i64);
                }
                None => {
                    self.writer.byte(9);
                    self.writer.zz(tenths);
                }
            }
            return;
        }
        self.write_value(value);
    }

    /// `uidVal`: a list uid as `prefix + number`, interned and delta-encoded.
    fn uid_value(&mut self, value: &Value, uid_prefix: &mut Option<String>, uid_number: &mut i64) {
        let Some((prefix, number_value)) = split_uid(value) else {
            self.write_value(value);
            return;
        };
        if uid_prefix.as_deref() == Some(prefix.as_str()) {
            self.writer.byte(13);
            self.writer.zz(number_value - *uid_number);
        } else {
            let index = self.key_index(&prefix);
            self.writer.byte(11);
            self.writer.uv(index as u64);
            self.writer.uv(number_value as u64);
            *uid_prefix = Some(prefix);
        }
        *uid_number = number_value;
    }

    /// `KIND_L` delta: upserts against the previous uid map, template-reused for
    /// new entries, then the removals sorted by uid.
    fn list_delta(
        &mut self,
        old: Option<&Ordered<Map<String, Value>>>,
        value: &Value,
    ) -> Ordered<Map<String, Value>> {
        let items = value.as_array().cloned().unwrap_or_default();
        let mut upserts: Vec<Value> = Vec::new();
        let mut next: Ordered<Map<String, Value>> = Ordered::new();
        for item in &items {
            let key = uid_key(item);
            match old.and_then(|old| old.get(&key)) {
                Some(previous) if !changed(previous, item) => {
                    next.insert(key, previous.clone());
                }
                _ => upserts.push(item.clone()),
            }
        }
        self.writer.uv(upserts.len() as u64);
        let mut prefix: Option<String> = None;
        let mut number_value = 0i64;
        let mut template: Option<Map<String, Value>> = None;
        for item in &upserts {
            let uid = item.get("uid").cloned().unwrap_or(Value::Null);
            let key = uid_key(item);
            self.uid_value(&uid, &mut prefix, &mut number_value);
            match old.and_then(|old| old.get(&key)) {
                Some(previous) => {
                    let delta = self.obj_delta(Some(previous), item, true);
                    next.insert(key, delta);
                }
                None => {
                    // A new entry is a delta from the previous new entry, or
                    // from `{ uid }` for the first one this frame.
                    let base = template.clone().unwrap_or_else(|| {
                        let mut base = Map::new();
                        base.insert("uid".to_string(), uid.clone());
                        base
                    });
                    let delta = self.obj_delta(Some(&base), item, true);
                    template = Some(delta.clone());
                    next.insert(key, delta);
                }
            }
        }
        let mut gone: Vec<String> = Vec::new();
        if let Some(old) = old {
            for key in old.keys() {
                if !next.contains(key) {
                    gone.push(key.clone());
                }
            }
        }
        gone.sort_by(|left, right| uid_order(left, right));
        self.writer.uv(gone.len() as u64);
        for key in &gone {
            self.uid_value(&Value::String(key.clone()), &mut prefix, &mut number_value);
        }
        next
    }
}

fn kind_of(value: &Value) -> u8 {
    match value {
        Value::Array(items) => {
            if items.iter().all(|item| {
                matches!(item, Value::Object(fields) if !fields.get("uid").is_none_or(Value::is_null))
            }) {
                KIND_L
            } else {
                KIND_V
            }
        }
        Value::Object(_) => KIND_O,
        _ => KIND_V,
    }
}

fn kind_of_prev(previous: &Prev) -> u8 {
    match previous {
        Prev::Obj(_) => KIND_O,
        Prev::List(_) => KIND_L,
        Prev::Value(_) => KIND_V,
    }
}

fn changed(old: &Map<String, Value>, value: &Value) -> bool {
    let object = match value.as_object() {
        Some(object) => object,
        None => return true,
    };
    for (field, raw) in object {
        match old.get(field) {
            Some(previous) if same(previous, &quantize(field, raw)) => {}
            _ => return true,
        }
    }
    old.keys().any(|field| !object.contains_key(field))
}

fn list_dirty(old: Option<&Ordered<Map<String, Value>>>, value: &Value) -> bool {
    let items = value.as_array().map(Vec::as_slice).unwrap_or_default();
    let Some(old) = old else {
        return true;
    };
    if old.len() != items.len() {
        return true;
    }
    items.iter().any(|item| {
        old.get(&uid_key(item))
            .is_none_or(|previous| changed(previous, item))
    })
}

/// The `uid` a list entry is keyed by: the string itself, or the value's JSON.
fn uid_key(value: &Value) -> String {
    value
        .get("uid")
        .map(super::uid_key_value)
        .unwrap_or_default()
}

/// `^(\D*)(0|[1-9]\d{0,14})$`: a non-digit prefix and a number without leading
/// zeros, at most 15 digits.
fn split_uid(value: &Value) -> Option<(String, i64)> {
    let Value::String(text) = value else {
        return None;
    };
    let digits = text.trim_end_matches(|ch: char| ch.is_ascii_digit());
    if digits.len() == text.len() {
        return None;
    }
    let number = &text[digits.len()..];
    if number.len() > 1 && number.starts_with('0') {
        return None;
    }
    if number.len() > 15 {
        return None;
    }
    number
        .parse::<i64>()
        .ok()
        .map(|number| (digits.to_string(), number))
}

/// `_uidOrder`: matching uids sort by prefix then number, ahead of the rest.
fn uid_order(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let left_uid = split_uid(&Value::String(left.to_string()));
    let right_uid = split_uid(&Value::String(right.to_string()));
    match (left_uid, right_uid) {
        (Some((left_prefix, left_number)), Some((right_prefix, right_number))) => left_prefix
            .cmp(&right_prefix)
            .then_with(|| left_number.cmp(&right_number)),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

/// `packReplayFrames`: the frames concatenated, plus each one's length.
pub fn pack_frames(frames: &[Vec<u8>]) -> (Vec<u8>, Vec<u32>) {
    let mut packed = Vec::with_capacity(frames.iter().map(Vec::len).sum());
    let mut lens = Vec::with_capacity(frames.len());
    for frame in frames {
        packed.extend_from_slice(frame);
        lens.push(frame.len() as u32);
    }
    (packed, lens)
}
