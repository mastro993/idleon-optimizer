//! The stone focus: each stone's next digit, its drops per kill and the stone
//! worth focusing.
//!
//! Ports `STONE_BASE_RATES`, `stoneEfficiency` and `bestStoneFocus` of
//! `js/stats/systems/w5/cglunko.js`, and the Stones tab of `cglunko-calc.html`
//! that prints them. A stone adds a digit once its count reaches the next power
//! of ten, and each digit is worth the colour's Collector bonus; the focus slot
//! multiplies one stone's drops by the colour's `One Big Blue`/`One Big Purp`
//! bonus. So the stone to focus is the one whose next digit is fewest focused
//! kills away.

use super::{Cglunko, Page, STONE_COUNT, data, digits};

/// The upgrade that multiplies a focused blue stone's drops: `stone-value`.
const BLUE_FOCUS_UPGRADE: usize = 9;
/// The upgrade that multiplies a focused purple stone's drops: `stone-value2`.
const PURPLE_FOCUS_UPGRADE: usize = 16;
/// The upgrade that turns a blue digit into drop rate: `dr-bdig`.
const BLUE_DIGIT_UPGRADE: usize = 10;
/// The upgrade that turns a purple digit into drop rate: `dr-pdig`.
const PURPLE_DIGIT_UPGRADE: usize = 22;

impl Page {
    /// The upgrade that multiplies the page's focused stone's drops.
    pub fn focus_upgrade(self) -> usize {
        match self {
            Page::Blue => BLUE_FOCUS_UPGRADE,
            Page::Purple => PURPLE_FOCUS_UPGRADE,
        }
    }

    /// The upgrade that turns one of the page's digits into drop rate.
    pub fn digit_upgrade(self) -> usize {
        match self {
            Page::Blue => BLUE_DIGIT_UPGRADE,
            Page::Purple => PURPLE_DIGIT_UPGRADE,
        }
    }
}

/// One stone's row in the upstream `stoneEfficiency` table.
#[derive(Clone, Debug, PartialEq)]
pub struct Stone {
    /// The stone across both pages: 0..6 blue, 6..12 purple.
    pub index: usize,
    /// The page the stone belongs to.
    pub page: Page,
    /// The stone's slot within its page, 0..6.
    pub slot: usize,
    /// The saved count, `OptLacc[654..666]`.
    pub current: f64,
    /// `ceil(getLOG(current))`, the digit the stone contributes.
    pub digits: u32,
    /// `10^digits`, the count the next digit needs.
    pub next_pow10: f64,
    /// How many more stones the next digit needs, 0 on a power of ten.
    pub deficit: f64,
    /// The stone's `STONE_BASE_RATES` drop rate, before the DR multiplier.
    pub base_rate: f64,
    /// `baseRate × DR × (focused ? focusMulti : 1)`.
    pub drops_per_kill: f64,
    /// The kills the deficit needs at that rate, 0 without a deficit.
    pub kills_needed: f64,
    /// Whether the save focuses this stone.
    pub focused: bool,
    /// The multiplier the row's rate received, 1 unless the stone is focused.
    pub focus_multi: f64,
}

/// One stone's row in the upstream `bestStoneFocus` table: what its next digit
/// costs with and without the focus slot.
#[derive(Clone, Debug, PartialEq)]
pub struct Focus {
    /// The stone across both pages, as [`Stone::index`].
    pub index: usize,
    /// The page the stone belongs to.
    pub page: Page,
    /// The stone's slot within its page, 0..6.
    pub slot: usize,
    /// How many more stones the next digit needs.
    pub deficit: f64,
    /// `deficit / (baseRate × DR × focusMulti)`.
    pub kills_with_focus: f64,
    /// `deficit / (baseRate × DR)`.
    pub kills_without: f64,
    /// The drop rate one digit adds, the colour's Collector bonus.
    pub dr_per_digit: f64,
}

impl Cglunko {
    /// The page's stone, 0-based, that the save focuses; the game stores the
    /// slot as 1..=6 with 0 for none, which is the upstream
    /// `Math.round(ola) - 1`.
    pub fn focus(&self, page: Page) -> Option<usize> {
        let saved = match page {
            Page::Blue => self.blue_focus,
            Page::Purple => self.purple_focus,
        };
        let slot = usize::try_from(saved).ok()?.checked_sub(1)?;
        (slot < STONE_COUNT).then_some(slot)
    }

    /// The multiplier the focus slot applies to the page's stones:
    /// `max(1, Cglunko_upgBon)`, so a saved level of zero still doubles nothing.
    pub fn focus_multiplier(&self, page: Page) -> f64 {
        self.bonus(page.focus_upgrade()).max(1.0)
    }

    /// The drop rate one digit of the page's stones adds, the upstream
    /// `drPerDigit`.
    pub fn digit_bonus(&self, page: Page) -> f64 {
        self.bonus(page.digit_upgrade())
    }

    /// The upstream `stoneEfficiency` table: every stone's next digit and the
    /// kills it takes at the saved drop rate and focus.
    pub fn stone_rows(&self) -> Vec<Stone> {
        let dr = self.dr();
        let mut rows = Vec::with_capacity(2 * STONE_COUNT);
        for page in Page::ALL {
            let focus = self.focus(page);
            let focus_multi = self.focus_multiplier(page);
            for (slot, current) in self.page_stones(page).iter().enumerate() {
                let index = page.index() * STONE_COUNT + slot;
                let digits = digits(*current);
                let next_pow10 = 10_f64.powi(digits as i32);
                let deficit = (next_pow10 - current).max(0.0);
                let base_rate = data::stone_base_rate(index);
                let focused = focus == Some(slot);
                let row_multi = if focused { focus_multi } else { 1.0 };
                let drops_per_kill = base_rate * dr * row_multi;
                rows.push(Stone {
                    index,
                    page,
                    slot,
                    current: *current,
                    digits,
                    next_pow10,
                    deficit,
                    base_rate,
                    drops_per_kill,
                    kills_needed: kills(deficit, drops_per_kill),
                    focused,
                    focus_multi: row_multi,
                });
            }
        }
        rows
    }

    /// The upstream `bestStoneFocus` table: the stones still short of their next
    /// digit, fewest focused kills first.
    pub fn best_focus(&self) -> Vec<Focus> {
        let dr = self.dr();
        let mut rows: Vec<Focus> = self
            .stone_rows()
            .into_iter()
            .filter(|stone| stone.deficit > 0.0)
            .map(|stone| {
                let focus_multi = self.focus_multiplier(stone.page);
                Focus {
                    index: stone.index,
                    page: stone.page,
                    slot: stone.slot,
                    deficit: stone.deficit,
                    kills_with_focus: kills(stone.deficit, stone.base_rate * dr * focus_multi),
                    kills_without: kills(stone.deficit, stone.base_rate * dr),
                    dr_per_digit: self.digit_bonus(stone.page),
                }
            })
            .collect();
        rows.sort_by(|a, b| a.kills_with_focus.total_cmp(&b.kills_with_focus));
        rows
    }
}

/// The upstream `deficit > 0 && dropsPerKill > 0 ? deficit / dropsPerKill : 0`.
fn kills(deficit: f64, drops_per_kill: f64) -> f64 {
    if deficit > 0.0 && drops_per_kill > 0.0 {
        deficit / drops_per_kill
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UPGRADE_COUNT;

    /// A page's stones with the focus slot and the two stone upgrades set.
    fn sample() -> Cglunko {
        let mut upgrades = [0_u32; UPGRADE_COUNT];
        upgrades[1] = 2; // Slop Drop: 15 per level.
        upgrades[9] = 2; // One Big Blue: 10 per level, ×2.0 focused.
        upgrades[10] = 2; // Blue Collector: 1 per level.
        upgrades[16] = 1; // One Big Purp: 10 per level.
        upgrades[22] = 1; // Purp Collector: 2 per level.
        Cglunko {
            upgrades,
            blue_stones: [1000.0, 15.0, 0.0, 0.0, 0.0, 0.0],
            purple_stones: [100.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            blue_focus: 2,
            purple_focus: 0,
            saved: true,
            ..Cglunko::default()
        }
    }

    #[test]
    fn the_saved_focus_is_a_one_based_slot() {
        let cglunko = sample();
        assert_eq!(cglunko.focus(Page::Blue), Some(1));
        assert_eq!(cglunko.focus(Page::Purple), None);
        let beyond = Cglunko {
            blue_focus: 7,
            ..Cglunko::default()
        };
        assert_eq!(beyond.focus(Page::Blue), None);
    }

    #[test]
    fn only_the_focused_stone_takes_the_focus_multiplier() {
        let cglunko = sample();
        let rows = cglunko.stone_rows();
        let dr = cglunko.dr();
        // One Big Blue grants 10% per level, so two levels focus ×20.
        assert_eq!(cglunko.focus_multiplier(Page::Blue), 20.0);
        assert!((rows[1].drops_per_kill - 0.02 * dr * 20.0).abs() < 1e-12);
        assert!((rows[0].drops_per_kill - 0.1 * dr).abs() < 1e-12);
        assert!(rows[1].focused);
        assert_eq!(rows[1].focus_multi, 20.0);
        assert!(!rows[0].focused);
        assert_eq!(rows[0].focus_multi, 1.0);
        // A purple stone sits at 6..12 and never takes the blue focus.
        assert_eq!(rows[6].index, 6);
        assert!((rows[6].drops_per_kill - 0.000001 * dr).abs() < 1e-12);
    }

    #[test]
    fn a_stone_on_its_power_of_ten_needs_nothing_and_an_empty_one_needs_one() {
        let cglunko = sample();
        let rows = cglunko.stone_rows();
        // 1000 is 10^3: three digits and no deficit.
        assert_eq!(rows[0].digits, 3);
        assert_eq!(rows[0].next_pow10, 1000.0);
        assert_eq!(rows[0].deficit, 0.0);
        assert_eq!(rows[0].kills_needed, 0.0);
        // A stone that never dropped reads zero digits and is one short of 10^0.
        assert_eq!(rows[2].digits, 0);
        assert_eq!(rows[2].next_pow10, 1.0);
        assert_eq!(rows[2].deficit, 1.0);
        assert!(rows[2].kills_needed > 0.0);
        assert!(!cglunko.best_focus().iter().any(|row| row.index == 0));
    }

    #[test]
    fn the_focus_recommendation_sorts_by_focused_kills() {
        let cglunko = sample();
        let focus = cglunko.best_focus();
        // Ten of the twelve stones are short of their next digit.
        assert_eq!(focus.len(), 10);
        assert!(
            focus
                .windows(2)
                .all(|pair| pair[0].kills_with_focus <= pair[1].kills_with_focus)
        );
        // Blue #2 is the only focused stone, and the focus cuts its kills.
        let blue = focus.iter().find(|row| row.index == 1).unwrap();
        assert!((blue.kills_with_focus - blue.kills_without / 20.0).abs() < 1e-9);
        assert_eq!(blue.dr_per_digit, 2.0);
        let purple = focus.iter().find(|row| row.page == Page::Purple).unwrap();
        assert_eq!(purple.dr_per_digit, 2.0);
        // Blue #3 has the highest base rate of the stones still short.
        assert_eq!(focus[0].index, 2);
        // A page without its stone upgrade still counts the focus as ×1.
        let unfocused = Cglunko {
            blue_focus: 5,
            ..Cglunko::default()
        };
        assert_eq!(unfocused.focus_multiplier(Page::Blue), 1.0);
    }
}
