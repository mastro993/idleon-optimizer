//! The save-derived combat config read from the committed synthetic save.

use idleon_save::{Save, json};
use idleon_stats::combat;

fn save() -> Save {
    let path = format!("{}/../../fixtures/data.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).expect("the synthetic save is committed");
    Save::new(serde_json::from_str(&text).expect("save JSON"))
}

#[test]
fn the_den_config_reads_the_saved_holes_rows() {
    let save = save();
    let config = combat::den(&save, 0).expect("the fixture save has a cavern pick");
    let holes = save.get("Holes").expect("Holes");
    let at = |path: &[usize]| {
        json::index(&holes, path)
            .and_then(json::number)
            .unwrap_or(0.0)
    };
    for (index, level) in config.amps.iter().enumerate() {
        assert_eq!(at(&[12, index]), *level, "amplifier {index}");
    }
    assert_eq!(1.0, config.b_upg23);
    assert_eq!(at(&[26, 2]), config.bolaia_level);
    assert_eq!(at(&[11, 8]), config.best_score);
    assert_eq!(at(&[7, 2]), config.opal_level);
    assert_eq!(idleon_combat::den::hound_hp(), config.hound_hp);
}
