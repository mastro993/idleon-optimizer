//! The multiplayer restriction flag behind the upstream "Am I shadow banned?" page.
//!
//! `shadow.html` reads `OptLacc[26]`: a finite zero answers NO, any other finite
//! number answers YES, and a save without a readable flag stays UNKNOWN. The
//! importer reports what it read instead of guessing an answer.

use idleon_save::json;
use serde_json::{Map, Value};

/// The three answers the upstream page shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShadowVerdict {
    /// No save yet, or the save has no readable `OptLacc[26]`.
    #[default]
    Unknown,
    /// `OptLacc[26]` is zero.
    No,
    /// `OptLacc[26]` is any other finite number.
    Yes,
}

impl ShadowVerdict {
    /// The word the page shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::No => "NO",
            Self::Yes => "YES",
        }
    }

    /// The answer a finite flag gives; a missing or non-finite flag stays UNKNOWN.
    pub fn of(flag: Option<f64>) -> Self {
        match flag {
            Some(0.0) => Self::No,
            Some(flag) if flag.is_finite() => Self::Yes,
            _ => Self::Unknown,
        }
    }
}

/// The flag the save carries and the answer it gives.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shadow {
    /// The finite `OptLacc[26]` the save carries, if any.
    pub flag: Option<f64>,
}

impl Shadow {
    /// Read `OptLacc[26]` from a decoded save object, `data`-wrapped or flat.
    pub fn read(save: &Map<String, Value>) -> Self {
        Self { flag: flag(save) }
    }

    /// The upstream answer for the flag that was read.
    pub fn verdict(self) -> ShadowVerdict {
        ShadowVerdict::of(self.flag)
    }
}

/// `OptLacc[26]`, when the save carries it as a finite number.
///
/// `parseSaveKey` turns a stringified `OptLacc` back into its array, and the
/// page accepts a numeric string in slot 26 the way `Number(value)` does.
fn flag(save: &Map<String, Value>) -> Option<f64> {
    let options = json::parsed_field(save, "OptLacc")?;
    let value = match &options {
        Value::Array(_) => json::index(&options, &[26]),
        Value::Object(fields) => fields.get("26"),
        _ => None,
    }?;
    json::number(value).filter(|flag| flag.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_verdict_names_each_answer() {
        assert_eq!(ShadowVerdict::Unknown.label(), "UNKNOWN");
        assert_eq!(ShadowVerdict::No.label(), "NO");
        assert_eq!(ShadowVerdict::Yes.label(), "YES");
    }

    #[test]
    fn only_a_finite_flag_answers_yes_or_no() {
        assert_eq!(ShadowVerdict::of(None), ShadowVerdict::Unknown);
        assert_eq!(ShadowVerdict::of(Some(0.0)), ShadowVerdict::No);
        assert_eq!(ShadowVerdict::of(Some(-0.0)), ShadowVerdict::No);
        assert_eq!(ShadowVerdict::of(Some(1.0)), ShadowVerdict::Yes);
        assert_eq!(ShadowVerdict::of(Some(-1.0)), ShadowVerdict::Yes);
        assert_eq!(ShadowVerdict::of(Some(f64::NAN)), ShadowVerdict::Unknown);
        assert_eq!(
            ShadowVerdict::of(Some(f64::INFINITY)),
            ShadowVerdict::Unknown
        );
    }
}
