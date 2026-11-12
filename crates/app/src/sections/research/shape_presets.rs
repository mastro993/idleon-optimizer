//! Custom shape priority presets: the named tier lists the project keeps.
//!
//! Upstream stores user presets in browser storage (`idleon_shapePresets`).
//! This build keeps them in the project's research config, so an export
//! carries them and an import restores them. A preset holds the two tier lists
//! only; editing a tier detaches the active preset, it never rewrites the
//! preset or a built-in.

use idleon_core::Tiers;

/// One named tier list.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomPreset {
    pub name: String,
    pub tiers: Tiers,
}

impl CustomPreset {
    /// How many presets one project keeps.
    pub const MAX: usize = 40;
    /// How long a preset name may be.
    const NAME_MAX: usize = 60;

    /// Save `tiers` under `name`, replacing a preset of the same name. Returns
    /// the status line the window shows.
    pub fn store(presets: &mut Vec<Self>, name: &str, tiers: &Tiers) -> Result<String, String> {
        let name: String = name.trim().chars().take(Self::NAME_MAX).collect();
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
        if let Some(index) = presets.iter().position(|entry| entry.name == name) {
            presets[index] = preset;
        } else if presets.len() >= Self::MAX {
            return Err(format!(
                "{} presets is the limit; delete one before saving another.",
                Self::MAX
            ));
        } else {
            presets.push(preset);
        }
        Ok(format!("Saved the shape preset “{name}”."))
    }

    /// The tiers the preset called `name` holds.
    pub fn find<'a>(presets: &'a [Self], name: &str) -> Option<&'a Self> {
        presets.iter().find(|preset| preset.name == name)
    }

    /// Forget the preset at `index`.
    pub fn delete(presets: &mut Vec<Self>, index: usize) -> Option<Self> {
        (index < presets.len()).then(|| presets.remove(index))
    }

    /// Drop nameless and duplicate presets read from a document, keeping the
    /// first of each name and the project's limit.
    pub fn sanitise(presets: Vec<Self>) -> Vec<Self> {
        let mut kept: Vec<Self> = Vec::new();
        for mut preset in presets {
            preset.name = preset.name.trim().chars().take(Self::NAME_MAX).collect();
            if preset.name.is_empty()
                || preset.name.starts_with('_')
                || kept.iter().any(|other| other.name == preset.name)
            {
                continue;
            }
            if kept.len() >= Self::MAX {
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
    fn an_empty_name_or_a_full_list_is_refused() {
        let tiers = Tiers::default();
        let mut presets = Vec::new();
        assert!(CustomPreset::store(&mut presets, "  ", &tiers).is_err());
        for index in 0..CustomPreset::MAX {
            CustomPreset::store(&mut presets, &format!("preset {index}"), &tiers).expect("saved");
        }
        assert!(CustomPreset::store(&mut presets, "one more", &tiers).is_err());
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
