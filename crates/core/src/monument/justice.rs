//! The Monument (W5) Justice response advisor.
//!
//! Ported from the upstream `js/stats/systems/w5/monument-justice.js` and the
//! `justiceCases` helper its worksheet reads. The advisor is a local worksheet,
//! not a live-game connection: the save only fixes the fresh-story start, and
//! the player records the cases and responses they actually saw. Every recorded
//! step (a stat edit, an effect toggle, a response, an opened case) is undoable,
//! so [`Worksheet`] owns the history and the app only renders it.
//!
//! The heuristic is upstream's: survival first, then mental health, chests,
//! dismissals and balanced coin/popularity reserves, including up to ten cases
//! of scheduled investment income. It is not a whole-story optimum.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::Save;
use super::data::data;

/// One Justice case: `HolesInfo[62]` split as `justiceCases()` does.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Case {
    pub id: usize,
    pub visitor: usize,
    pub name: String,
    pub text: String,
    /// The approve deltas, such as `c-1.h2`; `.` separates terms.
    pub approve: String,
    /// The reject deltas, in the same form.
    pub reject: String,
    /// The resource codes whose delta scales with the case number.
    pub scaling: String,
    /// The approve gate: `coincheck`, `popcheck`, `chestcheck`,
    /// `dismisscheck`, or `no`.
    pub gate: String,
}

/// Every case row, parsed once from the extracted table.
pub fn cases() -> &'static [Case] {
    static CASES: OnceLock<Vec<Case>> = OnceLock::new();
    CASES.get_or_init(|| {
        data()
            .justice_cases()
            .iter()
            .enumerate()
            .filter_map(|(id, row)| parse_case(id, row))
            .collect()
    })
}

fn parse_case(id: usize, row: &str) -> Option<Case> {
    let mut fields = row.split('|');
    let visitor = fields.next()?.parse().ok()?;
    let text = fields.next()?.replace('_', " ");
    let approve = fields.next()?.to_string();
    let reject = fields.next()?.to_string();
    let scaling = fields.next().unwrap_or_default().to_string();
    let gate = fields.next().unwrap_or_default().to_string();
    Some(Case {
        id,
        visitor,
        name: data().justice_visitor(visitor).replace('_', " "),
        text,
        approve,
        reject,
        scaling,
        gate,
    })
}

/// A case row, or the upstream rejection for an id no table row has.
pub fn case(id: usize) -> Result<&'static Case, Error> {
    cases().get(id).ok_or(Error::UnknownCase)
}

/// The deals, promises and investments a worksheet can carry.
///
/// The serialised name is the upstream save key, so an exported worksheet reads
/// like the calculator's own state.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Effect {
    StudentLoanInvest,
    ScripticusLoan,
    PoppyLoan,
    TimeBizInvestQ,
    TimeBizInvestT,
    CryptoInvest,
    RetireInvest,
    BeanLoan,
    BeanDeal,
    NoApprove,
    ChestMag,
    DrinksHad,
    TimmyRejections,
    TimmyIceCream,
    RupieMagicz,
    #[serde(rename = "HaveMoreCoinsOrLoseHP")]
    HaveMoreCoinsOrLoseHp,
    #[serde(rename = "popularityExample")]
    PopularityExample,
    RejectTimmyDeal,
    ScripticusApprovePromise,
    ScareLegal,
    HandLaw,
    PermaPop,
    #[serde(rename = "lifeInsurance")]
    LifeInsurance,
    #[serde(rename = "noDismissDeath")]
    NoDismissDeath,
    BrokeDeath,
}

/// One effect's upstream key, label and explanation.
struct EffectRow {
    effect: Effect,
    key: &'static str,
    name: &'static str,
    help: &'static str,
}

/// `JUSTICE_EFFECTS`, in the upstream order the delayed-effect table renders.
const EFFECT_ROWS: [EffectRow; 25] = [
    EffectRow {
        effect: Effect::StudentLoanInvest,
        key: "StudentLoanInvest",
        name: "Student loan principal",
        help: "Next Big Biz visit repays twice this amount.",
    },
    EffectRow {
        effect: Effect::ScripticusLoan,
        key: "ScripticusLoan",
        name: "Scripticus loan principal",
        help: "Next Scripticus visit repays 1.25x, rounded up.",
    },
    EffectRow {
        effect: Effect::PoppyLoan,
        key: "PoppyLoan",
        name: "Poppy loan principal",
        help: "Next Scripticus visit repays 1.35x, rounded up.",
    },
    EffectRow {
        effect: Effect::TimeBizInvestQ,
        key: "TimeBizInvestQ",
        name: "Time business principal",
        help: "Returns 2.5x when its case countdown expires.",
    },
    EffectRow {
        effect: Effect::TimeBizInvestT,
        key: "TimeBizInvestT",
        name: "Time business cases left",
        help: "Decreases at each new case, including the case immediately after purchase.",
    },
    EffectRow {
        effect: Effect::CryptoInvest,
        key: "CryptoInvest",
        name: "Crypto cases left",
        help: "One coin per new case.",
    },
    EffectRow {
        effect: Effect::RetireInvest,
        key: "RetireInvest",
        name: "Retirement fund cases left",
        help: "Two coins per new case.",
    },
    EffectRow {
        effect: Effect::BeanLoan,
        key: "BeanLoan",
        name: "Bean loan principal",
        help: "Returned at the next Bean visit, before your response.",
    },
    EffectRow {
        effect: Effect::BeanDeal,
        key: "BeanDeal",
        name: "Bean deal rejections left",
        help: "Approvals give four coins; the last rejection wipes popularity; dismissal ends the deal.",
    },
    EffectRow {
        effect: Effect::NoApprove,
        key: "NoApprove",
        name: "Approval ban cases left",
        help: "Cannot approve; countdown decreases at each new case.",
    },
    EffectRow {
        effect: Effect::ChestMag,
        key: "ChestMag",
        name: "Chest magnet cases left",
        help: "Forces chest-seller cases on non-Harbinger turns.",
    },
    EffectRow {
        effect: Effect::DrinksHad,
        key: "DrinksHad",
        name: "Drinks since last Bean secret",
        help: "Three drinks force the Bean secret case on an eligible turn.",
    },
    EffectRow {
        effect: Effect::TimmyRejections,
        key: "TimmyRejections",
        name: "Timmy rejections",
        help: "Three rejections can force his Harbinger guilt case.",
    },
    EffectRow {
        effect: Effect::TimmyIceCream,
        key: "TimmyIceCream",
        name: "Timmy ice creams given",
        help: "Controls the next ice-cream request; the final hug grants mental health.",
    },
    EffectRow {
        effect: Effect::RupieMagicz,
        key: "RupieMagicz",
        name: "Slug favors",
        help: "Three unlock the Slug swap offer.",
    },
    EffectRow {
        effect: Effect::HaveMoreCoinsOrLoseHp,
        key: "HaveMoreCoinsOrLoseHP",
        name: "Promised Chester coin balance",
        help: "Next Chester visit loses one mental health if coins are below this.",
    },
    EffectRow {
        effect: Effect::PopularityExample,
        key: "popularityExample",
        name: "Poppy example popularity",
        help: "Next Poppy visit adds round(1.6x this amount) popularity.",
    },
    EffectRow {
        effect: Effect::RejectTimmyDeal,
        key: "RejectTimmyDeal",
        name: "Must reject next Timmy",
        help: "Blocks approval for Timmy until rejection or dismissal.",
    },
    EffectRow {
        effect: Effect::ScripticusApprovePromise,
        key: "ScripticusApprovePromise",
        name: "Promised next approval",
        help: "Rejecting or dismissing the next case loses one mental health.",
    },
    EffectRow {
        effect: Effect::ScareLegal,
        key: "ScareLegal",
        name: "Scaring is legal",
        help: "Loses one popularity on each non-Harbinger case until repealed.",
    },
    EffectRow {
        effect: Effect::HandLaw,
        key: "HandLaw",
        name: "Hand above the law",
        help: "Each Hand visit gives four coins and costs two popularity.",
    },
    EffectRow {
        effect: Effect::PermaPop,
        key: "PermaPop",
        name: "Permanent popularity floor",
        help: "After each response, popularity is at least five.",
    },
    EffectRow {
        effect: Effect::LifeInsurance,
        key: "lifeInsurance",
        name: "Life insurance",
        help: "Next Big Biz visit sets mental health, coins and popularity to one.",
    },
    EffectRow {
        effect: Effect::NoDismissDeath,
        key: "noDismissDeath",
        name: "Never dismiss Harbinger",
        help: "Blocks Harbinger dismissal.",
    },
    EffectRow {
        effect: Effect::BrokeDeath,
        key: "BrokeDeath",
        name: "Already rescued from bankruptcy",
        help: "The next broke Harbinger visit forces mental-health loss.",
    },
];

impl Effect {
    /// Every effect, in the upstream table order.
    pub const ALL: [Effect; 25] = [
        Effect::StudentLoanInvest,
        Effect::ScripticusLoan,
        Effect::PoppyLoan,
        Effect::TimeBizInvestQ,
        Effect::TimeBizInvestT,
        Effect::CryptoInvest,
        Effect::RetireInvest,
        Effect::BeanLoan,
        Effect::BeanDeal,
        Effect::NoApprove,
        Effect::ChestMag,
        Effect::DrinksHad,
        Effect::TimmyRejections,
        Effect::TimmyIceCream,
        Effect::RupieMagicz,
        Effect::HaveMoreCoinsOrLoseHp,
        Effect::PopularityExample,
        Effect::RejectTimmyDeal,
        Effect::ScripticusApprovePromise,
        Effect::ScareLegal,
        Effect::HandLaw,
        Effect::PermaPop,
        Effect::LifeInsurance,
        Effect::NoDismissDeath,
        Effect::BrokeDeath,
    ];

    /// The effect's upstream key, which is also its serialised name.
    pub fn key(self) -> &'static str {
        self.row().key
    }

    /// The editor's label.
    pub fn name(self) -> &'static str {
        self.row().name
    }

    /// What the effect does, drawn under its editor row.
    pub fn help(self) -> &'static str {
        self.row().help
    }

    /// Whether the effect is on or off, instead of carrying a number.
    pub fn flag(self) -> bool {
        matches!(
            self,
            Effect::RejectTimmyDeal
                | Effect::ScripticusApprovePromise
                | Effect::ScareLegal
                | Effect::HandLaw
                | Effect::PermaPop
                | Effect::LifeInsurance
                | Effect::NoDismissDeath
                | Effect::BrokeDeath
        )
    }

    fn row(self) -> &'static EffectRow {
        EFFECT_ROWS
            .iter()
            .find(|row| row.effect == self)
            .expect("every effect has a table row")
    }
}

impl std::fmt::Display for Effect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.key())
    }
}

/// The worksheet's stats and active effects.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    /// Cases seen so far; the heal and the scaling read it.
    pub round: i64,
    pub health: i64,
    pub coins: i64,
    /// The one stat the game lets fall below zero.
    pub popularity: i64,
    pub dismissals: i64,
    pub chests: i64,
    pub effects: BTreeMap<Effect, i64>,
}

impl State {
    fn has(&self, effect: Effect) -> bool {
        self.effects.contains_key(&effect)
    }

    fn get(&self, effect: Effect) -> i64 {
        self.effects.get(&effect).copied().unwrap_or(0)
    }

    fn set(&mut self, effect: Effect, value: i64) {
        self.effects.insert(effect, value);
    }

    fn clear(&mut self, effect: Effect) {
        self.effects.remove(&effect);
    }

    /// Rejects the inputs upstream refuses.
    pub fn validate(&self) -> Result<(), Error> {
        for (field, value, nonnegative) in [
            ("round", self.round, true),
            ("health", self.health, true),
            ("coins", self.coins, true),
            ("popularity", self.popularity, false),
            ("dismissals", self.dismissals, true),
            ("chests", self.chests, true),
        ] {
            if nonnegative && value < 0 {
                return Err(Error::Negative(field));
            }
        }
        Ok(())
    }
}

/// Why the worksheet rejected an input or a step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// One of the five nonnegative stats was negative.
    Negative(&'static str),
    /// No case row has that id.
    UnknownCase,
    /// The story is over; only a fresh worksheet can continue.
    Ended,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Negative("popularity") => {
                write!(formatter, "Justice popularity must be an integer.")
            }
            Error::Negative(field) => {
                write!(formatter, "Justice {field} must be a nonnegative integer.")
            }
            Error::UnknownCase => write!(formatter, "Unknown Justice case."),
            Error::Ended => write!(formatter, "The Justice story has ended."),
        }
    }
}

impl std::error::Error for Error {}

/// The response a case offers: approve, reject or dismiss.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Approve,
    Reject,
    Dismiss,
}

impl Action {
    /// The three responses, in the upstream order they are evaluated.
    pub const ALL: [Action; 3] = [Action::Approve, Action::Reject, Action::Dismiss];

    /// The lower-case id the UI and the export use.
    pub fn id(self) -> &'static str {
        match self {
            Action::Approve => "approve",
            Action::Reject => "reject",
            Action::Dismiss => "dismiss",
        }
    }

    /// The button and card title.
    pub fn label(self) -> &'static str {
        match self {
            Action::Approve => "Approve",
            Action::Reject => "Reject",
            Action::Dismiss => "Dismiss",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }
}

/// One possible result of a response, with the chance it happens.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub label: String,
    pub probability: f64,
    pub state: State,
}

/// One response's chances: the outcomes, how often the story survives and the
/// heuristic's value for the resulting state.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub action: Action,
    /// Why the response is unavailable, or `None` when it is legal.
    pub blocked: Option<String>,
    pub outcomes: Vec<Outcome>,
    pub survival: f64,
    pub value: f64,
}

/// Everything the Justice advisor says about the observed case.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// The case row with its `{`/`}` placeholders filled for this state.
    pub row: Case,
    /// Whether several native rows share the visitor's wording, so their
    /// outcomes are the equally likely variants.
    pub ambiguous: bool,
    pub responses: Vec<Response>,
    /// The action the heuristic suggests, or `None` when the story has ended.
    pub recommended: Option<Action>,
    pub heuristic: String,
}

/// One active delayed effect and its value.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Delayed {
    pub key: &'static str,
    pub name: &'static str,
    pub help: &'static str,
    pub value: i64,
}

/// A case opened to settle the effects it triggers.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    pub state: State,
    pub notes: Vec<String>,
}

const RESOURCES: [(char, &str); 5] = [
    ('m', "health"),
    ('c', "coins"),
    ('h', "popularity"),
    ('d', "dismissals"),
    ('r', "chests"),
];

/// `Math.round`: halves join the larger integer, so negative halves round up.
fn round(value: f64) -> i64 {
    (value + 0.5).floor() as i64
}

fn resource(state: &State, code: char) -> i64 {
    match code {
        'm' => state.health,
        'c' => state.coins,
        'h' => state.popularity,
        'd' => state.dismissals,
        _ => state.chests,
    }
}

fn set_resource(state: &mut State, code: char, value: i64) {
    match code {
        'm' => state.health = value,
        'c' => state.coins = value,
        'h' => state.popularity = value,
        'd' => state.dismissals = value,
        _ => state.chests = value,
    }
}

/// `_delta`: one response's change to one resource, scaled by the case number
/// and by the Harbinger's coin growth.
fn delta(state: &State, row: &Case, action: Action, code: char) -> i64 {
    let terms = match action {
        Action::Approve => &row.approve,
        Action::Reject => &row.reject,
        Action::Dismiss => return 0,
    };
    let mut value = 0;
    for term in terms.split('.') {
        if let Some(rest) = term.strip_prefix(code) {
            value = rest.parse().unwrap_or(0);
        }
    }
    if row.scaling.contains(code) {
        value = round(value as f64 * ((state.round + 1) as f64 / 5.0).ceil());
    }
    if action == Action::Approve && row.visitor == 4 && code == 'c' {
        value = round(value as f64 * (1.0 + 0.25 * (state.round / 13) as f64));
    }
    value
}

/// The case row's wording with its coin and popularity ranges filled in.
pub fn case_text(state: &State, id: usize) -> Result<Case, Error> {
    state.validate()?;
    let row = case(id)?;
    let coin = delta(state, row, Action::Approve, 'c')
        .abs()
        .max(delta(state, row, Action::Reject, 'c').abs());
    let popularity = delta(state, row, Action::Approve, 'h')
        .abs()
        .max(delta(state, row, Action::Reject, 'h').abs());
    Ok(Case {
        text: row
            .text
            .replace('{', &coin.to_string())
            .replace('}', &popularity.to_string()),
        ..row.clone()
    })
}

/// `_blocked`: why a response is unavailable, if it is.
fn blocked(state: &State, row: &Case, action: Action) -> Option<String> {
    if state.health <= 0 {
        return Some("The story has ended.".into());
    }
    match action {
        Action::Dismiss => {
            if state.dismissals <= 0 {
                return Some("No dismissals remain.".into());
            }
            if row.visitor == 4 && state.has(Effect::NoDismissDeath) {
                return Some("Your Harbinger contract blocks dismissal.".into());
            }
        }
        Action::Approve => {
            if state.has(Effect::NoApprove) {
                return Some("Approval ban is active.".into());
            }
            if row.visitor == 0 && state.has(Effect::RejectTimmyDeal) {
                return Some("You promised not to approve Timmy.".into());
            }
            let gate = match row.gate.as_str() {
                "coincheck" => Some(('c', "coins")),
                "popcheck" => Some(('h', "popularity")),
                "chestcheck" => Some(('r', "chests")),
                _ => None,
            };
            if let Some((code, name)) = gate
                && resource(state, code) < delta(state, row, action, code).abs()
            {
                return Some(format!("Not enough {name}."));
            }
            if row.gate == "dismisscheck" && state.dismissals <= 0 {
                return Some("This offer requires a dismissal.".into());
            }
        }
        Action::Reject => {}
    }
    None
}

/// `_apply`: the response's effect on a copy of the worksheet state.
fn apply(state: &State, row: &Case, action: Action, variant: usize) -> State {
    let mut state = state.clone();
    let id = row.id;
    if action == Action::Approve {
        if id == 2 {
            state.set(
                Effect::StudentLoanInvest,
                delta(&state, row, action, 'c').abs(),
            );
        }
        if id == 34 {
            state.set(
                Effect::ScripticusLoan,
                delta(&state, row, action, 'c').abs(),
            );
        }
        if id == 36 {
            state.set(
                Effect::TimeBizInvestQ,
                delta(&state, row, action, 'c').abs(),
            );
            state.set(Effect::TimeBizInvestT, 15);
        }
        if id == 63 {
            state.set(Effect::NoApprove, 3);
        }
        if id == 62 {
            state.set(Effect::HandLaw, 1);
        }
        if id == 26 {
            state.set(Effect::BeanLoan, state.coins);
            state.coins = 0;
        }
        if id == 27 {
            state.coins = (state.coins as f64 / 2.0).floor() as i64;
        }
        if id == 60 && variant == 1 {
            state.coins =
                round(state.coins as f64 + (6 * delta(&state, row, action, 'c')).abs() as f64);
        }
        if id == 58 {
            state.set(Effect::PermaPop, 1);
        }
        if id == 29 {
            state.set(Effect::BeanDeal, 2);
        }
        if row.visitor == 13 {
            state.set(Effect::DrinksHad, state.get(Effect::DrinksHad) + 1);
        }
        if id == 43 {
            state.set(Effect::CryptoInvest, 15);
        }
        if id == 53 {
            state.set(Effect::RetireInvest, 10);
        }
        if id == 55 {
            state.set(Effect::ChestMag, 3);
        }
        if variant == 1 {
            if id == 40 {
                state.coins =
                    round(state.coins as f64 - 1.65 * delta(&state, row, action, 'c') as f64);
            }
            if id == 41 {
                state.popularity =
                    round(state.popularity as f64 - 1.65 * delta(&state, row, action, 'h') as f64);
            }
            if id == 42 {
                state.chests = round(state.chests as f64 - 3.0).max(-2);
            }
            if id == 54 {
                state.health = round(state.health as f64 - 3.0).max(-1);
            }
        }
        if id == 31 {
            state.set(Effect::RejectTimmyDeal, 1);
        }
        if id == 33 && state.health >= 2 {
            state.health = round(state.health as f64 - 1.0);
        }
        if id == 50 {
            state.set(Effect::ScareLegal, 1);
        }
        if id == 59 {
            state.clear(Effect::ScareLegal);
        }
        state.clear(Effect::ScripticusApprovePromise);
        if id == 24 {
            state.set(Effect::ScripticusApprovePromise, 1);
        }
        if id == 3 {
            state.coins = round(state.coins as f64 + 2.0 * state.popularity as f64);
            state.popularity = 0;
        }
        if state.has(Effect::BeanDeal) {
            state.coins = round(state.coins as f64 + 4.0);
        }
        if id == 64 {
            state.set(Effect::HaveMoreCoinsOrLoseHp, state.coins);
        }
        if id == 65 {
            let popularity = state.popularity as f64;
            state.coins = (state.coins as f64 + 0.3 * popularity).ceil() as i64;
            state.popularity = (popularity - 0.2 * popularity).ceil() as i64;
        }
        if id == 66 {
            let popularity = state.popularity as f64;
            let coins = state.coins as f64;
            state.popularity = (popularity + (popularity / 3.0).min(0.3 * coins)).ceil() as i64;
            state.coins = (coins - 0.2 * coins).ceil() as i64;
        }
        if id == 72 {
            state.set(Effect::LifeInsurance, 1);
        }
        if id == 74 {
            state.set(Effect::PopularityExample, state.popularity);
            state.popularity = 1;
        }
        if id == 75 {
            state.set(Effect::PoppyLoan, delta(&state, row, action, 'c').abs());
        }
        if id == 80 {
            match variant {
                0 => state.chests = round(state.chests as f64 + 2.0),
                1 => state.chests = round(state.chests as f64 + 3.0),
                2 => state.dismissals = round(state.dismissals as f64 + 2.0),
                _ => state.coins = round(state.coins as f64 + 2.0),
            }
        }
        if id == 85 {
            state.set(Effect::NoDismissDeath, 1);
        }
        if id == 86 {
            std::mem::swap(&mut state.coins, &mut state.popularity);
            state.clear(Effect::RupieMagicz);
        }
        if (65..=69).contains(&id) {
            state.set(Effect::RupieMagicz, state.get(Effect::RupieMagicz) + 1);
        }
        if (81..=84).contains(&id) {
            state.set(
                Effect::TimmyIceCream,
                round(state.get(Effect::TimmyIceCream) as f64 + 1.0),
            );
        }
        // The game applies the table's own deltas after every special case.
        for (code, _) in RESOURCES {
            let value = resource(&state, code) + delta(&state, row, action, code);
            set_resource(&mut state, code, round(value as f64));
        }
    } else if action == Action::Reject {
        for (code, _) in RESOURCES {
            let value = resource(&state, code) + delta(&state, row, action, code);
            set_resource(&mut state, code, round(value as f64));
        }
        if state.has(Effect::ScripticusApprovePromise) {
            state.health -= 1;
            state.clear(Effect::ScripticusApprovePromise);
        }
        if row.visitor == 0 {
            state.set(
                Effect::TimmyRejections,
                state.get(Effect::TimmyRejections) + 1,
            );
            state.clear(Effect::RejectTimmyDeal);
        }
        if state.has(Effect::BeanDeal) {
            let deal = round(state.get(Effect::BeanDeal) as f64 - 1.0);
            state.set(Effect::BeanDeal, deal);
            if deal == 0 {
                state.popularity = 0;
                state.clear(Effect::BeanDeal);
            }
        }
    } else {
        state.dismissals -= 1;
        if id == 15 {
            state.health -= 1;
        }
        if id == 28 {
            state.chests += 1;
        }
        if state.has(Effect::ScripticusApprovePromise) {
            state.health -= 1;
            state.clear(Effect::ScripticusApprovePromise);
        }
        state.clear(Effect::BeanDeal);
        if row.visitor == 0 {
            state.clear(Effect::RejectTimmyDeal);
        }
    }
    for code in ['m', 'c', 'h', 'd'] {
        let value = resource(&state, code).max(0);
        set_resource(&mut state, code, value);
    }
    if state.has(Effect::PermaPop) {
        state.popularity = state.popularity.max(5);
    }
    if state.health > 0 {
        state.round += 1;
    }
    state
}

/// `_variants`: the random outcomes an approve can roll, in the upstream order.
fn variants(row: &Case, action: Action) -> Vec<(f64, &'static str)> {
    if action != Action::Approve {
        return vec![(1.0, "Outcome")];
    }
    match row.id {
        40 | 41 | 42 | 54 => vec![(0.5, "Heads"), (0.5, "Tails")],
        60 => vec![(2.0 / 3.0, "Ordinary glass"), (1.0 / 3.0, "Limited glass")],
        80 => vec![
            (0.3, "Two chests"),
            (0.63, "Three chests"),
            (0.035, "Two dismissals"),
            (0.035, "Two coins"),
        ],
        _ => vec![(1.0, "Outcome")],
    }
}

/// `_value`: the heuristic's score for one resulting state.
fn value(state: &State) -> f64 {
    let scheduled = (state.get(Effect::CryptoInvest).min(10) as f64)
        + 2.0 * state.get(Effect::RetireInvest).min(10) as f64
        + if state.has(Effect::TimeBizInvestT) && state.get(Effect::TimeBizInvestT) <= 10 {
            2.5 * state.get(Effect::TimeBizInvestQ) as f64
        } else {
            0.0
        };
    30.0 * state.health as f64
        + 12.0 * state.chests as f64
        + 6.0 * state.dismissals as f64
        + 8.0 * (state.coins as f64 + scheduled).ln_1p()
        + 8.0 * (state.popularity as f64).ln_1p()
        - if state.has(Effect::LifeInsurance) {
            30.0 * (state.health - 1).max(0) as f64
        } else {
            0.0
        }
        - if state.has(Effect::ScareLegal) {
            8.0
        } else {
            0.0
        }
        - 3.0 * state.get(Effect::NoApprove) as f64
        + if state.has(Effect::PermaPop) {
            8.0
        } else {
            0.0
        }
}

/// The three responses' outcomes, which one the heuristic suggests, and why.
pub fn responses(state: &State, id: usize) -> Result<Report, Error> {
    state.validate()?;
    let row = case(id)?;
    let group: Vec<&Case> = cases()
        .iter()
        .filter(|candidate| candidate.visitor == row.visitor && candidate.text == row.text)
        .collect();
    let mut responses = Vec::with_capacity(3);
    for action in Action::ALL {
        if let Some(reason) = blocked(state, row, action) {
            responses.push(Response {
                action,
                blocked: Some(reason),
                outcomes: Vec::new(),
                survival: 0.0,
                value: 0.0,
            });
            continue;
        }
        let mut outcomes: Vec<Outcome> = Vec::new();
        for candidate in &group {
            for (variant, (probability, label)) in
                variants(candidate, action).into_iter().enumerate()
            {
                let next = apply(state, candidate, action, variant);
                let probability = probability / group.len() as f64;
                if let Some(same) = outcomes.iter_mut().find(|outcome| outcome.state == next) {
                    same.probability += probability;
                } else {
                    outcomes.push(Outcome {
                        label: if group.len() == 1 {
                            label.to_string()
                        } else {
                            resource_delta(state, &next)
                        },
                        probability,
                        state: next,
                    });
                }
            }
        }
        let survival = outcomes
            .iter()
            .map(|outcome| outcome.probability * f64::from(outcome.state.health > 0))
            .sum();
        let score = outcomes
            .iter()
            .map(|outcome| outcome.probability * value(&outcome.state))
            .sum();
        responses.push(Response {
            action,
            blocked: None,
            outcomes,
            survival,
            value: score,
        });
    }
    let mut legal: Vec<&Response> = responses.iter().filter(|r| r.blocked.is_none()).collect();
    legal.sort_by(|a, b| {
        b.survival
            .total_cmp(&a.survival)
            .then_with(|| b.value.total_cmp(&a.value))
    });
    Ok(Report {
        row: case_text(state, id)?,
        ambiguous: group.len() > 1,
        recommended: legal.first().map(|response| response.action),
        heuristic: HEURISTIC.to_string(),
        responses,
    })
}

/// The label an ambiguous case uses: what changed, or that nothing did.
fn resource_delta(state: &State, next: &State) -> String {
    let changes: Vec<String> = RESOURCES
        .iter()
        .filter_map(|(code, name)| {
            let change = resource(next, *code) - resource(state, *code);
            (change != 0).then(|| format!("{}{} {name}", if change > 0 { "+" } else { "" }, change))
        })
        .collect();
    if changes.is_empty() {
        "No resource change".to_string()
    } else {
        changes.join(", ")
    }
}

const HEURISTIC: &str = "Survival-first heuristic, then mental health, chests, dismissals and balanced coin/popularity reserves. Includes up to ten cases of scheduled investment income; visitor-dependent repayments are listed, not treated as guaranteed. Not a whole-story optimum.";

/// The effects active right now, in the editor's order.
pub fn delayed_effects(state: &State) -> Vec<Delayed> {
    Effect::ALL
        .into_iter()
        .filter(|effect| state.has(*effect))
        .map(|effect| Delayed {
            key: effect.key(),
            name: effect.name(),
            help: effect.help(),
            value: state.get(effect),
        })
        .collect()
}

/// `justiceOpenCase`: settle the timers and visitor payments one time, when the
/// next observed case opens.
pub fn open_case(state: &State, id: usize) -> Result<Opened, Error> {
    state.validate()?;
    if state.health <= 0 {
        return Err(Error::Ended);
    }
    let row = case(id)?;
    let mut state = state.clone();
    let mut notes = Vec::new();
    if state.has(Effect::TimeBizInvestT) {
        state.set(
            Effect::TimeBizInvestT,
            round(state.get(Effect::TimeBizInvestT) as f64 - 1.0),
        );
        if state.get(Effect::TimeBizInvestT) <= 0 {
            let before = state.coins;
            state.coins =
                round(state.coins as f64 + 2.5 * state.get(Effect::TimeBizInvestQ) as f64);
            notes.push(format!(
                "Time business paid {} coins.",
                state.coins - before
            ));
            state.clear(Effect::TimeBizInvestT);
            state.clear(Effect::TimeBizInvestQ);
        }
    }
    for (effect, coins) in [(Effect::CryptoInvest, 1), (Effect::RetireInvest, 2)] {
        if state.has(effect) {
            state.set(effect, round(state.get(effect) as f64 - 1.0));
            state.coins = round(state.coins as f64 + coins as f64);
            notes.push(format!(
                "{} paid {coins} coins.",
                if effect == Effect::CryptoInvest {
                    "Crypto"
                } else {
                    "Retirement fund"
                }
            ));
            if state.get(effect) <= 0 {
                state.clear(effect);
            }
        }
    }
    if state.has(Effect::NoApprove) {
        state.set(
            Effect::NoApprove,
            round(state.get(Effect::NoApprove) as f64 - 1.0),
        );
        if state.get(Effect::NoApprove) <= 0 {
            state.clear(Effect::NoApprove);
        }
    }
    if (state.round + 1) % 5 == 0 {
        if row.id == 14 {
            state.set(Effect::BrokeDeath, 1);
        }
        if row.id == 15 {
            state.clear(Effect::BrokeDeath);
        }
        if row.id == 13 {
            state.clear(Effect::TimmyRejections);
        }
    } else {
        if state.has(Effect::ScareLegal) {
            state.popularity = (state.popularity - 1).max(0);
            notes.push("Legal scaring cost one popularity.".to_string());
        }
        if state.has(Effect::ChestMag) {
            state.set(Effect::ChestMag, state.get(Effect::ChestMag) - 1);
            if state.get(Effect::ChestMag) <= 0 {
                state.clear(Effect::ChestMag);
            }
        } else if state.has(Effect::DrinksHad) && state.get(Effect::DrinksHad) >= 3 {
            state.clear(Effect::DrinksHad);
        }
    }
    if row.visitor == 2 {
        if state.has(Effect::StudentLoanInvest) {
            state.coins =
                round(state.coins as f64 + 2.0 * state.get(Effect::StudentLoanInvest) as f64);
            state.clear(Effect::StudentLoanInvest);
            notes.push("Student loan repaid.".to_string());
        }
        if state.has(Effect::LifeInsurance) {
            state.health = 1;
            state.coins = 1;
            state.popularity = 1;
            state.clear(Effect::LifeInsurance);
            notes.push(
                "Life insurance reset mental health, coins and popularity to one.".to_string(),
            );
        }
    }
    if row.visitor == 7 && state.has(Effect::BeanLoan) {
        state.coins = round(state.coins as f64 + state.get(Effect::BeanLoan) as f64);
        state.clear(Effect::BeanLoan);
        notes.push("Bean loan repaid.".to_string());
    }
    if row.visitor == 6 {
        for (effect, factor) in [(Effect::ScripticusLoan, 1.25), (Effect::PoppyLoan, 1.35)] {
            if state.has(effect) {
                state.coins =
                    (state.coins as f64 + factor * state.get(effect) as f64).ceil() as i64;
                state.clear(effect);
                notes.push(format!(
                    "Scripticus repaid {} loan.",
                    if effect == Effect::PoppyLoan {
                        "Poppy's"
                    } else {
                        "his"
                    }
                ));
            }
        }
    }
    if row.visitor == 1 && state.has(Effect::HandLaw) {
        state.coins = (state.coins as f64 + 4.0).ceil() as i64;
        state.popularity = (state.popularity as f64 - 2.0).ceil() as i64;
        notes.push("Hand law: +4 coins, -2 popularity.".to_string());
    }
    if row.visitor == 5 && state.has(Effect::HaveMoreCoinsOrLoseHp) {
        if state.coins < state.get(Effect::HaveMoreCoinsOrLoseHp) {
            state.health -= 1;
            notes.push("Chester coin promise cost one mental health.".to_string());
        }
        state.clear(Effect::HaveMoreCoinsOrLoseHp);
    }
    if row.visitor == 15 && state.has(Effect::PopularityExample) {
        state.popularity += round(1.6 * state.get(Effect::PopularityExample) as f64);
        state.clear(Effect::PopularityExample);
        notes.push("Poppy returned example popularity.".to_string());
    }
    Ok(Opened { state, notes })
}

/// The fresh-story worksheet the Justice tab starts from.
pub fn initial_state(save: &Save) -> State {
    let stats = super::justice_stats(save);
    State {
        round: 0,
        health: stats.health,
        coins: stats.coins,
        popularity: stats.popularity,
        dismissals: stats.dismissals,
        chests: 0,
        effects: BTreeMap::new(),
    }
}

/// One undone step: the state plus the case and whether a response was
/// recorded but its next case not yet opened.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Step {
    state: State,
    case_id: usize,
    awaiting: bool,
}

/// How many steps the worksheet remembers, as upstream caps them.
const HISTORY: usize = 30;

/// The undoable Justice worksheet: the observed case, the stats and the steps.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Worksheet {
    pub state: State,
    pub case_id: usize,
    /// A response was recorded and its next case has not been opened yet.
    pub awaiting: bool,
    /// The steps Undo walks back. A project file keeps the state, not the trail.
    #[serde(skip)]
    history: Vec<Step>,
}

impl Worksheet {
    /// A restored worksheet: the case id must still index the table, and the
    /// history is a session convenience, so it starts empty.
    pub fn restored(mut self) -> Self {
        self.history.clear();
        if self.case_id >= cases().len() {
            self.case_id = 0;
        }
        self
    }

    /// Selects the observed case; choosing a case is not an undoable step.
    pub fn select(&mut self, id: usize) {
        if id < cases().len() {
            self.case_id = id;
        }
    }

    /// Records a stat or effect edit as one undoable step.
    pub fn edit(&mut self, next: State) {
        self.save();
        self.state = next;
    }

    /// Records a response outcome and waits for the next case.
    pub fn record(&mut self, next: State) {
        self.save();
        self.state = next;
        self.awaiting = true;
    }

    /// Opens the selected case once, settling the delayed effects it triggers.
    pub fn open_next(&mut self) -> Result<Vec<String>, Error> {
        let opened = open_case(&self.state, self.case_id)?;
        self.save();
        self.state = opened.state;
        self.awaiting = false;
        Ok(opened.notes)
    }

    /// Whether a step is waiting to be undone.
    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    /// Walks one step back; `false` when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        match self.history.pop() {
            Some(step) => {
                self.state = step.state;
                self.case_id = step.case_id;
                self.awaiting = step.awaiting;
                true
            }
            None => false,
        }
    }

    /// Starts over from the save's fresh-story stats; the trail starts empty.
    pub fn reset(&mut self, initial: State) {
        self.state = initial;
        self.case_id = 0;
        self.awaiting = false;
        self.history.clear();
    }

    fn save(&mut self) {
        let step = Step {
            state: self.state.clone(),
            case_id: self.case_id,
            awaiting: self.awaiting,
        };
        if self.history.len() == HISTORY {
            self.history.remove(0);
        }
        self.history.push(step);
    }
}

#[cfg(test)]
mod tests;
