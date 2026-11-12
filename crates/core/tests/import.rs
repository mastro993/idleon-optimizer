//! The one import pipeline: detection, migration to the current schema and the
//! message each failure gets.

use base64::Engine as _;
use flate2::Compression;
use flate2::write::GzEncoder;
use idleon_core::import::{self, Document, Error};
use idleon_core::project::{self, FORMAT, SECTION_FORMAT};
use serde_json::{Value, json};
use std::io::Write as _;

/// The research rows the save reader expects, with two levels on the grid.
fn research_rows() -> Value {
    let mut grid = vec![0.0; 240];
    grid[50] = 3.0;
    grid[52] = 4.0;
    let overlay = vec![-1.0; 240];
    let mut found = vec![0.0; 80];
    found[0] = 1.0;
    let row = |length: usize| vec![0.0; length];
    Value::Array(vec![
        json!(grid),
        json!(overlay),
        json!(found),
        json!(row(80)),
        json!(row(80)),
        json!([]),
        json!([]),
    ])
}

/// A small flat `save.json`: one character at research level 42 and the rows.
fn save() -> Value {
    let mut character = vec![0.0; 30];
    character[20] = 42.0;
    json!({
        "Research": research_rows().to_string(),
        "Lv0_0": Value::Array(character.into_iter().map(|cell| json!(cell)).collect()).to_string(),
    })
}

/// A project document with a research state and one section config.
fn project_document() -> Value {
    json!({
        "format": FORMAT,
        "version": project::VERSION,
        "section": "killroy",
        "research": {
            "researchLevel": 123,
            "gridLevels": vec![0; 240],
            "occFound": vec![0; 80],
            "insightLvs": vec![0; 80],
            "insightProgress": vec![0; 80],
            "magData": [],
            "shapePositions": [],
        },
        "sections": { "killroy": { "weeks": 5 } },
    })
}

/// An upstream `ROv2:` state string: base64 of gzip of the state object.
fn upstream_state(version: u32) -> String {
    let state = json!({
        "_format": "research-optimizer-state",
        "_version": version,
        "researchLevel": 140,
        "gridLevels": vec![1; 240],
        "occFound": vec![0; 80],
        "insightLvs": vec![0; 80],
        "insightProgress": vec![0; 80],
        "magData": [],
        "shapePositions": [],
    });
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(state.to_string().as_bytes())
        .expect("gzip");
    format!(
        "ROv2:{}",
        base64::engine::general_purpose::STANDARD.encode(encoder.finish().expect("gzip"))
    )
}

#[test]
fn every_accepted_format_is_detected_and_named() {
    let project = import::read(&project_document().to_string()).expect("the project reads");
    assert_eq!(project.format(), "project");
    assert_eq!(project.research_level(), Some(123));
    assert_eq!(project.section_configs(), 1);

    let state = import::read(&upstream_state(2)).expect("the state string reads");
    assert_eq!(state.format(), "upstream state");
    assert_eq!(state.research_level(), Some(140));
    // The migrated state carries no save, so it names no character count.
    assert_eq!(state.characters(), None);

    let section = json!({
        "format": SECTION_FORMAT,
        "version": project::VERSION,
        "section": "sailing",
        "config": { "island": 3 },
    });
    let section = import::read(&section.to_string()).expect("the config reads");
    assert_eq!(section.format(), "section config");
    assert_eq!(section.section_id(), Some("sailing"));
    assert_eq!(section.section_configs(), 1);

    let flat = import::read(&save().to_string()).expect("the flat save reads");
    assert_eq!(flat.format(), "save.json");
    assert_eq!(flat.research_level(), Some(42));
    assert_eq!(flat.characters(), Some(1));

    let wrapped = json!({ "data": save(), "serverVars": {} });
    let wrapped = import::read(&wrapped.to_string()).expect("the wrapped save reads");
    assert_eq!(wrapped.format(), "it.json");
    assert_eq!(wrapped.characters(), Some(1));

    let research = json!({ "researchLevel": 60, "gridLevels": vec![0; 240] });
    let research = import::read(&research.to_string()).expect("the research state reads");
    assert_eq!(research.format(), "research state");
    assert_eq!(research.research_level(), Some(60));
}

#[test]
fn malformed_json_names_the_parser_detail() {
    let error = import::read("not json at all").expect_err("garbage cannot be read");
    assert!(matches!(error, Error::NotJson(_)), "{error:?}");
    let message = error.to_string();
    assert!(message.starts_with("not valid JSON: "), "{message}");
    assert!(matches!(import::read("{ trailing"), Err(Error::NotJson(_))));
    assert!(matches!(import::read("   "), Err(Error::NotJson(_))));
}

#[test]
fn a_wrong_format_marker_names_the_marker_and_the_known_ones() {
    let text = json!({ "format": "some-other-tool", "version": 1 }).to_string();
    let error = import::read(&text).expect_err("an unknown marker is not a save");
    assert!(
        matches!(&error, Error::Document(project::Error::Format(found)) if found == "some-other-tool"),
        "{error:?}"
    );
    let message = error.to_string();
    assert!(message.contains("some-other-tool"), "{message}");
    assert!(message.contains(FORMAT), "{message}");
    assert!(message.contains(SECTION_FORMAT), "{message}");
}

#[test]
fn an_unsupported_version_names_the_version_and_what_the_build_reads() {
    let mut document = project_document();
    document["version"] = json!(9);
    let error = import::read(&document.to_string()).expect_err("version 9 is newer");
    assert!(
        matches!(&error, Error::Document(project::Error::Version(detail)) if detail.contains('9')),
        "{error:?}"
    );
    let message = error.to_string();
    assert!(message.contains("version 9"), "{message}");
    assert!(message.contains(&project::VERSION.to_string()), "{message}");

    let error = import::read(&upstream_state(7)).expect_err("state version 7 is unknown");
    let message = error.to_string();
    assert!(message.contains("upstream state 7"), "{message}");
}

#[test]
fn a_project_whose_embedded_save_is_unreadable_fails_before_anything_applies() {
    let mut document = project_document();
    document["save"] = json!({ "not": "a save" });
    let error = import::read(&document.to_string()).expect_err("the save cannot be read");
    assert!(matches!(error, Error::EmbeddedSave(_)), "{error:?}");
    let message = error.to_string();
    assert!(message.starts_with("save: "), "{message}");
    assert!(message.contains("no save fields found"), "{message}");
    // Nothing is prepared: there is no document to apply.
    assert!(matches!(
        import::read(&document.to_string()),
        Err(Error::EmbeddedSave(_))
    ));
}

#[test]
fn a_project_including_a_save_prepares_it_before_apply() {
    let mut document = project_document();
    document["save"] = save();
    let Document::Project(project) =
        import::read(&document.to_string()).expect("the project reads")
    else {
        panic!("a project file is a project document");
    };
    let saved = project.save.as_ref().expect("the embedded save is decoded");
    assert_eq!(saved.characters, 1);
    assert_eq!(saved.simulation.research_level, 42);
    // The project's own research state is what applies, not the save's.
    assert_eq!(project.document.simulation.research_level, 123);
    assert_eq!(project.characters(), Some(1));
}

#[test]
fn an_ro_v2_state_string_is_decompressed_and_migrated() {
    let Document::Project(project) = import::read(&upstream_state(1)).expect("the state reads")
    else {
        panic!("a state string is a project document");
    };
    assert_eq!(project.document.origin, project::Origin::UpstreamState);
    assert_eq!(project.document.version, project::VERSION);
    assert_eq!(project.document.simulation.research_level, 140);
    assert!(project.save.is_none());
    // The upstream export carries no save, and the migration says so.
    assert!(
        project
            .document
            .warnings
            .iter()
            .any(|warning| warning.starts_with("save: ")),
        "{:?}",
        project.document.warnings
    );
}

#[test]
fn warnings_name_the_field_and_the_fallback_used() {
    let text = json!({
        "format": FORMAT,
        "version": project::VERSION,
        "research": { "gridLevels": [] },
        "frogs": 3,
    });
    let document = import::read(&text.to_string()).expect("a bare project reads");
    let warnings = document.warnings();
    assert!(
        warnings
            .iter()
            .any(|warning| warning.starts_with("researchLevel: ") && warning.contains("kept at 1")),
        "{warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.starts_with("gridLevels: ") && warning.contains("stays empty")),
        "{warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.starts_with("project: ") && warning.contains("frogs")),
        "{warnings:?}"
    );
}

#[test]
fn a_caller_can_add_the_warnings_the_reader_cannot_know() {
    let mut document = import::read(&save().to_string()).expect("the save reads");
    let before = document.warnings().len();
    document
        .warnings_mut()
        .push("section: nope is not a section this build declares".into());
    assert_eq!(document.warnings().len(), before + 1);
    assert!(
        document
            .warnings()
            .last()
            .is_some_and(|warning| warning.contains("nope")),
        "{:?}",
        document.warnings()
    );
}

#[test]
fn an_exported_state_string_reads_back_through_the_pipeline() {
    use idleon_core::model::{SimContext, Simulation};

    let simulation = Simulation::default();
    let context = SimContext {
        server_var_res_xp: 1.3,
        extra_shapes: 2,
        extra_magnifiers: 1,
        bonus_grid_points: 30,
        ..SimContext::default()
    };
    let text = project::write_state(&simulation, &context);
    let document = import::read(&text).expect("the export reads back");
    assert_eq!(document.format(), "upstream state");
    assert_eq!(document.research_level(), Some(simulation.research_level));
    // An upstream state carries no save, so nothing else decodes one.
    assert_eq!(document.characters(), None);
    let Document::Project(project) = document else {
        panic!("a state string is a project document");
    };
    assert_eq!(project.document.simulation, simulation);
    assert_eq!(project.document.context, context);
}
