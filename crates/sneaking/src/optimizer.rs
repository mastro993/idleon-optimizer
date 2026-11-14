//! SN-03: the charm placement search.
//!
//! Ported from `optimizeSneakingLoadout` in
//! `js/stats/systems/w6/sneaking-loadout-search.js` (its `_context`, `_saved`,
//! `_capture` and `_metric`) with the pair rules of `canEquipCharmPair` in
//! `js/stats/systems/w6/sneaking-optimizer.js`. The saved placement is measured
//! first, so a run always has a baseline; then every untied twin is offered the
//! legal charm pairs the owned pool can supply, the whole layout is measured
//! again, and the pair is kept when it beats what the twin had. Passes repeat
//! until nothing improves.
//!
//! The search moves charms only: the saved floors, hats and weapons stay put, so
//! a recommendation is always a loadout the player's own floors can run. A charm
//! never materialises — the pool is the account's equipped and inventory charms,
//! and a pair is only offered when the owned copies can cover it — and the
//! account-unique Scroll of Power can still only be worn once.
//!
//! The objective is upstream's `_metric`: Jade per hour, EXP per hour, the item
//! claim rate, the door hits the remaining HP still needs, and untie progress per
//! hour. The item objective scores the deterministic rate upstream shortlists
//! loadouts with (successful actions × item find), not its Monte-Carlo claim
//! simulation, which needs the flower pass this section does not build yet.

use super::data::ItemDef;
use super::loadout::SCROLL_OF_POWER;
use super::{ActionMode, INVENTORY_SLOTS, Item, Loadout, SlotKind, can_equip_pair};
use idleon_common::{Control, Progress};

/// How many candidate placements one run may measure.
///
/// Upstream caps its search at 2048, but every one of its evaluations simulates a
/// batch of items; this port scores analytically, so a wider budget still lands in
/// well under a second on a full roster.
const MAX_EVALUATIONS: usize = 24_000;
/// How many improving sweeps the search may take over the roster.
const MAX_PASSES: usize = 3;
/// The charm slots a twin carries.
const CHARMS: usize = 2;

/// The objective the optimizer maximizes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Objective {
    /// Jade per hour from every twin on its normal floor action.
    #[default]
    Jade,
    /// Sneaking EXP per hour from the twins training or sneaking.
    Exp,
    /// Item claims per hour, weighted by the twin's item-find bonus.
    Items,
    /// How fast the twins hit the one unfinished door, in remaining HP.
    Door,
    /// Untie progress per hour on the floors with a captive waiting.
    Untie,
}

impl Objective {
    /// Every objective, in the order the optimizer's picker lists them.
    pub const ALL: [Objective; 5] = [
        Objective::Jade,
        Objective::Exp,
        Objective::Items,
        Objective::Door,
        Objective::Untie,
    ];

    /// The word the picker prints.
    pub fn label(self) -> &'static str {
        match self {
            Objective::Jade => "Jade",
            Objective::Exp => "EXP",
            Objective::Items => "Items",
            Objective::Door => "Door",
            Objective::Untie => "Untie",
        }
    }

    /// The stable id the picker selects by.
    pub fn id(self) -> &'static str {
        match self {
            Objective::Jade => "jade",
            Objective::Exp => "exp",
            Objective::Items => "items",
            Objective::Door => "door",
            Objective::Untie => "untie",
        }
    }

    /// The objective an id names.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|entry| entry.id() == id)
    }

    /// What one unit of the objective means, for the result caption.
    pub fn unit(self) -> &'static str {
        match self {
            Objective::Jade => "Jade / hour",
            Objective::Exp => "EXP / hour",
            Objective::Items => "claims / hour",
            Objective::Door => "door clears / hour",
            Objective::Untie => "untie progress / hour",
        }
    }
}

impl std::fmt::Display for Objective {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

/// What the optimizer is asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Request {
    pub objective: Objective,
}

/// One twin's share of the objective under a layout.
#[derive(Clone, Debug, PartialEq)]
pub struct TwinScore {
    pub twin: usize,
    pub name: String,
    pub floor: usize,
    pub mode: ActionMode,
    /// The chance an action costs a KO.
    pub detection: f64,
    /// The team-coupled successful actions per hour.
    pub hits_per_hour: f64,
    /// This twin's contribution to the objective's score.
    pub contribution: f64,
}

/// A charm pair per untied twin, and the objective it reaches.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The pair each searchable twin wears, in save order.
    pub charms: Vec<(usize, [Item; CHARMS])>,
    pub score: f64,
    pub twins: Vec<TwinScore>,
}

/// A finished optimisation: the saved placement and the best one found.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub request: Request,
    pub baseline: Layout,
    pub winner: Layout,
    /// The candidates the search measured.
    pub evaluations: usize,
    /// The sweeps it took, the last one improving nothing.
    pub passes: usize,
}

impl Outcome {
    /// Whether the search found a placement that scores above the saved one.
    pub fn improved(&self) -> bool {
        self.winner.score > self.baseline.score
    }

    /// The winner's gain over the baseline, in percent.
    pub fn gain_percent(&self) -> f64 {
        if self.baseline.score <= 0.0 {
            return 0.0;
        }
        (self.winner.score / self.baseline.score - 1.0) * 100.0
    }
}

impl Loadout {
    /// The objective's value under the current charms.
    pub fn score(&self, objective: Objective) -> f64 {
        self.measure(objective).score
    }

    /// The search's result for `request`, reporting nothing and never stopping.
    pub fn optimize(&self, request: Request) -> Outcome {
        self.optimize_with(request, |_| Control::Continue)
            .expect("a reporter that always continues never stops the search")
    }

    /// The search's result, reporting each twin it finishes and stopping when
    /// the reporter asks. `None` means the run was cancelled and stored nothing.
    ///
    /// The loadout is only read: every candidate is measured on a copy, so a
    /// cancelled or finished run leaves the editor exactly as it was.
    pub fn optimize_with(
        &self,
        request: Request,
        mut report: impl FnMut(Progress) -> Control,
    ) -> Option<Outcome> {
        let baseline = self.measure(request.objective);
        let searchable = self.searchable();
        let pool = self.charm_pool(&searchable);
        let mut scratch = self.clone();
        let mut evaluations = 0;
        let mut passes = 0;
        let budget = searchable.len().max(1);
        'search: for pass in 0..MAX_PASSES {
            passes = pass + 1;
            let mut improved = false;
            for (position, twin) in searchable.iter().copied().enumerate() {
                let others = scratch.equipped_elsewhere(twin);
                let current = scratch.pair(twin);
                let mut best: Option<[Item; CHARMS]> = None;
                let mut best_score = scratch.measure(request.objective).score;
                for first in 0..=pool.len() {
                    for second in 0..=pool.len() {
                        let pair = candidate(&pool, first, second);
                        if pair == current || !legal(&pool, &others, &pair, scratch.charmed) {
                            continue;
                        }
                        scratch.set_pair(twin, &pair);
                        let score = scratch.measure(request.objective).score;
                        evaluations += 1;
                        if score > best_score {
                            best_score = score;
                            best = Some(pair);
                        }
                        if evaluations >= MAX_EVALUATIONS {
                            scratch.set_pair(twin, &current);
                            break 'search;
                        }
                    }
                }
                scratch.set_pair(twin, &current);
                if let Some(pair) = best {
                    scratch.set_pair(twin, &pair);
                    improved = true;
                }
                if report(Progress {
                    fraction: ((pass * budget + position + 1) as f32)
                        / ((MAX_PASSES * budget).max(1) as f32),
                    message: format!(
                        "Sweep {passes}: twin {} of {budget} · {evaluations} placements measured",
                        position + 1
                    ),
                }) == Control::Stop
                {
                    return None;
                }
            }
            if !improved {
                break;
            }
        }
        let winner = scratch.measure(request.objective);
        if report(Progress {
            fraction: 1.0,
            message: format!(
                "{evaluations} placements measured; {} found",
                if winner.score > baseline.score {
                    "a better one"
                } else {
                    "no improvement"
                }
            ),
        }) == Control::Stop
        {
            return None;
        }
        Some(Outcome {
            request,
            baseline,
            winner,
            evaluations,
            passes,
        })
    }

    /// Write a recommended layout into the loadout.
    ///
    /// The charms the recommendation no longer wears go back to the inventory
    /// rows the pool scan read them from, so the account keeps every charm it
    /// owned. `false` means the visible inventory has no free row to bank one of
    /// them, and the loadout is left untouched.
    pub fn apply(&mut self, layout: &Layout) -> bool {
        let mut banked = self.owned_charms();
        let pairs: Vec<(usize, [Item; CHARMS])> = self
            .twins
            .iter()
            .map(|twin| {
                let pair = layout
                    .charms
                    .iter()
                    .find(|(index, _)| *index == twin.index)
                    .map_or_else(|| self.pair(twin.index), |(_, pair)| pair.clone());
                (twin.index, pair)
            })
            .collect();
        for (_, pair) in &pairs {
            for charm in pair {
                discount(&mut banked, charm);
            }
        }
        let rows: Vec<usize> = (0..INVENTORY_SLOTS.min(self.inventory.len()))
            .filter(|row| self.inventory[*row].is_blank() || representable(&self.inventory[*row]))
            .collect();
        let pending: usize = banked.iter().map(|(_, count)| *count).sum();
        if pending > rows.len() {
            return false;
        }
        for row in &rows {
            self.inventory[*row] = Item::blank();
        }
        let mut next = 0;
        for (item, count) in &banked {
            for _ in 0..*count {
                self.inventory[rows[next]] = item.clone();
                next += 1;
            }
        }
        for (twin, pair) in &pairs {
            self.set_pair(*twin, pair);
        }
        true
    }

    /// The charm pair one twin wears.
    fn pair(&self, twin: usize) -> [Item; CHARMS] {
        let blank = Item::blank();
        let Some(entry) = self.twins.get(twin) else {
            return [blank.clone(), blank];
        };
        [
            entry.slot(SlotKind::Charm(0)).clone(),
            entry.slot(SlotKind::Charm(1)).clone(),
        ]
    }

    /// Put a pair into one twin's charm slots.
    fn set_pair(&mut self, twin: usize, pair: &[Item; CHARMS]) {
        if let Some(entry) = self.twins.get_mut(twin) {
            for (charm, item) in pair.iter().enumerate() {
                entry.slots[SlotKind::Charm(charm).index()] = item.clone();
            }
        }
    }

    /// The untied twins whose charm slots the search may rewrite. A twin whose
    /// slot holds something the pool cannot represent keeps its charms.
    fn searchable(&self) -> Vec<usize> {
        self.twins
            .iter()
            .take(self.players.min(self.twins.len()))
            .filter(|twin| twin.untied)
            .filter(|twin| {
                (0..CHARMS).all(|charm| representable(twin.slot(SlotKind::Charm(charm))))
            })
            .map(|twin| twin.index)
            .collect()
    }

    /// The distinct charms the account owns, with how many copies: everything the
    /// untied twins wear plus every inventory row the pool scan reads.
    fn charm_pool(&self, searchable: &[usize]) -> Vec<(Item, usize)> {
        let mut pool: Vec<(Item, usize)> = Vec::new();
        for twin in searchable {
            if let Some(entry) = self.twins.get(*twin) {
                for charm in 0..CHARMS {
                    tally(&mut pool, entry.slot(SlotKind::Charm(charm)));
                }
            }
        }
        for item in self.inventory.iter().take(INVENTORY_SLOTS) {
            tally(&mut pool, item);
        }
        pool
    }

    /// How many copies of each charm every twin except `twin` wears.
    fn equipped_elsewhere(&self, twin: usize) -> Vec<(Item, usize)> {
        let mut worn: Vec<(Item, usize)> = Vec::new();
        for entry in &self.twins {
            if entry.index == twin {
                continue;
            }
            for charm in 0..CHARMS {
                tally(&mut worn, entry.slot(SlotKind::Charm(charm)));
            }
        }
        worn
    }

    /// Every charm the account owns, across the twins and the inventory.
    fn owned_charms(&self) -> Vec<(Item, usize)> {
        let mut owned: Vec<(Item, usize)> = Vec::new();
        for entry in &self.twins {
            for charm in 0..CHARMS {
                tally(&mut owned, entry.slot(SlotKind::Charm(charm)));
            }
        }
        for item in self.inventory.iter().take(INVENTORY_SLOTS) {
            tally(&mut owned, item);
        }
        owned
    }

    /// `_metric`: the objective's score under the current charms, with each
    /// twin's share.
    fn measure(&self, objective: Objective) -> Layout {
        let gold = self.gold_bonuses();
        let slots = self
            .twins
            .iter()
            .map(|twin| twin.index + 1)
            .max()
            .unwrap_or(0);
        let mut rates = vec![None; slots];
        for entry in &self.twins {
            let on_floor = rates[entry.index].is_some();
            if !on_floor {
                for (index, rate) in self.floor_rates(entry.floor, &gold) {
                    rates[index] = Some(rate);
                }
            }
        }
        let mut twins = Vec::new();
        let mut score = 0.0;
        for entry in &self.twins {
            let index = entry.index;
            let mode = self.action_mode(index);
            let hits = rates[index].unwrap_or_default().hits_per_hour;
            let contribution = match objective {
                Objective::Jade if mode == ActionMode::Normal => {
                    self.jade_coin_with(index, &gold).coin * hits
                }
                Objective::Exp if matches!(mode, ActionMode::Normal | ActionMode::Training) => {
                    self.exp_action_with(index, &gold).exp * hits
                }
                Objective::Items if matches!(mode, ActionMode::Normal | ActionMode::Training) => {
                    hits * (1.0 + self.bonuses_with(index, &gold).get(5) / 100.0)
                }
                Objective::Door if mode == ActionMode::Door => {
                    let door = self.door_state(entry.floor);
                    let damage = self.door_damage_with(index, &gold).damage;
                    if door.active && door.remaining_hp > 0.0 && damage > 0.0 {
                        hits / (door.remaining_hp / damage).ceil().max(1.0)
                    } else {
                        0.0
                    }
                }
                Objective::Untie if mode == ActionMode::Untie => {
                    self.untie_progress(index).progress * hits
                }
                _ => 0.0,
            };
            score += contribution;
            twins.push(TwinScore {
                twin: index,
                name: entry.name.clone(),
                floor: entry.floor,
                mode,
                detection: self.detection_with(index, &gold),
                hits_per_hour: hits,
                contribution,
            });
        }
        Layout {
            charms: self
                .searchable()
                .into_iter()
                .map(|twin| (twin, self.pair(twin)))
                .collect(),
            score,
            twins,
        }
    }
}

/// Whether a slot holds something the pool scan can move: an empty slot or a
/// catalog charm that is not a Gold inventory charm.
fn representable(item: &Item) -> bool {
    if item.is_blank() {
        return true;
    }
    item.def()
        .is_some_and(|def: &ItemDef| def.kind == 2 && !def.is_gold())
}

/// Count one more copy of a charm. Blanks, unknown keys and the Gold inventory
/// charms the pool never equips are skipped.
fn tally(counts: &mut Vec<(Item, usize)>, item: &Item) {
    if !representable(item) || item.is_blank() {
        return;
    }
    match counts.iter_mut().find(|(known, _)| known == item) {
        Some((_, count)) => *count += 1,
        None => counts.push((item.clone(), 1)),
    }
}

/// Take one copy of a charm out of a multiset, when it holds one.
fn discount(counts: &mut [(Item, usize)], item: &Item) {
    if let Some(entry) = counts.iter_mut().find(|(known, _)| known == item) {
        entry.1 = entry.1.saturating_sub(1);
    }
}

/// The pair a candidate index names: `0` is the empty slot, then one entry per
/// distinct pool charm.
fn candidate(pool: &[(Item, usize)], first: usize, second: usize) -> [Item; CHARMS] {
    let pick = |index: usize| match index {
        0 => Item::blank(),
        _ => pool[index - 1].0.clone(),
    };
    [pick(first), pick(second)]
}

/// Whether the owned copies can cover a pair while the other twins keep theirs,
/// the pair is legal in one twin, and the Scroll of Power stays account-unique.
fn legal(
    pool: &[(Item, usize)],
    others: &[(Item, usize)],
    pair: &[Item; CHARMS],
    charmed: bool,
) -> bool {
    if pair[0].is_blank() && pair[1].is_blank() {
        return true;
    }
    if !can_equip_pair(&pair[0].key, &pair[1].key, charmed) {
        return false;
    }
    let scrolls = pair
        .iter()
        .filter(|item| item.key == SCROLL_OF_POWER)
        .count()
        + others
            .iter()
            .filter(|(item, _)| item.key == SCROLL_OF_POWER)
            .map(|(_, count)| *count)
            .sum::<usize>();
    if scrolls > 1 {
        return false;
    }
    pair.iter().all(|item| {
        if item.is_blank() {
            return true;
        }
        let needed = pair.iter().filter(|other| *other == item).count();
        let owned = pool
            .iter()
            .find(|(known, _)| known == item)
            .map_or(0, |(_, count)| *count);
        let worn = others
            .iter()
            .find(|(known, _)| known == item)
            .map_or(0, |(_, count)| *count);
        worn + needed <= owned
    })
}
