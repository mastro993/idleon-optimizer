//! A project's research half is the decoded state the save importer reads.

use idleon_import::{Format, import};
use idleon_project::{Origin, Project, VERSION, write};
use idleon_research::model::{Mag, MagKind, Placement, SimContext, Simulation};
use idleon_research::{GridState, Tiers, game};
use serde_json::{Map, Value};

fn simulation() -> Simulation {
    let mut simulation = Simulation {
        grid: GridState::seeded(2),
        ..Simulation::default()
    };
    simulation.research_level = 122;
    simulation.research_exp = 4_200.0;
    simulation.observations.found[0] = true;
    simulation.observations.insight_level[0] = 12;
    simulation.observations.insight_exp[3] = 55.5;
    simulation.mags = vec![
        Mag::new(MagKind::Regular, Some(0)),
        Mag::new(MagKind::Monocle, None),
    ];
    simulation.placements = vec![Placement {
        shape: 1,
        x: 30.0,
        y: 60.0,
        rot: 90.0,
    }];
    simulation.saved_overlay = Some(vec![0; game().cells()]);
    simulation.preset = "_none".into();
    simulation.shape_tiers = Tiers::parse("J6,I5|").expect("the fixture tiers");
    simulation
}

#[test]
fn an_exported_project_reads_as_a_research_state_in_the_save_importer() {
    let project = Project {
        version: VERSION,
        section: None,
        save: None,
        simulation: simulation(),
        context: SimContext::default(),
        sections: Map::new(),
        warnings: Vec::new(),
        origin: Origin::Project,
    };
    let document: Value = serde_json::from_str(&write(&project)).expect("the project is JSON");
    let imported = import(&document["research"].to_string()).expect("the research half imports");
    assert_eq!(imported.simulation, simulation());
    assert_eq!(imported.format, Format::ResearchState);
}
