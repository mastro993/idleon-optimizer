//! Spelunking depth profiles from an imported save.

use idleon_import::import;
use idleon_spelunking::{Account, DepthRange, Inputs, ReferenceTool, Requested, Save, profile};

/// The fixture save's inputs under a neutral account.
fn inputs() -> Inputs {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("the fixture is committed");
    let imported = import(&text).expect("the fixture imports");
    let save = Save::from_parts(&imported.save).expect("the fixture carries Spelunk rows");
    save.inputs(&Account::default())
}

#[test]
fn the_depth_range_is_bounded() {
    let inputs = inputs();
    for (from, to) in [(0, 10), (10, 5), (1, 10_001), (1, 1_002)] {
        let error = profile(
            &inputs,
            &Requested::default(),
            &DepthRange {
                from,
                to,
                tool: ReferenceTool::Best,
            },
        )
        .expect_err("the range is out of bounds");
        assert_eq!(
            error.to_string(),
            "Choose depths 1-10,000 spanning at most 1,001 floors."
        );
    }
}
