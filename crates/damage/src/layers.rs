//! The three damage layers and the mastery ratio behind the summary.
//!
//! The damage descriptor returns `Max Damage`, `Min Damage` and `Mastery` beside
//! three layers, and `damage-calc.html` prints one card per layer: `DDL0: Base
//! Damage`, `DDL1: Percentage Damage Multiplier` and `DDL2: Multiplicative
//! Damage Bonuses`. Each card shows the layer's value, the descriptor's own
//! `note` (`after softcaps` on the last two) and the `preSoftcap` value the
//! descriptor keeps in `_debug` for the card's second line. The maximum damage
//! is the three multiplied together.
//!
//! The mastery ratio is the descriptor's
//! `min(0.8, 0.35 - talent113 / 100 + (bubble + card21 + talent123 + etc21) / 100)`.
//! Every term is held here as the share of the ratio, which is the provider's
//! percentage divided by 100 and exactly what the page's `Mastery (min/max
//! ratio)` tree shows: the base, then one node per bonus. The page prints the
//! untaxed `0.35` as `Base`; the descriptor's own name for the node after the
//! talent-113 penalty is `Base Mastery minus talent`, so the terms still add up
//! to the ratio the bar shows.

use idleon_common::formula::{FormulaNode, ValueFormat};

/// One of the three layers the damage summary multiplies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerId {
    /// DDL0, the base damage after its softcaps.
    Base,
    /// DDL1, the percentage damage multiplier after its softcaps.
    Percent,
    /// DDL2, the multiplicative bonuses after their softcap chain.
    Multiplier,
}

impl LayerId {
    /// Every layer, in the order the page lists them.
    pub const ALL: [LayerId; 3] = [LayerId::Base, LayerId::Percent, LayerId::Multiplier];
    /// The page's card id; also the id the segmented control uses.
    pub fn id(self) -> &'static str {
        match self {
            LayerId::Base => "ddl0",
            LayerId::Percent => "ddl1",
            LayerId::Multiplier => "ddl2",
        }
    }

    /// The card title, as `damage-calc.html` writes it.
    pub fn title(self) -> &'static str {
        match self {
            LayerId::Base => "DDL0: Base Damage",
            LayerId::Percent => "DDL1: Percentage Damage Multiplier",
            LayerId::Multiplier => "DDL2: Multiplicative Damage Bonuses",
        }
    }

    /// The descriptor's `note` for the layer; the base layer carries none.
    pub fn note(self) -> &'static str {
        match self {
            LayerId::Base => "",
            LayerId::Percent | LayerId::Multiplier => "after softcaps",
        }
    }

    /// How the page prints the layer's value: plain for the base, `×` for the
    /// two multiplier layers.
    pub fn format(self) -> ValueFormat {
        match self {
            LayerId::Base => ValueFormat::Raw,
            LayerId::Percent | LayerId::Multiplier => ValueFormat::Multiplier,
        }
    }

    /// The layer an id names, for a restored tab.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|layer| layer.id() == id)
    }
}

/// The first tab the page opens, `DDL0`.
impl Default for LayerId {
    fn default() -> Self {
        LayerId::Base
    }
}

/// One layer's value and the pre-softcap value the card notes under it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layer {
    /// The value the summary multiplies.
    pub value: f64,
    /// `_debug.preSoftcap`: the same layer before its softcap chain.
    pub pre_softcap: f64,
}

/// The three layers and the mastery terms they combine with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layers {
    /// DDL0: base damage after its softcaps.
    pub base: Layer,
    /// DDL1: percentage damage multiplier after its softcaps.
    pub percent: Layer,
    /// DDL2: multiplicative bonuses after their softcap chain.
    pub multiplier: Layer,
    /// The minimum/maximum ratio's terms.
    pub mastery: Mastery,
}

impl Layers {
    /// One layer by id.
    pub fn get(&self, id: LayerId) -> Layer {
        match id {
            LayerId::Base => self.base,
            LayerId::Percent => self.percent,
            LayerId::Multiplier => self.multiplier,
        }
    }

    /// `MaxDmg = DDL[0] * DDL[1] * DDL[2]`: the summary the layers produce.
    pub fn max(&self) -> f64 {
        self.base.value * self.percent.value * self.multiplier.value
    }
}

/// The `Mastery (min/max ratio)` terms, each as a share of the ratio.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mastery {
    /// `talent(113, tab 2)`'s penalty: the descriptor's `talent113 / 100`.
    pub penalty: f64,
    /// The `Mastery` bubble: `bubbleValByKey('Mastery') / 100`.
    pub bubble: f64,
    /// `computeCardBonusByType(21) / 100`.
    pub cards: f64,
    /// Talent 123, `talent(123) / 100`.
    pub talent: f64,
    /// `etcBonus('21') / 100`.
    pub etc: f64,
}

impl Mastery {
    /// The ratio's starting value, `Base Mastery` upstream.
    pub const BASE: f64 = 0.35;
    /// The ratio's cap, the `0.8` the descriptor's `min` holds.
    pub const CAP: f64 = 0.8;

    /// `0.35 - talent113 / 100`, the descriptor's `Base Mastery minus talent`.
    pub fn base(&self) -> f64 {
        Self::BASE - self.penalty
    }

    /// The four bonus terms added up.
    pub fn bonus(&self) -> f64 {
        self.bubble + self.cards + self.talent + self.etc
    }

    /// `min(0.8, 0.35 - talent113 / 100 + (bubble + card21 + talent123 + etc21) / 100)`.
    pub fn value(&self) -> f64 {
        (self.base() + self.bonus()).min(Self::CAP)
    }

    /// The page's `Mastery (min/max ratio)` tree: the base and one node per
    /// bonus, so the nodes add up to the ratio the bar shows.
    pub fn tree(&self) -> FormulaNode {
        FormulaNode::new("Mastery (min/max ratio)", self.value())
            .with_note("cap 0.8; min(0.8, sum of terms)")
            .with_children([
                FormulaNode::new("Base Mastery minus talent", self.base())
                    .with_note("Starting mastery value 0.35, less talent 113's penalty")
                    .with_source("0.35 - talent(113, tab 2) / 100"),
                FormulaNode::new("Mastery Bubble", self.bubble)
                    .with_source("bubbleValByKey('Mastery') / 100"),
                FormulaNode::new("Card Bonus: Mastery", self.cards)
                    .with_source("computeCardBonusByType(21) / 100"),
                FormulaNode::new("Talent: Mastery Up", self.talent)
                    .with_source("talent(123) / 100"),
                FormulaNode::new("Combined Item Sources: MASTERY", self.etc)
                    .with_source("etcBonus('21') / 100"),
            ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_maximum_is_the_three_layers_multiplied() {
        let layers = Layers {
            base: Layer {
                value: 100.0,
                pre_softcap: 400.0,
            },
            percent: Layer {
                value: 2.0,
                pre_softcap: 8.0,
            },
            multiplier: Layer {
                value: 3.0,
                pre_softcap: 12.0,
            },
            mastery: Mastery::default(),
        };
        assert_eq!(layers.max(), 600.0);
        assert_eq!(layers.get(LayerId::Percent).pre_softcap, 8.0);
        assert_eq!(LayerId::ALL.map(|id| layers.get(id)).len(), 3);
    }

    #[test]
    fn the_mastery_ratio_is_the_base_plus_the_terms_capped_at_eight_tenths() {
        let mastery = Mastery {
            penalty: 0.1,
            bubble: 0.5,
            cards: 0.05,
            talent: 0.1,
            etc: 0.02,
        };
        assert!((mastery.base() - 0.25).abs() < 1e-12, "{}", mastery.base());
        assert!(
            (mastery.bonus() - 0.67).abs() < 1e-12,
            "{}",
            mastery.bonus()
        );
        assert_eq!(mastery.value(), 0.8);
        let tree = mastery.tree();
        assert_eq!(tree.value, 0.8);
        let sum: f64 = tree.children.iter().map(|child| child.value).sum();
        assert!((sum - 0.92).abs() < 1e-12, "{sum}");
    }

    #[test]
    fn every_layer_keeps_the_upstream_card_text() {
        assert_eq!(LayerId::Base.title(), "DDL0: Base Damage");
        assert_eq!(LayerId::Base.note(), "");
        assert_eq!(LayerId::Percent.note(), "after softcaps");
        assert_eq!(LayerId::Multiplier.format(), ValueFormat::Multiplier);
        assert_eq!(LayerId::Base.format(), ValueFormat::Raw);
        for id in LayerId::ALL {
            assert_eq!(LayerId::from_id(id.id()), Some(id));
        }
        assert_eq!(LayerId::from_id("ddl3"), None);
    }
}
