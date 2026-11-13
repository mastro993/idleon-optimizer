//! The card and obol halves of the reward engine (CS-05).
//!
//! Ported from `js/stats/systems/common/combat-sim-rewards.js`:
//!
//! - [`drop_odds`] is the game's `DropOdds` (src 66955): one stream draw and the
//!   expected-count shortcut that makes a huge chance land every kill;
//! - [`State::roll`] is `generateMonsterDrops` (src 66420) over the card, obol
//!   and drop-table rows the extractor keeps, including the shared `DN2`
//!   quantity the nested tables read and the merge of same-name rows;
//! - [`State::take`] is the card and obol halves of `ItemPickupInTheFirstPlace`:
//!   an obol fills its page's first blank slot or, with Silver Jellyfish
//!   unlocked, rolls up to three fragments; a card counts into the saved card
//!   table, and a count still below three re-reads the unowned card chance;
//! - [`State::reset`] is `lootDropOddsReset` (src 74357): the pristine rows are
//!   re-copied, an owned-zero card's row is multiplied by five, every card row
//!   by the card-chance multiplier, and the obol task's rows are spliced into
//!   the drop tables their roll reads.
//!
//! `crates/combat/data/drops.json` and `fixtures/combat-drops.json` are written
//! by `script/combat-drops-oracle.mjs` from one upstream checkout. Every row
//! that cannot produce a card or an obol is left out of both, so the fixture's
//! upstream calls roll the same stream as this port and their results compare
//! exactly. The other reward paths (coins, equipment, EXP, inventory) stay
//! unmodelled and are listed by [`crate::encounter::unsupported`].

use crate::overrides::{Overrides, Value};
use idleon_common::Rng;
use idleon_save::Save;
use idleon_save::json::{index, number};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const DROPS_JSON: &str = include_str!("../data/drops.json");

/// The upstream fragment balance cap.
const FRAGMENT_CAP: f64 = 2e9;

/// One drop row: the item, its per-kill chance and the quantity one roll pays.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Row {
    pub name: String,
    pub chance: f64,
    pub qty: f64,
}

/// One monster: its display name, its `Type` gate and its kept drop rows.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Monster {
    pub name: String,
    pub kind: String,
    pub rows: Vec<Row>,
}

/// One drop table: its kept rows.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Table {
    pub rows: Vec<Row>,
}

/// One obol row the obol task splices into a drop table: where it lands, its
/// item and the base chance the task multiplier scales.
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Splice {
    table: String,
    index: usize,
    name: String,
    chance: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Obol {
    page: usize,
    hyper: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Card {
    /// `desc_line1`: the key the save's `Cards0` table and the ledger use.
    key: String,
}

#[derive(Debug, Deserialize)]
struct Data {
    monsters: BTreeMap<String, Monster>,
    tables: BTreeMap<String, Table>,
    #[serde(rename = "obolSplices")]
    obol_splices: Vec<Splice>,
    obols: BTreeMap<String, Obol>,
    cards: BTreeMap<String, Card>,
}

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(DROPS_JSON).expect("bundled drop rows are valid"))
}

/// A monster's display name, with the game data's underscores as spaces; the
/// upstream `monName`.
pub fn monster_name(kind: &str) -> String {
    data()
        .monsters
        .get(kind)
        .map(|monster| monster.name.clone())
        .unwrap_or_else(|| kind.to_string())
}

/// Whether the monster rolls drops at all: upstream gates the drop branch on
/// the definition's `Type` being `Monster`, so ore, trees, bugs and fish do not.
pub fn rolls(kind: &str) -> bool {
    data()
        .monsters
        .get(kind)
        .is_some_and(|monster| monster.kind == "Monster")
}

/// The stat multipliers the drop rolls read, resolved from the mechanics
/// overrides: the character's Drop Rate and Card Drop Chance, and the
/// Rares Everywhere! talent's rare-row bonus. Neutral values roll base chances.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inputs {
    pub drop_rate: f64,
    pub card_chance: f64,
    pub talent29: f64,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            drop_rate: 1.0,
            card_chance: 1.0,
            talent29: 0.0,
        }
    }
}

impl Inputs {
    /// The inputs the overrides resolve to; an absent override keeps the
    /// neutral value, matching the catalog's `neutral`.
    pub fn from_overrides(overrides: &Overrides) -> Self {
        let read = |key: &str, neutral: f64| match overrides.get(key) {
            Some(Value::Number(value)) => *value,
            _ => neutral,
        };
        Self {
            drop_rate: read("dropRate", 1.0),
            card_chance: read("cardChanceMulti", 1.0),
            talent29: read("talent29", 0.0),
        }
    }
}

/// The save state a run's drops start from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// `Cards0`: every card count the account owns, keyed by `desc_line1`.
    pub cards: BTreeMap<String, f64>,
    /// `ObolInvOr`: the obol pages, one slot map per page.
    pub pages: Vec<BTreeMap<usize, String>>,
    /// `ObolInvOwn`: the owned obol inventory rows per page.
    pub own: Vec<f64>,
    /// `CYObolFragments`.
    pub fragments: f64,
    /// Whether the save carried the fragment balance; without it the ending
    /// balance is unknown and the run reports the gains only.
    pub fragments_known: bool,
    /// Silver Jellyfish auto-trash (`Menagerie` unlock 10).
    pub auto_trash: bool,
    /// `(10 + Menagerie[0][10]) / 100`: the per-obol fragment roll's base.
    pub auto_trash_rate: f64,
    /// `TaskZZ2[1][0]`: the obol task level the drop-table splices read.
    pub tasks_obol: f64,
}

/// What a run's drops produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    /// Cards found, keyed by the card's `desc_line1`.
    pub cards: BTreeMap<String, f64>,
    /// Fragments the auto-trash rolled.
    pub fragments_earned: f64,
    /// Obols the auto-trash consumed.
    pub fragments_auto_trashed: u64,
    /// The running fragment balance.
    pub fragments_balance: f64,
    /// Whether the starting balance was in the save.
    pub fragments_known: bool,
    /// Whether the auto-trash is unlocked at all.
    pub auto_trash: bool,
    /// Obols neither placed nor trashed: a full page without the auto-trash
    /// would fall back to the inventory, which this port does not model.
    pub obols_unpicked: u64,
}

/// Read the save state cards, obol pages and fragments start from.
pub fn snapshot(save: &Save) -> Snapshot {
    let field = |key: &str| save.get(key);
    let cards = field("Cards0")
        .and_then(|value| value.as_object().cloned())
        .map(|rows| {
            rows.iter()
                .filter_map(|(key, value)| number(value).map(|count| (key.clone(), count)))
                .collect()
        })
        .unwrap_or_default();
    let pages = field("ObolInvOr")
        .and_then(|value| value.as_array().cloned())
        .map(|pages| {
            pages
                .iter()
                .map(|page| {
                    page.as_object()
                        .map(|slots| {
                            slots
                                .iter()
                                .filter_map(|(slot, item)| {
                                    Some((slot.parse().ok()?, item.as_str()?.to_string()))
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();
    let own = field("ObolInvOwn")
        .and_then(|value| value.as_array().cloned())
        .map(|rows| rows.iter().filter_map(number).collect())
        .unwrap_or_default();
    let fragments = field("CYObolFragments").as_ref().and_then(number);
    let menagerie = field("Menagerie");
    let row = |path: &[usize]| {
        menagerie
            .as_ref()
            .and_then(|saved| index(saved, path))
            .and_then(number)
            .unwrap_or(0.0)
    };
    Snapshot {
        cards,
        pages,
        own,
        fragments: fragments.unwrap_or(0.0),
        fragments_known: fragments.is_some(),
        auto_trash: row(&[2, 10]) >= 1.0,
        auto_trash_rate: (10.0 + row(&[0, 10])) / 100.0,
        tasks_obol: field("TaskZZ2")
            .as_ref()
            .and_then(|tasks| index(tasks, &[1, 0]))
            .and_then(number)
            .unwrap_or(0.0),
    }
}

/// The game's `DropOdds`: always one draw, then the expected count when the
/// chance is large, else zero, one or two.
pub fn drop_odds(rng: &mut Rng, e: f64, t: f64, dr: f64) -> f64 {
    let roll = rng.float();
    let p = t * dr;
    if 2.0 < p * e {
        return (p * e).round();
    }
    let empty = (1.0 - p).powf(e);
    if roll < empty {
        return 0.0;
    }
    if roll < p * (1.0 - p).powf(e - 1.0) * e + empty {
        return 1.0;
    }
    2.0
}

/// What one picked-up drop did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pickup {
    /// The card counted, or the obol filled a blank page slot.
    Placed,
    /// A full obol page auto-trashed the drop for this many fragments.
    Fragments(f64),
    /// Neither: the drop needs a path this port does not model.
    Unpicked,
}

/// The drop ledger one run builds up.
#[derive(Clone, Debug)]
pub struct State {
    inputs: Inputs,
    /// The saved card counts, mutated by the run's pickups like `st.cards0`.
    cards0: BTreeMap<String, f64>,
    pages: Vec<BTreeMap<usize, String>>,
    own: Vec<f64>,
    fragments: f64,
    fragments_known: bool,
    auto_trash: bool,
    auto_trash_rate: f64,
    tasks_obol: f64,
    /// The reset rows the current card odds produce.
    monsters: BTreeMap<String, Vec<Row>>,
    tables: BTreeMap<String, Vec<Row>>,
    cards: BTreeMap<String, f64>,
    fragments_earned: f64,
    auto_trashed: u64,
    unpicked: u64,
}

impl State {
    /// A run's drop ledger, with the odds reset the way the engine does before
    /// its first kill.
    pub fn new(snapshot: Snapshot, inputs: Inputs) -> Self {
        let mut state = Self {
            inputs,
            cards0: snapshot.cards,
            pages: snapshot.pages,
            own: snapshot.own,
            fragments: snapshot.fragments,
            fragments_known: snapshot.fragments_known,
            auto_trash: snapshot.auto_trash,
            auto_trash_rate: snapshot.auto_trash_rate,
            tasks_obol: snapshot.tasks_obol,
            monsters: BTreeMap::new(),
            tables: BTreeMap::new(),
            cards: BTreeMap::new(),
            fragments_earned: 0.0,
            auto_trashed: 0,
            unpicked: 0,
        };
        state.reset();
        state
    }

    /// The rows one monster rolls after the current card odds.
    pub fn rows(&self, kind: &str) -> &[Row] {
        self.monsters.get(kind).map(Vec::as_slice).unwrap_or(&[])
    }

    /// One drop table's rows after the obol task's splices.
    pub fn table(&self, name: &str) -> &[Row] {
        self.tables.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The obol pages as the run leaves them.
    pub fn pages(&self) -> &[BTreeMap<usize, String>] {
        &self.pages
    }

    /// `lootDropOddsReset`: rebuild the odds from the pristine rows.
    fn reset(&mut self) {
        let data = data();
        self.monsters = data
            .monsters
            .iter()
            .map(|(kind, monster)| {
                let rows = monster.rows.iter().map(|row| self.card_odds(row)).collect();
                (kind.clone(), rows)
            })
            .collect();
        self.tables = data
            .tables
            .iter()
            .map(|(name, table)| (name.clone(), table.rows.clone()))
            .collect();
        self.splice_obols();
    }

    /// One row's chance after `lootDropOddsReset`'s card rules: an owned-zero
    /// card pays five times, and every card row scales by the card multiplier.
    fn card_odds(&self, row: &Row) -> Row {
        let mut row = row.clone();
        if row.name.starts_with("Cards") {
            if !row.name.starts_with("CardsZ") && self.unowned(&row.name) {
                row.chance *= 5.0;
            }
            row.chance *= self.inputs.card_chance;
        }
        row
    }

    /// `0 == Cards[0][desc]` with the key present: a card the account has never
    /// found, which the game makes five times as likely until three are owned.
    fn unowned(&self, name: &str) -> bool {
        data()
            .cards
            .get(name)
            .is_some_and(|card| self.cards0.get(&card.key) == Some(&0.0))
    }

    /// The obol task's rows, spliced into the drop tables their rolls read.
    /// Source 74357: twelve rows at `1 + obol/20`, then six more, in the order
    /// the extractor recorded from the game's own splices.
    fn splice_obols(&mut self) {
        if self.tasks_obol <= 0.0 {
            return;
        }
        let scale = 1.0 + self.tasks_obol / 20.0;
        for splice in &data().obol_splices {
            let Some(rows) = self.tables.get_mut(&splice.table) else {
                continue;
            };
            let at = splice.index.min(rows.len());
            rows.insert(
                at,
                Row {
                    name: splice.name.clone(),
                    chance: splice.chance * scale,
                    qty: 1.0,
                },
            );
        }
    }

    /// `generateMonsterDrops` over the kept rows: roll each row, expand the
    /// drop-table references at the shared `DN2` quantity, and merge repeats.
    pub fn roll(&mut self, rng: &mut Rng, kind: &str) -> Vec<(String, f64)> {
        let Some(rows) = self.monsters.get(kind).cloned() else {
            return Vec::new();
        };
        let dr = self.inputs.drop_rate;
        let mut out: Vec<(String, f64)> = Vec::new();
        for row in rows {
            let n = drop_odds(rng, 1.0, row.chance, dr);
            if n == 0.0 {
                continue;
            }
            let product = row.qty * n;
            let qty = if product > 1e9 {
                product
            } else {
                product.round()
            };
            if row.name.starts_with("DropTable") {
                let mut dn2 = qty;
                self.nested(rng, &row.name, &mut dn2, &mut out);
            } else {
                merge(&mut out, &row.name, qty);
            }
        }
        out
    }

    /// The rows of one drop table, at the quantity the caller rolled.
    fn nested(&self, rng: &mut Rng, table: &str, dn2: &mut f64, out: &mut Vec<(String, f64)>) {
        let dr = self.inputs.drop_rate.max(1.0);
        let talent = 1.0 + self.inputs.talent29 / 100.0;
        let rows = self.tables.get(table).cloned().unwrap_or_default();
        for row in rows {
            let chance = if row.name.starts_with("DropTable") {
                row.chance / dr
            } else {
                row.chance * talent / dr
            };
            let n = drop_odds(rng, *dn2, chance, self.inputs.drop_rate);
            if n == 0.0 {
                continue;
            }
            let product = row.qty * n;
            let qty = if product > 1e9 {
                product
            } else {
                product.round()
            };
            if row.name.starts_with("DropTable") {
                *dn2 = qty;
                // The third level reads the same quantity, with no talent bonus.
                let third = self.tables.get(&row.name).cloned().unwrap_or_default();
                for row in third {
                    let n = drop_odds(rng, *dn2, row.chance / dr, self.inputs.drop_rate);
                    if n == 0.0 {
                        continue;
                    }
                    let product = row.qty * n;
                    let qty = if product > 1e9 {
                        product
                    } else {
                        product.round()
                    };
                    merge(out, &row.name, qty);
                }
            } else {
                merge(out, &row.name, qty);
            }
        }
    }

    /// One kill's drops: roll the monster's rows, then pick up each card and
    /// obol in the order they were rolled.
    pub fn kill(&mut self, rng: &mut Rng, kind: &str) {
        for (name, amount) in self.roll(rng, kind) {
            self.take(rng, &name, amount);
        }
    }

    /// `ItemPickupInTheFirstPlace` for a card or an obol drop.
    pub fn take(&mut self, rng: &mut Rng, name: &str, amount: f64) -> Pickup {
        if name.starts_with("Obol") {
            return self.take_obol(rng, name);
        }
        if name.starts_with("Cards") {
            return self.take_card(name, amount);
        }
        Pickup::Unpicked
    }

    fn take_obol(&mut self, rng: &mut Rng, name: &str) -> Pickup {
        let data = data();
        let page = data.obols.get(name).map_or(3, |obol| obol.page);
        let hyper = data.obols.get(name).is_some_and(|obol| obol.hyper);
        let slots = (4.0 * self.own.get(page).copied().unwrap_or(0.0)) as usize;
        let blank = (0..slots).find(|slot| {
            self.pages
                .get(page)
                .and_then(|page| page.get(slot))
                .is_some_and(|item| item == "Blank")
        });
        if let Some(slot) = blank
            && let Some(slots) = self.pages.get_mut(page)
        {
            slots.insert(slot, name.to_string());
            return Pickup::Placed;
        }
        if !self.auto_trash || hyper {
            self.unpicked += 1;
            return Pickup::Unpicked;
        }
        let rolled = (rng.float() + self.auto_trash_rate).floor().min(3.0);
        let before = self.fragments;
        self.fragments = (before + rolled).round().min(FRAGMENT_CAP);
        let gained = (self.fragments - before).max(0.0);
        self.fragments_earned += gained;
        self.auto_trashed += 1;
        Pickup::Fragments(gained)
    }

    fn take_card(&mut self, name: &str, amount: f64) -> Pickup {
        let key = data()
            .cards
            .get(name)
            .map(|card| card.key.clone())
            .unwrap_or_else(|| name.to_string());
        *self.cards0.entry(key.clone()).or_insert(0.0) += amount;
        *self.cards.entry(key.clone()).or_insert(0.0) += amount;
        // A pickup that leaves the count below three re-reads the unowned x5.
        if self.cards0.get(&key).copied().unwrap_or(0.0) < 3.0 {
            self.reset();
        }
        Pickup::Placed
    }

    /// The run's card and obol results.
    pub fn summary(&self) -> Summary {
        Summary {
            cards: self.cards.clone(),
            fragments_earned: self.fragments_earned,
            fragments_auto_trashed: self.auto_trashed,
            fragments_balance: self.fragments,
            fragments_known: self.fragments_known,
            auto_trash: self.auto_trash,
            obols_unpicked: self.unpicked,
        }
    }
}

fn merge(out: &mut Vec<(String, f64)>, name: &str, qty: f64) {
    if let Some(entry) = out.iter_mut().find(|(item, _)| item == name) {
        entry.1 = (entry.1 + qty).round();
    } else {
        out.push((name.to_string(), qty));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drop_inputs_read_the_mechanics_overrides() {
        let mut overrides = Overrides::new();
        assert_eq!(Inputs::from_overrides(&overrides), Inputs::default());
        overrides.insert("dropRate".into(), Value::Number(3.5));
        overrides.insert("cardChanceMulti".into(), Value::Number(2.0));
        overrides.insert("talent29".into(), Value::Number(40.0));
        assert_eq!(
            Inputs::from_overrides(&overrides),
            Inputs {
                drop_rate: 3.5,
                card_chance: 2.0,
                talent29: 40.0,
            }
        );
        // A mechanic the drop rolls do not read leaves the inputs alone.
        overrides.insert("monsterCash".into(), Value::Number(5.0));
        assert_eq!(Inputs::from_overrides(&overrides).drop_rate, 3.5);
    }

    #[test]
    fn a_save_without_the_drop_fields_starts_empty() {
        let save = idleon_save::Save::new(serde_json::json!({}));
        let snapshot = snapshot(&save);
        assert!(snapshot.cards.is_empty());
        assert!(snapshot.pages.is_empty());
        assert!(!snapshot.fragments_known);
        assert!(!snapshot.auto_trash);
        // The upstream rate is `(10 + Menagerie[0][10]) / 100`, so a save
        // without the row still rolls on its own tenth.
        assert!((snapshot.auto_trash_rate - 0.1).abs() < 1e-12);
        assert_eq!(snapshot.tasks_obol, 0.0);
        let state = State::new(snapshot, Inputs::default());
        assert_eq!(state.summary(), Summary::default());
    }

    #[test]
    fn only_monster_types_roll_drops() {
        assert!(rolls("mushG"));
        assert!(!rolls("not-a-monster"));
        assert_eq!(monster_name("mushG"), "Green Mushroom");
        assert_eq!(monster_name("not-a-monster"), "not-a-monster");
    }
}
