//! The Decision Tree's event stepper against the upstream tree simulator.
//!
//! `fixtures/decision-tree.json` is written by `script/decision-tree-oracle.mjs`
//! from upstream `dtStepSim` and `dtAdvanceAutoInsight`: the first chain steps
//! the committed account fixture, the second a small synthetic state that
//! reaches insight level-ups within hours. The port is compared with the
//! calculator's own output rather than numbers picked here.

use idleon_common::{Control, Progress};
use idleon_research::model::{GridState, Mag, MagKind, Observations, SimContext, Simulation};
use idleon_research::sim::Event;
use idleon_research::tree::{self, Tree};
use serde_json::Value;

fn oracle() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/decision-tree.json"
    );
    let text = std::fs::read_to_string(path).expect("fixtures/decision-tree.json is committed");
    serde_json::from_str(&text).expect("the oracle fixture is valid JSON")
}

fn fixture_save() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

/// The fixture records relative rates; the port is within the documented
/// `fixtures/data.json` tolerance of the upstream calculator.
fn close_rate(port: f64, upstream: f64, what: &str) {
    let scale = port.abs().max(upstream.abs()).max(1.0);
    assert!(
        (port - upstream).abs() <= 1e-4 * scale,
        "{what}: port {port} vs upstream {upstream}"
    );
}

fn close_hours(port: f64, upstream: f64, what: &str) {
    let scale = port.abs().max(upstream.abs()).max(1.0);
    assert!(
        (port - upstream).abs() <= 1e-6 * scale,
        "{what}: port {port} vs upstream {upstream}"
    );
}

fn event(name: &str) -> Event {
    match name {
        "level-up" => Event::Level,
        "insight-up" => Event::Insight,
        "level+insight" => Event::LevelAndInsight,
        other => panic!("unknown event {other}"),
    }
}

fn no_report() -> impl FnMut(Progress) -> Control {
    |_| Control::Continue
}

/// The synthetic state both chains below start from.
fn synth(oracle: &Value) -> (Simulation, SimContext) {
    let imported = idleon_import::import(&fixture_save()).expect("the fixture imports");
    let state = &oracle["synthetic"]["state"];
    let levels: Vec<u64> = state["gridLevels"]
        .as_array()
        .expect("a grid")
        .iter()
        .map(|level| level.as_u64().expect("a level"))
        .collect();
    let mut grid = GridState::empty();
    for (cell, level) in levels.into_iter().enumerate() {
        grid.set(cell, level as u32);
    }
    let insight = |key: &str| -> Vec<Value> { state[key].as_array().expect("an array").clone() };
    let observations = Observations {
        found: insight("occFound")
            .iter()
            .map(|flag| flag.as_u64().unwrap_or(0) >= 1)
            .collect(),
        insight_level: insight("insightLevels")
            .iter()
            .map(|level| level.as_u64().unwrap_or(0) as u32)
            .collect(),
        insight_exp: insight("insightProgress")
            .iter()
            .map(|progress| progress.as_f64().unwrap_or(0.0))
            .collect(),
    };
    let mags = state["mags"]
        .as_array()
        .expect("a magnifier pool")
        .iter()
        .map(|mag| {
            let kind = match mag["type"].as_u64().expect("a kind") {
                0 => MagKind::Regular,
                1 => MagKind::Monocle,
                _ => MagKind::Kaleidoscope,
            };
            Mag::new(
                kind,
                mag["slot"]
                    .as_u64()
                    .map(|slot| slot as usize)
                    .filter(|slot| *slot < 80),
            )
        })
        .collect();
    let simulation = Simulation {
        research_level: state["researchLevel"].as_u64().expect("a level") as u32,
        research_exp: state["researchExp"].as_f64().expect("EXP"),
        grid,
        observations,
        mags,
        placements: Vec::new(),
        saved_overlay: Some(
            insight("shapeOverlay")
                .iter()
                .map(|shape| shape.as_i64().unwrap_or(-1) as i32)
                .collect(),
        ),
        ..Simulation::default()
    };
    (simulation, imported.context)
}

fn compare_chain(tree: &mut Tree, chain: &Value, ctx: &SimContext) {
    let mut from = 1;
    let events = chain.as_array().expect("a chain");
    for (index, expected) in events.iter().enumerate() {
        let mut report = no_report();
        let next = tree
            .advance(from, ctx, &mut report)
            .unwrap_or_else(|| panic!("the port stepped nowhere at event {index}"));
        let node = tree.node(next).expect("the new node");
        assert_eq!(
            node.event,
            event(expected["event"].as_str().expect("an event")),
            "event {index}"
        );
        close_hours(
            node.hours,
            expected["stepHours"].as_f64().unwrap(),
            "step hours",
        );
        close_hours(
            tree.total_hours(next),
            expected["totalHours"].as_f64().unwrap(),
            "total hours",
        );
        assert_eq!(
            node.state.research_level,
            expected["level"].as_u64().unwrap() as u32,
            "level after event {index}"
        );
        close_rate(
            idleon_research::sim::exp_rate(&node.state, ctx).total,
            expected["expHr"].as_f64().unwrap(),
            "EXP/hr after event",
        );
        from = next;
    }
}

#[test]
fn the_save_derived_chain_matches_upstream() {
    let oracle = oracle();
    let imported = idleon_import::import(&fixture_save()).expect("the fixture imports");
    let start = &oracle["start"];
    assert_eq!(
        u64::from(imported.simulation.research_level),
        start["level"].as_u64().unwrap()
    );
    assert_eq!(
        imported.simulation.grid.levels().len(),
        240,
        "the fixture grid decodes"
    );
    close_hours(
        imported.simulation.research_exp,
        start["exp"].as_f64().unwrap(),
        "starting research EXP",
    );
    close_rate(
        idleon_research::sim::exp_rate(&imported.simulation, &imported.context).total,
        start["expHr"].as_f64().unwrap(),
        "starting EXP/hr",
    );
    let owned = idleon_research::sim::magnifiers_owned(&imported.simulation, &imported.context);
    assert_eq!(owned, start["magnifiers"].as_u64().unwrap() as u32);
    assert!(
        imported.simulation.mags.len() >= owned as usize,
        "the decoded pool holds at least the owned magnifiers"
    );
    assert!(
        imported.simulation.placements.len() <= start["shapes"].as_u64().unwrap() as usize,
        "the decoded placements are the owned shapes"
    );

    let mut tree = Tree::from_state(&imported.simulation);
    compare_chain(&mut tree, &oracle["chain"], &imported.context);
}

#[test]
fn the_synthetic_chain_matches_upstream() {
    let oracle = oracle();
    let (simulation, context) = synth(&oracle);
    let mut tree = Tree::from_state(&simulation);
    compare_chain(&mut tree, &oracle["synthetic"]["chain"], &context);
}

#[test]
fn auto_insight_matches_upstream() {
    let oracle = oracle();
    let (simulation, context) = synth(&oracle);
    let expected = &oracle["synthetic"]["autoInsight"];
    let mut tree = Tree::from_state(&simulation);
    assert!(tree.toggle_auto_insight(1));
    let mut progress = Vec::new();
    let mut report = |line: Progress| {
        progress.push(line.message);
        Control::Continue
    };
    let tail = tree
        .advance(1, &context, &mut report)
        .expect("the chain reaches a research level-up");
    let node = tree.node(tail).expect("the tail");
    assert_eq!(
        u64::from(node.state.research_level),
        expected["tailLevel"].as_u64().unwrap()
    );
    let upstream = expected["nodes"].as_array().expect("nodes");
    assert_eq!(
        tree.nodes().len(),
        upstream.len(),
        "node count (implicit chain included)"
    );
    let timeline = upstream
        .last()
        .and_then(|node| node["timeline"].as_array())
        .expect("the upstream timeline");
    assert_eq!(node.timeline.len(), timeline.len(), "timeline entries");
    for (entry, expected) in node.timeline.iter().zip(timeline) {
        assert_eq!(entry.obs, expected["obs"].as_u64().unwrap() as usize);
        assert_eq!(entry.before, expected["prevIL"].as_u64().unwrap() as u32);
        assert_eq!(entry.after, expected["newIL"].as_u64().unwrap() as u32);
    }
    assert!(!progress.is_empty(), "the chain reported no progress");
}

#[test]
fn a_stopped_chain_keeps_the_tree_as_it_was() {
    let oracle = oracle();
    let (simulation, context) = synth(&oracle);
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
