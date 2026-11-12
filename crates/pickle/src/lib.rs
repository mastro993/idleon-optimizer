//! BoneJoePickle stacking and time-to-kill.
//!
//! Ported from `pickle-calc.html` in the upstream calculators. Each pickle
//! multiplies a miniboss's HP by 1.10; the calculator finds how many fit inside
//! a kill-time budget and what the effective HP and kill time become.

/// The multiplier one pickle stacks onto a miniboss's HP.
const PICKLE_GROWTH: f64 = 1.1;

/// The DPS ceiling the input accepts, so a typed value cannot overflow the maths.
pub const MAX_DPS: f64 = 1e18;

/// The shortest kill-time budget the slider offers, in seconds.
pub const MIN_TTK: f64 = 5.0;

/// The longest kill-time budget the slider offers, in seconds.
pub const MAX_TTK: f64 = 120.0;

/// The budget the section starts on, in seconds.
pub const DEFAULT_TTK: f64 = 30.0;

/// One miniboss on the Revenge of the Pickle Death Note page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Miniboss {
    pub code: &'static str,
    pub name: &'static str,
    pub base_hp: f64,
    pub world: &'static str,
}

/// The upstream miniboss list, in table order.
const MINIBOSSES: [Miniboss; 9] = [
    Miniboss {
        code: "slimeB",
        name: "Glunko The Massive",
        base_hp: 10_000.0,
        world: "W1",
    },
    Miniboss {
        code: "poopBig",
        name: "Dr Defecaus",
        base_hp: 750_000.0,
        world: "W2",
    },
    Miniboss {
        code: "babayaga",
        name: "Baba Yaga",
        base_hp: 150_000.0,
        world: "W2",
    },
    Miniboss {
        code: "babaHour",
        name: "Biggie Hours",
        base_hp: 800_000.0,
        world: "W2",
    },
    Miniboss {
        code: "babaMummy",
        name: "King Doot",
        base_hp: 3_000_000.0,
        world: "W2",
    },
    Miniboss {
        code: "mini3a",
        name: "Dilapidated Slush",
        base_hp: 12_500_000.0,
        world: "W3",
    },
    Miniboss {
        code: "mini4a",
        name: "Mutated Mush",
        base_hp: 120_000_000.0,
        world: "W4",
    },
    Miniboss {
        code: "mini5a",
        name: "Domeo Magmus",
        base_hp: 1_000_000_000.0,
        world: "W5",
    },
    Miniboss {
        code: "mini6a",
        name: "Demented Spiritlord",
        base_hp: 25_000_000_000_000.0,
        world: "W6",
    },
];

/// What stacking pickles does to one miniboss.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    pub miniboss: &'static Miniboss,
    /// Pickles that still fit inside the budget, or `None` when even the base
    /// miniboss outlives it. `Some(0)` means base HP only.
    pub max_pickles: Option<u32>,
    /// HP after the pickles: what the table's effective-HP column shows.
    pub effective_hp: f64,
    /// Seconds to kill the effective HP; infinite without DPS.
    pub kill_time: f64,
}

/// Solve every miniboss for `dps` inside a `max_ttk`-second budget.
pub fn rows(dps: f64, max_ttk: f64) -> impl Iterator<Item = Row> {
    let (dps, max_ttk) = clean(dps, max_ttk);
    MINIBOSSES
        .iter()
        .map(move |miniboss| row(miniboss, dps, max_ttk))
}

/// The upstream `fmtNum`: `1.23M`, `1.00B`, `25.00T`, or plain digits.
pub fn compact(value: f64) -> String {
    if value >= 1e15 {
        format!("{:.2}Q", value / 1e15)
    } else if value >= 1e12 {
        format!("{:.2}T", value / 1e12)
    } else if value >= 1e9 {
        format!("{:.2}B", value / 1e9)
    } else if value >= 1e6 {
        format!("{:.2}M", value / 1e6)
    } else if value >= 1e3 {
        format!("{:.1}K", value / 1e3)
    } else {
        format!("{value:.0}")
    }
}

/// Keep the maths finite and inside the input's bounds.
fn clean(dps: f64, max_ttk: f64) -> (f64, f64) {
    let dps = if dps.is_finite() && dps > 0.0 {
        dps.min(MAX_DPS)
    } else {
        0.0
    };
    let max_ttk = if max_ttk.is_finite() && max_ttk > 0.0 {
        max_ttk.min(MAX_TTK)
    } else {
        0.0
    };
    (dps, max_ttk)
}

fn row(miniboss: &'static Miniboss, dps: f64, max_ttk: f64) -> Row {
    if dps <= 0.0 {
        return Row {
            miniboss,
            max_pickles: None,
            effective_hp: miniboss.base_hp,
            kill_time: f64::INFINITY,
        };
    }
    let base_time = miniboss.base_hp / dps;
    let ratio = (dps * max_ttk) / miniboss.base_hp;
    if max_ttk <= 0.0 || ratio < 1.0 {
        return Row {
            miniboss,
            max_pickles: None,
            effective_hp: miniboss.base_hp,
            kill_time: base_time,
        };
    }
    let max_pickles = (ratio.ln() / PICKLE_GROWTH.ln()).floor() as u32;
    let effective_hp = miniboss.base_hp * PICKLE_GROWTH.powf(f64::from(max_pickles));
    Row {
        miniboss,
        max_pickles: Some(max_pickles),
        effective_hp,
        kill_time: effective_hp / dps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solvable(rows: &[Row]) -> Vec<Option<u32>> {
        rows.iter().map(|row| row.max_pickles).collect()
    }

    #[test]
    fn fifty_million_dps_inside_thirty_seconds_matches_upstream() {
        let outcomes: Vec<Row> = rows(50_000_000.0, 30.0).collect();
        assert_eq!(
            solvable(&outcomes),
            [
                Some(125),
                Some(79),
                Some(96),
                Some(79),
                Some(65),
                Some(50),
                Some(26),
                Some(4),
                None
            ]
        );
        let first = outcomes[0];
        assert!((first.effective_hp - 1_493_088_824.218_05).abs() < 1e-6);
        assert!((first.kill_time - 29.861_776_484_361).abs() < 1e-9);
        let spiritlord = outcomes[8];
        assert_eq!(spiritlord.effective_hp, 25_000_000_000_000.0);
        assert_eq!(spiritlord.kill_time, 500_000.0);
        assert_eq!(compact(spiritlord.kill_time), "500.0K");
    }

    #[test]
    fn a_long_budget_fits_more_pickles_on_every_killable_miniboss() {
        let outcomes: Vec<Row> = rows(1_000_000_000.0, 120.0).collect();
        assert_eq!(
            solvable(&outcomes),
            [
                Some(171),
                Some(125),
                Some(142),
                Some(125),
                Some(111),
                Some(96),
                Some(72),
                Some(50),
                None
            ]
        );
        assert!((outcomes[0].kill_time - 119.715_163_240_283_46).abs() < 1e-9);
    }

    #[test]
    fn a_short_budget_only_kills_the_minibosses_it_can() {
        let outcomes: Vec<Row> = rows(10_000.0, 5.0).collect();
        assert_eq!(
            solvable(&outcomes),
            [Some(16), None, None, None, None, None, None, None, None]
        );
        assert_eq!(compact(outcomes[1].kill_time), "75");
        assert!((outcomes[1].kill_time - 75.0).abs() < 1e-9);
    }

    #[test]
    fn no_dps_lists_every_miniboss_as_unkillable() {
        let outcomes: Vec<Row> = rows(0.0, 30.0).collect();
        assert!(outcomes.iter().all(|row| row.max_pickles.is_none()));
        assert!(outcomes.iter().all(|row| row.kill_time.is_infinite()));
        assert_eq!(outcomes[0].effective_hp, 10_000.0);
    }

    #[test]
    fn invalid_inputs_are_treated_as_no_kill() {
        for dps in [-1.0, f64::NAN, f64::INFINITY] {
            let first = rows(dps, 30.0).next().expect("a row");
            assert_eq!(first.max_pickles, None);
            assert_eq!(first.kill_time, f64::INFINITY);
        }
        let first = rows(1_000.0, 0.0).next().expect("a row");
        assert_eq!(first.max_pickles, None);
        assert!((first.kill_time - 10.0).abs() < 1e-9);
    }

    #[test]
    fn compact_uses_the_upstream_suffixes() {
        assert_eq!(compact(999.0), "999");
        assert_eq!(compact(1_000.0), "1.0K");
        assert_eq!(compact(1_500_000.0), "1.50M");
        assert_eq!(compact(1_000_000_000.0), "1.00B");
        assert_eq!(compact(25_000_000_000_000.0), "25.00T");
        assert_eq!(compact(1_000_000_000_000_000.0), "1.00Q");
    }
}
