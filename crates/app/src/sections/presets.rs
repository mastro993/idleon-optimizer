//! The rules every section's named presets share: a trimmed, bounded name,
//! replace-by-name, and the project's limit.

/// How many presets one project keeps per list.
pub(crate) const MAX: usize = 40;
/// How long a preset name may be.
const NAME_MAX: usize = 60;

/// `name` trimmed and cut to the length a preset name may have.
pub(crate) fn clean_name(name: &str) -> String {
    name.trim().chars().take(NAME_MAX).collect()
}

/// Replace the preset `same` picks out, or add `preset` while the list has
/// room. A full list refuses with a line naming the `kind` of preset.
pub(crate) fn upsert<T>(
    presets: &mut Vec<T>,
    preset: T,
    same: impl Fn(&T) -> bool,
    kind: &str,
) -> Result<(), String> {
    if let Some(existing) = presets.iter_mut().find(|entry| same(entry)) {
        *existing = preset;
    } else if presets.len() >= MAX {
        return Err(format!(
            "{MAX} {kind} is the limit; delete one before saving another."
        ));
    } else {
        presets.push(preset);
    }
    Ok(())
}

/// Forget the preset at `index`.
pub(crate) fn remove<T>(presets: &mut Vec<T>, index: usize) -> Option<T> {
    (index < presets.len()).then(|| presets.remove(index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_trimmed_and_bounded() {
        assert_eq!(clean_name("  Minehead run "), "Minehead run");
        assert_eq!(clean_name(&"x".repeat(100)).len(), NAME_MAX);
    }

    #[test]
    fn a_matching_preset_is_replaced_and_a_full_list_refuses_a_new_one() {
        let mut presets: Vec<(usize, u32)> = Vec::new();
        for index in 0..MAX {
            upsert(
                &mut presets,
                (index, 0),
                |entry| entry.0 == index,
                "presets",
            )
            .expect("under the limit");
        }
        upsert(&mut presets, (3, 1), |entry| entry.0 == 3, "presets").expect("replaces");
        assert_eq!(presets.len(), MAX);
        assert_eq!(presets[3], (3, 1));
        let full = upsert(&mut presets, (MAX, 0), |entry| entry.0 == MAX, "presets")
            .expect_err("the list is full");
        assert!(full.starts_with("40 presets is the limit"), "{full}");
    }

    #[test]
    fn a_preset_is_removed_by_index() {
        let mut presets = vec!["Run"];
        assert_eq!(remove(&mut presets, 1), None);
        assert_eq!(remove(&mut presets, 0), Some("Run"));
        assert!(presets.is_empty());
    }
}
