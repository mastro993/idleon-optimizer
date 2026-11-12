//! The Jelly Operator operation estimate and outlook against the upstream JS.
//!
//! `fixtures/jelly.json` carries the upstream `simulateJellyTrials` aggregate,
//! its first trial and `jellyOperationInsights` for the committed save and three
//! documented overrides; see `tests/common/mod.rs` for the fixture helpers.

mod common;

use common::{close, close_opt, jelly};
use idleon_core::jelly::Trial;
use idleon_core::jelly::estimate::Estimate;
use idleon_core::jelly::insights::Insights;
use serde_json::Value;

#[test]
fn estimate_matches_upstream() {
    for name in ["save", "small-board", "clear"] {
        let (jelly, case) = jelly(name);
        let options = &case["estimateOptions"];
        let expected = &case["estimate"];
        let trials = options["trials"].as_u64().unwrap() as u32;
        let seed = options["seed"].as_u64().unwrap() as u32;
        let estimate = Estimate::run(&jelly, &jelly.layout, trials, seed);
        assert!(estimate.valid, "{name}: {}", estimate.reason);
        assert_eq!(
            estimate.trials,
            expected["trials"].as_u64().unwrap() as u32,
            "{name}: trials"
        );
        assert_eq!(
            estimate.success_count,
            expected["successCount"].as_u64().unwrap() as u32,
            "{name}: successes"
        );
        close(
            estimate.clear_probability,
            expected["clearProbability"].as_f64().unwrap(),
            "clear probability",
        );
        close(
            estimate.clear_probability_low,
            expected["clearProbabilityLow"].as_f64().unwrap(),
            "clear probability low",
        );
        close(
            estimate.clear_probability_high,
            expected["clearProbabilityHigh"].as_f64().unwrap(),
            "clear probability high",
        );
        close(
            estimate.mean_time,
            expected["meanTime"].as_f64().unwrap(),
            "mean time",
        );
        close_opt(estimate.median_time, &expected["medianTime"], "median time");
        close_opt(estimate.p90_time, &expected["p90Time"], "p90 time");
        close(
            estimate.mean_damage,
            expected["meanDamage"].as_f64().unwrap(),
            "mean damage",
        );
        close(
            estimate.mean_damage_standard_error,
            expected["meanDamageStandardError"].as_f64().unwrap(),
            "damage standard error",
        );
        close_opt(
            estimate.median_damage,
            &expected["medianDamage"],
            "median damage",
        );
        close(
            estimate.mean_bloodcells,
            expected["meanBloodcells"].as_f64().unwrap(),
            "mean bloodcells",
        );
        close_opt(
            estimate.median_bloodcells,
            &expected["medianBloodcells"],
            "median bloodcells",
        );
        close_opt(
            estimate.p90_bloodcells,
            &expected["p90Bloodcells"],
            "p90 bloodcells",
        );
        close(
            estimate.max_bloodcells,
            expected["maxBloodcells"].as_f64().unwrap(),
            "max bloodcells",
        );
        close(
            estimate.mean_cell_exp,
            expected["meanCellExp"].as_f64().unwrap(),
            "mean cell EXP",
        );
        for cell_type in 0..9 {
            close(
                estimate.mean_cell_exp_by_type[cell_type],
                expected["meanCellExpByType"][cell_type].as_f64().unwrap(),
                "mean cell EXP by type",
            );
        }
        close(
            estimate.mean_critical_time,
            expected["meanCriticalTime"].as_f64().unwrap(),
            "mean critical time",
        );
        close(
            estimate.mean_deaths,
            expected["meanDeaths"].as_f64().unwrap(),
            "mean deaths",
        );
        close(
            estimate.mean_revives,
            expected["meanRevives"].as_f64().unwrap(),
            "mean revives",
        );
        close(
            estimate.mean_best_dps,
            expected["meanBestDps"].as_f64().unwrap(),
            "mean best DPS",
        );
        close(
            estimate.mean_best_dps_multiplier,
            expected["meanBestDpsMultiplier"].as_f64().unwrap(),
            "mean best DPS multiplier",
        );
        close(
            estimate.mean_peak_dps,
            expected["meanPeakDps"].as_f64().unwrap(),
            "mean peak DPS",
        );
        close(
            estimate.mean_normal_damage,
            expected["meanNormalDamage"].as_f64().unwrap(),
            "mean normal damage",
        );
        close(
            estimate.mean_critical_damage,
            expected["meanCriticalDamage"].as_f64().unwrap(),
            "mean critical damage",
        );
        close(
            estimate.mean_shots,
            expected["meanShots"].as_f64().unwrap(),
            "mean shots",
        );
        close(
            estimate.mean_hits,
            expected["meanHits"].as_f64().unwrap(),
            "mean hits",
        );
        close(
            estimate.mean_boss_attacks,
            expected["meanBossAttacks"].as_f64().unwrap(),
            "mean boss attacks",
        );
        close(
            estimate.mean_immunoid_focus_time,
            expected["meanImmunoidFocusTime"].as_f64().unwrap(),
            "mean immunoid focus",
        );
        close(
            estimate.mean_live_slots,
            expected["meanLiveSlots"].as_f64().unwrap(),
            "mean live slots",
        );
        close(
            estimate.mean_attempt_delta,
            expected["meanAttemptDelta"].as_f64().unwrap(),
            "mean attempt delta",
        );
        close(
            estimate.mean_amoeba_stacks_at_critical_start,
            expected["meanAmoebaStacksAtCriticalStart"]
                .as_f64()
                .unwrap(),
            "mean amoeba start",
        );
        close(
            estimate.mean_amoeba_stacks_at_critical_end,
            expected["meanAmoebaStacksAtCriticalEnd"].as_f64().unwrap(),
            "mean amoeba end",
        );
        close(
            estimate.min_damage,
            expected["minDamage"].as_f64().unwrap(),
            "min damage",
        );
        close(
            estimate.max_damage,
            expected["maxDamage"].as_f64().unwrap(),
            "max damage",
        );
        for (contribution, expected) in estimate
            .mean_contribution_by_type
            .iter()
            .zip(expected["contribution"].as_array().unwrap())
        {
            close(
                contribution.shots,
                expected["shots"].as_f64().unwrap(),
                "contribution shots",
            );
            close(
                contribution.hits,
                expected["hits"].as_f64().unwrap(),
                "contribution hits",
            );
            close(
                contribution.damage,
                expected["damage"].as_f64().unwrap(),
                "contribution damage",
            );
            close(
                contribution.bloodcells,
                expected["bloodcells"].as_f64().unwrap(),
                "contribution bloodcells",
            );
            close(
                contribution.exp,
                expected["exp"].as_f64().unwrap(),
                "contribution EXP",
            );
        }
        compare_trial(&estimate.representative, &expected["representative"], name);
    }
}

fn compare_trial(trial: &Trial, expected: &Value, name: &str) {
    assert_eq!(
        trial.success,
        expected["success"].as_bool().unwrap(),
        "{name}: success"
    );
    assert_eq!(
        trial.ended_by,
        expected["endedBy"].as_str().unwrap(),
        "{name}: end"
    );
    close(trial.time, expected["time"].as_f64().unwrap(), "trial time");
    close(
        trial.normal_time,
        expected["normalTime"].as_f64().unwrap(),
        "normal time",
    );
    close(
        trial.critical_time,
        expected["criticalTime"].as_f64().unwrap(),
        "critical time",
    );
    close(
        trial.damage,
        expected["damage"].as_f64().unwrap(),
        "trial damage",
    );
    close(
        trial.shots,
        expected["shots"].as_f64().unwrap(),
        "trial shots",
    );
    close(trial.hits, expected["hits"].as_f64().unwrap(), "trial hits");
    close(
        trial.bloodcells,
        expected["bloodcells"].as_f64().unwrap(),
        "trial bloodcells",
    );
    close(
        trial.cell_exp,
        expected["cellExp"].as_f64().unwrap(),
        "trial cell EXP",
    );
    for cell_type in 0..9 {
        close(
            trial.exp_by_type[cell_type],
            expected["expByType"][cell_type].as_f64().unwrap(),
            "trial EXP by type",
        );
    }
    close(
        trial.fever_seconds,
        expected["feverSeconds"].as_f64().unwrap(),
        "fever seconds",
    );
    close(
        trial.best_dps_before,
        expected["bestDpsBefore"].as_f64().unwrap(),
        "best DPS before",
    );
    close(
        trial.best_dps_after,
        expected["bestDpsAfter"].as_f64().unwrap(),
        "best DPS after",
    );
    close(
        trial.peak_dps,
        expected["peakDps"].as_f64().unwrap(),
        "peak DPS",
    );
    close(trial.deaths, expected["deaths"].as_f64().unwrap(), "deaths");
    close(
        trial.boss_attacks,
        expected["bossAttacks"].as_f64().unwrap(),
        "boss attacks",
    );
    close(
        trial.immunoid_hits,
        expected["immunoidHits"].as_f64().unwrap(),
        "immunoid hits",
    );
    close(
        trial.immunoid_focus_time,
        expected["immunoidFocusTime"].as_f64().unwrap(),
        "immunoid focus time",
    );
    close(
        trial.live_slots,
        expected["liveSlots"].as_f64().unwrap(),
        "live slots",
    );
    close(
        trial.revives_used,
        expected["revivesUsed"].as_f64().unwrap(),
        "revives used",
    );
    close(
        trial.revives_remaining,
        expected["revivesRemaining"].as_f64().unwrap(),
        "revives remaining",
    );
    assert_eq!(
        trial.roid_used,
        expected["roidUsed"].as_bool().unwrap(),
        "{name}: roid"
    );
    match expected["roidStart"].as_f64() {
        Some(start) => close(trial.roid_start.unwrap_or(f64::NAN), start, "roid start"),
        None => assert!(trial.roid_start.is_none(), "{name}: no roid start"),
    }
    close(
        trial.amoeba_stacks,
        expected["amoebaStacks"].as_f64().unwrap(),
        "amoeba stacks",
    );
    close(
        trial.amoeba_stacks_at_critical_start,
        expected["amoebaStacksAtCriticalStart"].as_f64().unwrap(),
        "amoeba at critical",
    );
    close(
        trial.damage_at_critical_start,
        expected["damageAtCriticalStart"].as_f64().unwrap(),
        "damage at critical",
    );
    close(
        trial.attempt_delta,
        expected["attemptDelta"].as_f64().unwrap(),
        "attempt delta",
    );
    assert_eq!(
        trial.obstruction_after,
        expected["obstructionAfter"].as_u64().unwrap() as usize,
        "{name}: obstruction after"
    );
    let deaths = expected["deathLog"].as_array().unwrap();
    for (index, expected) in deaths.iter().enumerate() {
        let death = trial
            .death_log
            .get(index)
            .unwrap_or_else(|| panic!("{name}: death {index} is missing"));
        close(death.time, expected["time"].as_f64().unwrap(), "death time");
        assert_eq!(death.slot, expected["slot"].as_u64().unwrap() as usize);
        assert_eq!(death.focused, expected["focused"].as_bool().unwrap());
        assert_eq!(
            death.disabled_anchor,
            expected["disabledAnchor"].as_bool().unwrap()
        );
        assert_eq!(death.revived, expected["revived"].as_bool().unwrap());
    }
    for (contribution, expected) in trial
        .contribution
        .iter()
        .zip(expected["contribution"].as_array().unwrap())
    {
        close(
            contribution.shots,
            expected["shots"].as_f64().unwrap(),
            "trial contribution shots",
        );
        close(
            contribution.hits,
            expected["hits"].as_f64().unwrap(),
            "trial contribution hits",
        );
        close(
            contribution.damage,
            expected["damage"].as_f64().unwrap(),
            "trial contribution damage",
        );
        close(
            contribution.bloodcells,
            expected["bloodcells"].as_f64().unwrap(),
            "trial contribution bloodcells",
        );
        close(
            contribution.exp,
            expected["exp"].as_f64().unwrap(),
            "trial contribution EXP",
        );
    }
}

#[test]
fn insights_match_upstream() {
    for name in ["save", "small-board", "clear"] {
        let (jelly, case) = jelly(name);
        let options = &case["estimateOptions"];
        let estimate = Estimate::run(
            &jelly,
            &jelly.layout,
            options["trials"].as_u64().unwrap() as u32,
            options["seed"].as_u64().unwrap() as u32,
        );
        let insights = Insights::of(&jelly, &jelly.layout, &estimate);
        let expected = &case["insights"];
        assert_eq!(
            insights.valid,
            expected["valid"].as_bool().unwrap(),
            "{name}: valid"
        );
        close(
            insights.daily_attempts,
            expected["dailyAttempts"].as_f64().unwrap(),
            "daily attempts",
        );
        close_opt(
            insights.expected_attempts_to_clear,
            &expected["expectedAttemptsToClear"],
            "attempts to clear",
        );
        close(
            insights.probability_within_daily_attempts,
            expected["probabilityWithinDailyAttempts"].as_f64().unwrap(),
            "daily probability",
        );
        close_opt(
            insights.expected_days_to_clear,
            &expected["expectedDaysToClear"],
            "days to clear",
        );
        close(
            insights.expected_bloodcells_per_attempt,
            expected["expectedBloodcellsPerAttempt"].as_f64().unwrap(),
            "bloodcells per attempt",
        );
        close(
            insights.saved_daily_passive_bloodcells,
            expected["savedDailyPassiveBloodcells"].as_f64().unwrap(),
            "passive bloodcells",
        );
        close(
            insights.expected_bloodcells_per_daily_cycle,
            expected["expectedBloodcellsPerDailyCycle"]
                .as_f64()
                .unwrap(),
            "bloodcells per cycle",
        );
        close(
            insights.expected_net_attempt_delta,
            expected["expectedNetAttemptDelta"].as_f64().unwrap(),
            "net attempts",
        );
        close(
            insights.normal_damage,
            expected["normalDamage"].as_f64().unwrap(),
            "normal damage",
        );
        close(
            insights.critical_damage,
            expected["criticalDamage"].as_f64().unwrap(),
            "critical damage",
        );
        close_opt(
            insights.normal_damage_share,
            &expected["normalDamageShare"],
            "normal share",
        );
        close_opt(
            insights.critical_damage_share,
            &expected["criticalDamageShare"],
            "critical share",
        );
        for (row, expected) in insights
            .exp_rows
            .iter()
            .zip(expected["expRows"].as_array().unwrap())
        {
            assert_eq!(row.cell_type, expected["type"].as_u64().unwrap() as usize);
            assert_eq!(
                row.current_level,
                expected["currentLevel"].as_u64().unwrap() as u32
            );
            close(
                row.current_exp,
                expected["currentExp"].as_f64().unwrap(),
                "cell EXP",
            );
            close(
                row.requirement,
                expected["requirement"].as_f64().unwrap(),
                "requirement",
            );
            close(
                row.remaining_exp,
                expected["remainingExp"].as_f64().unwrap(),
                "remaining",
            );
            close(
                row.mean_exp_per_attempt,
                expected["meanExpPerAttempt"].as_f64().unwrap(),
                "EXP per attempt",
            );
            close_opt(
                row.attempts_to_next_level,
                &expected["attemptsToNextLevel"],
                "attempts to level",
            );
            close_opt(
                row.days_to_next_level,
                &expected["daysToNextLevel"],
                "days to level",
            );
        }
        for (row, expected) in insights
            .obstruction_outlook
            .iter()
            .zip(expected["obstructionOutlook"].as_array().unwrap())
        {
            assert_eq!(
                row.obstruction,
                expected["obstruction"].as_u64().unwrap() as usize
            );
            close(row.hp, expected["hp"].as_f64().unwrap(), "outlook HP");
            close(row.time, expected["time"].as_f64().unwrap(), "outlook time");
            close_opt(
                row.estimated_clear_probability,
                &expected["estimatedClearProbability"],
                "outlook probability",
            );
            close_opt(row.damage_ratio, &expected["damageRatio"], "outlook ratio");
            close_opt(
                row.expected_attempts,
                &expected["expectedAttempts"],
                "outlook attempts",
            );
            close_opt(
                row.clear_within_daily_attempts,
                &expected["clearWithinDailyAttempts"],
                "outlook daily",
            );
        }
    }
}
