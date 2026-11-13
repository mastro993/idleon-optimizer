//! The enchant history: the snapshots the jar section keeps.
//!
//! Ported from `_saveSnapshot` and the `#tab-enchanthistory` half of
//! `jar-calc.html`. The upstream keeps the snapshots in `localStorage` under
//! `idleon-enchant-history`; here they are part of the project document, so an
//! export carries them and the session restores them on the next launch. A
//! snapshot is recorded once per applied import, exactly where the upstream page
//! records one per render, and a snapshot identical to the last one is skipped.

use crate::Jar;
use serde::{Deserialize, Serialize};

/// How many snapshots the history keeps (`_HISTORY_KEY`'s own cap).
pub const HISTORY_LIMIT: usize = 200;

/// One saved spread: the cavern's enchant levels at one moment.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EnchantSnapshot {
    /// Unix seconds the snapshot was taken.
    pub at: i64,
    /// `totalEnchantLevels`: the sum of `max(0, level - 1)`.
    pub total: u32,
    /// Every collectible's enchant levels, in save order.
    pub spread: Vec<u32>,
}

impl EnchantSnapshot {
    /// The snapshot of `jar` taken at unix second `at`.
    pub fn of(jar: &Jar, at: i64) -> Self {
        Self {
            at,
            total: jar.enchant_levels(),
            spread: jar
                .collectible_levels()
                .iter()
                .map(|level| level.saturating_sub(1))
                .collect(),
        }
    }

    /// One collectible's saved enchant levels.
    pub fn level(&self, index: usize) -> u32 {
        self.spread.get(index).copied().unwrap_or(0)
    }
}

/// Append `snapshot` unless it repeats the last one, keeping the newest
/// [`HISTORY_LIMIT`]. Returns whether it was added.
pub fn record_snapshot(history: &mut Vec<EnchantSnapshot>, snapshot: EnchantSnapshot) -> bool {
    if history
        .last()
        .is_some_and(|last| last.total == snapshot.total && last.spread == snapshot.spread)
    {
        return false;
    }
    history.push(snapshot);
    let excess = history.len().saturating_sub(HISTORY_LIMIT);
    if excess > 0 {
        history.drain(..excess);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(at: i64, total: u32, spread: &[u32]) -> EnchantSnapshot {
        EnchantSnapshot {
            at,
            total,
            spread: spread.to_vec(),
        }
    }

    #[test]
    fn a_new_spread_is_recorded_and_a_repeat_is_skipped() {
        let mut history = Vec::new();
        assert!(record_snapshot(&mut history, snapshot(1, 2, &[0, 2])));
        assert!(!record_snapshot(&mut history, snapshot(2, 2, &[0, 2])));
        assert_eq!(history.len(), 1);
        assert!(record_snapshot(&mut history, snapshot(3, 3, &[1, 2])));
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].at, 3);
    }

    #[test]
    fn the_history_keeps_the_newest_snapshots() {
        let mut history = Vec::new();
        for at in 0..(HISTORY_LIMIT as i64 + 5) {
            record_snapshot(&mut history, snapshot(at, at as u32, &[at as u32]));
        }
        assert_eq!(history.len(), HISTORY_LIMIT);
        assert_eq!(history[0].at, 5);
        assert_eq!(
            history.last().map(|snapshot| snapshot.at),
            Some(HISTORY_LIMIT as i64 + 4)
        );
    }

    #[test]
    fn a_snapshot_reads_back_from_the_project_document() {
        let saved =
            serde_json::to_value(snapshot(1_700_000_000, 5, &[0, 5])).expect("a snapshot saves");
        assert_eq!(saved["at"], 1_700_000_000);
        assert_eq!(saved["total"], 5);
        let back: EnchantSnapshot = serde_json::from_value(saved).expect("a snapshot reads back");
        assert_eq!(back, snapshot(1_700_000_000, 5, &[0, 5]));
        assert_eq!(back.level(1), 5);
        assert_eq!(back.level(40), 0);
        // A snapshot from an older build without fields keeps the defaults.
        let legacy: EnchantSnapshot =
            serde_json::from_value(serde_json::json!({})).expect("an empty snapshot reads");
        assert_eq!(legacy, EnchantSnapshot::default());
    }
}
