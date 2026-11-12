//! The Jelly Operator board: the saved layout, its slots and the placement rules.
//!
//! Ports `jellyLayoutFromSave`, `jellyFootprintSlots`, `jellyLayoutOccupancy`,
//! `jellyValidateLayout`, `jellyPlaceCell`, `jellyRemoveCell`,
//! `jellyUnlockedSlots`, `jellySlotPurchasesLeft` and `jellyLayoutKey` from
//! `js/stats/systems/w7/jelly-operator.js`.
//!
//! The layout is an anchor-to-type map, like the upstream object: a cell covers
//! its anchor plus the type's footprint offsets, so a slot never holds more than
//! one cell and a footprint that runs off the board is rejected.

use super::{Jelly, data};
use std::collections::BTreeMap;

/// The board's layout: cell anchors to cell types, in slot order.
pub type Layout = BTreeMap<usize, usize>;

/// `jellyLayoutFromSave`: the saved `Research[14]` row as a layout.
pub fn layout_from_save(grid: &[Option<f64>]) -> Layout {
    let mut layout = Layout::new();
    for (anchor, raw) in grid.iter().enumerate() {
        let Some(value) = raw else { continue };
        if value.is_finite() && value.fract() == 0.0 && (0.0..=8.0).contains(value) {
            layout.insert(anchor, *value as usize);
        }
    }
    layout
}

/// `jellyFootprintSlots`: every slot a cell covers, or `None` when it runs off
/// the board or wraps around a row edge.
pub fn footprint(anchor: usize, cell_type: usize) -> Option<Vec<usize>> {
    let data = data();
    let size = data.size();
    let cols = data.cols;
    let mut slots = Vec::new();
    for offset in data.footprint(cell_type) {
        let slot = anchor as i64 + i64::from(*offset);
        if slot < 0 || slot >= size as i64 {
            return None;
        }
        if *offset > -3 && *offset < 3 {
            // JS `%` keeps the sign, so a left offset subtracts its column.
            let column = (anchor % cols) as i64 + i64::from(*offset) % cols as i64;
            if !(0..cols as i64).contains(&column) {
                return None;
            }
        }
        slots.push(slot as usize);
    }
    Some(slots)
}

/// `normalizeSimulatorLayout`'s re-place: every cell in anchor order, dropping
/// the ones the sandbox's unlocked slots no longer fit. Returns the layout and
/// how many cells were dropped.
pub fn normalize(jelly: &Jelly, layout: &Layout) -> (Layout, usize) {
    let mut next = Layout::new();
    let mut removed = 0;
    for (anchor, cell_type) in layout {
        match place(jelly, &next, *anchor, *cell_type) {
            Ok(placed) => next = placed,
            Err(_) => removed += 1,
        }
    }
    (next, removed)
}

/// `jellyLayoutOccupancy`: which cell covers each slot.
pub fn occupancy(layout: &Layout) -> BTreeMap<usize, (usize, usize)> {
    let mut occupied = BTreeMap::new();
    for (anchor, cell_type) in layout {
        let Some(slots) = footprint(*anchor, *cell_type) else {
            continue;
        };
        for slot in slots {
            occupied.insert(slot, (*anchor, *cell_type));
        }
    }
    occupied
}

/// `jellyUnlockedSlots`: the starting slots, the two clears and every plot.
pub fn unlocked_slots(jelly: &Jelly) -> Vec<bool> {
    let data = data();
    let mut unlocked = vec![false; data.size()];
    let mut unlock = |slot: usize| {
        if let Some(cell) = unlocked.get_mut(slot) {
            *cell = true;
        }
    };
    for slot in super::STARTING_SLOTS {
        unlock(slot);
    }
    if jelly.progress.obstruction > 0 {
        for slot in super::FIRST_CLEAR_SLOTS {
            unlock(slot);
        }
    }
    if jelly.progress.obstruction > 1 {
        for slot in super::SECOND_CLEAR_SLOTS {
            unlock(slot);
        }
    }
    for plot_id in &jelly.plots {
        let Some(plot) = data.plot(*plot_id) else {
            continue;
        };
        for y in 0..plot.height {
            for x in 0..plot.width {
                unlock(plot.start + x + data.cols * y);
            }
        }
    }
    for slot in &data.obstruction_slots {
        if let Some(cell) = unlocked.get_mut(*slot) {
            *cell = false;
        }
    }
    unlocked
}

/// How many slots the save has unlocked.
pub fn unlocked_count(jelly: &Jelly) -> usize {
    unlocked_slots(jelly).iter().filter(|slot| **slot).count()
}

/// `jellySlotPurchasesLeft`: the plot tokens still unbought.
pub fn slot_purchases_left(jelly: &Jelly) -> f64 {
    let bundle = if jelly.account.bundle_ban_j { 1.0 } else { 0.0 };
    let completion = jelly.completion_bonus(44);
    let granted = bundle + (jelly.legend_slot_tokens() + 3.0 * jelly.account.event_shop_57);
    (jelly.upgrade_quantity(9) + (jelly.upgrade_quantity(8) + (completion + granted))
        - jelly.plots.len() as f64)
        .round()
}

/// `jellyValidateLayout`: the upstream rules, as the reason a layout is refused.
pub fn validate(jelly: &Jelly, layout: &Layout) -> Result<(), String> {
    let data = data();
    let unlocked = unlocked_slots(jelly);
    let units_owned = jelly.units_owned();
    let mut occupied = vec![false; data.size()];
    let mut viruses = 0;
    for (anchor, cell_type) in layout {
        if *anchor >= data.size() {
            return Err(format!("Invalid anchor {anchor}"));
        }
        if *cell_type >= units_owned {
            return Err(format!("Cell {cell_type} is locked"));
        }
        let Some(slots) = footprint(*anchor, *cell_type) else {
            return Err(format!(
                "{} wraps outside the grid",
                data.cell_name(*cell_type)
            ));
        };
        for slot in slots {
            if !unlocked.get(slot).copied().unwrap_or(false) {
                return Err(format!("Slot {slot} is locked"));
            }
            if occupied[slot] {
                return Err(format!("Cells overlap at slot {slot}"));
            }
            occupied[slot] = true;
        }
        if *cell_type == 5 {
            viruses += 1;
        }
    }
    if f64::from(viruses) > 1.0 + jelly.upgrade_quantity(15) {
        return Err("Virus limit exceeded".to_string());
    }
    Ok(())
}

/// `jellyPlaceCell`: the layout with `cell_type` anchored at `anchor`, or why
/// the placement is refused.
pub fn place(
    jelly: &Jelly,
    layout: &Layout,
    anchor: usize,
    cell_type: usize,
) -> Result<Layout, String> {
    if cell_type >= jelly.units_owned() {
        return Err(format!("Cell {cell_type} is locked"));
    }
    let Some(slots) = footprint(anchor, cell_type) else {
        return Err(format!(
            "{} wraps outside the grid",
            data().cell_name(cell_type)
        ));
    };
    let unlocked = unlocked_slots(jelly);
    if let Some(locked) = slots
        .iter()
        .find(|slot| !unlocked.get(**slot).copied().unwrap_or(false))
    {
        return Err(format!("Slot {locked} is locked"));
    }
    let mut next = layout.clone();
    let target: Vec<usize> = slots;
    next.retain(|other_anchor, other_type| {
        let other = footprint(*other_anchor, *other_type).unwrap_or_default();
        !other.iter().any(|slot| target.contains(slot))
    });
    next.insert(anchor, cell_type);
    validate(jelly, &next)?;
    Ok(next)
}

/// `jellyRemoveCell`: the layout without the cell anchored at `anchor`.
pub fn remove(layout: &Layout, anchor: usize) -> Layout {
    let mut next = layout.clone();
    next.remove(&anchor);
    next
}
