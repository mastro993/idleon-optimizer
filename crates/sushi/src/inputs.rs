//! The save side of the Sushi Station tab: the station rows, the account terms
//! the currency and fuel formulas read, and the RoG table.
//!
//! Upstream keeps the station on `saveData.sushiData` (`Sushi`), the unique
//! tier count on `cachedUniqueSushi` and the rest on the account systems the
//! shared decoder already reads. A field the save does not carry is reported,
//! never guessed.

use crate::math::{External, Station, compute_unique_sushi};
use idleon_account::Decoder;
use idleon_research::model::Simulation;
use idleon_save::Save;
use idleon_save::json;
use serde_json::Value;

/// Everything the tab reads, decoded from one save.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inputs {
    /// The save's research level, which gates the upgrades.
    pub research_level: u32,
    pub station: Station,
    pub external: External,
}

/// One row of the `Sushi` save field as numbers.
///
/// Rows the formulas read with `Number(x) || 0` keep their zero default; the
/// rows that test for `NaN` (tiers, slot types, fireplaces) keep it.
fn row(rows: &[Vec<Value>], index: usize, nan: bool) -> Vec<f64> {
    rows.get(index)
        .map(|cells| {
            cells
                .iter()
                .map(|cell| json::number(cell).unwrap_or(if nan { f64::NAN } else { 0.0 }))
                .collect()
        })
        .unwrap_or_default()
}

/// Decode the tab's inputs, with a warning per field the save does not carry.
pub fn inputs(save: &Save, simulation: &Simulation) -> (Inputs, Vec<String>) {
    let mut warnings = Vec::new();
    let rows = sushi_rows(save, &mut warnings);
    let found = row(&rows, 5, false);
    let station = Station {
        tiers: row(&rows, 0, true),
        types: row(&rows, 1, true),
        levels: row(&rows, 2, false),
        fires: row(&rows, 3, true),
        misc: row(&rows, 4, false),
        unique: compute_unique_sushi(&found),
        found,
        knowledge: row(&rows, 7, false),
        doublers: Vec::new(),
    };

    let Some((fields, root)) = save.parts() else {
        warnings.push("save: not a JSON object; the Sushi Station tab has no inputs".into());
        return (
            Inputs {
                station,
                ..Default::default()
            },
            warnings,
        );
    };
    let characters = idleon_save::characters(fields);
    let research_level = idleon_research::save::research_level(fields, &characters).unwrap_or(0);
    let decoder = Decoder::new(fields, root, &characters, simulation, &mut warnings);
    let external = External {
        arcade67: decoder.arcade(67),
        grid188: decoder.grid_bonus(188),
        grid189: decoder.grid_bonus(189),
        minehead11: decoder.minehead_bonus(11),
        atom14: decoder.atom(14),
        sailing39: sailing_artifact(save, 39),
        // Upstream reads `bon_v` here (`sushi-ui.js`), not the `bun_v` id.
        bundle_v: decoder.bundle("bon_v"),
        super_bit67: decoder.superbit(67),
        button2: decoder.button_bonus(2),
        jelly18: decoder.jelly(18),
        event_shop66: decoder.event_shop(66),
    };
    let station = Station {
        doublers: decoder.sushi_doublers(),
        ..station
    };
    (
        Inputs {
            research_level,
            station,
            external,
        },
        warnings,
    )
}

/// The `Sushi` rows, decoded from the save's JSON string when needed.
fn sushi_rows(save: &Save, warnings: &mut Vec<String>) -> Vec<Vec<Value>> {
    let Some(fields) = save.as_object() else {
        return Vec::new();
    };
    let Some(sushi) = json::parsed_field(fields, "Sushi") else {
        warnings.push("Sushi: not in the save; the station renders empty".into());
        return Vec::new();
    };
    let Some(rows) = sushi.as_array() else {
        warnings.push("Sushi: not an array of rows; the station renders empty".into());
        return Vec::new();
    };
    rows.iter()
        .map(|row| row.as_array().cloned().unwrap_or_default())
        .collect()
}

/// `Sailing[3][index]`, the artifacts the station reads.
fn sailing_artifact(save: &Save, index: usize) -> f64 {
    save.get("Sailing")
        .and_then(|sailing| json::index(&sailing, &[3, index]).and_then(json::number))
        .unwrap_or(0.0)
}
