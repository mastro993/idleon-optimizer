//! The Decision Tree's event stepper on the committed account fixture.

use idleon_common::{Control, Progress};
use idleon_research::tree::{self, Tree};

fn fixture_save() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

#[test]
fn a_stopped_chain_keeps_the_tree_as_it_was() {
    let imported = idleon_import::import(&fixture_save()).expect("the fixture imports");
    let (simulation, context) = (imported.simulation, imported.context);
    let mut tree = Tree::from_state(&simulation);
    tree.toggle_auto_insight(1);
    let before = tree.nodes().len();
    let mut report = |_: Progress| Control::Stop;
    assert!(
        tree::auto_chain(&simulation, &context, &mut report).is_none(),
        "a cancelled chain returns nothing"
    );
    assert_eq!(tree.nodes().len(), before);
}
