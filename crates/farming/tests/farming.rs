//! Farming import: the farm fields map onto plots, ranks, crops and market
//! levels, and the plot metrics follow the upstream formulas.

use idleon_farming::{PLOTS, RANK_UPGRADES, data, land_rank_requirement, overgrowth_multiplier};
use idleon_import::Format;
use idleon_import::import;
use serde_json::{Value, json};

/// A row of `length` zeros with `(index, value)` entries set.
fn row(length: usize, entries: &[(usize, f64)]) -> Vec<f64> {
    let mut row = vec![0.0; length];
    for (index, value) in entries {
        row[*index] = *value;
    }
    row
}

/// The 36 plots: plot one BASIC at evolution 2, plot two MEDAL, the rest empty.
fn plots() -> Value {
    let mut plots = vec![json!([]); PLOTS];
    plots[0] = json!([0.0, 1000.0, 2.0, 0.0, 1.0, 0.0, 0.0]);
    plots[1] = json!([6.0, 0.0, 0.0, 0.0, 1.0, 3.0, 0.0]);
    Value::Array(plots)
}

/// A save with every farming field the importer reads.
fn farming_save() -> Value {
    let mut spelunk = vec![json!([]); 60];
    spelunk[50] = json!([5.0]);
    let mut gems = vec![0.0; 150];
    gems[139] = 2.0;
    json!({
        "FarmPlot": Value::String(plots().to_string()),
        "FarmRank": Value::String(
            json!([
                row(PLOTS, &[(0, 1.0), (1, 2.0)]),
                row(PLOTS, &[(0, 10.0)]),
                row(RANK_UPGRADES, &[(0, 1.0)]),
            ])
            .to_string()
        ),
        "FarmUpg": Value::String(json!(row(100, &[])).to_string()),
        "FarmCrop": Value::String(json!({ "0": 250.0, "1": "5" }).to_string()),
        "Spelunk": Value::String(Value::Array(spelunk).to_string()),
        "GemItemsPurchased": Value::String(json!(gems).to_string()),
        "Lv0_0": Value::String(json!(row(24, &[(16, 60.0)])).to_string()),
    })
}

#[test]
fn farming_fields_import_into_the_field_and_its_ranks() {
    let imported = import(&farming_save().to_string()).expect("the farming save imports");
    let farm = &imported.farming;

    assert_eq!(imported.format, Format::SaveJson);
    assert_eq!(imported.characters, 1);
    assert_eq!(farm.farm_level, 60);
    assert_eq!(farm.plots.len(), PLOTS);
    assert_eq!(farm.plots[0].seed_family, 0);
    assert_eq!(farm.plots[0].evolution, 2);
    assert_eq!(farm.plots[0].growth_progress, 1000.0);
    assert_eq!(farm.plots[1].seed_family, 6);
    assert_eq!(farm.plots[1].overgrowth, 3);
    assert_eq!(farm.plots[2], idleon_farming::Plot::default());

    assert_eq!(farm.rank_ranks[0], 1);
    assert_eq!(farm.rank_ranks[1], 2);
    assert_eq!(farm.rank_progress[0], 10.0);
    assert_eq!(farm.rank_levels[0], 1);
    assert_eq!(farm.total_land_rank(), 3);
    assert_eq!(farm.land_rank_points_left(), 2);
    assert_eq!(farm.market_levels.len(), 100);

    assert_eq!(farm.crop_count, 2);
    assert_eq!(farm.crop_quantities.get(&0), Some(&250.0));
    assert_eq!(farm.crop_quantities.get(&1), Some(&5.0));
    assert_eq!(farm.crop_threshold_counts(), [1.0, 0.0, 0.0, 0.0, 0.0]);
    assert_eq!(farm.organic_exotics, [5]);
    assert_eq!(farm.gem_items.get(139), Some(&2.0));

    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning.contains("farming multipliers")),
        "the import names the multipliers it cannot read: {:?}",
        imported.warnings
    );
    assert!(
        !imported
            .warnings
            .iter()
            .any(|warning| warning.contains("FarmPlot") || warning.contains("FarmRank")),
        "the save carries the farm fields: {:?}",
        imported.warnings
    );
}

#[test]
fn plot_metrics_follow_the_upstream_formulas() {
    let imported = import(&farming_save().to_string()).expect("the farming save imports");
    let farm = &imported.farming;
    let snapshot = farm.snapshot();
    assert_eq!(snapshot.growth_rate, 1.0);

    let first = farm.plot_metrics_with(0, &snapshot);
    assert_eq!(first.crop_id, 2);
    assert_eq!(first.seed_family, 0);
    assert_eq!(first.growth_requirement, 14_400.0);
    assert_eq!(first.growth_seconds, 13_400.0);
    let plot_one_rank = 1.0 + snapshot.rank.bonuses[0] / 100.0;
    assert!((first.next_crop_chance - 0.3 * 0.75_f64.powi(2) * plot_one_rank).abs() < 1e-12);
    assert!((first.next_overgrowth_chance - 0.4).abs() < 1e-12);
    assert_eq!(first.farming_exp, 5.0);
    assert_eq!(first.crop_value.expected, 1.0);
    assert!((first.crop_value.expected_harvest - 1.4).abs() < 1e-12);
    assert_eq!(first.rank, 1);
    assert_eq!(first.rank_requirement, land_rank_requirement(1));
    assert_eq!(first.rank_xp, 0.0);

    let medal = farm.plot_metrics_with(1, &snapshot);
    assert_eq!(medal.crop_id, 230);
    assert_eq!(medal.growth_requirement, 25_200.0);
    let evolution_bonus = 1.0 + snapshot.rank.bonuses[0] * 2.0 / 100.0;
    assert!((medal.next_crop_chance - 0.3 * evolution_bonus).abs() < 1e-12);
    assert!(
        (medal.next_overgrowth_chance - 0.4_f64.powi(4) * snapshot.overgrowth_common).abs() < 1e-12
    );
    assert_eq!(
        medal.crop_value.expected_harvest,
        overgrowth_multiplier(3) * snapshot.expected_vines * medal.crop_value.expected
    );
}

#[test]
fn a_decoded_farming_state_imports_under_its_own_format() {
    let text = json!({
        "farmPlotData": plots(),
        "farmRankData": [
            row(PLOTS, &[(3, 7.0)]),
            row(PLOTS, &[]),
            row(RANK_UPGRADES, &[(4, 2.0)]),
        ],
        "farmUpgData": row(100, &[(20, 4.0)]),
        "farmCropData": { "44": 1000.0 },
        "farmCropCount": 42,
    });
    let imported = import(&text.to_string()).expect("a decoded farm state imports");
    assert_eq!(imported.format, Format::FarmingState);
    assert_eq!(imported.farming.rank_ranks[3], 7);
    assert_eq!(imported.farming.rank_levels[4], 2);
    assert_eq!(imported.farming.crop_count, 42);
    assert_eq!(imported.farming.market_levels[20], 4.0);
}

#[test]
fn a_save_without_farming_fields_keeps_an_empty_field_silently() {
    let imported = import(
        &json!({
            "Research": "[[]]",
            "Lv0_0": "[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,4]",
        })
        .to_string(),
    )
    .expect("a research-only save imports");
    assert!(!imported.farming.has_plots());
    assert!(
        !imported
            .warnings
            .iter()
            .any(|warning| warning.starts_with("Farm") || warning.contains("farming multipliers")),
        "a research-only save does not warn about farming: {:?}",
        imported.warnings
    );
}

#[test]
fn the_bundled_extract_covers_the_upstream_tables() {
    let data = data();
    assert_eq!(data.seeds.len(), 7);
    assert_eq!(data.market.len(), 16);
    assert_eq!(data.exotics.len(), 80);
    assert_eq!(data.rank_names.len(), RANK_UPGRADES);
    assert_eq!(data.rank_unlocks.len(), RANK_UPGRADES);
    assert_eq!(data.ninja_rank_bases.len(), RANK_UPGRADES);
    assert_eq!(
        data.gmo_thresholds,
        [200.0, 1000.0, 2500.0, 10_000.0, 100_000.0]
    );
    assert!(data.market(99, 3).is_some_and(|row| row.page == 1));
}
