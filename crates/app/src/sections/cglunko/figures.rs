//! What the tabs list from the save, worked out when the save or the page
//! changes instead of on every frame: the page's upgrade priority, its stones
//! in table order and its focus targets, plus the drop-rate formula tree.

use super::stones::stone_order;
use crate::state::AppState;
use gpui::{Context, Entity};
use idleon_cglunko::{Cglunko, Focus, Page, Rank, Stone};
use idleon_ui::formula::FormulaTree;

#[derive(Default)]
pub(crate) struct Figures {
    key: Option<(u64, Page)>,
    tree_revision: Option<u64>,
    /// The page's upgrades, ranked by DR gain per cost.
    pub(super) ranks: Vec<Rank>,
    /// The page's stones, the next digit first.
    pub(super) stones: Vec<Stone>,
    /// The page's focus targets, the best first.
    pub(super) focus: Vec<Focus>,
}

impl Figures {
    /// Rebuild the lists when another save is applied or the page changes,
    /// and give the formula tree the new save's breakdown.
    pub(super) fn sync(
        &mut self,
        revision: u64,
        cglunko: &Cglunko,
        page: Page,
        tree: &Entity<FormulaTree>,
        cx: &mut Context<AppState>,
    ) {
        if self.tree_revision != Some(revision) {
            self.tree_revision = Some(revision);
            let root = cglunko.breakdown();
            tree.update(cx, |tree, cx| tree.set_root(root, cx));
        }
        if self.key == Some((revision, page)) {
            return;
        }
        self.key = Some((revision, page));
        self.ranks = cglunko.priority(page);
        self.stones = cglunko
            .stone_rows()
            .into_iter()
            .filter(|stone| stone.page == page)
            .collect();
        self.stones.sort_by(stone_order);
        self.focus = cglunko
            .best_focus()
            .into_iter()
            .filter(|row| row.page == page)
            .collect();
    }
}
