//! Strategy inference from recorded Play decisions, ported from
//! `js/minehead/strategy-inferrer.js`.
//!
//! The Play tab records one [`Ctx`] per decision the player made. [`infer`]
//! sweeps the optimizer's grid for the knob set that agrees with the most
//! decisions, refines the human-behaviour knobs greedily for three passes, and
//! labels the result the way upstream's profile strings do.

use crate::optimize::{default_grid, get, inference_grid, set};
use crate::sim::{Ctx, Decision, Params, decide};

/// How often the player picked each zone, against the zone's tile share.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ZoneBias {
    pub corner: f64,
    pub edge: f64,
    pub center: f64,
}

/// One click's position and its neighbourhood.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spatial {
    pub idx: usize,
    pub row: usize,
    pub col: usize,
    pub cols: usize,
    pub rows: usize,
    pub is_corner: bool,
    pub is_edge: bool,
    pub is_center: bool,
    pub adj_mines: u32,
    pub adj_safe: u32,
    pub adj_unrevealed: u32,
    pub click_order: u32,
}

/// One decision the player made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recorded {
    pub ctx: Ctx,
    pub choice: Decision,
    pub spatial: Option<Spatial>,
}

/// The spatial habits the inferrer reports.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialReport {
    pub zone_bias: ZoneBias,
    pub first_click_zone: Option<ZoneBias>,
    pub adj_mine_bias: f64,
    pub adj_safe_bias: f64,
    pub heatmap: Vec<Vec<f64>>,
    pub traits: Vec<&'static str>,
    pub total_reveals: usize,
    pub heatmap_size: usize,
}

/// What one inference run found.
#[derive(Clone, Debug, PartialEq)]
pub struct Inferred {
    pub params: Option<Params>,
    pub agreement: f64,
    pub total_decisions: usize,
    pub profile: String,
    pub spatial: Option<SpatialReport>,
}

/// Infer the knobs that best explain the recorded decisions.
pub fn infer(decisions: &[Recorded]) -> Inferred {
    if decisions.is_empty() {
        return Inferred {
            params: None,
            agreement: 0.0,
            total_decisions: 0,
            profile: "none".into(),
            spatial: None,
        };
    }

    // Phase 1: the optimizer's own grid.
    let mut best_params = Params::default();
    let mut best_score = -1i64;
    for params in default_grid().expand() {
        let score = agreement(&params, decisions);
        if score > best_score {
            best_score = score;
            best_params = params;
        }
    }

    // Phase 2: three greedy passes over the human-behaviour knobs.
    let grid = inference_grid();
    for _ in 0..3 {
        for (key, values) in &grid.knobs {
            let mut best_value = get(&best_params, key);
            let mut best_knob_score = agreement(&best_params, decisions);
            for value in values {
                let mut candidate = best_params;
                set(&mut candidate, key, *value);
                let score = agreement(&candidate, decisions);
                if score > best_knob_score {
                    best_knob_score = score;
                    best_value = *value;
                }
            }
            set(&mut best_params, key, best_value);
        }
    }

    let final_score = agreement(&best_params, decisions);
    let spatial = analyze_spatial(decisions);
    let profile = classify(&best_params, spatial.as_ref());
    Inferred {
        params: Some(best_params),
        agreement: final_score as f64 / decisions.len() as f64,
        total_decisions: decisions.len(),
        profile,
        spatial,
    }
}

/// How many decisions a knob set agrees with.
pub fn agreement(params: &Params, decisions: &[Recorded]) -> i64 {
    decisions
        .iter()
        .filter(|record| decide(params, &record.ctx) == record.choice)
        .count() as i64
}

/// `analyzeSpatial`: zone, adjacency, heatmap and habitual-click tendencies.
pub fn analyze_spatial(decisions: &[Recorded]) -> Option<SpatialReport> {
    let reveals: Vec<&Recorded> = decisions
        .iter()
        .filter(|record| {
            record.spatial.is_some() && matches!(record.choice, Decision::Reveal | Decision::Golden)
        })
        .collect();
    if reveals.len() < 5 {
        return None;
    }

    let mut corner_clicks = 0.0;
    let mut edge_clicks = 0.0;
    let mut center_clicks = 0.0;
    let mut corner_expected = 0.0;
    let mut edge_expected = 0.0;
    let mut center_expected = 0.0;
    let mut first_corner = 0.0;
    let mut first_edge = 0.0;
    let mut first_center = 0.0;
    let mut first_total = 0.0;
    let mut first_corner_exp = 0.0;
    let mut first_edge_exp = 0.0;
    let mut first_center_exp = 0.0;
    let mut adj_mine_sum = 0.0;
    let mut adj_safe_sum = 0.0;
    let mut total_adj = 0.0;

    const HEATMAP: usize = 6;
    let mut heatmap = vec![vec![0.0; HEATMAP]; HEATMAP];
    let mut heat_total = 0.0;

    for record in &reveals {
        let spatial = record.spatial.expect("a spatial record");
        let total_tiles = (spatial.rows * spatial.cols) as f64;
        let corners = 4.0;
        let edges = 2.0 * (spatial.rows as f64 - 2.0) + 2.0 * (spatial.cols as f64 - 2.0);
        let centers = (total_tiles - corners - edges).max(0.0);

        if spatial.is_corner {
            corner_clicks += 1.0;
        } else if spatial.is_edge {
            edge_clicks += 1.0;
        } else {
            center_clicks += 1.0;
        }
        corner_expected += corners / total_tiles;
        edge_expected += edges / total_tiles;
        center_expected += centers / total_tiles;

        if spatial.click_order == 0 {
            first_total += 1.0;
            if spatial.is_corner {
                first_corner += 1.0;
            } else if spatial.is_edge {
                first_edge += 1.0;
            } else {
                first_center += 1.0;
            }
            first_corner_exp += corners / total_tiles;
            first_edge_exp += edges / total_tiles;
            first_center_exp += centers / total_tiles;
        }

        if spatial.click_order > 0 {
            let total = f64::from(spatial.adj_mines + spatial.adj_safe + spatial.adj_unrevealed);
            if total > 0.0 {
                adj_mine_sum += f64::from(spatial.adj_mines) / total;
                adj_safe_sum += f64::from(spatial.adj_safe) / total;
                total_adj += 1.0;
            }
        }

        let row = ((spatial.row as f64 / spatial.rows as f64) * HEATMAP as f64).floor() as usize;
        let col = ((spatial.col as f64 / spatial.cols as f64) * HEATMAP as f64).floor() as usize;
        let row = row.min(HEATMAP - 1);
        let col = col.min(HEATMAP - 1);
        heatmap[row][col] += 1.0;
        heat_total += 1.0;
    }

    let count = reveals.len() as f64;
    let ratio = |actual: f64, expected: f64| {
        if expected > 0.0 {
            (actual / count) / (expected / count)
        } else {
            1.0
        }
    };
    let zone_bias = ZoneBias {
        corner: ratio(corner_clicks, corner_expected),
        edge: ratio(edge_clicks, edge_expected),
        center: ratio(center_clicks, center_expected),
    };
    let first_click_zone = (first_total >= 3.0).then(|| ZoneBias {
        corner: if first_corner_exp > 0.0 {
            (first_corner / first_total) / (first_corner_exp / first_total)
        } else {
            1.0
        },
        edge: if first_edge_exp > 0.0 {
            (first_edge / first_total) / (first_edge_exp / first_total)
        } else {
            1.0
        },
        center: if first_center_exp > 0.0 {
            (first_center / first_total) / (first_center_exp / first_total)
        } else {
            1.0
        },
    });
    let adj_mine_bias = if total_adj > 0.0 {
        adj_mine_sum / total_adj
    } else {
        0.0
    };
    let adj_safe_bias = if total_adj > 0.0 {
        adj_safe_sum / total_adj
    } else {
        0.0
    };
    let heatmap: Vec<Vec<f64>> = heatmap
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|value| {
                    if heat_total > 0.0 {
                        value / heat_total
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect();

    let mut traits = Vec::new();
    if zone_bias.corner > 1.5 {
        traits.push("Corner-Hugger");
    } else if zone_bias.corner < 0.5 {
        traits.push("Corner-Avoider");
    }
    if zone_bias.edge > 1.4 {
        traits.push("Edge-Walker");
    }
    if zone_bias.center > 1.3 {
        traits.push("Center-Diver");
    }
    if let Some(first) = first_click_zone {
        if first.corner > 2.0 {
            traits.push("Corner-Starter");
        }
        if first.edge > 1.8 {
            traits.push("Edge-Starter");
        }
        if first.center > 1.5 {
            traits.push("Center-Starter");
        }
    }
    if adj_safe_bias > 0.5 {
        traits.push("Clusterer");
    }
    if adj_mine_bias > 0.15 {
        traits.push("Mine-Adjacent");
    } else if adj_mine_bias < 0.03 && total_adj > 10.0 {
        traits.push("Mine-Shy");
    }

    let values: Vec<f64> = heatmap.iter().flat_map(|row| row.iter().copied()).collect();
    let entropy: f64 = values
        .iter()
        .filter(|value| **value > 0.0)
        .map(|value| -value * value.log2())
        .sum();
    let max_entropy = ((HEATMAP * HEATMAP) as f64).log2();
    if entropy < max_entropy * 0.7 && heat_total >= 20.0 {
        traits.push("Habitual");
    }
    if entropy > max_entropy * 0.95 && heat_total >= 20.0 {
        traits.push("Randomizer");
    }

    Some(SpatialReport {
        zone_bias,
        first_click_zone,
        adj_mine_bias,
        adj_safe_bias,
        heatmap,
        traits,
        total_reveals: reveals.len(),
        heatmap_size: HEATMAP,
    })
}

/// `_classifyProfile`: the human-readable habits a knob set and its clicks imply.
pub fn classify(params: &Params, spatial: Option<&SpatialReport>) -> String {
    let mut traits: Vec<&'static str> = Vec::new();
    if params.ev_multiplier <= 0.8 {
        traits.push("Daredevil");
    } else if params.ev_multiplier >= 2.0 {
        traits.push("Cautious");
    } else if params.ev_multiplier >= 1.3 {
        traits.push("Conservative");
    }
    if params.post_mine_panic >= 0.5 {
        traits.push("Panic-Prone");
    }
    if params.hot_streak >= 0.3 {
        traits.push("Momentum-Rider");
    }
    if params.crown_chase >= 0.3 || params.crown_or_die >= 0.3 {
        traits.push("Crown-Chaser");
    }
    if params.golden_first {
        traits.push("Golden-First");
    }
    if params.block_hoard >= 0.3 {
        traits.push("Block-Hoarder");
    }
    if params.last_turn_aggro >= 0.3 {
        traits.push("Finisher");
    }
    if params.target_reveals >= 5.0 {
        traits.push("Methodical");
    } else if params.target_reveals > 0.0 && params.target_reveals <= 3.0 {
        traits.push("Quick-Draw");
    }
    if params.life_aggro <= 0.6 {
        traits.push("YOLO");
    }
    if params.turn1_ev_mul > 0.0 && params.turn1_ev_mul < 0.8 {
        traits.push("Fast-Starter");
    }
    if params.commit_min > 0.0 {
        traits.push("Greedy");
    }
    if params.corner_bias >= 2.0 {
        traits.push("Corner-Hugger");
    } else if params.corner_bias >= 1.5 {
        traits.push("Corner-Leaner");
    }
    if params.edge_bias >= 1.5 {
        traits.push("Edge-Walker");
    }
    if params.cluster_bias >= 2.5 {
        traits.push("Clusterer");
    } else if params.cluster_bias >= 1.5 {
        traits.push("Near-Safe");
    }
    if let Some(spatial) = spatial {
        for habit in &spatial.traits {
            if !traits.contains(habit) {
                traits.push(habit);
            }
        }
    }
    if traits.is_empty() {
        traits.push("EV-Optimal");
    }
    traits.join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(choice: Decision) -> Recorded {
        Recorded {
            ctx: Ctx {
                safe_revealed_this_turn: 0,
                mines_remaining: 4.0,
                unrevealed_count: 20.0,
                boss_hp: 1000.0,
                remaining_hp: 1000.0,
                ..Ctx::default()
            },
            choice,
            spatial: None,
        }
    }

    #[test]
    fn an_empty_record_set_infers_nothing() {
        let inferred = infer(&[]);
        assert!(inferred.params.is_none());
        assert_eq!(inferred.profile, "none");
    }

    #[test]
    fn the_inferrer_finds_a_strategy_that_agrees() {
        let params = Params::default();
        let decisions: Vec<Recorded> = (0..40)
            .map(|index| {
                let mut record = record(Decision::Reveal);
                record.ctx.unrevealed_count = 30.0 - f64::from(index % 20);
                record.ctx.mines_remaining = 4.0;
                record.choice = decide(&params, &record.ctx);
                record
            })
            .collect();
        let inferred = infer(&decisions);
        assert!(inferred.agreement > 0.9, "{}", inferred.agreement);
        assert!(!inferred.profile.is_empty());
    }
}
