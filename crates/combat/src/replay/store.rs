//! The decoder: a frame is read from the nearest keyframe at or before it.

use super::ordered::Ordered;
use super::{KIND_L, KIND_O, KIND_V, SMALL, js_round, number, tenths, uid_key_value};
use serde_json::{Map, Value};

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn byte(&mut self) -> u8 {
        let byte = self.bytes.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        byte
    }

    fn peek(&self) -> u8 {
        self.bytes.get(self.pos).copied().unwrap_or(0)
    }

    fn uv(&mut self) -> u64 {
        let mut value = 0u64;
        let mut mul = 1u64;
        loop {
            let byte = self.byte();
            value += u64::from(byte & 127) * mul;
            mul = mul.saturating_mul(128);
            if byte & 128 == 0 {
                return value;
            }
        }
    }

    fn zz(&mut self) -> i64 {
        let zigzag = self.uv();
        if zigzag % 2 == 1 {
            -(zigzag.div_ceil(2) as i64)
        } else {
            (zigzag / 2) as i64
        }
    }

    fn f64(&mut self) -> f64 {
        let mut bytes = [0u8; 8];
        for (index, slot) in bytes.iter_mut().enumerate() {
            *slot = self.bytes.get(self.pos + index).copied().unwrap_or(0);
        }
        self.pos += 8;
        f64::from_le_bytes(bytes)
    }
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// One batch a recorder hands the store: the new dictionary entries, then the
/// frames encoded against them.
#[derive(Clone, Debug, Default)]
pub struct Batch {
    pub keys_from: Option<usize>,
    pub keys: Vec<String>,
    pub frames: Vec<Vec<u8>>,
}

/// One top-level key's decoded state.
#[derive(Clone, Debug)]
enum StateValue {
    Obj(Map<String, Value>),
    List(Ordered<Map<String, Value>>),
    Value(Value),
}

impl StateValue {
    fn for_kind(kind: u8) -> Self {
        match kind {
            KIND_O => StateValue::Obj(Map::new()),
            KIND_L => StateValue::List(Ordered::new()),
            _ => StateValue::Value(Value::Null),
        }
    }

    fn kind(&self) -> u8 {
        match self {
            StateValue::Obj(_) => KIND_O,
            StateValue::List(_) => KIND_L,
            StateValue::Value(_) => KIND_V,
        }
    }
}

/// The previous list uid's prefix and number, shared by one list's uids.
#[derive(Clone, Debug, Default)]
struct UidState {
    prefix: Option<String>,
    number: i64,
}

/// Decodes encoded frames back into snapshots, from the nearest keyframe.
#[derive(Clone, Debug)]
pub struct Store {
    chunks: Vec<Vec<u8>>,
    frame_chunk: Vec<usize>,
    frame_offset: Vec<usize>,
    frame_len: Vec<usize>,
    times: Vec<Value>,
    keyframes: Vec<usize>,
    keys: Vec<String>,
    bytes: usize,
    current: isize,
    state: Ordered<StateValue>,
    uid: UidState,
    last: Option<(usize, Value)>,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            frame_chunk: Vec::new(),
            frame_offset: Vec::new(),
            frame_len: Vec::new(),
            times: Vec::new(),
            keyframes: Vec::new(),
            keys: Vec::new(),
            bytes: 0,
            current: -1,
            state: Ordered::new(),
            uid: UidState::default(),
            last: None,
        }
    }

    /// Frames the store holds.
    pub fn len(&self) -> usize {
        self.frame_len.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frame_len.is_empty()
    }

    /// The encoded size of every frame.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// The dictionary, in first-seen order.
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// `append`: the dictionary entries first, then the frames.
    pub fn append(&mut self, batch: Batch) {
        if !batch.keys.is_empty() || batch.keys_from.is_some() {
            let from = batch.keys_from.unwrap_or(0);
            if from <= self.keys.len() {
                for (index, key) in batch.keys.into_iter().enumerate() {
                    let slot = from + index;
                    if slot < self.keys.len() {
                        self.keys[slot] = key;
                    } else {
                        self.keys.push(key);
                    }
                }
            }
        }
        for frame in batch.frames {
            self.add_chunk(frame);
        }
    }

    fn add_chunk(&mut self, frame: Vec<u8>) {
        let mut reader = Reader::new(&frame);
        let index = self.frame_len.len();
        if reader.byte() & 1 != 0 {
            self.keyframes.push(index);
        }
        let time = read_value(&self.keys, &mut reader, &mut UidState::default());
        self.times.push(time);
        self.frame_chunk.push(self.chunks.len());
        self.frame_offset.push(0);
        self.frame_len.push(frame.len());
        self.bytes += frame.len();
        self.chunks.push(frame);
    }

    /// `time(k)`: a frame's simulated time without decoding the frame.
    pub fn time(&self, index: usize) -> Option<&Value> {
        self.times.get(index)
    }

    /// The simulated time of every frame, in milliseconds when it is a number.
    pub fn time_ms(&self, index: usize) -> Option<f64> {
        self.times.get(index).and_then(Value::as_f64)
    }

    /// `get(k)`: frame `k` decoded into its snapshot shape.
    pub fn get(&mut self, index: usize) -> Option<Value> {
        if index >= self.len() {
            return None;
        }
        if let Some((cached, snapshot)) = &self.last
            && *cached == index
        {
            return Some(snapshot.clone());
        }
        let keyframe = self.last_keyframe(index)?;
        if !(self.current >= keyframe as isize && (self.current as usize) < index) {
            self.state.clear();
            self.current = keyframe as isize - 1;
        }
        let mut time = Value::Null;
        while self.current < index as isize {
            self.current += 1;
            time = self.apply(self.current as usize);
        }
        let snapshot = self.materialize(time);
        self.last = Some((index, snapshot.clone()));
        Some(snapshot)
    }

    fn frame_bytes(&self, index: usize) -> &[u8] {
        let chunk = self.frame_chunk[index];
        let offset = self.frame_offset[index];
        &self.chunks[chunk][offset..offset + self.frame_len[index]]
    }

    fn last_keyframe(&self, index: usize) -> Option<usize> {
        let mut best = None;
        for keyframe in &self.keyframes {
            if *keyframe <= index {
                best = Some(*keyframe);
            } else {
                break;
            }
        }
        best
    }

    /// Decode one frame into the running state; returns its time.
    fn apply(&mut self, index: usize) -> Value {
        let bytes = self.frame_bytes(index).to_vec();
        let mut reader = Reader::new(&bytes);
        if reader.byte() & 1 != 0 {
            self.state.clear();
        }
        let keys = std::mem::take(&mut self.keys);
        let mut uid = std::mem::take(&mut self.uid);
        let mut state = std::mem::take(&mut self.state);
        let time = read_value(&keys, &mut reader, &mut uid);
        let count = reader.uv();
        for _ in 0..count {
            let key = keys.get(reader.uv() as usize).cloned().unwrap_or_default();
            let kind = reader.byte();
            let existing = state.get(&key).map(StateValue::kind);
            if existing != Some(kind) {
                state.insert(key.clone(), StateValue::for_kind(kind));
            }
            match kind {
                KIND_O => {
                    if let Some(StateValue::Obj(object)) = state.get_mut(&key) {
                        read_fields(&keys, &mut reader, &mut uid, object);
                    }
                }
                KIND_L => {
                    if let Some(StateValue::List(list)) = state.get_mut(&key) {
                        read_list(&keys, &mut reader, &mut uid, list);
                    }
                }
                _ => {
                    let value = read_value(&keys, &mut reader, &mut uid);
                    state.insert(key, StateValue::Value(value));
                }
            }
        }
        self.keys = keys;
        self.uid = uid;
        self.state = state;
        time
    }

    fn materialize(&self, time: Value) -> Value {
        let mut snapshot = Map::new();
        snapshot.insert("t".to_string(), time);
        for (key, value) in &self.state.entries {
            let materialized = match value {
                StateValue::Obj(object) => Value::Object(object.clone()),
                StateValue::List(list) => Value::Array(
                    list.entries
                        .iter()
                        .map(|(_, entry)| Value::Object(entry.clone()))
                        .collect(),
                ),
                StateValue::Value(value) => value.clone(),
            };
            snapshot.insert(key.clone(), materialized);
        }
        Value::Object(snapshot)
    }
}

/// `fields(r, e)`: the changed fields, with deletes.
fn read_fields(
    keys: &[String],
    reader: &mut Reader<'_>,
    uid: &mut UidState,
    object: &mut Map<String, Value>,
) {
    let count = reader.uv();
    for _ in 0..count {
        let field = keys.get(reader.uv() as usize).cloned().unwrap_or_default();
        if reader.peek() == 12 {
            reader.byte();
            object.remove(&field);
            continue;
        }
        let old = object.get(&field).and_then(Value::as_f64);
        let value = read_field_value(keys, reader, uid, old);
        object.insert(field, value);
    }
}

/// `KIND_L` state: upserts against the running uid map, template-reused for new
/// entries, then the removals.
fn read_list(
    keys: &[String],
    reader: &mut Reader<'_>,
    uid: &mut UidState,
    list: &mut Ordered<Map<String, Value>>,
) {
    let upserts = reader.uv();
    let mut template: Option<Map<String, Value>> = None;
    *uid = UidState::default();
    for _ in 0..upserts {
        let id = read_value(keys, reader, uid);
        let key = uid_key_value(&id);
        match list.get_mut(&key) {
            Some(entry) => read_fields(keys, reader, uid, entry),
            None => {
                let mut entry = template.clone().unwrap_or_default();
                entry.insert("uid".to_string(), id);
                read_fields(keys, reader, uid, &mut entry);
                template = Some(entry.clone());
                list.insert(key, entry);
            }
        }
    }
    let gone = reader.uv();
    for _ in 0..gone {
        let id = read_value(keys, reader, uid);
        list.remove(&uid_key_value(&id));
    }
}

/// `fieldVal`: absolute or differential tenths, or a plain value.
fn read_field_value(
    keys: &[String],
    reader: &mut Reader<'_>,
    uid: &mut UidState,
    old: Option<f64>,
) -> Value {
    match reader.peek() {
        9 => {
            reader.byte();
            tenths(reader.zz() as f64 / 10.0)
        }
        10 => {
            reader.byte();
            let base = old.unwrap_or(0.0);
            tenths((js_round(base * 10.0) + reader.zz() as f64) / 10.0)
        }
        _ => read_value(keys, reader, uid),
    }
}

/// `val(r)`: one tagged value.
fn read_value(keys: &[String], reader: &mut Reader<'_>, uid: &mut UidState) -> Value {
    let tag = reader.byte();
    if tag >= SMALL {
        return Value::from(u64::from(tag - SMALL));
    }
    match tag {
        0 => Value::Null,
        1 => Value::Bool(false),
        2 => Value::Bool(true),
        3 => Value::from(reader.uv()),
        4 => Value::from(-(reader.uv() as i64)),
        5 => number(reader.f64()),
        6 => Value::String(keys.get(reader.uv() as usize).cloned().unwrap_or_default()),
        7 => {
            let len = reader.uv() as usize;
            Value::Array((0..len).map(|_| read_value(keys, reader, uid)).collect())
        }
        8 => {
            let len = reader.uv() as usize;
            let mut object = Map::new();
            for _ in 0..len {
                let field = keys.get(reader.uv() as usize).cloned().unwrap_or_default();
                object.insert(field, read_value(keys, reader, uid));
            }
            Value::Object(object)
        }
        11 => {
            let prefix = keys.get(reader.uv() as usize).cloned().unwrap_or_default();
            let number_value = reader.uv() as i64;
            uid.prefix = Some(prefix.clone());
            uid.number = number_value;
            Value::String(format!("{prefix}{number_value}"))
        }
        13 => {
            uid.number += reader.zz();
            let prefix = uid.prefix.clone().unwrap_or_default();
            Value::String(format!("{prefix}{}", uid.number))
        }
        _ => Value::Null,
    }
}

/// Two stores are equal when they hold the same dictionary and encoded frames;
/// the decode cache is not part of the value.
impl PartialEq for Store {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self.keys == other.keys
            && self.times == other.times
            && self.keyframes == other.keyframes
            && self.frame_len == other.frame_len
            && self.chunks == other.chunks
    }
}
