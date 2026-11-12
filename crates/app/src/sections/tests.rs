//! Registry invariants: ids, grouping, cards, spec links and roadmap agreement.

use super::*;
use std::path::Path;

#[test]
fn keeps_every_upstream_page_once_and_grouped() {
    assert_eq!(SECTIONS.len(), 30);
    let mut ids: Vec<&str> = SECTIONS.iter().map(|section| section.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        ids.len(),
        SECTIONS.len(),
        "section ids must be stable and unique"
    );
    assert!(
        SECTIONS
            .windows(2)
            .all(|pair| group_index(pair[0].group) <= group_index(pair[1].group)),
        "sections must stay grouped"
    );
    let counts: Vec<usize> = Group::ALL
        .iter()
        .map(|group| {
            SECTIONS
                .iter()
                .filter(|section| section.group == *group)
                .count()
        })
        .collect();
    assert_eq!(counts, [5, 3, 2, 2, 7, 2, 5, 4]);
    for section in &SECTIONS {
        assert!(!section.title.is_empty());
        assert!(!section.icon.is_empty());
        assert!(!section.description.is_empty());
        assert_eq!(section.spec, format!("docs/sections/{}.md", section.id));
    }
}

#[test]
fn the_dashboard_shows_the_upstream_cards_and_hides_the_tools() {
    assert_eq!(card_count(), 25);
    let counts: Vec<usize> = Group::ALL
        .iter()
        .map(|group| cards().filter(|section| section.group == *group).count())
        .collect();
    assert_eq!(counts, [4, 3, 2, 2, 7, 2, 5, 0]);
    let hidden: Vec<&str> = SECTIONS
        .iter()
        .filter(|section| !section.dashboard)
        .map(|section| section.id)
        .collect();
    assert_eq!(
        hidden,
        ["stats-all", "damage", "drop-rate", "coin-multi", "pickle"]
    );
}

#[test]
fn every_section_links_a_spec_that_exists() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for section in &SECTIONS {
        assert!(
            root.join(section.spec).is_file(),
            "{} links a missing {}",
            section.id,
            section.spec
        );
    }
}

#[test]
fn the_registry_matches_the_roadmaps_section_keys() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/roadmap.json");
    let roadmap: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("docs/roadmap.json"))
            .expect("roadmap JSON");
    let mut keys: Vec<String> = roadmap["milestones"]
        .as_array()
        .expect("milestones")
        .iter()
        .filter_map(|milestone| milestone["key"].as_str())
        // The foundation milestone groups features; it is not a section page.
        .filter(|key| *key != "foundation")
        .map(str::to_string)
        .collect();
    keys.sort();
    let mut ids: Vec<String> = SECTIONS
        .iter()
        .map(|section| section.id.to_string())
        .collect();
    ids.sort();
    assert_eq!(
        ids, keys,
        "the registry and the roadmap must declare the same sections"
    );
}

#[test]
fn the_manifest_lists_every_section_for_the_harness() {
    let manifest: serde_json::Value = serde_json::from_str(&manifest()).expect("manifest JSON");
    let entries = manifest.as_array().expect("manifest array");
    assert_eq!(entries.len(), SECTIONS.len());
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry["dashboard"].as_bool() == Some(true))
            .count(),
        card_count()
    );
    for (entry, section) in entries.iter().zip(&SECTIONS) {
        assert_eq!(entry["id"], section.id);
        assert_eq!(entry["title"], section.title);
        assert_eq!(entry["group"], section.group.id());
        assert_eq!(entry["status"], section.status.label());
    }
}

#[test]
fn search_reads_title_description_and_world() {
    let research = find("research").expect("research card");
    assert!(research.matches("research"));
    assert!(research.matches("EXP allocation"));
    assert!(research.matches("w7"));
    assert!(research.matches("world 7"));
    assert!(!research.matches("fishing"));

    let combat = find("combat-sim").expect("combat card");
    assert!(combat.matches("general"));
    assert!(combat.matches("replays"));

    let damage = find("damage").expect("the hidden damage page");
    assert!(damage.matches("tools"));
}

fn group_index(group: Group) -> usize {
    Group::ALL
        .into_iter()
        .position(|candidate| candidate == group)
        .expect("a listed group")
}
