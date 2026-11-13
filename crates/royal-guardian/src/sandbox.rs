//! The Royal Guardian sandbox: a saved state a plan can edit, and the moves it
//! may apply to it.
//!
//! Ported from `cloneRoyalState`, `decodePackedUnitDigits`,
//! `encodePackedUnitDigits`, `barracksUnlockSlot`, `outpostAssignableSlotCount`,
//! `validateRoyalSandbox`, `applyRoyalMove` and the gate helpers of
//! `js/stats/systems/w7/royal-guardian-optimizer.js`.
//!
//! A move edits the two saved rows the kingdom lives in - `RoyalG[2]` for the
//! Armory and `RoyalMaps` for the outposts - and nothing else. Every applied
//! move is re-validated, so a plan can only ever describe a layout the game
//! accepts: a known outpost type, units in assignable slots of an unlocked
//! profession, connections that are in range and legal for the outpost's type,
//! points not overspent and no world over its Support or Savage limit.
//!
//! The upstream gates some moves behind Armory shelves; the port keeps those
//! gates, so a plan on a save without the shelf says which move it cannot make.

use super::*;

/// The Armory shelf that unlocks each rank bar, in bar order: upstream
/// `BAR_ARMORY`.
pub const BAR_ARMORY: [usize; 5] = [27, 29, 73, 74, 75];

/// How much reach a Support link needs beyond the outpost's own range.
const SUPPORT_LINK_ALLOWANCE: f64 = 8.0;

/// The Armory shelf that lets a Normal outpost drain a node.
const RESOURCE_TOOL: usize = 30;
/// The Armory shelf that lets a Support outpost link a map.
const MAP_TOOL: usize = 31;
/// The Armory shelf that opens the outpost upgrade list.
const UPGRADE_TOOL: usize = 32;
/// The Armory shelf that opens the third outpost upgrade.
const EDUCATION_TOOL: usize = 57;

/// What one outpost upgrade spends, by upgrade kind: upstream
/// `_outpostUpgradeCosts`.
pub const UPGRADE_COSTS: [f64; 3] = [12.0, 2.0, 1.0];

/// One profession a packed unit digit names.
pub const PROFESSIONS: [&str; 4] = ["Worker", "Trader", "Guard", "Surveyor"];

/// The name of one outpost type: 0 Normal, 1 Support, 2 Savage.
pub fn outpost_type_name(kind: usize) -> &'static str {
    match kind {
        1 => "Support",
        2 => "Savage",
        _ => "Normal",
    }
}

/// The name of one transient assignment: 4 Militia to 7 Priest.
pub fn transient_name(kind: i64) -> &'static str {
    match kind {
        4 => "Militia",
        5 => "Commander",
        6 => "Knight",
        7 => "Priest",
        _ => "Assignment",
    }
}

/// The name of one rank bar: upstream `outpostRankName`.
pub fn rank_name(bar: usize) -> &'static str {
    match bar {
        0 => "Trading Rank",
        1 => "Intel Rank",
        2 => "Command Rank",
        3 => "Military Rank",
        4 => "Purity Rank",
        _ => "Rank",
    }
}

/// One edit a plan may apply to the saved state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Move {
    /// Change an outpost's type to the next one in the unlocked order.
    Type { map: usize, kind: usize },
    /// Write the outpost's first connection slot.
    Connection { map: usize, endpoint: Endpoint },
    /// Change one stationed unit's profession.
    Profession {
        map: usize,
        slot: usize,
        profession: usize,
    },
    /// Buy one outpost upgrade: 0 barracks, 1 logistics, 2 education.
    OutpostUpgrade { map: usize, upgrade: usize },
    /// Set the three outpost upgrade levels.
    Levels { map: usize, levels: [f64; 3] },
}

impl Move {
    /// The line the plan's step list prints.
    pub fn label(&self) -> String {
        match *self {
            Move::Type { map, kind } => {
                format!("Map {map}: {} outpost", outpost_type_name(kind))
            }
            Move::Connection { map, endpoint } => format!(
                "Map {map}: {}",
                match endpoint {
                    Endpoint::Empty => "clear the connection".to_owned(),
                    Endpoint::Resource(index) => format!("drain resource {}", index + 1),
                    Endpoint::Map(target) => format!("link map {target}"),
                }
            ),
            Move::Profession {
                map,
                slot,
                profession,
            } => format!(
                "Map {map}: slot {slot} to {}",
                PROFESSIONS.get(profession).copied().unwrap_or("unit")
            ),
            Move::OutpostUpgrade { map, upgrade } => {
                let name = ["barracks", "logistics", "education"]
                    .get(upgrade)
                    .copied()
                    .unwrap_or("upgrade");
                format!("Map {map}: buy {name}")
            }
            Move::Levels { map, levels } => format!(
                "Map {map}: levels {} / {} / {}",
                levels[0], levels[1], levels[2]
            ),
        }
    }
}

/// A saved state a plan edits. The rows are owned, so a move never touches the
/// save the section renders.
#[derive(Clone, Debug)]
pub struct Sandbox {
    state: RoyalGuardian,
}

impl Sandbox {
    /// A sandbox over a saved state.
    pub fn new(state: &RoyalGuardian) -> Self {
        Self {
            state: state.clone(),
        }
    }

    /// The state the sandbox holds.
    pub fn state(&self) -> &RoyalGuardian {
        &self.state
    }

    /// The state the sandbox holds, for a step that writes a saved row.
    pub(crate) fn state_mut(&mut self) -> &mut RoyalGuardian {
        &mut self.state
    }

    /// The state the sandbox holds, consuming it.
    pub fn into_state(self) -> RoyalGuardian {
        self.state
    }

    /// Writes one currency balance.
    pub(crate) fn set_currency(&mut self, slot: i64, value: f64) {
        self.state
            .write_g_cell(row::CURRENCIES, slot.max(0) as usize, value);
    }

    /// Writes one Armory level.
    pub(crate) fn set_armory(&mut self, index: usize, value: f64) {
        self.state.write_g_cell(row::ARMORY, index, value);
    }

    /// Writes one options cell, which carries the daily discount counter.
    pub(crate) fn set_option(&mut self, index: usize, value: f64) {
        while self.state.options.len() <= index {
            self.state.options.push(Value::from(0.0));
        }
        self.state.options[index] = Value::from(value);
    }

    /// Writes one resource node's saved progress.
    pub(crate) fn set_progress(&mut self, resource: usize, value: f64) {
        self.state.write_g_cell(row::RESOURCES, resource, value);
    }

    /// Appends a transient assignment to a world's saved rows.
    pub(crate) fn push_assignment(&mut self, world: usize, kind: f64, map: f64) {
        let row = row::MILITIA + 2 * world;
        self.state.write_g_push(row, kind);
        self.state.write_g_push(row + 1, map);
    }
}

impl RoyalGuardian {
    /// Writes one `RoyalMaps` cell, growing the row to reach it.
    fn write_map_cell(&mut self, map: usize, cell: usize, value: f64) {
        while self.maps.len() <= map {
            self.maps.push(Value::Array(Vec::new()));
        }
        if !matches!(self.maps.get(map), Some(Value::Array(_))) {
            self.maps[map] = Value::Array(Vec::new());
        }
        if let Some(Value::Array(row)) = self.maps.get_mut(map) {
            while row.len() <= cell {
                row.push(Value::from(0.0));
            }
            row[cell] = Value::from(value);
        }
    }

    /// Writes one `RoyalG` cell, growing the row to reach it.
    pub(crate) fn write_g_cell(&mut self, row: usize, cell: usize, value: f64) {
        while self.g.len() <= row {
            self.g.push(Value::Array(Vec::new()));
        }
        if !matches!(self.g.get(row), Some(Value::Array(_))) {
            self.g[row] = Value::Array(Vec::new());
        }
        let Some(Value::Array(cells)) = self.g.get_mut(row) else {
            return;
        };
        while cells.len() <= cell {
            cells.push(Value::from(0.0));
        }
        cells[cell] = Value::from(value);
    }

    /// Appends one `RoyalG` cell to a row, the way a purchase adds an
    /// assignment.
    fn write_g_push(&mut self, row: usize, value: f64) {
        while self.g.len() <= row {
            self.g.push(Value::Array(Vec::new()));
        }
        if !matches!(self.g.get(row), Some(Value::Array(_))) {
            self.g[row] = Value::Array(Vec::new());
        }
        if let Some(Value::Array(cells)) = self.g.get_mut(row) {
            cells.push(Value::from(value));
        }
    }

    /// Every outpost type the saved Armory unlocks, in escalation order:
    /// upstream `_allowedTypes`.
    pub fn allowed_types(&self) -> Vec<usize> {
        let mut types = vec![0];
        if self.support_limit() > 0.0 {
            types.push(1);
        }
        if self.savage_limit() > 0.0 {
            types.push(2);
        }
        types
    }

    /// How many Support outposts a world may hold: upstream `_supportLimit`.
    pub fn support_limit(&self) -> f64 {
        self.armory_bonus(42).round().max(0.0)
    }

    /// How many Savage outposts a world may hold: upstream `_savageLimit`.
    pub fn savage_limit(&self) -> f64 {
        self.armory_bonus(44).round().max(0.0)
    }

    /// How many outpost upgrades the Armory has opened, which is also the count
    /// of assignable unit slots: upstream `outpostAssignableSlotCount`.
    pub fn assignable_slots(&self, map: usize) -> usize {
        let barracks = self.map_cell(map, 0).max(0.0).floor();
        (1.0 + barracks).clamp(0.0, 7.0) as usize
    }

    /// Every unit digit a `RoyalMaps` row carries, as `(slot, profession)`:
    /// upstream `decodePackedUnits`.
    pub fn stationed_units(&self, map: usize) -> Vec<(usize, usize)> {
        Self::packed_units(self.map_cell(map, cell::UNITS))
    }

    /// The stationed units in an assignable slot: upstream `outpostUnits`.
    pub fn units(&self, map: usize) -> Vec<(usize, usize)> {
        let slots = self.assignable_slots(map);
        self.stationed_units(map)
            .into_iter()
            .filter(|(slot, _)| *slot < slots)
            .collect()
    }

    /// The profession the Armory unlocks at each rank of its worker shelves:
    /// upstream `_unitTypes`.
    pub fn unit_types(&self) -> Vec<usize> {
        let count = 1
            + (self.armory_level(27) >= 1.0) as usize
            + (self.armory_level(28) >= 1.0) as usize
            + (self.armory_level(29) >= 1.0) as usize;
        (0..count.min(4)).collect()
    }

    /// Which rank bars the Armory unlocks: upstream `outpostUnlockedBars`.
    pub fn unlocked_bars(&self) -> [bool; 5] {
        BAR_ARMORY.map(|shelf| self.armory_level(shelf) >= 1.0)
    }

    /// Whether Armory `shelf` is owned: upstream `_armoryUnlocked`.
    pub fn shelf_unlocked(&self, shelf: usize) -> bool {
        self.armory_level(shelf) >= 1.0
    }

    /// Whether `to` is a legal Support target from `from`: upstream
    /// `outpostReachable`.
    pub fn outpost_reachable(&self, from: usize, to: usize) -> bool {
        let target = self.outpost(to);
        Self::world_of(from) == Self::world_of(to)
            && self.outpost(from).built
            && (target.built || target.eligible)
            && Self::outpost_distance(from, to) <= self.range(from) + SUPPORT_LINK_ALLOWANCE
    }

    /// The slot a barracks level opens, when the packed row still carries the
    /// `1` the game writes for it: upstream `barracksUnlockSlot`.
    fn barracks_unlock_slot(packed: f64) -> Option<usize> {
        let digits = format!("{:09}", packed.max(0.0) as u64);
        let digits = &digits[digits.len().saturating_sub(9)..];
        digits
            .char_indices()
            .find(|(index, digit)| *digit == '1' && (1..7).contains(index))
            .map(|(index, _)| index)
    }

    /// The packed unit row with one digit replaced: upstream `_setProfession`.
    fn write_profession(packed: f64, slot: usize, profession: usize) -> f64 {
        let mut digits = Self::packed_digits(packed);
        if let Some(digit) = digits.get_mut(slot) {
            *digit = (profession + 2) as u8;
        }
        digits
            .iter()
            .fold(0.0, |value, digit| value * 10.0 + *digit as f64)
    }

    /// The packed unit row's nine digits, most significant first.
    fn packed_digits(packed: f64) -> [u8; 9] {
        let text = format!("{:09}", packed.max(0.0) as u64);
        let text = &text[text.len().saturating_sub(9)..];
        std::array::from_fn(|index| text.as_bytes()[index].saturating_sub(b'0'))
    }
}

/// Applies one move, or says why it is not legal. Every accepted state passes
/// [`validate`], so a plan built from accepted moves can only describe a layout
/// the game accepts.
pub fn apply(state: &Sandbox, change: &Move) -> Result<Sandbox, Vec<String>> {
    let mut next = state.clone();
    let mut errors = Vec::new();
    match *change {
        Move::Type { map, kind } => apply_type(&mut next.state, map, kind, &mut errors),
        Move::Connection { map, endpoint } => {
            apply_connection(&mut next.state, map, endpoint, &mut errors);
        }
        Move::Profession {
            map,
            slot,
            profession,
        } => apply_profession(&mut next.state, map, slot, profession, &mut errors),
        Move::OutpostUpgrade { map, upgrade } => {
            apply_upgrade(&mut next.state, map, upgrade, &mut errors);
        }
        Move::Levels { map, levels } => apply_levels(&mut next.state, map, levels, &mut errors),
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let errors = validate(&next);
    if errors.is_empty() {
        Ok(next)
    } else {
        Err(errors)
    }
}

/// Every layout violation the state carries; empty when it is legal.
pub fn validate(state: &Sandbox) -> Vec<String> {
    let royal = state.state();
    let mut errors = Vec::new();
    let allowed = royal.allowed_types();
    let units = royal.unit_types();
    let mut support = [0usize; WORLDS];
    let mut savage = [0usize; WORLDS];
    for map in 0..royal.maps.len() {
        if !royal.outpost(map).built {
            continue;
        }
        let kind = royal.outpost_type(map) as usize;
        if !allowed.contains(&kind) {
            errors.push(format!("map {map}: type {kind} is locked"));
        }
        match kind {
            1 => support[RoyalGuardian::world_of(map)] += 1,
            2 => savage[RoyalGuardian::world_of(map)] += 1,
            _ => {}
        }
        if royal.points_left(map) < 0.0 {
            errors.push(format!("map {map}: negative points"));
        }
        for (_, profession) in royal.units(map) {
            if !units.contains(&profession) {
                errors.push(format!("map {map}: unit type {profession} is locked"));
            }
        }
        for connection in royal.outpost_connections(map) {
            match connection.endpoint {
                Endpoint::Empty => {}
                Endpoint::Resource(resource) => {
                    if data::resource(resource).is_none() {
                        errors.push(format!("map {map}: resource endpoint is invalid"));
                    } else if kind == 1 {
                        errors.push(format!(
                            "map {map}: resource endpoint requires a collecting outpost"
                        ));
                    }
                }
                Endpoint::Map(target) => {
                    if !royal.shelf_unlocked(MAP_TOOL) {
                        errors.push(format!("map {map}: map connection tool is locked"));
                    } else if kind != 1 {
                        errors.push(format!(
                            "map {map}: map endpoint requires a support outpost"
                        ));
                    } else if !royal.outpost(target).built {
                        errors.push(format!("map {map}: map endpoint is unbuilt"));
                    } else if target == map {
                        errors.push(format!("map {map}: self-link"));
                    }
                }
            }
        }
    }
    for world in 0..WORLDS {
        if (support[world] as f64) > royal.support_limit() {
            errors.push(format!(
                "world {world}: support count exceeds allowed limit"
            ));
        }
        if (savage[world] as f64) > royal.savage_limit() {
            errors.push(format!("world {world}: savage count exceeds allowed limit"));
        }
    }
    errors
}

/// The edits that take `before` to `after`, in the order a player would make
/// them: the type first, then the levels, professions and connections.
pub fn changes(before: &RoyalGuardian, after: &RoyalGuardian) -> Vec<Move> {
    let mut moves = Vec::new();
    for map in 0..before.maps.len().max(after.maps.len()) {
        if !after.outpost(map).built {
            continue;
        }
        let kind = after.outpost_type(map) as usize;
        if before.outpost_type(map) as usize != kind {
            moves.push(Move::Type { map, kind });
        }
        let levels = [
            after.map_cell(map, 0),
            after.map_cell(map, 1),
            after.map_cell(map, 2),
        ];
        if [0usize, 1, 2]
            .iter()
            .any(|cell| before.map_cell(map, *cell) != levels[*cell])
        {
            moves.push(Move::Levels { map, levels });
        }
        let before_units = before.stationed_units(map);
        for (slot, profession) in after.units(map) {
            let current = before_units
                .iter()
                .find(|(before_slot, _)| *before_slot == slot)
                .map(|(_, profession)| *profession);
            if current != Some(profession) {
                moves.push(Move::Profession {
                    map,
                    slot,
                    profession,
                });
            }
        }
        let saved = before
            .outpost_connections(map)
            .first()
            .map(|connection| connection.endpoint)
            .unwrap_or(Endpoint::Empty);
        let next = after
            .outpost_connections(map)
            .first()
            .map(|connection| connection.endpoint)
            .unwrap_or(Endpoint::Empty);
        if saved != next {
            moves.push(Move::Connection {
                map,
                endpoint: next,
            });
        }
    }
    moves
}

/// Every move the state offers: the upgrades it can buy, the adjacent types it
/// can switch to, the links it can make and the professions it can change.
///
/// Upstream `_candidateMoves` also offers a `resource` move for simulation
/// budgets and an `units` move for pool edits; a plan never needs either, so the
/// port leaves them out.
pub fn candidates(state: &Sandbox) -> Vec<Move> {
    let royal = state.state();
    let allowed = royal.allowed_types();
    let units = royal.unit_types();
    let mut moves = Vec::new();
    for map in 0..royal.maps.len() {
        if !royal.outpost(map).built {
            continue;
        }
        let kind = royal.outpost_type(map) as usize;
        let position = allowed.iter().position(|value| *value == kind);
        for direction in [-1i8, 1] {
            if let Some(position) = position {
                let target = position as i8 + direction;
                if let Some(next) = allowed.get(target as usize)
                    && *next != kind
                {
                    moves.push(Move::Type { map, kind: *next });
                }
            }
        }
        if royal.shelf_unlocked(UPGRADE_TOOL) {
            let points = royal.points_left(map);
            for (upgrade, cost) in UPGRADE_COSTS.iter().enumerate() {
                let unlocked = upgrade != 2 || royal.shelf_unlocked(EDUCATION_TOOL);
                if unlocked && points >= *cost {
                    moves.push(Move::OutpostUpgrade { map, upgrade });
                }
            }
        }
        if royal.shelf_unlocked(RESOURCE_TOOL) && kind != 1 {
            for node in royal.reachable_resources(map) {
                if node.reachable
                    && royal.resource_remaining(node.resource) > 0.0
                    && Endpoint::Resource(node.resource)
                        != royal.outpost_connections(map)[0].endpoint
                {
                    moves.push(Move::Connection {
                        map,
                        endpoint: Endpoint::Resource(node.resource),
                    });
                }
            }
        }
        if kind == 1 && royal.shelf_unlocked(MAP_TOOL) {
            for target in 0..royal.maps.len() {
                if target != map
                    && RoyalGuardian::world_of(target) == RoyalGuardian::world_of(map)
                    && royal.outpost_reachable(map, target)
                    && Endpoint::Map(target) != royal.outpost_connections(map)[0].endpoint
                {
                    moves.push(Move::Connection {
                        map,
                        endpoint: Endpoint::Map(target),
                    });
                }
            }
        }
        if royal.outpost_connections(map)[0].endpoint != Endpoint::Empty {
            moves.push(Move::Connection {
                map,
                endpoint: Endpoint::Empty,
            });
        }
        for (slot, profession) in royal.units(map) {
            for candidate in &units {
                if *candidate != profession {
                    moves.push(Move::Profession {
                        map,
                        slot,
                        profession: *candidate,
                    });
                }
            }
        }
    }
    moves
}

fn apply_type(royal: &mut RoyalGuardian, map: usize, kind: usize, errors: &mut Vec<String>) {
    if !royal.outpost(map).built {
        errors.push(format!("map {map} is not built"));
        return;
    }
    let allowed = royal.allowed_types();
    let Some(position) = allowed
        .iter()
        .position(|value| *value == royal.outpost_type(map) as usize)
    else {
        errors.push("current outpost type is not unlocked".to_owned());
        return;
    };
    let Some(target) = allowed.iter().position(|value| *value == kind) else {
        errors.push("outpost type is locked".to_owned());
        return;
    };
    if target.abs_diff(position) != 1 {
        errors.push("outpost type must change by one adjacent step".to_owned());
        return;
    }
    if kind == 1 || kind == 2 {
        let limit = if kind == 1 {
            royal.support_limit()
        } else {
            royal.savage_limit()
        };
        if limit < 1.0 {
            errors.push("outpost type has no available capacity".to_owned());
            return;
        }
        // Upstream demotes the last outpost of the type the world already
        // holds when the switch would exceed the limit.
        let world = RoyalGuardian::world_of(map);
        let mut count = 0;
        let mut displaced = None;
        for candidate in world * MAPS_PER_WORLD..(world + 1) * MAPS_PER_WORLD {
            if !royal.outpost(candidate).built || royal.outpost_type(candidate) as usize != kind {
                continue;
            }
            count += 1;
            if count as f64 >= limit {
                displaced = Some(candidate);
                break;
            }
        }
        if let Some(displaced) = displaced {
            royal.write_map_cell(displaced, cell::KIND, 0.0);
        }
    }
    royal.write_map_cell(map, cell::KIND, kind as f64);
    royal.write_map_cell(map, cell::LINKS, -1.0);
    royal.write_map_cell(map, cell::LINKS + 1, -1.0);
}

fn apply_connection(
    royal: &mut RoyalGuardian,
    map: usize,
    endpoint: Endpoint,
    errors: &mut Vec<String>,
) {
    if !royal.outpost(map).built {
        errors.push(format!("map {map} is not built"));
        return;
    }
    let kind = royal.outpost_type(map) as usize;
    match endpoint {
        Endpoint::Empty => royal.write_map_cell(map, cell::LINKS, -1.0),
        Endpoint::Resource(resource) => {
            if data::resource(resource).is_none() {
                errors.push(format!("map {map}: resource endpoint is invalid"));
            } else if kind == 1 {
                errors.push(format!(
                    "map {map}: resource endpoint requires a collecting outpost"
                ));
            } else if !royal.shelf_unlocked(RESOURCE_TOOL) {
                errors.push(format!("map {map}: resource connection tool is locked"));
            } else if RoyalGuardian::resource_world(resource) != RoyalGuardian::world_of(map) {
                errors.push(format!("map {map}: resource endpoint is cross-world"));
            } else if !royal.resource_reachable(map, resource) {
                errors.push(format!("map {map}: resource endpoint is out of range"));
            } else {
                royal.write_map_cell(map, cell::LINKS, endpoint.encode() as f64);
            }
        }
        Endpoint::Map(target) => {
            if !royal.shelf_unlocked(MAP_TOOL) {
                errors.push(format!("map {map}: map connection tool is locked"));
            } else if kind != 1 {
                errors.push(format!(
                    "map {map}: map endpoint requires a support outpost"
                ));
            } else if !royal.outpost(target).built {
                errors.push(format!("map {map}: map endpoint is unbuilt"));
            } else if RoyalGuardian::world_of(target) != RoyalGuardian::world_of(map) {
                errors.push(format!("map {map}: map endpoint is cross-world"));
            } else if target == map {
                errors.push(format!("map {map}: map endpoint cannot self-link"));
            } else if !royal.outpost_reachable(map, target) {
                errors.push(format!("map {map}: map endpoint is out of range"));
            } else {
                royal.write_map_cell(map, cell::LINKS, endpoint.encode() as f64);
            }
        }
    }
}

fn apply_profession(
    royal: &mut RoyalGuardian,
    map: usize,
    slot: usize,
    profession: usize,
    errors: &mut Vec<String>,
) {
    if !royal.outpost(map).built {
        errors.push(format!("map {map} is not built"));
        return;
    }
    let packed = royal.map_cell(map, cell::UNITS);
    let digits = RoyalGuardian::packed_digits(packed);
    if slot >= royal.assignable_slots(map) {
        errors.push("profession slot is inactive".to_owned());
        return;
    }
    let Some(digit) = digits.get(slot).copied() else {
        errors.push("profession slot is inactive".to_owned());
        return;
    };
    if !(2..=5).contains(&digit) {
        errors.push("profession slot is unoccupied".to_owned());
        return;
    }
    if !royal.unit_types().contains(&profession) {
        errors.push("profession is locked".to_owned());
        return;
    }
    let packed = RoyalGuardian::write_profession(packed, slot, profession);
    royal.write_map_cell(map, cell::UNITS, packed);
}

fn apply_upgrade(royal: &mut RoyalGuardian, map: usize, upgrade: usize, errors: &mut Vec<String>) {
    if !royal.outpost(map).built {
        errors.push(format!("map {map} is not built"));
        return;
    }
    if upgrade >= 3 {
        errors.push("outpost upgrade kind must be 0, 1, or 2".to_owned());
        return;
    }
    if !royal.shelf_unlocked(UPGRADE_TOOL) {
        errors.push("outpost upgrade tool is locked".to_owned());
        return;
    }
    if upgrade == 2 && !royal.shelf_unlocked(EDUCATION_TOOL) {
        errors.push("third outpost upgrade is locked".to_owned());
        return;
    }
    let cost = UPGRADE_COSTS[upgrade];
    if royal.points_left(map) < cost {
        errors.push("not enough outpost points".to_owned());
        return;
    }
    let level = royal.map_cell(map, upgrade).floor() + 1.0;
    royal.write_map_cell(map, upgrade, level);
    if upgrade == 0 {
        let packed = royal.map_cell(map, cell::UNITS);
        if let Some(slot) = RoyalGuardian::barracks_unlock_slot(packed) {
            let packed = RoyalGuardian::write_profession(packed, slot, 0);
            royal.write_map_cell(map, cell::UNITS, packed);
        }
    }
}

fn apply_levels(royal: &mut RoyalGuardian, map: usize, levels: [f64; 3], errors: &mut Vec<String>) {
    if !royal.outpost(map).built {
        errors.push(format!("map {map} is not built"));
        return;
    }
    if !royal.shelf_unlocked(UPGRADE_TOOL) {
        errors.push("outpost upgrade tool is locked".to_owned());
        return;
    }
    if levels[2] != royal.map_cell(map, 2) && !royal.shelf_unlocked(EDUCATION_TOOL) {
        errors.push("third outpost upgrade is locked".to_owned());
        return;
    }
    for (cell, level) in levels.into_iter().enumerate() {
        royal.write_map_cell(map, cell, level.max(0.0).floor());
    }
}
