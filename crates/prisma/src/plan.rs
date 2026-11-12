//! The Prisma plan: which bubbles can take Prisma, what the save owns, and the
//! bonus each bubble produces today.
//!
//! Ported from `prismaPlan` in `js/stats/prisma.js`, `prismaVariant` for the
//! saved-state checks, and the bubble bonus in `js/stats/systems/w2/alchemy.js`
//! (`isBubblePrismad`, `getPrismaBonusMult`, `formulaEval`).

use super::data::{self, Bubble};
use idleon_save::{Save, at, number};
use idleon_stats::Snapshot;

/// The cauldron tab labels, in the upstream order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cauldron(pub usize);

impl Cauldron {
    pub const ALL: [Cauldron; 4] = [Cauldron(0), Cauldron(1), Cauldron(2), Cauldron(3)];

    pub fn index(self) -> usize {
        self.0
    }

    /// The upstream tab label.
    pub fn label(self) -> &'static str {
        data::cauldron_label(self.0).unwrap_or("Power")
    }

    /// The section's project-file id.
    pub fn id(self) -> &'static str {
        match self.0 {
            0 => "power",
            1 => "quicc",
            2 => "high-iq",
            _ => "kazam",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|cauldron| cauldron.id() == id)
    }
}

/// What a bubble's saved state says about it, before any comparison runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Owned and not Prisma-ed: the search can add Prisma.
    Ready,
    /// Owned and already Prisma-ed: the search can measure the retained benefit.
    Already,
    /// Large bubbles cannot receive Prisma.
    Ineligible,
    /// The saved level or the Prisma ownership list is missing or invalid.
    Unavailable,
    /// Owned at level zero: purchase it first.
    Unpurchased,
}

impl Status {
    pub fn id(self) -> &'static str {
        match self {
            Status::Ready => "ready",
            Status::Already => "already",
            Status::Ineligible => "ineligible",
            Status::Unavailable => "unavailable",
            Status::Unpurchased => "unpurchased",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Ready => "Ready",
            Status::Already => "Already Prisma-ed",
            Status::Ineligible => "Ineligible",
            Status::Unavailable => "Unavailable",
            Status::Unpurchased => "Unpurchased",
        }
    }
}

/// One character the plan can run for, as the upstream character selector lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Character {
    pub index: usize,
    pub name: String,
    pub class_name: String,
}

/// One bubble's saved state and base bonus.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The upstream row id, `"<cauldron>:<index>"`.
    pub id: String,
    pub cauldron: usize,
    pub index: usize,
    /// The display label, e.g. `Heart Diggy`.
    pub name: String,
    /// The upstream row key, e.g. `HEARTY_DIGGY`.
    pub key: String,
    /// The effect key downstream formulas read.
    pub effect: String,
    /// The saved level, or `None` when the save carries no usable cell.
    pub level: Option<f64>,
    pub status: Status,
    /// Why a row is not comparable, in the upstream wording.
    pub reason: String,
    /// Whether a Prisma comparison can run for this row.
    pub comparable: bool,
    /// The bubble bonus before Prisma at the saved level.
    pub bonus: f64,
    /// Whether the save's Prisma list already carries this bubble.
    pub prisma: bool,
}

impl Row {
    /// The effect value the bubble adds once Prisma is applied.
    pub fn with_prisma(&self, multiplier: f64) -> f64 {
        self.bonus * multiplier.max(1.0)
    }
}

/// Why a plan cannot be built from the imported save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    /// The cauldron is not one of the four tabs.
    UnknownCauldron(usize),
    /// The save does not carry that character index.
    UnknownCharacter(usize),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::UnknownCauldron(index) => write!(formatter, "Unknown cauldron {index}."),
            PlanError::UnknownCharacter(index) => {
                write!(
                    formatter,
                    "The imported save has no character {}.",
                    index + 1
                )
            }
        }
    }
}

/// Everything one Prisma search needs from the save.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub character: usize,
    pub cauldron: usize,
    pub characters: Vec<Character>,
    pub rows: Vec<Row>,
    /// `OptionsListAccount[383]`.
    pub fragments: Option<f64>,
    /// `CurrentMap_<character>`.
    pub map: Option<usize>,
    /// `getPrismaBonusMult`: what a Prisma bubble's bonus is multiplied by.
    pub multiplier: f64,
}

impl Plan {
    /// How many bubbles the search compares.
    pub fn comparable(&self) -> usize {
        self.rows.iter().filter(|row| row.comparable).count()
    }

    /// The upstream tab label of this plan's cauldron.
    pub fn cauldron_label(&self) -> &'static str {
        Cauldron(self.cauldron).label()
    }

    /// Whether the save has enough fragments for one more Prisma (10).
    pub fn affordable(&self) -> bool {
        self.fragments.is_some_and(|fragments| fragments >= 10.0)
    }
}

/// Build the plan for one character and cauldron.
pub fn plan(save: &Save, character: usize, cauldron: usize) -> Result<Plan, PlanError> {
    if cauldron >= 4 {
        return Err(PlanError::UnknownCauldron(cauldron));
    }
    let count = save.character_count();
    if character >= count {
        return Err(PlanError::UnknownCharacter(character));
    }
    let snapshot = Snapshot::from_save(save);
    let characters = (0..count)
        .map(|index| Character {
            index,
            name: save.character_name(index),
            class_name: idleon_combat::data::class_name(snapshot.class_id(index) as u32),
        })
        .collect();
    let options = save
        .get("OptionsListAccount")
        .or_else(|| save.get("OptLacc"));
    let fragments = options
        .as_ref()
        .and_then(|options| at(options, 383))
        .and_then(number);
    let prisma = options
        .as_ref()
        .and_then(|options| at(options, 384))
        .and_then(prime_list);
    let prisma_available = prisma.is_some();
    let prisma = prisma.unwrap_or_default();
    let levels = save.get("CauldronInfo");
    let multiplier = idleon_account::prisma_multiplier(save);

    let rows = data::cauldron(cauldron)
        .map(|bubble| {
            let level = levels
                .as_ref()
                .and_then(|levels| at(levels, cauldron))
                .and_then(|row| at(row, bubble.index))
                .and_then(number);
            let level = level.filter(|level| *level >= 0.0);
            let prismad = bubble_code(bubble, cauldron).is_some_and(|code| prisma.contains(&code));
            let (status, reason) = status_of(bubble, level, prisma_available, prismad);
            let comparable = matches!(status, Status::Ready | Status::Already)
                && level.is_some_and(|level| level > 0.0);
            let bonus = level.filter(|level| *level > 0.0).map_or(0.0, |level| {
                idleon_common::curve::eval(&bubble.formula, bubble.x1, bubble.x2, level)
            });
            Row {
                id: format!("{cauldron}:{}", bubble.index),
                cauldron,
                index: bubble.index,
                name: bubble.label.clone(),
                key: bubble.name.clone(),
                effect: bubble.effect.clone(),
                level,
                status,
                reason,
                comparable,
                bonus,
                prisma: prismad,
            }
        })
        .collect();

    Ok(Plan {
        character,
        cauldron,
        characters,
        rows,
        fragments,
        map: save
            .get(&format!("CurrentMap_{character}"))
            .as_ref()
            .and_then(number)
            .map(|map| map.max(0.0) as usize),
        multiplier,
    })
}

/// The upstream status ladder, in its own order.
fn status_of(
    bubble: &Bubble,
    level: Option<f64>,
    prisma_available: bool,
    prismad: bool,
) -> (Status, String) {
    if !bubble.prisma_eligible {
        return (
            Status::Ineligible,
            "Large bubbles cannot receive Prisma.".into(),
        );
    }
    if !prisma_available || level.is_none() {
        return (
            Status::Unavailable,
            "Saved bubble level or Prisma ownership is missing or invalid.".into(),
        );
    }
    if prismad {
        return (Status::Already, "Already Prisma-ed in this save.".into());
    }
    if level == Some(0.0) {
        return (Status::Unpurchased, "Purchase this bubble first.".into());
    }
    (Status::Ready, String::new())
}

/// The saved Prisma ownership text, when the save carries one.
///
/// Upstream reads `OptionsListAccount[384]` and accepts a string or `0`; any
/// other shape means the list is unavailable. It then searches the text for
/// `<letter><index>,`, so the raw text is kept rather than split: the fixture's
/// own list starts with a `0c9,` entry the substring search still matches.
fn prime_list(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) if number.as_f64() == Some(0.0) => Some(String::new()),
        _ => None,
    }
}

/// `N2L[cauldron] + index + ","`, the code `isBubblePrismad` searches for.
fn bubble_code(bubble: &Bubble, cauldron: usize) -> Option<String> {
    let letter = data::letter(cauldron)?;
    Some(format!("{letter}{},", bubble.index))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> Save {
        Save::new(
            serde_json::from_str(include_str!("../tests/prisma-save.json")).expect("tiny save"),
        )
    }

    fn row<'a>(plan: &'a Plan, id: &str) -> &'a Row {
        plan.rows
            .iter()
            .find(|row| row.id == id)
            .unwrap_or_else(|| panic!("row {id}"))
    }

    #[test]
    fn the_plan_lists_the_saved_statuses_and_bubble_bonuses() {
        let plan = plan(&tiny(), 0, 0).expect("the tiny save plans");
        assert_eq!(plan.characters.len(), 1);
        assert_eq!(plan.characters[0].name, "Tester");
        assert_eq!(plan.fragments, Some(120.0));
        assert_eq!(plan.map, Some(5));
        assert_eq!(plan.multiplier, 2.0);
        assert_eq!(plan.comparable(), 2);
        let already = row(&plan, "0:0");
        assert_eq!(already.status, Status::Already);
        assert_eq!(already.level, Some(10.0));
        assert_eq!(already.bonus, 10.0);
        assert_eq!(already.with_prisma(plan.multiplier), 20.0);
        let ready = row(&plan, "0:2");
        assert_eq!(ready.status, Status::Ready);
        assert!(ready.comparable);
        let unpurchased = row(&plan, "0:1");
        assert_eq!(unpurchased.status, Status::Unpurchased);
        assert!(!unpurchased.comparable);
        let ineligible = row(&plan, "0:3");
        assert_eq!(ineligible.status, Status::Ineligible);
        assert_eq!(ineligible.bonus, 31.0);
        assert!(plan.affordable(), "120 fragments cover one more Prisma");
    }

    #[test]
    fn a_missing_level_or_ownership_list_leaves_rows_unavailable() {
        let save = Save::new(serde_json::json!({
            "Lv0_0": "[1]",
            "CauldronInfo": [[5]],
        }));
        let plan = plan(&save, 0, 0).expect("a save without options still plans");
        let row = row(&plan, "0:0");
        assert_eq!(row.status, Status::Unavailable);
        assert!(!row.comparable);
        assert_eq!(plan.fragments, None);
        assert_eq!(plan.multiplier, 2.0, "the minimum Prisma multiplier");
    }

    #[test]
    fn the_plan_rejects_an_unknown_cauldron_or_character() {
        assert_eq!(plan(&tiny(), 0, 4), Err(PlanError::UnknownCauldron(4)));
        assert_eq!(plan(&tiny(), 7, 0), Err(PlanError::UnknownCharacter(7)));
        // A save with no characters reports character 0, which is where the
        // app's fall-back to the first character has to stop retrying.
        assert_eq!(
            plan(&Save::new(serde_json::json!({})), 0, 0),
            Err(PlanError::UnknownCharacter(0))
        );
    }

    #[test]
    fn every_cauldron_round_trips_its_tab_id() {
        for cauldron in Cauldron::ALL {
            assert_eq!(Cauldron::from_id(cauldron.id()), Some(cauldron));
            assert!(!cauldron.label().is_empty());
        }
        assert_eq!(Cauldron::ALL[3].label(), "Kazam");
        assert_eq!(Cauldron::from_id("nope"), None);
    }
}
