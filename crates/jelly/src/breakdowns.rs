//! The formula breakdowns: the four multipliers the board reads and the two the
//! layout's own passives add, as trees the shared formula component draws.
//!
//! Ports `jellyCellDamageBreakdown`, `jellyCellSpeedBreakdown`,
//! `jellyCellExpBreakdown`, `jellyBloodcellBreakdown` and `jellyLayoutBreakdown`
//! from `js/stats/systems/w7/jelly-operator.js` into [`FormulaNode`]s.
//!
//! Every root is a product of its children, so a breakdown multiplies back to
//! the number the section reports; the `Additive …` groups add their own
//! children and turn the sum into the multiplier their formula reads.
//!
//! Two breakdowns are locked until the upgrade that names their system is
//! bought, and the layout pair needs a valid board: [`Breakdown::available`]
//! says which, and [`Breakdown::reason`] names what is missing.

use idleon_common::formula::{FormulaNode, ValueFormat};

use super::board::Layout;
use super::data::data;
use super::metrics::{MetricOptions, Metrics, best_dps_multiplier_from_value};
use super::{BASE_BLOODCELL_BONUS_PCT, Jelly, fever_mode_name};

/// Which breakdown the Breakdowns tab shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    #[default]
    Damage,
    Speed,
    Exp,
    Bloodcells,
    LayoutDamage,
    LayoutSpeed,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Damage,
        Category::Speed,
        Category::Exp,
        Category::Bloodcells,
        Category::LayoutDamage,
        Category::LayoutSpeed,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Category::Damage => "damage",
            Category::Speed => "speed",
            Category::Exp => "exp",
            Category::Bloodcells => "bloodcells",
            Category::LayoutDamage => "layout-damage",
            Category::LayoutSpeed => "layout-speed",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Damage => "Cell Damage",
            Category::Speed => "Cell Speed",
            Category::Exp => "Cell EXP per hit",
            Category::Bloodcells => "Bloodcells per damage",
            Category::LayoutDamage => "Layout Damage",
            Category::LayoutSpeed => "Layout Speed",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|category| category.id() == id)
    }
}

/// One breakdown: its tree, whether the save can compute it and what is missing.
#[derive(Clone, Debug, PartialEq)]
pub struct Breakdown {
    pub category: Category,
    pub root: FormulaNode,
    pub available: bool,
    /// What the save lacks when `available` is false.
    pub reason: String,
}

/// The layout's own breakdown: the two passives and the per-cell rows the page's
/// table prints beside them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutBreakdown {
    pub available: bool,
    /// The layout's own refusal, when the board is not valid.
    pub reason: String,
    pub damage: FormulaNode,
    pub speed: FormulaNode,
    /// The steady-state metrics the rows are read from.
    pub metrics: Metrics,
}

impl LayoutBreakdown {
    /// The damage and speed multipliers the passives multiply to.
    pub fn damage_multiplier(&self) -> f64 {
        self.metrics.damage_passive * self.metrics.virus_multiplier
    }

    pub fn speed_multiplier(&self) -> f64 {
        self.metrics.speed_passive
    }

    /// The page's `cellRows`: one row per shipped cell type.
    pub fn cell_rows(&self) -> Vec<CellRow> {
        let names = &data().cells.names;
        (0..8)
            .map(|cell_type| {
                let cells: Vec<&super::metrics::CellMetrics> = self
                    .metrics
                    .cells
                    .iter()
                    .filter(|cell| cell.cell_type == cell_type)
                    .collect();
                let hit: f64 = cells.iter().map(|cell| cell.damage).sum();
                CellRow {
                    cell_type,
                    name: names
                        .get(cell_type)
                        .cloned()
                        .unwrap_or_else(|| format!("Cell {}", cell_type + 1)),
                    placed: cells.len(),
                    average_hit: if cells.is_empty() {
                        0.0
                    } else {
                        hit / cells.len() as f64
                    },
                    attacks_per_second: cells.iter().map(|cell| cell.attacks_per_second).sum(),
                    dps: cells.iter().map(|cell| cell.dps).sum(),
                }
            })
            .collect()
    }
}

/// One cell type's output on the board: the page's per-cell table row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CellRow {
    pub cell_type: usize,
    pub name: String,
    pub placed: usize,
    pub average_hit: f64,
    pub attacks_per_second: f64,
    pub dps: f64,
}

impl Jelly {
    /// The breakdown for `category`, over the board on screen.
    pub fn breakdown(
        &self,
        category: Category,
        layout: &Layout,
        options: MetricOptions,
    ) -> Breakdown {
        match category {
            Category::Damage => self.damage_breakdown(options),
            Category::Speed => self.speed_breakdown(options.fever),
            Category::Exp => self.exp_breakdown(options.fever),
            Category::Bloodcells => self.bloodcell_breakdown(options),
            Category::LayoutDamage | Category::LayoutSpeed => {
                let layout = self.layout_breakdown(layout, options.fever);
                let root = if category == Category::LayoutDamage {
                    layout.damage.clone()
                } else {
                    layout.speed.clone()
                };
                Breakdown {
                    category,
                    root,
                    available: layout.available,
                    reason: layout.reason,
                }
            }
        }
    }

    /// `jellyCellDamageBreakdown`.
    pub fn damage_breakdown(&self, options: MetricOptions) -> Breakdown {
        let total_cell_level = options.total_cell_level.unwrap_or(self.total_cell_level());
        let fever = options.fever.unwrap_or(self.progress.fever);
        let additive = additive_group(
            "Additive Cell Damage",
            vec![
                self.upgrade_leaf(18, "Cell Destruction I"),
                self.upgrade_leaf(19, "Cell Destruction II"),
                self.upgrade_leaf(20, "Cell Destruction III"),
                FormulaNode::leaf(
                    "Spelunking Palette bonus",
                    self.account.palette_1,
                    ValueFormat::Percent,
                ),
            ],
        );
        let legend = self.account.legend_41;
        let bloodclot = self.account.bloodclot_tier;
        let fever_value = self.fever_bonus(0, 0, options.fever_ramp_pct, options.fever)
            + self.fever_bonus(1, 0, 0.0, options.fever)
            + self.fever_bonus(4, 0, 0.0, options.fever);
        let fever_label = if fever == 0 {
            "COLD Fever at operation start (+1% each elapsed second)".to_string()
        } else {
            format!("{} Fever", fever_mode_name(fever))
        };
        let factors = vec![
            additive,
            factor(
                "Legend Talent 41",
                1.0 + legend / 100.0,
                vec![
                    FormulaNode::leaf(
                        "Legend 41 points",
                        self.account.legend_points,
                        ValueFormat::Raw,
                    ),
                    FormulaNode::leaf("Legend bonus", legend, ValueFormat::Percent),
                ],
            ),
            factor(
                "Bloodclot artifact",
                1.0 + 10.0 * bloodclot / 100.0,
                vec![FormulaNode::leaf(
                    "Artifact 40 tier (10% per tier)",
                    bloodclot,
                    ValueFormat::Raw,
                )],
            ),
            self.upgrade_factor(21, "Cell Desolation I"),
            self.upgrade_factor(22, "Cell Desolation II"),
            factor(
                "Research Grid Cell Damage",
                1.0 + self.account.grid_185 / 100.0,
                vec![FormulaNode::leaf(
                    "Grid bonus",
                    self.account.grid_185,
                    ValueFormat::Percent,
                )],
            ),
            factor(
                &upgrade_name(32, "Cell Metabolism"),
                1.0 + self.upgrade_quantity(32) * (total_cell_level / 100.0).floor() / 100.0,
                vec![
                    FormulaNode::leaf("Combined cell levels", total_cell_level, ValueFormat::Raw),
                    FormulaNode::leaf(
                        "Runtime groups of 100 levels",
                        (total_cell_level / 100.0).floor(),
                        ValueFormat::Raw,
                    ),
                    FormulaNode::leaf(
                        "Damage per group",
                        self.upgrade_quantity(32),
                        ValueFormat::Percent,
                    ),
                ],
            ),
            factor(
                "Selected Fever damage",
                1.0 + fever_value / 100.0,
                vec![FormulaNode::leaf(
                    fever_label,
                    fever_value,
                    ValueFormat::Percent,
                )],
            ),
        ];
        Breakdown {
            category: Category::Damage,
            root: root(
                Category::Damage.label(),
                self.cell_damage_multiplier(options),
                factors,
            ),
            available: true,
            reason: String::new(),
        }
    }

    /// `jellyCellSpeedBreakdown`.
    pub fn speed_breakdown(&self, fever: Option<u32>) -> Breakdown {
        let rabies = self.fever_bonus(4, 1, 0.0, fever);
        let plague = self.fever_bonus(5, 0, 0.0, fever);
        let name = fever.unwrap_or(self.progress.fever);
        Breakdown {
            category: Category::Speed,
            root: root(
                Category::Speed.label(),
                self.cell_speed_multiplier(fever),
                vec![factor(
                    "Selected Fever speed",
                    1.0 + (rabies + plague) / 100.0,
                    vec![FormulaNode::leaf(
                        format!("{} Fever", fever_mode_name(name)),
                        rabies + plague,
                        ValueFormat::Percent,
                    )],
                )],
            ),
            available: true,
            reason: String::new(),
        }
    }

    /// `jellyCellExpBreakdown`.
    pub fn exp_breakdown(&self, fever: Option<u32>) -> Breakdown {
        let additive = additive_group(
            "Additive Cell EXP",
            vec![
                self.upgrade_leaf(30, "Cell Adaptation I"),
                self.upgrade_leaf(31, "Cell Adaptation II"),
                self.upgrade_leaf(10, "Cell Biology"),
            ],
        );
        let fever_value = self.fever_bonus(3, 0, 0.0, fever);
        let name = fever.unwrap_or(self.progress.fever);
        let available = self.upgrade_quantity(10) >= 1.0;
        Breakdown {
            category: Category::Exp,
            root: root(
                Category::Exp.label(),
                self.cell_exp_multiplier(fever),
                vec![
                    factor(
                        "Selected Fever EXP",
                        1.0 + fever_value / 100.0,
                        vec![FormulaNode::leaf(
                            format!("{} Fever", fever_mode_name(name)),
                            fever_value,
                            ValueFormat::Percent,
                        )],
                    ),
                    additive,
                    self.upgrade_factor(11, "Cell Evolution"),
                ],
            ),
            available,
            reason: if available {
                String::new()
            } else {
                "Cell EXP unlocks with Cell Biology (upgrade 10).".to_string()
            },
        }
    }

    /// `jellyBloodcellBreakdown`.
    pub fn bloodcell_breakdown(&self, options: MetricOptions) -> Breakdown {
        let total_cell_level = options.total_cell_level.unwrap_or(self.total_cell_level());
        let best_dps = options.best_dps.unwrap_or(self.progress.best_dps);
        let fever = options.fever.unwrap_or(self.progress.fever);
        let fever_value = self.fever_bonus(2, 0, 0.0, options.fever);
        let additive = additive_group(
            "Additive Bloodcell Gain",
            vec![
                self.upgrade_leaf(23, "Bloodcell Coagulation"),
                self.upgrade_leaf(24, "Bloodletting I"),
                self.upgrade_leaf(25, "Bloodletting II"),
                FormulaNode::new(
                    upgrade_name(33, "Cell Dialysis"),
                    self.upgrade_quantity(33) * total_cell_level,
                )
                .with_format(ValueFormat::Percent)
                .with_children([
                    FormulaNode::leaf("Combined cell levels", total_cell_level, ValueFormat::Raw),
                    FormulaNode::leaf(
                        "Bloodcell gain per level",
                        self.upgrade_quantity(33),
                        ValueFormat::Percent,
                    ),
                ]),
                FormulaNode::leaf(
                    "Base Bloodcell bonus",
                    BASE_BLOODCELL_BONUS_PCT,
                    ValueFormat::Percent,
                ),
            ],
        );
        let available = self.upgrade_quantity(23) >= 1.0;
        Breakdown {
            category: Category::Bloodcells,
            root: root(
                Category::Bloodcells.label(),
                self.currency_multiplier(options.fever, total_cell_level, best_dps),
                vec![
                    additive,
                    factor(
                        "Arcade bonus",
                        1.0 + self.account.arcade_72 / 100.0,
                        vec![FormulaNode::leaf(
                            "Arcade contribution",
                            self.account.arcade_72,
                            ValueFormat::Percent,
                        )],
                    ),
                    factor(
                        "Research Grid Bloodcells",
                        1.0 + self.account.grid_187 / 100.0,
                        vec![FormulaNode::leaf(
                            "Grid bonus",
                            self.account.grid_187,
                            ValueFormat::Percent,
                        )],
                    ),
                    factor(
                        "Selected Fever Bloodcells",
                        1.0 + fever_value / 100.0,
                        vec![FormulaNode::leaf(
                            format!("{} Fever", fever_mode_name(fever)),
                            fever_value,
                            ValueFormat::Percent,
                        )],
                    ),
                    factor(
                        "Jelly bundle",
                        1.0 + if self.account.bundle_ban_j { 1.0 } else { 0.0 },
                        Vec::new(),
                    ),
                    factor(
                        "Apol obstruction reward",
                        1.0 + self.completion_bonus(24) / 100.0,
                        Vec::new(),
                    ),
                    factor(
                        &upgrade_name(12, "DPS Biometrics"),
                        best_dps_multiplier_from_value(best_dps),
                        vec![FormulaNode::leaf("Best DPS", best_dps, ValueFormat::Raw)],
                    ),
                    self.upgrade_factor(26, "Blood Tribunal I"),
                    self.upgrade_factor(27, "Blood Tribunal II"),
                    factor("Big Clot", 1.0 + self.account.event_shop_56, Vec::new()),
                    factor(
                        "Sulfur atom",
                        1.0 + self.account.atom_15 / 100.0,
                        vec![FormulaNode::leaf(
                            "Atom contribution",
                            self.account.atom_15,
                            ValueFormat::Percent,
                        )],
                    ),
                ],
            ),
            available,
            reason: if available {
                String::new()
            } else {
                "Bloodcell gain unlocks with Bloodcell Coagulation (upgrade 23).".to_string()
            },
        }
    }

    /// `jellyLayoutBreakdown`.
    pub fn layout_breakdown(&self, layout: &Layout, fever: Option<u32>) -> LayoutBreakdown {
        let metrics = self.layout_metrics(
            layout,
            MetricOptions {
                fever,
                ..MetricOptions::default()
            },
        );
        if !metrics.valid {
            return LayoutBreakdown {
                available: false,
                reason: metrics.reason,
                ..LayoutBreakdown::default()
            };
        }
        let counts = metrics.counts;
        let damage = root(
            Category::LayoutDamage.label(),
            metrics.damage_passive * metrics.virus_multiplier,
            vec![
                factor(
                    "Additive damage passive",
                    1.0 + 0.5 * counts[2] + 0.1 * counts[0],
                    vec![
                        FormulaNode::leaf("Effective Amoebas", counts[0], ValueFormat::Raw),
                        FormulaNode::leaf("Effective Ribosomes", counts[2], ValueFormat::Raw),
                    ],
                ),
                factor(
                    "Gigacyst passive",
                    1.0 + 2.0 * counts[7],
                    vec![FormulaNode::leaf(
                        "Effective Gigacysts",
                        counts[7],
                        ValueFormat::Raw,
                    )],
                ),
                factor(
                    "Virus infected slots",
                    metrics.virus_multiplier,
                    vec![FormulaNode::leaf(
                        "Unique infected slots",
                        metrics.infected_slots as f64,
                        ValueFormat::Raw,
                    )],
                ),
            ],
        );
        let speed = root(
            Category::LayoutSpeed.label(),
            metrics.speed_passive,
            vec![
                factor(
                    "Additive speed passive",
                    1.0 + 0.25 * counts[3] + 0.15 * counts[1],
                    vec![
                        FormulaNode::leaf("Effective Plasmids", counts[1], ValueFormat::Raw),
                        FormulaNode::leaf("Effective Organelles", counts[3], ValueFormat::Raw),
                    ],
                ),
                factor(
                    "Mitochondria passive",
                    1.0 + 0.5 * counts[6],
                    vec![FormulaNode::leaf(
                        "Effective Mitochondria",
                        counts[6],
                        ValueFormat::Raw,
                    )],
                ),
            ],
        );
        LayoutBreakdown {
            available: true,
            reason: String::new(),
            damage,
            speed,
            metrics,
        }
    }

    /// One upgrade's own row as a breakdown leaf: its quantity in percent.
    fn upgrade_leaf(&self, id: usize, fallback: &str) -> FormulaNode {
        FormulaNode::leaf(
            upgrade_name(id, fallback),
            self.upgrade_quantity(id),
            ValueFormat::Percent,
        )
    }

    /// One upgrade's `1 + quantity / 100` factor.
    fn upgrade_factor(&self, id: usize, fallback: &str) -> FormulaNode {
        factor(
            &upgrade_name(id, fallback),
            1.0 + self.upgrade_quantity(id) / 100.0,
            Vec::new(),
        )
    }
}

/// A multiplier node: `×` format, its children the factors it multiplies.
fn factor(label: &str, value: f64, children: Vec<FormulaNode>) -> FormulaNode {
    FormulaNode::new(label, value)
        .with_format(ValueFormat::Multiplier)
        .with_children(children)
}

/// A group whose own value is `1 + Σ children / 100`: the `Additive …` nodes.
fn additive_group(label: &str, children: Vec<FormulaNode>) -> FormulaNode {
    let total: f64 = children.iter().map(|child| child.value).sum();
    FormulaNode::new(label, 1.0 + total / 100.0)
        .with_format(ValueFormat::Multiplier)
        .with_children(children)
}

/// The tree's root: the category's name over its factors.
fn root(label: &str, value: f64, factors: Vec<FormulaNode>) -> FormulaNode {
    FormulaNode::new(label, value)
        .with_format(ValueFormat::Multiplier)
        .with_children(factors)
}

/// The upgrade's own name, or the page's fallback when the table lacks it.
fn upgrade_name(id: usize, fallback: &str) -> String {
    data()
        .upgrade(id)
        .map(|upgrade| upgrade.name.clone())
        .unwrap_or_else(|| fallback.to_string())
}
