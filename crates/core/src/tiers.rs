//! Shape priority tiers: the nodes shapes cover before Res EXP, and the nodes
//! only a leftover shape may cover.
//!
//! Ported from the tier list in the upstream `js/renderers/upgrade-eval.js`.
//! The stored lists are the two the upstream state persists:
//!
//! - `above` holds nodes forced ahead of the Res EXP divider. Any node may sit
//!   there, including a Res EXP node dragged up to pin a shape onto it.
//! - `below` holds every node whose goal is not a Res EXP category. Those are
//!   covered only after the above list and the EXP-valued nodes.
//!
//! The Res EXP zone between them is not stored: it is every live EXP node the
//! above list does not pin. A non-EXP node can still be worth EXP (Boony
//! Crowns feed the sticker), so it stays in `below` while the UI shows it in
//! the EXP zone; [`Tiers::reconcile`] keeps the stored lists complete.

use crate::data::game;
use serde::{Deserialize, Serialize};

/// The goal names the upstream counts as Res EXP nodes, the keys of
/// `NODE_GOAL_COLORS` in `js/game-data.js`.
pub const EXP_GOALS: [&str; 8] = [
    "Res EXP",
    "Res True×",
    "Obs×Insight",
    "Insight",
    "Kaleido",
    "Rolls",
    "AFK",
    "Path",
];

/// Whether a node goal is one of the Res EXP categories.
pub fn is_exp_goal(goal: Option<&str>) -> bool {
    goal.is_some_and(|goal| EXP_GOALS.contains(&goal))
}

/// Whether a grid cell is a Res EXP node.
pub fn is_exp_node(cell: usize) -> bool {
    game()
        .node_at(cell)
        .is_some_and(|node| is_exp_goal(node.goal.as_deref()))
}

/// Whether a cell has a goal, and so can sit in a tier list. One upstream grid
/// index has no `NODE_GOAL` entry and never appears in the tier list.
pub fn is_tiered(cell: usize) -> bool {
    game().node_at(cell).is_some_and(|node| node.goal.is_some())
}

/// Whether a cell belongs in the below list: a goal that is not a Res EXP one.
pub fn is_below_node(cell: usize) -> bool {
    is_tiered(cell) && !is_exp_node(cell)
}

/// Every node that is not a Res EXP node, in cell order: the upstream
/// `UE_NON_EXP_NODES` and the default `below` list.
pub fn non_exp_nodes() -> Vec<usize> {
    game()
        .nodes
        .iter()
        .filter(|node| is_below_node(node.id))
        .map(|node| node.id)
        .collect()
}

/// Which zone of the tier list a node is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// Pinned above the Res EXP divider.
    Above,
    /// In the Res EXP zone: not pinned, and an EXP node.
    Exp,
    /// Below the divider.
    Below,
}

/// The two stored priority lists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tiers {
    /// Nodes placed before any Res EXP node, in priority order.
    pub above: Vec<usize>,
    /// Non-EXP nodes, in priority order for the shapes EXP leaves over.
    pub below: Vec<usize>,
}

impl Default for Tiers {
    fn default() -> Self {
        Self {
            above: Vec::new(),
            below: non_exp_nodes(),
        }
    }
}

impl Tiers {
    /// The tiers of a preset that only names `above` nodes; every non-EXP node
    /// the list leaves out joins `below`.
    pub fn from_above(above: Vec<usize>) -> Self {
        let mut tiers = Self {
            above,
            below: Vec::new(),
        };
        tiers.reconcile();
        tiers
    }

    /// Read the upstream preset form, `above|below` coordinates such as
    /// `J6,I5|N4,M4`; every non-EXP node the string leaves out joins `below`.
    pub fn parse(data: &str) -> Option<Self> {
        let (above, below) = data.split_once('|')?;
        let coord = |text: &str| -> Option<usize> {
            let text = text.trim();
            let column = text.as_bytes().first()?.checked_sub(b'A')? as usize;
            let row: usize = text.get(1..)?.parse().ok()?;
            let game = game();
            if column >= game.grid.cols || row == 0 || row > game.grid.rows {
                return None;
            }
            Some((game.grid.rows - row) * game.grid.cols + column)
        };
        let nodes = |list: &str| -> Vec<usize> {
            list.split(',')
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .filter_map(coord)
                .filter(|cell| game().node_at(*cell).is_some())
                .collect()
        };
        let above = nodes(above);
        let below = nodes(below)
            .into_iter()
            .filter(|cell| is_below_node(*cell))
            .collect();
        let mut tiers = Self { above, below };
        tiers.reconcile();
        Some(tiers)
    }

    /// The upstream preset form, in the lists' order.
    pub fn label(&self) -> String {
        let coords = |list: &[usize]| {
            list.iter()
                .map(|cell| crate::model::coord(*cell))
                .collect::<Vec<_>>()
                .join(",")
        };
        format!("{}|{}", coords(&self.above), coords(&self.below))
    }

    /// Drop unknown and duplicate nodes, keep EXP nodes out of `below`, and
    /// put every non-EXP node the lists miss at the end of `below`, the
    /// upstream `_loadShapeTiers` reconciliation.
    pub fn reconcile(&mut self) {
        let nodes = game().cells();
        let mut seen = vec![false; nodes];
        self.above
            .retain(|cell| *cell < nodes && !std::mem::replace(&mut seen[*cell], true));
        let mut below_seen = vec![false; nodes];
        self.below.retain(|cell| {
            *cell < nodes
                && !seen[*cell]
                && is_below_node(*cell)
                && !std::mem::replace(&mut below_seen[*cell], true)
        });
        for node in non_exp_nodes() {
            if !seen[node] && !below_seen[node] {
                self.below.push(node);
            }
        }
    }

    /// A reconciled copy, for a value read from a document.
    pub fn reconciled(mut self) -> Self {
        self.reconcile();
        self
    }

    /// Which zone a node is in; `None` for a node no list names.
    pub fn zone(&self, cell: usize) -> Option<Tier> {
        if self.above.contains(&cell) {
            Some(Tier::Above)
        } else if is_exp_node(cell) {
            Some(Tier::Exp)
        } else if self.below.contains(&cell) {
            Some(Tier::Below)
        } else {
            None
        }
    }

    /// Move a node into a zone, before `before` when that node is in the same
    /// zone, otherwise to the end. Returns whether the lists changed.
    ///
    /// Only a non-EXP node can go below the divider, and only an EXP node can
    /// return to the Res EXP zone, matching the upstream drop rules. The moved
    /// node is one copy: the built-in preset it came from is never touched.
    pub fn move_node(&mut self, cell: usize, to: Tier, before: Option<usize>) -> bool {
        if game().node_at(cell).is_none() || before == Some(cell) {
            return false;
        }
        if (to == Tier::Below && is_exp_node(cell)) || (to == Tier::Exp && !is_exp_node(cell)) {
            return false;
        }
        let was = self.above.clone();
        let was_below = self.below.clone();
        self.above.retain(|node| *node != cell);
        self.below.retain(|node| *node != cell);
        match to {
            Tier::Above => insert(&mut self.above, cell, before),
            Tier::Below => insert(&mut self.below, cell, before),
            Tier::Exp => {}
        }
        self.reconcile();
        self.above != was || self.below != was_below
    }
}

/// Insert `node` before `before` in `list`, or at its end.
fn insert(list: &mut Vec<usize>, node: usize, before: Option<usize>) {
    let at = before
        .and_then(|target| list.iter().position(|entry| *entry == target))
        .unwrap_or(list.len());
    list.insert(at, node);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Preset;

    #[test]
    fn the_default_tiers_list_every_non_exp_node_below() {
        let tiers = Tiers::default();
        assert!(tiers.above.is_empty());
        assert_eq!(tiers.below, non_exp_nodes());
        assert!(tiers.below.iter().all(|cell| !is_exp_node(*cell)));
        assert!(tiers.below.len() > 40, "{}", tiers.below.len());
    }

    #[test]
    fn every_built_in_preset_parses_and_names_the_same_nodes() {
        for preset in &game().presets {
            let tiers = Tiers::parse(&preset.data)
                .unwrap_or_else(|| panic!("{} does not parse", preset.id));
            assert_eq!(
                Tiers::parse(&tiers.label()).as_ref(),
                Some(&tiers),
                "{} does not round-trip",
                preset.id
            );
            assert_eq!(preset.tiers().as_ref(), Some(&tiers), "{}", preset.id);
        }
        let minehead = game()
            .presets
            .iter()
            .find(|preset| preset.id == "_minehead")
            .expect("the Minehead preset");
        let tiers = minehead.tiers().expect("Minehead's tiers");
        assert!(!tiers.above.is_empty());
        assert!(
            tiers
                .above
                .iter()
                .any(|cell| game().node_at(*cell).is_some())
        );
    }

    #[test]
    fn parsing_fills_in_the_missing_non_exp_nodes() {
        let tiers = Tiers::parse("J6|").expect("a preset");
        assert_eq!(tiers.above, vec![129]);
        assert_eq!(tiers.below.len(), non_exp_nodes().len() - 1);
        assert!(!tiers.below.contains(&129));
        assert!(Tiers::parse("not a preset").is_none());
    }

    #[test]
    fn moving_a_node_keeps_one_copy_and_leaves_the_preset_alone() {
        let preset: &Preset = game()
            .presets
            .iter()
            .find(|preset| preset.id == "_insight")
            .expect("the Insight preset");
        let original = preset.tiers().expect("Insight's tiers");
        let mut tiers = original.clone();
        let node = original.above[0];
        assert!(tiers.move_node(node, Tier::Exp, None));
        assert_eq!(tiers.zone(node), Some(Tier::Exp));
        assert!(!tiers.above.contains(&node));
        // The preset's own data still parses to the untouched lists.
        assert_eq!(preset.tiers(), Some(original));
        assert_eq!(preset.tiers().expect("still parses").above[0], node);
    }

    #[test]
    fn only_exp_nodes_return_to_the_exp_zone_and_only_non_exp_nodes_go_below() {
        let mut tiers = Tiers::default();
        let exp = game()
            .nodes
            .iter()
            .find(|node| is_exp_goal(node.goal.as_deref()))
            .expect("an EXP node")
            .id;
        let non_exp = tiers.below[0];
        assert!(is_below_node(non_exp));
        assert!(!tiers.move_node(non_exp, Tier::Exp, None));
        assert!(!tiers.move_node(exp, Tier::Below, None));
        assert!(tiers.move_node(non_exp, Tier::Above, None));
        assert_eq!(tiers.zone(non_exp), Some(Tier::Above));
        assert_eq!(tiers.above, vec![non_exp]);
    }

    #[test]
    fn reordering_within_a_zone_inserts_before_the_target() {
        let mut tiers = Tiers::parse("J6,I5,H5|").expect("a preset");
        let [first, second, third] = [tiers.above[0], tiers.above[1], tiers.above[2]];
        assert!(tiers.move_node(third, Tier::Above, Some(first)));
        assert_eq!(tiers.above[..3], [third, first, second]);
        assert_eq!(tiers.above.len(), 3);
    }
}
