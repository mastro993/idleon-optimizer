//! The bubble table the Prisma calculator reads.
//!
//! Extracted from the upstream JS by `script/extract-prisma-data.mjs`:
//! `js/stats/data/w2/alchemy.js` (`bubbleDefinitions`, `bubbleParams`) reads
//! `AlchemyDescription`, the display labels come from `js/stats/entity-names.js`
//! and the bubble-code letters from `js/stats/data/common/encoding.js` (`N2L`).

use serde::Deserialize;
use std::sync::OnceLock;

const PRISMA_JSON: &str = include_str!("../../data/prisma.json");

/// The extracted table, parsed once.
#[derive(Debug, Deserialize)]
pub(super) struct Data {
    /// `N2L`: the letter a Prisma ownership code prefixes the bubble index with.
    pub(super) letters: String,
    pub(super) bubbles: Vec<Bubble>,
}

/// One bubble the cauldrons carry.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bubble {
    pub cauldron: usize,
    pub index: usize,
    /// The upstream row key, e.g. `ROID_RAGIN`.
    pub name: String,
    /// The display label upstream's `entityName('Bubble', …)` produces.
    pub label: String,
    /// The effect key downstream formulas read, e.g. `TotalSTR`.
    pub effect: String,
    /// Large bubbles cannot take Prisma (`AlchemyDescription[…][10] !== 0`).
    pub prisma_eligible: bool,
    pub x1: f64,
    pub x2: f64,
    /// The `ArbitraryCode5Inputs` curve the bonus follows.
    pub formula: String,
}

pub(super) fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(PRISMA_JSON).expect("crates/core/data/prisma.json is valid")
    })
}

/// The four cauldrons, in the upstream tab order.
pub(super) const CAULDRONS: [&str; 4] = ["Power", "Quicc", "High IQ", "Kazam"];

/// One cauldron's bubbles, in the upstream index order.
pub fn cauldron(index: usize) -> impl Iterator<Item = &'static Bubble> {
    data()
        .bubbles
        .iter()
        .filter(move |bubble| bubble.cauldron == index)
}

/// The label of one cauldron tab.
pub fn cauldron_label(index: usize) -> Option<&'static str> {
    CAULDRONS.get(index).copied()
}

/// The ownership-code letter a cauldron's Prisma entries start with.
pub(super) fn letter(cauldron: usize) -> Option<char> {
    data().letters.chars().nth(cauldron)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_carries_the_upstream_bubbles_and_labels() {
        assert_eq!(data().bubbles.len(), 133);
        assert_eq!(cauldron(0).count(), 34);
        assert_eq!(cauldron(1).count(), 33);
        assert_eq!(cauldron(2).count(), 33);
        assert_eq!(cauldron(3).count(), 33);
        let first = cauldron(0).next().expect("the first bubble");
        assert_eq!(first.name, "ROID_RAGIN");
        assert_eq!(first.label, "Roid Ragin");
        assert_eq!(first.effect, "TotalSTR");
        assert!(first.prisma_eligible);
        assert_eq!(
            (first.x1, first.x2, first.formula.as_str()),
            (1.0, 0.0, "addDECAY")
        );
        let large = cauldron(0)
            .find(|bubble| bubble.index == 3)
            .expect("Wyoming Blood");
        assert!(!large.prisma_eligible);
    }

    #[test]
    fn the_letters_are_the_upstream_encoding_and_cauldrons_are_named() {
        assert_eq!(letter(0), Some('_'));
        assert_eq!(letter(1), Some('a'));
        assert_eq!(letter(3), Some('c'));
        assert_eq!(letter(4), Some('d'));
        assert_eq!(cauldron_label(0), Some("Power"));
        assert_eq!(cauldron_label(3), Some("Kazam"));
        assert_eq!(cauldron_label(4), None);
    }
}
