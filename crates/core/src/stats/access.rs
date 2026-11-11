//! The map access walk: which maps a character has proven it can reach.
//!
//! Ported from upstream `_buildAccess`/`_specialAccess` in
//! `js/stats/systems/common/map-access.js`: the world starters, recorded map
//! progress, the current map, the save's special unlocks, then the cleared
//! portals each character has walked, until nothing new opens.

use super::Snapshot;
use super::data::{data, map_count};
use super::maps::classify;
use std::collections::{HashMap, HashSet};

/// The maps each world starts with, and the map whose clear unlocks it.
const WORLD_STARTERS: [(i64, &[usize]); 7] = [
    (-1, &[0, 6]),
    (0, &[50, 51, 54]),
    (50, &[100, 101, 102]),
    (100, &[150]),
    (150, &[200]),
    (200, &[250]),
    (250, &[300]),
];

pub(super) struct Access {
    pub(super) accessible: HashSet<usize>,
    pub(super) reasons: HashMap<usize, String>,
    /// Insertion order, so a repeated walk sees the same sources in the same order.
    order: Vec<usize>,
}

impl Access {
    fn add(&mut self, map: usize, reason: &str) -> bool {
        if self.accessible.contains(&map) {
            return false;
        }
        self.accessible.insert(map);
        self.reasons.insert(map, reason.to_string());
        self.order.push(map);
        true
    }
}

/// Upstream `_buildAccess`: starter maps, recorded progress, the current map,
/// special unlocks and then cleared portals until nothing new opens.
pub(super) fn access(character: usize, snapshot: &Snapshot) -> Access {
    let mut access = Access {
        accessible: HashSet::new(),
        reasons: HashMap::new(),
        order: Vec::new(),
    };
    for (index, (gate, maps)) in WORLD_STARTERS.iter().enumerate() {
        if *gate >= 0 && !counter_cleared(snapshot, character, *gate as usize, 0) {
            continue;
        }
        for map in *maps {
            let reason = if index == 0 {
                "World 1 starter".to_string()
            } else {
                format!("World {} unlocked", index + 1)
            };
            access.add(*map, &reason);
        }
    }
    for map in 0..map_count() {
        if is_visited(snapshot, character, map) {
            access.add(map, "Recorded map progress");
        }
    }
    if let Some(current) = snapshot.current_map(character) {
        access.add(current, "Current map");
    }
    for (map, reason) in special_access(snapshot) {
        access.add(map, &reason);
    }
    let mut changed = true;
    while changed {
        changed = false;
        for source in access.order.clone() {
            let Some(row) = kla_row(snapshot, character, source) else {
                continue;
            };
            for portal in 0..row.len() {
                if !counter_cleared(snapshot, character, source, portal) {
                    continue;
                }
                if !has_multichar_clear(snapshot, source) {
                    continue;
                }
                let Some(destination) = transition(source, portal + 1) else {
                    continue;
                };
                if destination.is_empty() || destination == "Z" || destination == "PREVIOUSMAP" {
                    continue;
                }
                let Some(index) = index_by_internal_name(&destination) else {
                    continue;
                };
                if access.add(
                    index,
                    &format!("Cleared portal from {}", classify(source).name),
                ) {
                    changed = true;
                }
            }
        }
    }
    access
}

/// The maps the save itself unlocks, with upstream's reason strings.
fn special_access(snapshot: &Snapshot) -> Vec<(usize, String)> {
    let mut special = Vec::new();
    if snapshot.pet_park_level > 16.0 {
        special.push((41, "Pet Park level unlock".to_string()));
    }
    if option(snapshot, 265) == Some(1.0) {
        special.push((42, "Grand Owl unlock".to_string()));
    }
    if option(snapshot, 266) == Some(1.0) {
        special.push((73, "Oasis unlock".to_string()));
    }
    if snapshot.dreams.get(2).copied().unwrap_or(0.0) >= 1.0 {
        special.push((120, "Equinox unlock".to_string()));
    }
    if snapshot.rift_quest {
        special.push((166, "Rift quest unlock".to_string()));
    }
    if snapshot.hole_unlocked {
        special.push((216, "Hole unlock".to_string()));
    }
    special
}

fn option(snapshot: &Snapshot, index: usize) -> Option<f64> {
    snapshot.options.get(index).copied()
}

/// A character's `KLA` counter row for one map.
fn kla_row(snapshot: &Snapshot, character: usize, map: usize) -> Option<&[Option<f64>]> {
    snapshot
        .kla
        .get(character)?
        .get(map)
        .map(Vec::as_slice)
        .filter(|row| !row.is_empty())
}

/// Whether one `KLA` counter shows the map's requirement as met.
fn counter_cleared(snapshot: &Snapshot, character: usize, map: usize, counter: usize) -> bool {
    kla_row(snapshot, character, map)
        .and_then(|row| row.get(counter).copied().flatten())
        .is_some_and(|remaining| remaining < 1.0)
}

/// Whether any kills are recorded on a map: the requirement minus the row's
/// first counter, exactly as upstream `_isVisited` reads it.
pub(super) fn is_visited(snapshot: &Snapshot, character: usize, map: usize) -> bool {
    let Some(remaining) =
        kla_row(snapshot, character, map).and_then(|row| row.first().copied().flatten())
    else {
        return false;
    };
    let requirement = data().maps.get(map).map(|row| row.kill_req).unwrap_or(0.0);
    requirement - remaining > 0.0
}

/// Whether a multi-character map has the clears its portals need.
fn has_multichar_clear(snapshot: &Snapshot, map: usize) -> bool {
    let required = data()
        .maps
        .get(map)
        .map(|row| row.multichar_req)
        .unwrap_or(0.0);
    if required <= 1.0 {
        return true;
    }
    let cleared = (0..snapshot.characters)
        .filter(|character| counter_cleared(snapshot, *character, map, 0))
        .count();
    cleared as f64 >= required
}

/// The internal name one portal leads to, by upstream's `i + 1` indexing.
fn transition(map: usize, index: usize) -> Option<String> {
    data()
        .maps
        .get(map)?
        .transitions
        .get(index)
        .map(|(_, destination)| destination.clone())
}

/// The first map index carrying an internal name.
fn index_by_internal_name(name: &str) -> Option<usize> {
    data().maps.iter().position(|row| row.internal_name == name)
}
