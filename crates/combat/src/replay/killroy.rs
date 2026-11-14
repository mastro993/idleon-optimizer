//! The Killroy practice's replay handoff.

use super::{INVALID_KILLROY_REPLAY, killroy_maps};
use idleon_save::Save;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Killroy replay handoff
// ---------------------------------------------------------------------------

/// A captured Killroy replay: the imported save and the run it opens.
///
/// Ported from `captureKillroyReplay`, `storeKillroyReplay`, `readKillroyReplay`
/// and `_validate` in `js/ui/killroy-replay.js`. The payload is the same
/// version-1 document upstream writes to session storage, so a replay captured
/// from this build's Killroy tab both round-trips through the project and is
/// checked with the game's own rules.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillroyReplay {
    pub version: u32,
    /// The imported save, unchanged: the replay reads it, never the live edits.
    pub raw: Value,
    pub char_idx: i64,
    pub map_idx: i64,
    pub seconds: f64,
    pub seed: i64,
    pub label: String,
    pub keep_buffs: bool,
    pub unlimited_inventory: bool,
    #[serde(default)]
    pub toggles: Value,
    #[serde(default)]
    pub terms: Value,
}

impl KillroyReplay {
    /// `captureKillroyReplay`: everything a Killroy practice needs to re-open.
    #[allow(clippy::too_many_arguments)]
    pub fn capture(
        raw: Value,
        char_idx: usize,
        map_idx: usize,
        seconds: f64,
        seed: i64,
        label: impl Into<String>,
        keep_buffs: bool,
        unlimited_inventory: bool,
        toggles: Value,
        terms: Value,
    ) -> Self {
        Self {
            version: 1,
            raw,
            char_idx: char_idx as i64,
            map_idx: map_idx as i64,
            seconds,
            seed,
            label: label.into(),
            keep_buffs,
            unlimited_inventory,
            toggles,
            terms,
        }
    }

    /// `_validate`: the rules a payload must pass to be replayed.
    pub fn validate(&self) -> Result<(), String> {
        let invalid = || INVALID_KILLROY_REPLAY.to_string();
        if self.version != 1 || !is_truthy(&self.raw) {
            return Err(invalid());
        }
        let characters = Save::new(self.raw.clone()).character_count() as i64;
        if self.char_idx < 0 || self.char_idx >= characters {
            return Err(invalid());
        }
        if !killroy_maps().contains(&self.map_idx) {
            return Err(invalid());
        }
        if !self.seconds.is_finite() || !(1.0..=86_400.0).contains(&self.seconds) {
            return Err(invalid());
        }
        if self.seed < 1 || self.seed > 0xffff_ffff {
            return Err(invalid());
        }
        Ok(())
    }
}

/// `readKillroyReplay`: parse and validate a stored payload.
pub fn read_killroy_replay(payload: &Value) -> Result<KillroyReplay, String> {
    let replay: KillroyReplay =
        serde_json::from_value(payload.clone()).map_err(|_| INVALID_KILLROY_REPLAY.to_string())?;
    replay.validate()?;
    Ok(replay)
}

/// JavaScript truthiness for the payload's `raw` save.
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(entry) => entry.as_f64().is_some_and(|value| value != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
