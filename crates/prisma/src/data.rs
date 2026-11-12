//! The bubble table the Prisma calculator reads.
//!
//! Extracted from the upstream JS by `script/extract-prisma-data.mjs`:
//! `js/stats/data/w2/alchemy.js` (`bubbleDefinitions`, `bubbleParams`) reads
//! `AlchemyDescription`, the display labels come from `js/stats/entity-names.js`,
//! the bubble-code letters from `js/stats/data/common/encoding.js` (`N2L`) and
//! the native consumer audit from `js/stats/data/prisma-source-coverage.js`.

use serde::Deserialize;
use std::sync::OnceLock;

const PRISMA_JSON: &str = include_str!("../data/prisma.json");

/// The extracted table, parsed once.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Data {
    /// `N2L`: the letter a Prisma ownership code prefixes the bubble index with.
    pub(super) letters: String,
    pub(super) bubbles: Vec<Bubble>,
    pub(super) coverage: SourceCoverage,
}

/// The direct native references upstream's audit traced for each effect.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceCoverage {
    pub(super) reads: Vec<Consumer>,
}

/// The `StatType`/mode value a native reference passes: some are names, some
/// are numeric modes and a few pass a talent id.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Mode {
    Text(String),
    Number(f64),
}

impl Mode {
    /// The mode as the upstream UI prints it.
    pub fn label(&self) -> String {
        match self {
            Mode::Text(text) => text.clone(),
            Mode::Number(number) if number.fract() == 0.0 => format!("{}", *number as i64),
            Mode::Number(number) => format!("{number}"),
        }
    }
}

/// One native reference that reads a bubble effect.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Consumer {
    /// `cache` or `base-call`: a cache read or a literal bubble-bonus call.
    pub kind: String,
    /// The effect key the reference reads.
    pub effect: String,
    /// The game script that carries the reference, e.g. `scripts.ActorEvents_12`.
    pub owner: String,
    /// The script-level method, e.g. `_customBlock_TotalStats`.
    pub method: String,
    /// The `StatType`/mode the reference passes, when it passes one.
    pub mode: Option<Mode>,
    /// The source line in the dumped game script.
    pub line: usize,
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
        serde_json::from_str(PRISMA_JSON).expect("crates/prisma/data/prisma.json is valid")
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

/// The native references upstream's audit traced for one effect.
pub(super) fn consumers(effect: &str) -> impl Iterator<Item = &'static Consumer> + '_ {
    data()
        .coverage
        .reads
        .iter()
        .filter(move |consumer| consumer.effect == effect)
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

    #[test]
    fn the_coverage_table_names_the_native_consumers_of_each_effect() {
        let total = data().coverage.reads.len();
        assert!(total > 150, "the audit traces {total} direct references");
        let strength: Vec<&str> = consumers("TotalSTR")
            .map(|consumer| consumer.method.as_str())
            .collect();
        assert!(strength.contains(&"_customBlock_TotalStats"));
        let statue = consumers("StatueStartEXP").collect::<Vec<_>>();
        assert_eq!(statue.len(), 1);
        assert_eq!(statue[0].kind, "cache");
        assert_eq!(statue[0].mode, None);
        assert!(consumers("NOT_AN_EFFECT").next().is_none());
    }
}
