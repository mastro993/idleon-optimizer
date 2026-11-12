//! The Friend bonus, ported from `js/stats/systems/common/friend.js`.
//!
//! `OptionsListAccount[476]` holds one `type,score,name` entry per friend slot,
//! in slot order. The game assigns (overwrites) each type, so the last entry of
//! the requested type wins; its score runs through the friend curve and the
//! per-type scale. Companions 30 and 44 then multiply the whole bonus.

use super::Decoder;

/// `FRIEND_SCALE`: the per-type multiplier of one friend entry.
const SCALE: [f64; 7] = [100.0, 30.0, 50.0, 25.0, 30.0, 40.0, 10.0];
/// `FRIEND_DR` from `js/stats/data/game-constants.js`.
const SCALE_DEFAULT: f64 = 25.0;
const BASE: f64 = 0.25;
const HALF: f64 = 12_000.0;
const CAP: f64 = 30_000.0;
const MAX: f64 = 1.5;

impl Decoder<'_> {
    /// `friend.resolve(type)`: the bonus the last entry of that type pays.
    pub fn friend(&self, category: usize) -> f64 {
        let text = self.ola_text(476);
        if text.is_empty() || text == "0" {
            return 0.0;
        }
        let entries: Vec<&str> = text.split(';').collect();
        let slots = self.friend_slots();
        let scale = SCALE.get(category).copied().unwrap_or(SCALE_DEFAULT);
        let mut contribution = 0.0;
        for entry in entries.iter().take(slots.min(entries.len())) {
            let mut parts = entry.split(',');
            let (Some(kind), Some(count)) = (parts.next(), parts.next()) else {
                continue;
            };
            // `parseInt` reads an unreadable entry as NaN, which never equals
            // the requested type; an unreadable score is skipped rather than
            // poisoning the whole group.
            let (Ok(kind), Ok(count)) = (kind.trim().parse::<f64>(), count.trim().parse::<f64>())
            else {
                continue;
            };
            if kind != category as f64 || kind >= 18.0 {
                continue;
            }
            let count = count.clamp(0.0, CAP);
            contribution = scale * (BASE + count / (count + HALF) * MAX).min(MAX);
        }
        if contribution <= 0.0 {
            return 0.0;
        }
        let double = self.companion_level2(44) as u8 as f64;
        contribution * (1.0 + self.companion(30) + 0.25 * double)
    }

    /// `computeFriendBonusSlots`: the slots companions and the event shop grant.
    fn friend_slots(&self) -> usize {
        let slots = 2.0
            + self.companion(44)
            + 2.0 * self.companion(30)
            + self.event_shop(22)
            + self.event_shop(67);
        (slots.min(20.0).round().max(0.0)) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_research::model::Simulation;
    use serde_json::{Map, Value};

    /// A save whose `OptLacc[476]` holds `friends`, with companion ids owned.
    fn save(friends: &str, companions: &str) -> Map<String, Value> {
        let mut options = vec![Value::from(0); 701];
        options[476] = Value::from(friends);
        options[606] = Value::from(companions);
        serde_json::json!({
            "OptLacc": serde_json::to_string(&options).expect("an array"),
        })
        .as_object()
        .expect("an object")
        .clone()
    }

    fn friend(save: &Map<String, Value>, category: usize) -> f64 {
        let simulation = Simulation::default();
        let mut warnings = Vec::new();
        let view = Decoder::new(save, save, &[], &simulation, &mut warnings);
        view.friend(category)
    }

    #[test]
    fn the_last_entry_of_the_type_wins() {
        // Companion 44 opens the third slot; the type-2 entry is skipped.
        let save = save("2,9000,Ana;5,1000,Bo;5,24000,Cy", "44");
        // 40 × (0.25 + 24000 / 36000 × 1.5) = 40 × 1.25.
        assert_eq!(friend(&save, 5), 50.0);
        assert_eq!(friend(&save, 3), 0.0);
    }

    #[test]
    fn companion_thirty_doubles_the_bonus() {
        let base = save("5,1000,Bo", "");
        let doubled = save("5,1000,Bo", "30");
        // 40 × (0.25 + 1000 / 13000 × 1.5) = 40 × 0.36538…
        assert_eq!(friend(&base, 5), 40.0 * (0.25 + 1000.0 / 13000.0 * 1.5));
        assert_eq!(friend(&doubled, 5), 2.0 * friend(&base, 5));
    }

    #[test]
    fn only_the_first_slots_are_read() {
        // Two slots by default: the third entry never counts.
        let save = save("2,9000,Ana;5,1000,Bo;5,24000,Cy", "");
        assert_eq!(friend(&save, 5), 40.0 * (0.25 + 1000.0 / 13000.0 * 1.5));
    }

    #[test]
    fn no_friend_row_pays_nothing() {
        assert_eq!(friend(&save("", ""), 5), 0.0);
        assert_eq!(friend(&save("0", ""), 5), 0.0);
    }
}
