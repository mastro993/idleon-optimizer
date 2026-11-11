//! The game's universal formula evaluator, ported from `js/formulas.js`.

/// `ArbitraryCode5Inputs`: the curve type a data row names, its two
/// coefficients and the level.
pub fn eval(kind: &str, x1: f64, x2: f64, lv: f64) -> f64 {
    match kind {
        "add" => {
            if x2 != 0.0 {
                ((x1 + x2) / x2 + 0.5 * (lv - 1.0)) / (x1 / x2) * lv * x1
            } else {
                x1 * lv
            }
        }
        "addLower" => x1 + x2 * (lv + 1.0),
        "addDECAY" => {
            if lv < 50_001.0 {
                x1 * lv
            } else {
                x1 * 50_000.0f64.min(lv)
                    + (lv - 50_000.0) / (lv - 50_000.0 + 150_000.0) * x1 * 50_000.0
            }
        }
        "decay" => x1 * lv / (lv + x2),
        "decayLower" => x1 * (lv + 1.0) / (lv + 1.0 + x2) - x1 * lv / (lv + x2),
        "decayMulti" => 1.0 + x1 * lv / (lv + x2),
        "decayMultiLower" => x1 * (lv + 1.0) / (lv + 1.0 + x2) - x1 * lv / (lv + x2),
        "bigBase" => x1 + x2 * lv,
        "bigBaseLower" => x2,
        "intervalAdd" => x1 + (lv / x2).floor(),
        "intervalAddLower" => ((lv + 1.0) / x2).floor().max(0.0) - (lv / x2).floor().max(0.0),
        "reduce" => x1 - x2 * lv,
        "reduceLower" => x1 - x2 * (lv + 1.0),
        "PtsSpentOnGuildBonus" => {
            ((x1 + x2) / x2 + 0.5 * (lv - 1.0)) / (x1 / x2) * lv * x1 - x2 * lv
        }
        _ => 0.0,
    }
}

/// `getLOG`: the game's truncated `log10`.
pub fn log(value: f64) -> f64 {
    value.max(1.0).ln() / std::f64::consts::LN_10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decay_matches_the_game_curve() {
        // bubbleParams(3, 32) = decay 5 / 500.
        assert!((eval("decay", 5.0, 500.0, 20_070.0) - 4.878_4).abs() < 1e-3);
        assert!((eval("decay", 5.0, 500.0, 0.0)).abs() < 1e-12);
    }

    #[test]
    fn log_is_truncated_log10() {
        assert!((log(1000.0) - 3.0).abs() < 1e-3);
        assert!(log(0.0).abs() < 1e-12);
    }
}
