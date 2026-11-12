//! The sailing breakdowns: loot, trip cadence, artifact finds and chest value.
//!
//! Each tree is the arithmetic the route already reports, split into the
//! factors it multiplies, so the shared formula component can expand any of
//! them. Upstream draws the same four panels from `sailing-calc.html`; the
//! account bonuses it lists separately are one pool here, because the save
//! import only knows their sum.

use super::{RouteContext, RouteMetrics, chest_loot_multiplier, minimum_travel_details};
use idleon_common::formula::{FormulaNode, ValueFormat};

/// A percentage source as the factor it applies: `1 + percent / 100`.
fn percent_factor(label: &str, percent: f64, note: &str) -> FormulaNode {
    FormulaNode {
        label: label.into(),
        value: 1.0 + percent / 100.0,
        format: ValueFormat::Multiplier,
        note: Some(note.into()),
        children: vec![FormulaNode {
            label: "Bonus".into(),
            value: percent,
            format: ValueFormat::Percent,
            ..FormulaNode::default()
        }],
        ..FormulaNode::default()
    }
}

/// What a boat's stored chest value multiplies out to.
pub fn loot_tree(route: &RouteMetrics, context: &RouteContext) -> FormulaNode {
    let loot = route.loot;
    FormulaNode {
        label: format!("Boat {} loot value", route.boat + 1),
        value: loot.value,
        format: ValueFormat::Raw,
        note: Some("Base × captain × Davey Jones × loot multiplier".into()),
        children: vec![
            FormulaNode {
                label: "Base loot".into(),
                value: loot.base,
                format: ValueFormat::Raw,
                note: Some("5 + (2 + floor(loot level / 8)²) × loot level".into()),
                ..FormulaNode::default()
            },
            percent_factor(
                "Captain loot value",
                loot.captain,
                "Captain level × the rolled loot gain",
            ),
            FormulaNode {
                label: "Davey Jones".into(),
                value: loot.davey,
                format: ValueFormat::Multiplier,
                note: Some("Training purchases and Returns points; grid 105 doubles it".into()),
                ..FormulaNode::default()
            },
            FormulaNode {
                label: "Loot multiplier".into(),
                value: loot.multiplier,
                format: ValueFormat::Multiplier,
                note: Some("Additive pool × account multipliers × Unending Loot Search".into()),
                children: vec![
                    FormulaNode {
                        label: "Additive bonuses".into(),
                        value: 1.0 + loot.additive / 100.0,
                        format: ValueFormat::Multiplier,
                        note: Some(
                            "Account pool plus 25% per legendary captain, capped at 30".into(),
                        ),
                        children: vec![FormulaNode {
                            label: "Loot bonus pool".into(),
                            value: loot.additive,
                            format: ValueFormat::Percent,
                            ..FormulaNode::default()
                        }],
                        ..FormulaNode::default()
                    },
                    FormulaNode {
                        label: "Account multipliers".into(),
                        value: context.loot.multiplier,
                        format: ValueFormat::Multiplier,
                        note: Some(
                            "Companions, Ring of Glory, Fountain and Menagerie multiplied".into(),
                        ),
                        ..FormulaNode::default()
                    },
                    percent_factor(
                        "Unending Loot Search",
                        context.unending_loot_search,
                        "Talent 325 scales chest loot as well as gating near-full replacement",
                    ),
                ],
                ..FormulaNode::default()
            },
        ],
        ..FormulaNode::default()
    }
}

/// Trips per hour, the raw speed, the minimum-time cap and what sets it.
pub fn speed_tree(route: &RouteMetrics, context: &RouteContext) -> FormulaNode {
    let timing = route.timing;
    let speed = route.speed;
    let minimum = minimum_travel_details(context);
    let before_training = 120.0 / (1.0 + minimum.total_percent() / 100.0);
    let training = 4.0 * minimum.gem_purchases;
    let cap = if timing.minimum_minutes > 0.0 {
        60.0 * timing.distance / timing.minimum_minutes
    } else {
        f64::INFINITY
    };
    FormulaNode {
        label: "Trips per hour".into(),
        value: timing.trips_per_hour,
        format: ValueFormat::Rate,
        note: Some("3600 / trip seconds".into()),
        children: vec![
            FormulaNode {
                label: "Trip time".into(),
                value: timing.trip_seconds,
                format: ValueFormat::Raw,
                input: Some(seconds(timing.trip_seconds)),
                note: Some("3600 × distance / effective speed".into()),
                ..FormulaNode::default()
            },
            FormulaNode {
                label: "Effective route speed".into(),
                value: timing.effective_speed,
                format: ValueFormat::Raw,
                note: Some(if timing.capped {
                    "min(raw speed, minimum-time cap): the cap wins, so more speed adds no trips"
                        .into()
                } else {
                    "min(raw speed, minimum-time cap): raw speed wins".into()
                }),
                children: vec![
                    FormulaNode {
                        label: "Raw boat speed".into(),
                        value: timing.raw_speed,
                        format: ValueFormat::Raw,
                        note: Some("Every listed speed factor multiplies".into()),
                        children: vec![
                            FormulaNode {
                                label: "Base speed".into(),
                                value: speed.base,
                                format: ValueFormat::Raw,
                                note: Some(
                                    "10 + (5 + floor(speed level / 7)²) × speed level".into(),
                                ),
                                ..FormulaNode::default()
                            },
                            percent_factor(
                                "Captain boat speed",
                                speed.captain,
                                "Captain level × the rolled speed gain",
                            ),
                            FormulaNode {
                                label: "First global group".into(),
                                value: speed.first,
                                format: ValueFormat::Multiplier,
                                note: Some("1 + (divinity + cards + bubble) / 125".into()),
                                ..FormulaNode::default()
                            },
                            FormulaNode {
                                label: "Davey Jones".into(),
                                value: speed.davey,
                                format: ValueFormat::Multiplier,
                                ..FormulaNode::default()
                            },
                            percent_factor("Divinity blessing 4", speed.bless4, "1 + bonus / 100"),
                            percent_factor("Divinity blessing 6", speed.bless6, "1 + bonus / 100"),
                            percent_factor("Voting", speed.vote, "1 + bonus / 100"),
                            FormulaNode {
                                label: "Second global group".into(),
                                value: speed.second,
                                format: ValueFormat::Multiplier,
                                note: Some(
                                    "1 + (blessing 9, artifact, stamp, statue, meal, vial, rift, MSA, star sign, vault) / 125"
                                        .into(),
                                ),
                                ..FormulaNode::default()
                            },
                        ],
                        ..FormulaNode::default()
                    },
                    FormulaNode {
                        label: "Minimum-time speed cap".into(),
                        value: cap,
                        format: ValueFormat::Raw,
                        note: Some("60 × island distance / minimum travel minutes".into()),
                        children: vec![FormulaNode {
                            label: "Minimum travel time".into(),
                            value: timing.minimum_minutes,
                            format: ValueFormat::Raw,
                            input: Some("minutes".into()),
                            note: Some(
                                "round(max(15, 120 / (1 + reductions / 100) - 4 × Training purchases))"
                                    .into(),
                            ),
                            children: vec![
                                FormulaNode {
                                    label: "Base minutes".into(),
                                    value: 120.0,
                                    format: ValueFormat::Raw,
                                    ..FormulaNode::default()
                                },
                                FormulaNode {
                                    label: "Percent reductions".into(),
                                    value: 1.0 + minimum.total_percent() / 100.0,
                                    format: ValueFormat::Multiplier,
                                    note: Some(format!(
                                        "before Training: {:.2}m",
                                        before_training
                                    )),
                                    children: vec![
                                        FormulaNode {
                                            label: "Siege Breaker family".into(),
                                            value: minimum.family,
                                            format: ValueFormat::Percent,
                                            ..FormulaNode::default()
                                        },
                                        FormulaNode {
                                            label: "Shiny breeding pets".into(),
                                            value: minimum.shiny,
                                            format: ValueFormat::Percent,
                                            ..FormulaNode::default()
                                        },
                                        FormulaNode {
                                            label: "Davey Jones Returns".into(),
                                            value: minimum.legend,
                                            format: ValueFormat::Percent,
                                            ..FormulaNode::default()
                                        },
                                    ],
                                    ..FormulaNode::default()
                                },
                                FormulaNode {
                                    label: "Davey Jones Training".into(),
                                    value: -training,
                                    format: ValueFormat::Raw,
                                    note: Some(format!(
                                        "{} purchases × 4 minutes",
                                        minimum.gem_purchases
                                    )),
                                    ..FormulaNode::default()
                                },
                                FormulaNode {
                                    label: "Hard floor".into(),
                                    value: 15.0,
                                    format: ValueFormat::Raw,
                                    note: Some("the minimum cannot go below 15 minutes".into()),
                                    ..FormulaNode::default()
                                },
                            ],
                            ..FormulaNode::default()
                        }],
                        ..FormulaNode::default()
                    },
                ],
                ..FormulaNode::default()
            },
            FormulaNode {
                label: "Island distance".into(),
                value: timing.distance,
                format: ValueFormat::Raw,
                ..FormulaNode::default()
            },
        ],
        ..FormulaNode::default()
    }
}

/// Expected artifact finds per hour and the tier chances behind them.
pub fn artifact_tree(route: &RouteMetrics) -> FormulaNode {
    let mut tiers: Vec<FormulaNode> = route
        .tiers
        .iter()
        .filter(|tier| tier.kept)
        .map(|tier| FormulaNode {
            label: format!("Tier {} weighted chance", tier.tier),
            value: tier.probability * tier.artifact_chance,
            format: ValueFormat::Percent,
            note: Some("Tier chance × the artifact chance at that tier".into()),
            children: vec![
                FormulaNode {
                    label: "Exact tier chance".into(),
                    value: tier.probability,
                    format: ValueFormat::Percent,
                    ..FormulaNode::default()
                },
                FormulaNode {
                    label: "Artifact multiplier".into(),
                    value: tier.artifact_multiplier,
                    format: ValueFormat::Multiplier,
                    note: Some("Boat artifact multiplier × this chest tier's own".into()),
                    ..FormulaNode::default()
                },
                FormulaNode {
                    label: "Artifact chance at tier".into(),
                    value: tier.artifact_chance,
                    format: ValueFormat::Percent,
                    note: Some("Any artifact on the island dropping at that multiplier".into()),
                    ..FormulaNode::default()
                },
            ],
            ..FormulaNode::default()
        })
        .collect();
    if tiers.is_empty() {
        tiers.push(FormulaNode {
            label: "No tier passes the filter".into(),
            value: 0.0,
            format: ValueFormat::Percent,
            note: Some("Every tier is below the minimum chest tier".into()),
            ..FormulaNode::default()
        });
    }
    FormulaNode {
        label: "Expected artifact finds / hour".into(),
        value: route.artifact_finds_per_hour,
        format: ValueFormat::Rate,
        note: Some("Generated chests per hour × artifact chance per generated chest".into()),
        children: vec![
            FormulaNode {
                label: "Generated chests / hour".into(),
                value: route.generated_chests_per_hour,
                format: ValueFormat::Rate,
                ..FormulaNode::default()
            },
            FormulaNode {
                label: "Chance per generated chest".into(),
                value: route.artifact_chance_per_trip,
                format: ValueFormat::Percent,
                note: Some("Kept tier chance × its artifact chance, summed".into()),
                children: tiers,
                ..FormulaNode::default()
            },
            FormulaNode {
                label: "Base artifact multiplier".into(),
                value: route.artifact_base.value,
                format: ValueFormat::Multiplier,
                note: Some("max(1, 1 + (additive + captain) / 100) × account multipliers".into()),
                children: vec![
                    FormulaNode {
                        label: "Additive artifact find".into(),
                        value: route.artifact_base.additive_factor,
                        format: ValueFormat::Multiplier,
                        note: Some("max(1, 1 + (account pool + 25% per legendary captain + captain) / 100)".into()),
                        children: vec![
                            FormulaNode {
                                label: "Account pool".into(),
                                value: route.artifact_base.additive,
                                format: ValueFormat::Percent,
                                ..FormulaNode::default()
                            },
                            FormulaNode {
                                label: "Captain artifact find".into(),
                                value: route.artifact_base.captain,
                                format: ValueFormat::Percent,
                                ..FormulaNode::default()
                            },
                        ],
                        ..FormulaNode::default()
                    },
                    FormulaNode {
                        label: "Account multipliers".into(),
                        value: route.artifact_base.multiplier,
                        format: ValueFormat::Multiplier,
                        note: Some(
                            "Star sign, stickers, Nightmariners, Davey Jones and the rest".into(),
                        ),
                        ..FormulaNode::default()
                    },
                ],
                ..FormulaNode::default()
            },
        ],
        ..FormulaNode::default()
    }
}

/// Direct treasure per hour, tier by tier, and how it splits across currencies.
pub fn chest_tree(route: &RouteMetrics) -> FormulaNode {
    let treasure = route.treasure_per_hour;
    let expected_per_chest = if route.generated_chests_per_hour > 0.0 {
        treasure / route.generated_chests_per_hour
    } else {
        0.0
    };
    let tiers: Vec<FormulaNode> = route
        .tiers
        .iter()
        .map(|tier| {
            let label = if tier.kept {
                format!("Tier {}", tier.tier)
            } else {
                format!("Tier {} (discarded)", tier.tier)
            };
            FormulaNode {
                label,
                value: if tier.kept {
                    tier.probability * tier.treasure_value
                } else {
                    0.0
                },
                format: ValueFormat::Raw,
                note: Some(if tier.kept {
                    "Tier chance × full opened value".into()
                } else {
                    "Filtered tiers contribute nothing".into()
                }),
                children: vec![
                    FormulaNode {
                        label: "Exact tier chance".into(),
                        value: tier.probability,
                        format: ValueFormat::Percent,
                        ..FormulaNode::default()
                    },
                    FormulaNode {
                        label: "Value / payout multiplier".into(),
                        value: chest_loot_multiplier(tier.tier),
                        format: ValueFormat::Multiplier,
                        note: Some("Stored value per payout = boat value × this multiplier".into()),
                        ..FormulaNode::default()
                    },
                    FormulaNode {
                        label: "Stored value / payout".into(),
                        value: tier.chest_value,
                        format: ValueFormat::Raw,
                        ..FormulaNode::default()
                    },
                    FormulaNode {
                        label: "Guaranteed payouts".into(),
                        value: f64::from(tier.rolls),
                        format: ValueFormat::Count,
                        note: Some("1 + min(4, tier) + Achievement 300".into()),
                        ..FormulaNode::default()
                    },
                    FormulaNode {
                        label: "Full value if opened".into(),
                        value: tier.treasure_value,
                        format: ValueFormat::Raw,
                        ..FormulaNode::default()
                    },
                ],
                ..FormulaNode::default()
            }
        })
        .collect();
    let shares = route.treasure_shares;
    FormulaNode {
        label: "Direct treasure / hour".into(),
        value: treasure,
        format: ValueFormat::Rate,
        note: Some(
            "Generated chests × expected opened value; the stateful pile processing is excluded"
                .into(),
        ),
        children: vec![
            FormulaNode {
                label: "Generated chests / hour".into(),
                value: route.generated_chests_per_hour,
                format: ValueFormat::Rate,
                ..FormulaNode::default()
            },
            FormulaNode {
                label: "Expected value / generated chest".into(),
                value: expected_per_chest,
                format: ValueFormat::Raw,
                note: Some("Sum of exact tier chance × full opened value".into()),
                children: tiers,
                ..FormulaNode::default()
            },
            FormulaNode {
                label: "Currency split / hour".into(),
                value: treasure,
                format: ValueFormat::Rate,
                note: Some("Each guaranteed payout picks its currency independently".into()),
                children: vec![
                    FormulaNode {
                        label: "Gold bars".into(),
                        value: treasure * shares.gold,
                        format: ValueFormat::Rate,
                        note: Some(format!("{} of every payout", percent(shares.gold))),
                        ..FormulaNode::default()
                    },
                    FormulaNode {
                        label: "Island primary".into(),
                        value: treasure * shares.primary,
                        format: ValueFormat::Rate,
                        note: Some(format!("{} of every payout", percent(shares.primary))),
                        ..FormulaNode::default()
                    },
                    FormulaNode {
                        label: "Island rare".into(),
                        value: treasure * shares.rare,
                        format: ValueFormat::Rate,
                        note: Some(format!("{} of every payout", percent(shares.rare))),
                        ..FormulaNode::default()
                    },
                ],
                ..FormulaNode::default()
            },
        ],
        ..FormulaNode::default()
    }
}

fn seconds(value: f64) -> String {
    if !value.is_finite() {
        return "—".into();
    }
    if value < 60.0 {
        format!("{value:.0}s")
    } else if value < 3600.0 {
        format!("{:.1}m", value / 60.0)
    } else {
        format!("{:.2}h", value / 3600.0)
    }
}

fn percent(value: f64) -> String {
    format!("{:.1}%", 100.0 * value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Boat, Captain, Fleet, Goal, RouteContext, artifact_count, saved};
    use idleon_common::formula::FormulaNode;

    fn route() -> (RouteMetrics, RouteContext) {
        let fleet = Fleet {
            boats: vec![Boat {
                captain: Some(0),
                loot_level: 100,
                speed_level: 100,
                ..Boat::default()
            }],
            captains: vec![Captain {
                rarity: 6,
                bonus_types: [0, 3],
                level: 20,
                exp: 0.0,
                rolls: [20.0, 15.0],
            }],
            sailing_level: 10,
            artifact_tiers: vec![0; artifact_count()],
            island_progress: vec![-1.0; 20],
            currency_balances: Vec::new(),
        };
        let context = RouteContext {
            unending_loot_search: 50.0,
            ..RouteContext::default()
        };
        let plan = saved(&fleet, 0, Goal::Treasure, &context);
        (plan.routes.into_iter().next().expect("one route"), context)
    }

    /// A node's children multiply back to it.
    fn multiplies(node: &FormulaNode, fields: &[&str]) {
        let labels: Vec<&str> = node
            .children
            .iter()
            .filter(|child| fields.is_empty() || fields.contains(&child.label.as_str()))
            .map(|child| child.label.as_str())
            .collect();
        assert_eq!(
            labels.len(),
            if fields.is_empty() {
                node.children.len()
            } else {
                fields.len()
            }
        );
        let product: f64 = node
            .children
            .iter()
            .filter(|child| fields.is_empty() || fields.contains(&child.label.as_str()))
            .map(|child| child.value)
            .product();
        let tolerance = node.value.abs().max(1.0) * 1e-12;
        assert!(
            (product - node.value).abs() <= tolerance,
            "{}: {product} != {}",
            node.label,
            node.value
        );
    }

    fn child<'a>(node: &'a FormulaNode, label: &str) -> &'a FormulaNode {
        node.children
            .iter()
            .find(|child| child.label == label)
            .unwrap_or_else(|| panic!("{} has no child {label}", node.label))
    }

    #[test]
    fn the_loot_tree_multiplies_back_to_the_boat_value() {
        let (route, context) = route();
        let tree = loot_tree(&route, &context);
        assert_eq!(tree.value, route.loot.value);
        multiplies(&tree, &[]);
        multiplies(
            child(&tree, "Loot multiplier"),
            &[
                "Additive bonuses",
                "Account multipliers",
                "Unending Loot Search",
            ],
        );
        assert_eq!(
            child(&tree, "Captain loot value").children[0].value,
            route.loot.captain
        );
    }

    #[test]
    fn the_speed_tree_reads_the_raw_speed_and_the_cap() {
        let (route, context) = route();
        let tree = speed_tree(&route, &context);
        assert_eq!(tree.value, route.timing.trips_per_hour);
        let effective = child(&tree, "Effective route speed");
        assert_eq!(effective.value, route.timing.effective_speed);
        assert_eq!(
            effective.value,
            route
                .timing
                .raw_speed
                .min(child(effective, "Minimum-time speed cap").value)
        );
        multiplies(child(effective, "Raw boat speed"), &[]);
        assert_eq!(
            child(
                child(effective, "Minimum-time speed cap"),
                "Minimum travel time"
            )
            .value,
            route.timing.minimum_minutes
        );
    }

    #[test]
    fn the_artifact_tree_sums_the_kept_tier_chances() {
        let (route, context) = route();
        let _ = &context;
        let tree = artifact_tree(&route);
        assert_eq!(tree.value, route.artifact_finds_per_hour);
        let chance = child(&tree, "Chance per generated chest");
        assert_eq!(chance.value, route.artifact_chance_per_trip);
        let summed: f64 = chance.children.iter().map(|tier| tier.value).sum();
        assert!((summed - chance.value).abs() <= chance.value.max(1e-12) * 1e-12);
        let multiplier = child(&tree, "Base artifact multiplier");
        assert_eq!(multiplier.value, route.artifact_base.value);
        multiplies(multiplier, &[]);
    }

    #[test]
    fn the_chest_tree_adds_up_to_the_direct_treasure() {
        let (route, _) = route();
        let tree = chest_tree(&route);
        assert_eq!(tree.value, route.treasure_per_hour);
        let per_chest = child(&tree, "Expected value / generated chest");
        let summed: f64 = per_chest.children.iter().map(|tier| tier.value).sum();
        assert!((summed - per_chest.value).abs() <= per_chest.value.max(1e-12) * 1e-12);
        let split = child(&tree, "Currency split / hour");
        let currencies: f64 = split.children.iter().map(|currency| currency.value).sum();
        assert!((currencies - split.value).abs() <= split.value * 1e-12);
        assert!(
            per_chest
                .children
                .iter()
                .all(|tier| !tier.label.contains("discarded") || tier.value == 0.0)
        );
    }
}
