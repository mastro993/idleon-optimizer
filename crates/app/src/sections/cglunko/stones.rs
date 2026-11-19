//! The Cglunko stones: each page's six stones, the DR a digit adds and the
//! stone worth focusing.
//!
//! The Stones tab: each stone's efficiency and the best stone to focus. The
//! section header's page selector picks the colour, so both pages render
//! through this one tab: the page's stones with their next digit, the digit's
//! DR impact, and the focus target the card explains.

use super::table::{Col, table, text};
use super::{grouped, pane, tag};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, div};
use idleon_cglunko::{self as cglunko, Cglunko, Focus, Page, Stone};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, Tone};

pub(crate) fn stones_pane(state: &AppState, cx: &App) -> AnyElement {
    let cglunko = &state.cglunko;
    let page = state.cglunko_page;
    let figures = &state.cglunko_figures;
    let body = div()
        .id("cglunko-stones")
        .flex()
        .flex_col()
        .gap_3()
        .py_3()
        .child(digit_impact(cglunko))
        .child(stone_card(cglunko, page, &figures.stones, cx))
        .child(focus_card(cglunko, page, &figures.focus, cx));
    pane("cglunko-stones-scroll", body)
}

/// The Digit DR Impact figures: what one digit of each colour is worth.
fn digit_impact(cglunko: &Cglunko) -> AnyElement {
    let impact = |page: Page| {
        let digits = digits(cglunko, page);
        let bonus = cglunko.digit_bonus(page);
        format!(
            "{}: +{}% per {}-shape digit ({} = +{}% DR)",
            cglunko::upgrade_name(page.digit_upgrade()),
            format::value(bonus),
            page.label().to_lowercase(),
            digits,
            format::value(bonus * f64::from(digits))
        )
    };
    let summary = format!("{} · {}", impact(Page::Blue), impact(Page::Purple));
    let cards = Page::ALL.into_iter().fold(KpiStrip::new(), |strip, page| {
        strip.child(
            Statistic::new(
                format!("cglunko-digit-{}", page.id()),
                cglunko::upgrade_name(page.digit_upgrade()),
                grouped(cglunko.digit_bonus(page), 0),
            )
            .prefix("+")
            .suffix("%")
            .caption(format!(
                "DR per {}-shape digit · {} digits held",
                page.label().to_lowercase(),
                digits(cglunko, page)
            )),
        )
    });
    div()
        .id("cglunko-digit-impact")
        .role(gpui::Role::Label)
        .aria_label(summary.clone())
        .flex()
        .flex_col()
        .gap_2()
        .child(div().id("cglunko-digit-cards").child(cards))
        .child(Caption::new(summary))
        .into_any_element()
}

/// The selected page's six stones with the next digit and the kills it takes,
/// in [`stone_order`].
fn stone_card(cglunko: &Cglunko, page: Page, stones: &[Stone], cx: &App) -> AnyElement {
    let rows = stones.iter().enumerate().map(|(rank, stone)| {
        let priority = div()
            .id(("cglunko-stone-priority", stone.index))
            .flex()
            .items_center()
            .gap_1()
            .when(rank == 0 && stone.deficit > 0.0, |priority| {
                priority.child(
                    div()
                        .id(("cglunko-stone-next", stone.index))
                        .child(tag("Next", Tone::Accent)),
                )
            })
            .when(stone.focused, |priority| {
                priority.child(div().id(("cglunko-stone-focus", stone.index)).child(tag(
                    format!("Focused ×{}", format::value(stone.focus_multi)),
                    Tone::Warning,
                )))
            });
        vec![
            div()
                .id(("cglunko-stone", stone.index))
                .child(SharedString::from(stone_name(stone.page, stone.slot)))
                .into_any_element(),
            text(format::value(stone.current)),
            text(stone.digits.to_string()),
            text(format::value(stone.next_pow10)),
            text(format::value(stone.deficit)),
            text(drops(stone.drops_per_kill)),
            text(if stone.deficit > 0.0 {
                format::value(stone.kills_needed.ceil())
            } else {
                "—".into()
            }),
            priority.into_any_element(),
        ]
    });
    Card::new()
        .header(
            CardHeader::new(format!("{} Stones", page.label())).description(format!(
                "{}-shape Digits = {}. Sorted by the kills needed to reach the next digit, stones already on a power of ten last.",
                page.label(),
                digits(cglunko, page)
            )),
        )
        .child(table(
            "cglunko-stone-table",
            &[
                Col::Text("Stone"),
                Col::Number("Count"),
                Col::Number("Digits"),
                Col::Number("Next Pow10"),
                Col::Number("Deficit"),
                Col::Number("Drops / Kill"),
                Col::Number("Kills Needed"),
                Col::Text("Priority"),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}

/// The recommended focus target and the explanation of what focusing buys.
fn focus_card(cglunko: &Cglunko, page: Page, rows: &[Focus], cx: &App) -> AnyElement {
    let note = focus_note(cglunko, page, rows.first());
    let note = div()
        .id("cglunko-focus-note")
        .role(gpui::Role::Label)
        .aria_label(note.clone())
        .child(Caption::new(note));
    if rows.is_empty() {
        return Card::new()
            .header(CardHeader::new("Recommended focus target"))
            .child(div().id("cglunko-focus-body").child(note))
            .child(Caption::new(
                "Every stone on this page already sits on its next power of ten, so there is nothing left to focus.",
            ))
            .into_any_element();
    }
    let table = table(
        "cglunko-focus-table",
        &[
            Col::Number("#"),
            Col::Text("Stone"),
            Col::Number("Deficit"),
            Col::Number("Kills (focused)"),
            Col::Number("Kills (unfocused)"),
            Col::Number("DR / Digit"),
        ],
        rows.iter().enumerate().map(|(rank, row)| {
            let stone = div()
                .id(("cglunko-focus", row.index))
                .flex()
                .items_center()
                .gap_1()
                .child(SharedString::from(stone_name(row.page, row.slot)))
                .when(rank == 0, |stone| {
                    stone.child(
                        div()
                            .id(("cglunko-focus-recommend", row.index))
                            .child(tag("Recommend", Tone::Accent)),
                    )
                });
            vec![
                text((rank + 1).to_string()),
                stone.into_any_element(),
                text(format::value(row.deficit)),
                text(format::value(row.kills_with_focus.ceil())),
                text(format::value(row.kills_without.ceil())),
                text(format!("+{}%", format::value(row.dr_per_digit))),
            ]
        }),
        cx,
    );
    Card::new()
        .header(CardHeader::new("Recommended focus target").description(
            "Each colour has its own focus slot; the ranking is the stones still short of a digit, fewest focused kills first.",
        ))
        .child(
            div()
                .id("cglunko-focus-body")
                .flex()
                .flex_col()
                .gap_3()
                .child(note)
                .child(table),
        )
        .into_any_element()
}

/// The sentence under the focus table: what the slot multiplies, which stone the
/// save focuses and why the page's best stone is the one to move it to.
fn focus_note(cglunko: &Cglunko, page: Page, best: Option<&Focus>) -> String {
    let saved = cglunko
        .focus(page)
        .map_or_else(|| "no stone".to_string(), |slot| stone_name(page, slot));
    let multiplier = format!(
        "Focusing a stone multiplies only that stone's drops by the colour's stone-value upgrade: ×{} on blue ({}) and ×{} on purple ({}).",
        format::value(cglunko.focus_multiplier(Page::Blue)),
        cglunko::upgrade_name(Page::Blue.focus_upgrade()),
        format::value(cglunko.focus_multiplier(Page::Purple)),
        cglunko::upgrade_name(Page::Purple.focus_upgrade()),
    );
    let detail = best.map_or_else(
        || {
            format!(
                "The save focuses {saved}; every stone on this page is already on its next power of ten, so no focus target is needed."
            )
        },
        |row| {
            format!(
                "The save focuses {saved}; the page's best target is {}: its next digit takes {} more stones and {} kills focused against {} unfocused, and each digit is worth +{}% DR through {}.",
                stone_name(row.page, row.slot),
                format::value(row.deficit),
                format::value(row.kills_with_focus.ceil()),
                format::value(row.kills_without.ceil()),
                format::value(row.dr_per_digit),
                cglunko::upgrade_name(page.digit_upgrade()),
            )
        },
    );
    format!("{multiplier} {detail}")
}

/// The stone table order: the stones still short of a digit first,
/// fewest kills first, a stone already on its power of ten last.
pub(super) fn stone_order(a: &Stone, b: &Stone) -> std::cmp::Ordering {
    match (a.deficit > 0.0, b.deficit > 0.0) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.kills_needed.total_cmp(&b.kills_needed),
    }
}

/// A stone as the tables name it: `Blue #2` for the second slot.
fn stone_name(page: Page, slot: usize) -> String {
    format!("{} #{}", page.label(), slot + 1)
}

/// The page's digit total.
fn digits(cglunko: &Cglunko, page: Page) -> u32 {
    match page {
        Page::Blue => cglunko.blue_digits(),
        Page::Purple => cglunko.purple_digits(),
    }
}

/// A drop rate as the table prints it: two decimals, or an exponent
/// below a hundredth of a drop per kill.
fn drops(value: f64) -> String {
    if value >= 0.01 {
        format!("{value:.2}")
    } else {
        format!("{value:.2e}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Cglunko {
        let mut upgrades = [0_u32; cglunko::UPGRADE_COUNT];
        upgrades[9] = 3; // One Big Blue: 10 per level, so a ×30 focus.
        upgrades[10] = 4; // Blue Collector: 1 per level.
        Cglunko {
            upgrades,
            blue_stones: [1000.0, 100.0, 0.0, 0.0, 0.0, 0.0],
            purple_stones: [0.0; 6],
            blue_focus: 3,
            saved: true,
            ..Cglunko::default()
        }
    }

    #[test]
    fn the_stone_table_sorts_the_next_digit_first() {
        let cglunko = sample();
        let mut rows: Vec<Stone> = cglunko
            .stone_rows()
            .into_iter()
            .filter(|stone| stone.page == Page::Blue)
            .collect();
        rows.sort_by(stone_order);
        // Blue #1 (1000) and #2 (100) sit on their digits, so the empty Blue #3
        // leads and the two finished stones sink.
        assert_eq!(rows[0].slot, 2);
        assert_eq!(rows[4].slot, 0);
        assert_eq!(rows[5].slot, 1);
    }

    #[test]
    fn the_focus_note_names_the_best_stone_and_its_multiplier() {
        let cglunko = sample();
        let best = cglunko
            .best_focus()
            .into_iter()
            .find(|row| row.page == Page::Blue)
            .expect("the blue page has a target");
        assert_eq!(best.slot, 2);
        let note = focus_note(&cglunko, Page::Blue, Some(&best));
        assert!(note.contains("×30.0 on blue"), "{note}");
        assert!(note.contains("the page's best target is Blue #3"), "{note}");
        assert!(note.contains("+4.0% DR through Blue Collector"), "{note}");
        // An unfocused page still reads ×1 rather than nothing at all.
        assert!(focus_note(&cglunko, Page::Purple, None).contains("no focus target"));
        assert!(Cglunko::default().focus(Page::Purple).is_none());
    }

    #[test]
    fn a_dropped_stone_reads_two_decimals_or_an_exponent() {
        assert_eq!(drops(5.89336132), "5.89");
        assert_eq!(drops(0.01), "0.01");
        assert_eq!(drops(0.002), "2.00e-3");
        assert_eq!(drops(0.0000001), "1.00e-7");
    }
}
