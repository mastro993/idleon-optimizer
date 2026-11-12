//! The system formula explorer: the filters its list applies and the coverage
//! its ledger reports.
//!
//! Upstream `js/ui/stats-explorer.js` searches the catalogue
//! `js/stats/inspection-catalog.js` builds from every system resolver, and
//! `js/ui/formula-ledger.js` reports how far those resolvers reach into the
//! native JS. This build keeps no second catalogue: the explorer lists the
//! descriptor registry behind the breakdowns, so a card can only show the value
//! the breakdown already resolved. The ledger reports what the port can prove —
//! every descriptor, its category, and whether the formula is computed here or
//! names the source it still needs.

use super::breakdowns::{Breakdown, Category, Sheet};

/// Which formulas the explorer's list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    /// Every descriptor the registry declares.
    #[default]
    All,
    /// Only the descriptors whose formula this build resolves.
    Computed,
    /// Only the descriptors that name the source they still need.
    Unported,
}

impl Status {
    /// Every status, in the order the filter lists them.
    pub const ALL: [Status; 3] = [Status::All, Status::Computed, Status::Unported];

    /// The stable id the filter stores.
    pub fn id(self) -> &'static str {
        match self {
            Status::All => "all",
            Status::Computed => "computed",
            Status::Unported => "unported",
        }
    }

    /// The filter label.
    pub fn label(self) -> &'static str {
        match self {
            Status::All => "All formulas",
            Status::Computed => "Computed",
            Status::Unported => "Not modelled",
        }
    }

    /// The status an id names, or `None` for an id this build does not know.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.id() == id)
    }

    /// Whether `row` passes this status.
    pub fn accepts(self, row: &Breakdown) -> bool {
        match self {
            Status::All => true,
            Status::Computed => row.display_value().is_some(),
            Status::Unported => row.display_value().is_none(),
        }
    }
}

/// The explorer's list filters, as its controls show them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// One category, or every category.
    pub category: Option<Category>,
    /// Which formulas to keep.
    pub status: Status,
}

/// One category's share of the ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CategoryCoverage {
    pub category: Category,
    /// How many descriptors the category holds.
    pub total: usize,
    /// How many of them this build computes.
    pub computed: usize,
}

/// What the ledger reports: the descriptors the sheet carries and how many of
/// them have a formula, per category and in total.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coverage {
    /// One entry per category, in `CATEGORY_ORDER`.
    pub categories: Vec<CategoryCoverage>,
    pub total: usize,
    pub computed: usize,
}

impl Coverage {
    /// The ledger's one line, e.g. `10 of 60 formulas computed · Combat 0/8 · …`.
    pub fn summary(&self) -> String {
        let categories = self
            .categories
            .iter()
            .map(|entry| {
                format!(
                    "{} {}/{}",
                    entry.category.label(),
                    entry.computed,
                    entry.total
                )
            })
            .collect::<Vec<_>>()
            .join(" · ");
        format!(
            "{} of {} formulas computed · {categories}",
            self.computed, self.total
        )
    }
}

impl Sheet {
    /// Every descriptor `query` and `filter` leave, in registry order. A blank
    /// query matches every row, so the filters alone decide the list.
    pub fn matching(&self, query: &str, filter: Filter) -> Vec<&Breakdown> {
        let query = query.trim();
        self.descriptors()
            .iter()
            .filter(|row| {
                filter
                    .category
                    .is_none_or(|category| row.category == category)
                    && filter.status.accepts(row)
                    && (query.is_empty() || row.matches(query))
            })
            .collect()
    }

    /// The ledger's counts, one entry per category.
    pub fn coverage(&self) -> Coverage {
        let categories = Category::ALL
            .into_iter()
            .map(|category| CategoryCoverage {
                category,
                total: self.count(category),
                computed: self
                    .in_category(category)
                    .filter(|row| row.display_value().is_some())
                    .count(),
            })
            .collect();
        Coverage {
            categories,
            total: self.descriptors().len(),
            computed: self.resolved(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_id_round_trips() {
        for status in Status::ALL {
            assert_eq!(Status::from_id(status.id()), Some(status), "{status:?}");
        }
        assert_eq!(Status::from_id("partial"), None);
    }

    #[test]
    fn the_ledger_line_names_every_category_with_its_count() {
        let coverage = Coverage {
            categories: vec![
                CategoryCoverage {
                    category: Category::Combat,
                    total: 8,
                    computed: 0,
                },
                CategoryCoverage {
                    category: Category::Capacity,
                    total: 8,
                    computed: 8,
                },
            ],
            total: 16,
            computed: 8,
        };
        assert_eq!(
            coverage.summary(),
            "8 of 16 formulas computed · Combat 0/8 · Capacity 8/8"
        );
    }
}
