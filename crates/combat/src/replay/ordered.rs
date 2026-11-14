//! A map that keeps insertion order, so a decoded list has the game's order.

// ---------------------------------------------------------------------------
// Insertion-ordered maps
// ---------------------------------------------------------------------------

/// A map that keeps insertion order, so a decoded list has the order the game
/// inserted its actors in.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Ordered<V> {
    pub(super) entries: Vec<(String, V)>,
}

impl<V> Default for Ordered<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> Ordered<V> {
    pub(super) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.iter().map(|(key, _)| key)
    }

    pub(super) fn get(&self, key: &str) -> Option<&V> {
        self.entries
            .iter()
            .find(|(entry, _)| entry == key)
            .map(|(_, value)| value)
    }

    pub(super) fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.entries
            .iter_mut()
            .find(|(entry, _)| entry == key)
            .map(|(_, value)| value)
    }

    pub(super) fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub(super) fn insert(&mut self, key: String, value: V) {
        match self.get_mut(&key) {
            Some(slot) => *slot = value,
            None => self.entries.push((key, value)),
        }
    }

    pub(super) fn remove(&mut self, key: &str) {
        self.entries.retain(|(entry, _)| entry != key);
    }
}
