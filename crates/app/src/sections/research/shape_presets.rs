//! Custom shape priority presets: the named tier lists the project keeps.
//!
//! Upstream stores user presets in browser storage (`idleon_shapePresets`).
//! This build keeps them in the project's research config, so an export
//! carries them and an import restores them. A preset holds the two tier lists
//! only; editing a tier detaches the active preset, it never rewrites the
//! preset or a built-in.

use crate::sections::presets;
use idleon_research::Tiers;

/// One named tier list.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct CustomPreset {
    pub(crate) name: String,
    pub(crate) tiers: Tiers,
}

impl CustomPreset {
    /// Save `tiers` under `name`, replacing a preset of the same name. Returns
    /// the status line the window shows.
    pub(crate) fn store(
        presets: &mut Vec<Self>,
        name: &str,
        tiers: &Tiers,
    ) -> Result<String, String> {
        let name = presets::clean_name(name);
        if name.is_empty() {
            return Err("Type a preset name before saving.".into());
        }
        if name.starts_with('_') {
            return Err("Preset names cannot start with “_”; that prefix is for built-ins.".into());
        }
        let preset = Self {
            name: name.clone(),
            tiers: tiers.clone(),
        };
        presets::upsert(presets, preset, |entry| entry.name == name, "presets")?;
        Ok(format!("Saved the shape preset “{name}”."))
    }

    /// The tiers the preset called `name` holds.
    pub(crate) fn find<'a>(presets: &'a [Self], name: &str) -> Option<&'a Self> {
        presets.iter().find(|preset| preset.name == name)
    }

    /// Forget the preset at `index`.
    pub(crate) fn delete(presets: &mut Vec<Self>, index: usize) -> Option<Self> {
        presets::remove(presets, index)
    }

    /// Drop nameless and duplicate presets read from a document, keeping the
    /// first of each name and the project's limit.
    pub(crate) fn sanitise(presets: Vec<Self>) -> Vec<Self> {
        let mut kept: Vec<Self> = Vec::new();
        for mut preset in presets {
            preset.name = presets::clean_name(&preset.name);
            if preset.name.is_empty()
                || preset.name.starts_with('_')
                || kept.iter().any(|other| other.name == preset.name)
            {
                continue;
            }
            if kept.len() >= presets::MAX {
                break;
            }
            preset.tiers = preset.tiers.reconciled();
            kept.push(preset);
        }
        kept
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_preset_replaces_the_one_with_its_name() {
        let mut presets = Vec::new();
        let first = Tiers::parse("J6|").expect("the tiers");
        CustomPreset::store(&mut presets, " Minehead run ", &first).expect("saved");
        let second = Tiers::parse("I5|").expect("the tiers");
        CustomPreset::store(&mut presets, "Minehead run", &second).expect("saved");
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].name, "Minehead run");
        assert_eq!(presets[0].tiers, second);
    }

    #[test]
    fn an_empty_or_builtin_name_is_refused() {
        let tiers = Tiers::default();
        let mut presets = Vec::new();
        assert!(CustomPreset::store(&mut presets, "  ", &tiers).is_err());
        assert!(CustomPreset::store(&mut presets, "_none", &tiers).is_err());
        assert!(presets.is_empty());
    }

    #[test]
    fn sanitise_drops_nameless_and_duplicate_presets() {
        let tiers = Tiers::default();
        let presets = CustomPreset::sanitise(vec![
            CustomPreset {
                name: "  ".into(),
                tiers: tiers.clone(),
            },
            CustomPreset {
                name: "Minehead".into(),
                tiers: tiers.clone(),
            },
            CustomPreset {
                name: "Minehead".into(),
                tiers: Tiers::parse("J6|").expect("the tiers"),
            },
        ]);
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].tiers, tiers);
    }
}
