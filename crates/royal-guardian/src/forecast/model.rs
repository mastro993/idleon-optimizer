//! The state one forecast boundary reads: the drain links, the EXP models and
//! the events a collect would commit.

use super::*;

/// One resource node inside the forecast's model: upstream `_model` node.
pub(crate) struct ModelNode {
    pub(crate) resource: usize,
    pub(crate) quantity: f64,
    pub(crate) capacity: f64,
    pub(crate) grade: f64,
    pub(crate) drain_rate: f64,
    pub(crate) maps: Vec<usize>,
}

/// The state one boundary reads: upstream `_model`.
pub(crate) struct Model {
    pub(crate) nodes: Vec<ModelNode>,
    pub(crate) models: Vec<crate::collect_exp::ExpModel>,
    pub(crate) clears: Vec<crate::collect_exp::ClearModel>,
}

pub(crate) fn model_of(royal: &RoyalGuardian) -> Model {
    let exp = royal.exp_models();
    let (streams, _) = royal.resource_streams();
    let mut nodes: Vec<ModelNode> = Vec::new();
    for stream in &streams {
        let index = nodes
            .iter()
            .position(|node| node.resource == stream.resource);
        let node = match index {
            Some(index) => &mut nodes[index],
            None => {
                nodes.push(ModelNode {
                    resource: stream.resource,
                    quantity: royal.resource_raw_progress(stream.resource),
                    capacity: stream.capacity,
                    grade: royal.resource_grade(stream.resource),
                    drain_rate: 0.0,
                    maps: Vec::new(),
                });
                nodes.last_mut().expect("just pushed")
            }
        };
        node.drain_rate += stream.drain_rate;
        if !node.maps.contains(&stream.map) {
            node.maps.push(stream.map);
        }
    }
    Model {
        nodes,
        models: exp.models,
        clears: exp.clears,
    }
}

/// The hour a node fills, or `None` when it is drained or does not fill.
pub(crate) fn fill_hours(node: &ModelNode) -> Option<f64> {
    if node.quantity <= -1.0 || node.drain_rate <= 0.0 {
        return None;
    }
    Some(((node.capacity - node.quantity) / node.drain_rate).max(0.0))
}

/// What a collect of `banked` hours would credit each currency slot:
/// upstream `_gains`.
pub(crate) fn gains(royal: &RoyalGuardian, model: &Model, banked: f64) -> BTreeMap<i64, f64> {
    let mut quantities = vec![0.0; data::resource_count()];
    for node in &model.nodes {
        if let Some(slot) = quantities.get_mut(node.resource) {
            *slot = node.quantity;
        }
    }
    let mut currencies = vec![0.0; 80];
    let mut gain = BTreeMap::new();
    royal.apply_streams(&mut quantities, &mut currencies, banked, &mut gain);
    gain
}

/// The balances the save holds, by currency slot.
pub(crate) fn balances(royal: &RoyalGuardian) -> Vec<f64> {
    royal
        .g
        .get(row::CURRENCIES)
        .and_then(Value::as_array)
        .map(|row| {
            row.iter()
                .map(|cell| json::number(cell).unwrap_or(0.0))
                .collect()
        })
        .unwrap_or_default()
}

/// The order a saved rank-up still has to reach, if any: upstream `_rankGhosts`.
pub(crate) fn rank_ghosts(
    model: &crate::collect_exp::ExpModel,
    start: f64,
    from: f64,
    to: f64,
) -> Vec<RankGhost> {
    let mut ghosts = Vec::new();
    if to <= EPSILON {
        return ghosts;
    }
    let rates = model.rates();
    let at_start = model.preview(from).ranks;
    let at_end = model.preview(to).ranks;
    for bar in 0..5 {
        if rates[bar] <= 0.0 && at_end[bar] == model.ranks[bar] {
            continue;
        }
        if at_start[bar] > model.ranks[bar] {
            ghosts.push(RankGhost {
                map: model.map,
                bar,
                rank: at_start[bar],
                from: model.ranks[bar],
                at: start,
                ready: true,
            });
        }
        let mut rank = at_start[bar] + 1.0;
        while rank <= at_end[bar].min(at_start[bar] + RANK_GHOSTS) {
            let mut low = from;
            let mut high = to;
            while high - low > GHOST_TOLERANCE {
                let middle = (low + high) / 2.0;
                if model.preview(middle).ranks[bar] >= rank {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            ghosts.push(RankGhost {
                map: model.map,
                bar,
                rank,
                from: rank - 1.0,
                at: start + (high - from),
                ready: false,
            });
            rank += 1.0;
        }
    }
    ghosts
}

/// The Armory shelves a collect could make affordable: upstream
/// `_armoryCandidates`.
pub(crate) fn armory_candidates(royal: &RoyalGuardian, counter: f64) -> Vec<Affordable> {
    let unlocked = royal.armory_unlocked_count();
    let mut list = Vec::new();
    for order in 0..unlocked {
        let Some(index) = data::armory_order().get(order).map(|index| *index as usize) else {
            continue;
        };
        let Some(shelf) = data::armory(index) else {
            continue;
        };
        if royal.armory_level(index) >= shelf.max_level {
            continue;
        }
        let cost = royal.armory_upgrade_cost_at(order, royal.armory_level(index), counter);
        if !cost.value.is_finite() || cost.currency_slot < 0 {
            continue;
        }
        list.push(Affordable {
            index,
            order,
            name: shelf.name.as_str(),
            level: royal.armory_level(index),
            cost: cost.value,
            currency_slot: cost.currency_slot,
            at: 0.0,
        });
    }
    list
}
