//! The Decision Tree sandbox: a tree of research events the user can branch,
//! edit and compare.
//!
//! A node holds the whole simulation state a branch reached, the event that got
//! it there and the hours from its parent. Saving a node steps its state to the
//! next research or insight level-up and appends the child that event opens;
//! branching copies the node and steps the copy, so two layouts race from the
//! same point. Ported from upstream `js/dt/dt-sim.js` and
//! `js/dt/decision-tree.js`.

use crate::data::game;
use crate::model::{Effort, Mag, MagKind, SimContext, Simulation};
use crate::optimize::{mags, monos, shapes};
use crate::sim::{self, Event};
use idleon_common::{Control, Progress};

/// Event jumps one step may take before it gives up, as upstream `dtStepSim`.
const MAX_JUMPS: usize = 50_000;
/// Insight level-ups one auto-insight chain may skip before it gives up, as
/// upstream `dtAdvanceAutoInsight`.
const MAX_AUTO_STEPS: usize = 500;

/// One research or insight level-up.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineEntry {
    /// Hours from the chain's starting node.
    pub hours: f64,
    pub obs: usize,
    pub before: u32,
    pub after: u32,
    /// The EXP/hour the re-fitted layout reached at this level-up.
    pub rate: f64,
}

/// The next event a state reaches, with the state it left.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub event: Event,
    /// Hours from the state the step started at.
    pub hours: f64,
    pub state: Simulation,
    /// Observations whose insight level rose on this step.
    pub insight_obs: Vec<usize>,
}

/// One node of a stepped chain: an insight level-up the chain skipped through,
/// or the research level-up it stopped at.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainStep {
    pub event: Event,
    pub hours: f64,
    pub state: Simulation,
    pub insight_obs: Vec<usize>,
}

/// Step `state` to its next research or insight level-up.
///
/// The EXP rate is constant between events, so the step jumps straight to the
/// closest of the two and applies the insight collection that jump earns.
pub fn step(state: &Simulation, ctx: &SimContext) -> Option<Step> {
    let mut state = state.clone();
    let overlay = state.overlay();
    let kal_base = sim::kalei_base(&state.grid, &overlay, ctx);
    let rate = sim::exp_rate(&state, ctx).total;
    let mut hours = 0.0;

    for _ in 0..MAX_JUMPS {
        if rate <= 0.0 {
            return None;
        }
        let required = sim::research_exp_req(state.research_level, ctx.server_var_res_xp);
        let to_level = (required - state.research_exp) / rate;

        let kal_map = sim::kaleidoscope_map(&state.mags, game().observations.len());
        let mut to_insight = f64::INFINITY;
        for obs in 0..game().observations.len() {
            let insight = sim::insight_rate(obs, &state, &overlay, ctx, &kal_map, kal_base);
            if insight <= 0.0 {
                continue;
            }
            let required = sim::insight_exp_required(obs, state.observations.insight_level[obs]);
            let left = (required - state.observations.insight_exp[obs]).max(0.0) / insight;
            if left < to_insight {
                to_insight = left;
            }
        }

        let mut jump = to_level.min(to_insight);
        if jump <= 0.0 {
            jump = 1e-9;
        }
        if jump > 1e8 {
            return None;
        }
        state.research_exp += rate * jump;
        hours += jump;

        let before: Vec<u32> = state.observations.insight_level.clone();
        sim::advance_insight_hours(&mut state, ctx, jump);
        let insight_obs: Vec<usize> = state
            .observations
            .insight_level
            .iter()
            .zip(&before)
            .enumerate()
            .filter(|(_, (now, then))| now > then)
            .map(|(obs, _)| obs)
            .collect();

        let level_before = state.research_level;
        let mut level = state.research_level;
        let mut exp = state.research_exp;
        sim::advance_level(&mut level, &mut exp, ctx.server_var_res_xp);
        state.research_level = level;
        state.research_exp = exp;
        let leveled = state.research_level > level_before;

        if leveled || !insight_obs.is_empty() {
            let event = match (leveled, insight_obs.is_empty()) {
                (true, false) => Event::LevelAndInsight,
                (true, true) => Event::Level,
                (false, _) => Event::Insight,
            };
            return Some(Step {
                event,
                hours,
                state,
                insight_obs,
            });
        }
    }
    None
}

/// Step through every insight level-up, re-fitting the monocles at each, until
/// the next research level-up closes the chain.
///
/// Returns the chain (intermediate insight level-ups included) and the timeline
/// of everything it skipped through. `None` when the chain did not reach a
/// research level-up, or the reporter stopped it.
pub fn auto_chain(
    state: &Simulation,
    ctx: &SimContext,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Option<(Vec<ChainStep>, Vec<TimelineEntry>)> {
    let mut chain = Vec::new();
    let mut timeline = Vec::new();
    let mut current = state.clone();
    let mut total = 0.0;

    for _ in 0..MAX_AUTO_STEPS {
        if report(Progress {
            fraction: (chain.len() as f32 / 8.0).min(0.95),
            message: format!("Skipping insight level-up {}…", chain.len() + 1),
        }) == Control::Stop
        {
            return None;
        }
        let result = step(&current, ctx)?;
        total += result.hours;
        if matches!(result.event, Event::Level | Event::LevelAndInsight) {
            chain.push(ChainStep {
                event: result.event,
                hours: result.hours,
                state: result.state,
                insight_obs: result.insight_obs,
            });
            return Some((chain, timeline));
        }

        let mut state = result.state;
        let changes: Vec<(usize, u32, u32)> = state
            .observations
            .insight_level
            .iter()
            .enumerate()
            .filter_map(|(obs, now)| {
                let then = current.observations.insight(obs);
                (*now > then).then_some((obs, then, *now))
            })
            .collect();
        state.mags = monos::choose_mono_targets(&state, ctx);
        let rate = sim::exp_rate(&state, ctx).total;
        for (obs, before, after) in &changes {
            timeline.push(TimelineEntry {
                hours: total,
                obs: *obs,
                before: *before,
                after: *after,
                rate,
            });
        }
        chain.push(ChainStep {
            event: result.event,
            hours: result.hours,
            state: state.clone(),
            insight_obs: result.insight_obs,
        });
        current = state;
    }
    None
}

/// One branch of the tree.
#[derive(Clone, Debug)]
pub struct Node {
    pub id: u64,
    pub parent: Option<u64>,
    pub children: Vec<u64>,
    /// The event that opened this node.
    pub event: Event,
    /// Hours from the parent to this node.
    pub hours: f64,
    pub state: Simulation,
    /// Skip insight level-ups to the next research level-up when saving.
    pub auto_insight: bool,
    /// Created by an Auto Insight chain: hidden from the tree.
    pub implicit: bool,
    /// Observations whose insight level rose to open this node.
    pub insight_obs: Vec<usize>,
    /// What the auto-insight chain skipped through to reach a level-up here.
    pub timeline: Vec<TimelineEntry>,
}

/// The tree's node collection and the UI state over it.
#[derive(Clone, Debug)]
pub struct Tree {
    nodes: Vec<Node>,
    next_id: u64,
    /// Hide the insight level-up nodes, as the upstream level-ups-only toggle.
    pub level_ups_only: bool,
    /// The compared nodes, in the order they were picked; the first is the
    /// baseline every delta is measured against.
    compare: Vec<u64>,
}

/// A node placed on the tree canvas, in data pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub id: u64,
    pub x: f32,
    pub y: f32,
}

/// The tree canvas: where every visible node sits and how big it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub nodes: Vec<Placed>,
    pub width: f32,
    pub height: f32,
}

/// One node side by side in the comparison panel.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub id: u64,
    pub level: u32,
    pub rate: f64,
    pub hours: f64,
    /// Difference from the baseline node in the compare set.
    pub level_delta: i64,
    pub rate_delta: f64,
    pub hours_delta: f64,
}

/// Canvas constants, as upstream `dt-state.js`.
const NODE_W: f32 = 120.0;
const NODE_H: f32 = 58.0;
const COL_W: f32 = 140.0;
/// How far a time gap may push the next column: the square-root scale keeps a
/// day from dwarfing an hour, and the cap keeps a years-long gap on screen.
const GAP_W: f32 = 180.0;
const ROW_H: f32 = 70.0;
const PAD: f32 = 30.0;

impl Tree {
    /// A tree rooted at the state the page loads: the save, or a run's result.
    pub fn from_state(state: &Simulation) -> Self {
        let root = Node {
            id: 1,
            parent: None,
            children: Vec::new(),
            event: Event::Start,
            hours: 0.0,
            state: state.clone(),
            auto_insight: false,
            implicit: false,
            insight_obs: Vec::new(),
            timeline: Vec::new(),
        };
        Self {
            nodes: vec![root],
            next_id: 2,
            level_ups_only: false,
            compare: Vec::new(),
        }
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn node(&self, id: u64) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn node_mut(&mut self, id: u64) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|node| node.id == id)
    }

    pub fn root(&self) -> Option<&Node> {
        self.nodes.iter().find(|node| node.parent.is_none())
    }

    pub fn children(&self, id: u64) -> Vec<u64> {
        self.node(id)
            .map(|node| node.children.clone())
            .unwrap_or_default()
    }

    /// Hours from the root to a node.
    pub fn total_hours(&self, id: u64) -> f64 {
        let mut total = 0.0;
        let mut current = self.node(id);
        while let Some(node) = current {
            total += node.hours;
            current = node.parent.and_then(|parent| self.node(parent));
        }
        total
    }

    /// The EXP/hour a node's layout earns.
    pub fn rate(&self, id: u64, ctx: &SimContext) -> f64 {
        self.node(id)
            .map(|node| sim::exp_rate(&node.state, ctx).total)
            .unwrap_or(0.0)
    }

    /// Whether the tree draws a node.
    pub fn visible(&self, id: u64) -> bool {
        let Some(node) = self.node(id) else {
            return false;
        };
        if node.implicit {
            return false;
        }
        !self.level_ups_only || node.event != Event::Insight
    }

    /// The closest drawn ancestor of a node.
    pub fn visible_parent(&self, id: u64) -> Option<u64> {
        let mut current = self.node(id)?.parent;
        while let Some(parent) = current {
            if self.visible(parent) {
                return Some(parent);
            }
            current = self.node(parent)?.parent;
        }
        None
    }

    /// A node's drawn children, looking through hidden insight nodes.
    pub fn visible_children(&self, id: u64) -> Vec<u64> {
        let mut visible = Vec::new();
        let mut queue: std::collections::VecDeque<u64> = self.children(id).into();
        while let Some(child) = queue.pop_front() {
            if self.visible(child) {
                visible.push(child);
            } else {
                queue.extend(self.children(child));
            }
        }
        visible
    }

    /// Pick or unpick a node for the comparison panel.
    pub fn toggle_compare(&mut self, id: u64) {
        if let Some(index) = self.compare.iter().position(|picked| *picked == id) {
            self.compare.remove(index);
        } else {
            self.compare.push(id);
        }
    }

    pub fn compared(&self) -> &[u64] {
        &self.compare
    }

    pub fn clear_compare(&mut self) {
        self.compare.clear();
    }

    /// The compared nodes, the first one's figures repeated as the baseline.
    pub fn comparison(&self, ctx: &SimContext) -> Vec<Comparison> {
        let picked: Vec<&Node> = self
            .compare
            .iter()
            .filter_map(|id| self.node(*id))
            .collect();
        let Some(baseline) = picked.first() else {
            return Vec::new();
        };
        let base_rate = sim::exp_rate(&baseline.state, ctx).total;
        let base_hours = self.total_hours(baseline.id);
        let base_level = baseline.state.research_level;
        picked
            .into_iter()
            .map(|node| {
                let rate = sim::exp_rate(&node.state, ctx).total;
                let hours = self.total_hours(node.id);
                Comparison {
                    id: node.id,
                    level: node.state.research_level,
                    rate,
                    hours,
                    level_delta: i64::from(node.state.research_level) - i64::from(base_level),
                    rate_delta: rate - base_rate,
                    hours_delta: hours - base_hours,
                }
            })
            .collect()
    }

    /// What a node changed from its parent, as the sentences a card shows.
    pub fn changes(&self, id: u64) -> Vec<String> {
        let Some(node) = self.node(id) else {
            return Vec::new();
        };
        let Some(parent) = node.parent.and_then(|parent| self.node(parent)) else {
            return Vec::new();
        };
        let mut changes = Vec::new();
        let mut grid = Vec::new();
        for (cell, level) in parent.state.grid.levels().iter().enumerate() {
            let now = node.state.grid.level(cell);
            if now != *level
                && let Some(info) = game().node_at(cell)
            {
                grid.push(format!(
                    "{} {:+.0}",
                    info.name.replace('_', " "),
                    f64::from(now) - f64::from(*level)
                ));
            }
        }
        if !grid.is_empty() {
            changes.push(format!("Grid: {}", grid.join(", ")));
        }
        let moved = parent
            .state
            .placements
            .iter()
            .zip(&node.state.placements)
            .filter(|(before, now)| {
                before.shape != now.shape
                    || before.x != now.x
                    || before.y != now.y
                    || before.rot != now.rot
            })
            .count()
            + parent
                .state
                .placements
                .len()
                .abs_diff(node.state.placements.len());
        if moved > 0 {
            changes.push(format!("{moved} shape{} moved", plural(moved)));
        }
        let mags = parent
            .state
            .mags
            .iter()
            .zip(&node.state.mags)
            .filter(|(before, now)| before.kind != now.kind || before.slot != now.slot)
            .count()
            + parent.state.mags.len().abs_diff(node.state.mags.len());
        if mags > 0 {
            changes.push(format!("{mags} magnifier{} changed", plural(mags)));
        }
        changes
    }

    /// Where every visible node sits, in the upstream time-column and
    /// depth-first row layout. Time gaps grow with the square root of the hours
    /// they cover, so a day does not dwarf an hour.
    pub fn layout(&self) -> Layout {
        let Some(root) = self.root() else {
            return Layout::default();
        };
        let visible: Vec<u64> = self
            .nodes
            .iter()
            .filter(|node| self.visible(node.id))
            .map(|node| node.id)
            .collect();
        if visible.is_empty() {
            return Layout::default();
        }
        let mut times: Vec<f64> = visible.iter().map(|id| self.total_hours(*id)).collect();
        times.sort_by(f64::total_cmp);
        times.dedup_by(|left, right| (*left - *right).abs() < 1e-9);

        let mut columns = Vec::with_capacity(times.len());
        let mut x = 0.0;
        for (index, time) in times.iter().enumerate() {
            columns.push((*time, x));
            if let Some(next) = times.get(index + 1) {
                let gap = next - time;
                x += if gap < 0.01 {
                    COL_W
                } else {
                    COL_W + ((gap.abs().sqrt() as f32) * 50.0).min(GAP_W)
                };
            }
        }

        let mut placed = Vec::with_capacity(visible.len());
        let mut row = 0.0;
        self.place(root.id, &columns, &mut row, &mut placed);
        let width = placed
            .iter()
            .map(|node| node.x + NODE_W + PAD * 2.0)
            .fold(0.0_f32, f32::max);
        let height = placed
            .iter()
            .map(|node| node.y + NODE_H + PAD * 2.0)
            .fold(0.0_f32, f32::max);
        Layout {
            nodes: placed,
            width,
            height,
        }
    }

    fn place(&self, id: u64, columns: &[(f64, f32)], row: &mut f32, out: &mut Vec<Placed>) {
        if !self.visible(id) {
            for child in self.children(id) {
                self.place(child, columns, row, out);
            }
            return;
        }
        let time = self.total_hours(id);
        let x = PAD
            + columns
                .iter()
                .find(|(column, _)| (column - time).abs() < 1e-9)
                .map(|(_, x)| *x)
                .unwrap_or(0.0);
        let children = self.visible_children(id);
        let y = if children.is_empty() {
            let y = PAD + *row * ROW_H;
            *row += 1.0;
            y
        } else {
            let start = out.len();
            for child in children {
                self.place(child, columns, row, out);
            }
            let placed: Vec<&Placed> = out.iter().skip(start).collect();
            let min = placed.iter().map(|node| node.y).fold(f32::MAX, f32::min);
            let max = placed.iter().map(|node| node.y).fold(f32::MIN, f32::max);
            (min + max) / 2.0
        };
        out.push(Placed { id, x, y });
    }

    /// Append a child that `event` opened from `parent`.
    fn push(&mut self, parent: u64, event: Event, hours: f64, state: Simulation) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(Node {
            id,
            parent: Some(parent),
            children: Vec::new(),
            event,
            hours,
            state,
            auto_insight: false,
            implicit: false,
            insight_obs: Vec::new(),
            timeline: Vec::new(),
        });
        if let Some(node) = self.node_mut(parent) {
            node.children.push(id);
        }
        id
    }

    /// Step `id` to its next event and append the child; returns that child.
    ///
    /// With Auto Insight on, the insight level-ups the chain skips through are
    /// added as hidden nodes and the visible child is the research level-up.
    pub fn advance(
        &mut self,
        id: u64,
        ctx: &SimContext,
        report: &mut dyn FnMut(Progress) -> Control,
    ) -> Option<u64> {
        let node = self.node(id)?;
        let state = node.state.clone();
        let auto = node.auto_insight;
        if auto {
            let (chain, timeline) = auto_chain(&state, ctx, report)?;
            let mut parent = id;
            let count = chain.len();
            let mut last = None;
            for (index, step) in chain.into_iter().enumerate() {
                let child = self.push(parent, step.event, step.hours, step.state);
                if let Some(node) = self.node_mut(child) {
                    node.insight_obs = step.insight_obs;
                    node.implicit = index + 1 < count;
                    node.auto_insight = index + 1 == count;
                }
                if index + 1 == count
                    && let Some(node) = self.node_mut(child)
                {
                    node.timeline = timeline.clone();
                }
                parent = child;
                last = Some(child);
            }
            last
        } else {
            let result = step(&state, ctx)?;
            let child = self.push(id, result.event, result.hours, result.state);
            if let Some(node) = self.node_mut(child) {
                node.insight_obs = result.insight_obs;
            }
            Some(child)
        }
    }

    /// Save the node: advance it, or re-simulate the children it already has.
    /// Returns the node to select afterwards.
    pub fn save(
        &mut self,
        id: u64,
        ctx: &SimContext,
        report: &mut dyn FnMut(Progress) -> Control,
    ) -> Option<u64> {
        if self.children(id).is_empty() {
            return self.advance(id, ctx, report);
        }
        self.resim_children(id, ctx, report)?;
        Some(id)
    }

    /// Fork a node: copy it with its current layout, step the copy to its next
    /// event and return the new event's node.
    pub fn branch(
        &mut self,
        id: u64,
        ctx: &SimContext,
        report: &mut dyn FnMut(Progress) -> Control,
    ) -> Option<u64> {
        let node = self.node(id)?;
        let parent = node.parent?;
        let state = node.state.clone();
        let event = node.event;
        let hours = node.hours;
        let auto = node.auto_insight;
        let sibling = self.push(parent, event, hours, state);
        if let Some(node) = self.node_mut(sibling) {
            node.auto_insight = auto;
        }
        self.advance(sibling, ctx, report)
    }

    /// Delete a node and everything under it; returns the parent to select.
    pub fn delete(&mut self, id: u64) -> Option<u64> {
        let node = self.node(id)?;
        let parent = node.parent?;
        let mut doomed = vec![id];
        let mut index = 0;
        while index < doomed.len() {
            doomed.extend(self.children(doomed[index]));
            index += 1;
        }
        if let Some(node) = self.node_mut(parent) {
            node.children.retain(|child| !doomed.contains(child));
        }
        self.nodes.retain(|node| !doomed.contains(&node.id));
        self.compare.retain(|picked| !doomed.contains(picked));
        Some(parent)
    }

    /// Re-run a node's children from its current state.
    ///
    /// An auto-insight child is a hidden chain ending at a visible level-up: it
    /// is deleted and re-advanced, and the level-up's own children move to the
    /// new tail.
    fn resim_children(
        &mut self,
        id: u64,
        ctx: &SimContext,
        report: &mut dyn FnMut(Progress) -> Control,
    ) -> Option<()> {
        let node = self.node(id)?;
        let state = node.state.clone();
        for child in self.children(id) {
            let implicit = self.node(child).is_some_and(|node| node.implicit);
            if implicit {
                let mut chain = vec![child];
                let mut tail = child;
                let mut grandchildren = Vec::new();
                while let Some(node) = self.node(tail) {
                    if !node.implicit {
                        grandchildren = node.children.clone();
                        break;
                    }
                    match node.children.first() {
                        Some(next) => {
                            chain.push(*next);
                            tail = *next;
                        }
                        None => break,
                    }
                }
                if let Some(node) = self.node_mut(id) {
                    node.children.retain(|child| !chain.contains(child));
                }
                self.nodes.retain(|node| !chain.contains(&node.id));
                self.compare.retain(|picked| !chain.contains(picked));
                let Some(new_tail) = self.advance(id, ctx, report) else {
                    // The chain cannot be rebuilt; the parent keeps its state and
                    // the children the chain carried are promoted back under it.
                    for grandchild in grandchildren {
                        if let Some(node) = self.node_mut(grandchild) {
                            node.parent = Some(id);
                        }
                        if let Some(node) = self.node_mut(id) {
                            node.children.push(grandchild);
                        }
                        self.resim_children(grandchild, ctx, report);
                    }
                    continue;
                };
                if let Some(target) = self.node_mut(new_tail) {
                    target.children = grandchildren.clone();
                }
                for grandchild in grandchildren {
                    if let Some(node) = self.node_mut(grandchild) {
                        node.parent = Some(new_tail);
                    }
                    self.resim_children(grandchild, ctx, report);
                }
                continue;
            }
            let result = step(&state, ctx)?;
            if let Some(node) = self.node_mut(child) {
                node.event = result.event;
                node.hours = result.hours;
                node.state = result.state;
                node.insight_obs = result.insight_obs;
            }
            self.resim_children(child, ctx, report);
        }
        Some(())
    }

    /// Spend one grid level on a node.
    pub fn bump_grid(&mut self, id: u64, cell: usize, delta: i64) -> bool {
        let Some(node) = self.node_mut(id) else {
            return false;
        };
        let Some(info) = game().node_at(cell) else {
            return false;
        };
        let level = i64::from(node.state.grid.level(cell));
        let next = (level + delta).clamp(0, i64::from(info.max_lv)) as u32;
        if next == node.state.grid.level(cell) {
            return false;
        }
        node.state.grid.set(cell, next);
        true
    }

    /// Assign one owned magnifier of `kind` to an observation on a node.
    pub fn assign_mag(&mut self, id: u64, kind: MagKind, obs: usize, ctx: &SimContext) -> bool {
        self.node_mut(id)
            .is_some_and(|node| sim::place_mag(&mut node.state, ctx, kind, obs))
    }

    /// Lift every magnifier off an observation on a node.
    pub fn lift_mag(&mut self, id: u64, obs: usize) -> bool {
        self.node_mut(id)
            .is_some_and(|node| sim::lift_mag(&mut node.state, obs))
    }

    /// Flip a node's Auto Insight flag.
    pub fn toggle_auto_insight(&mut self, id: u64) -> bool {
        match self.node_mut(id) {
            Some(node) => {
                node.auto_insight = !node.auto_insight;
                true
            }
            None => false,
        }
    }

    /// The upstream Auto Mags action: place glasses and kaleidoscopes for the
    /// EXP rate, then spread the monocles with [`monos::choose_mono_targets`].
    pub fn auto_mags(&mut self, id: u64, ctx: &SimContext) -> usize {
        let Some(node) = self.node_mut(id) else {
            return 0;
        };
        let monocles = node
            .state
            .mags
            .iter()
            .filter(|mag| mag.kind == MagKind::Monocle)
            .count();
        let placed = mags::optimize_exp(&node.state, ctx, Effort::Quick).mags;
        let mut pool: Vec<Mag> = placed
            .into_iter()
            .filter(|mag| mag.kind != MagKind::Monocle)
            .collect();
        pool.extend(std::iter::repeat_n(
            Mag::new(MagKind::Monocle, None),
            monocles,
        ));
        let mut trial = node.state.clone();
        trial.mags = pool;
        node.state.mags = monos::choose_mono_targets(&trial, ctx);
        node.state
            .mags
            .iter()
            .filter(|mag| mag.slot.is_some())
            .count()
    }

    /// The upstream Auto Shapes action: place the owned shapes for the rate.
    pub fn auto_shapes(
        &mut self,
        id: u64,
        ctx: &SimContext,
        report: &mut dyn FnMut(Progress) -> Control,
    ) -> Option<()> {
        let node = self.node(id)?;
        let outcome = shapes::optimize(&node.state, ctx, Effort::Quick, report);
        let node = self.node_mut(id)?;
        node.state.placements = outcome.placements;
        node.state.saved_overlay = None;
        Some(())
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GridState, Observations};

    fn state() -> (Simulation, SimContext) {
        let mut simulation = Simulation {
            research_level: 60,
            grid: GridState::seeded(1),
            observations: Observations::all_at(0),
            ..Simulation::default()
        };
        simulation.grid.set(31, 5);
        simulation.grid.set(93, 2);
        let obs = (0..game().observations.len())
            .find(|obs| sim::observation_usable(*obs, 60, &simulation.observations))
            .expect("a usable observation");
        simulation.mags = vec![
            Mag::new(MagKind::Regular, Some(obs)),
            Mag::new(MagKind::Monocle, Some(obs)),
        ];
        (simulation, SimContext::default())
    }

    fn no_report() -> impl FnMut(Progress) -> Control {
        |_| Control::Continue
    }

    #[test]
    fn saving_a_node_steps_to_the_next_event() {
        let (state, ctx) = state();
        let mut tree = Tree::from_state(&state);
        let mut report = no_report();
        let child = tree.advance(1, &ctx, &mut report).expect("an event");
        assert_eq!(tree.total_hours(child), tree.node(child).unwrap().hours);
        assert_ne!(tree.node(child).unwrap().event, Event::Start);
        assert_eq!(tree.children(1), vec![child]);
        assert_eq!(tree.visible_parent(child), Some(1));
    }

    #[test]
    fn auto_insight_stops_at_a_research_level_up_and_hides_the_insight_nodes() {
        let (mut state, ctx) = state();
        // One hour short of the next level, so the chain must close soon.
        state.research_exp = sim::research_exp_req(60, ctx.server_var_res_xp) * 0.999;
        let mut tree = Tree::from_state(&state);
        tree.toggle_auto_insight(1);
        let mut report = no_report();
        let tail = tree.advance(1, &ctx, &mut report).expect("a level-up");
        let node = tree.node(tail).expect("the tail node");
        assert!(matches!(node.event, Event::Level | Event::LevelAndInsight));
        assert!(node.state.research_level > 60);
        assert!(!node.implicit && tree.visible(tail));
        let hidden = tree.nodes().iter().filter(|node| node.implicit).count();
        assert!(hidden > 0, "the chain skipped no insight level-up");
        assert_eq!(tree.visible_children(1), vec![tail]);
    }

    #[test]
    fn branching_copies_the_layout_and_steps_the_copy() {
        let (state, ctx) = state();
        let mut tree = Tree::from_state(&state);
        let mut report = no_report();
        let first = tree.advance(1, &ctx, &mut report).expect("an event");
        let based = tree.node(first).unwrap().state.clone();
        let forked = tree.branch(first, &ctx, &mut report).expect("a fork");
        assert_eq!(tree.children(1).len(), 2, "the fork is a sibling branch");
        let sibling = tree
            .children(1)
            .into_iter()
            .find(|child| *child != first)
            .expect("the copied node");
        assert_eq!(
            tree.node(sibling).unwrap().state,
            based,
            "the copy starts from the node's layout"
        );
        assert!(
            tree.visible(forked),
            "the forked event opens on its own node"
        );
        tree.delete(sibling).expect("the fork deletes");
        assert_eq!(tree.children(1), vec![first]);
        assert!(tree.node(forked).is_none(), "the fork's event went with it");
    }

    #[test]
    fn a_level_up_only_tree_hides_insight_events_but_keeps_the_path() {
        let (state, ctx) = state();
        let mut tree = Tree::from_state(&state);
        let mut report = no_report();
        let insight = tree.advance(1, &ctx, &mut report).expect("an event");
        assert_eq!(tree.node(insight).unwrap().event, Event::Insight);
        tree.level_ups_only = true;
        assert!(!tree.visible(insight));
        // The next research level, with no monocle left to interrupt it, so the
        // hidden insight node leads straight to a level-up.
        let required = sim::research_exp_req(60, ctx.server_var_res_xp);
        {
            let node = tree.node_mut(insight).unwrap();
            node.state.research_exp = required * 0.999;
            node.state.mags.retain(|mag| mag.kind != MagKind::Monocle);
        }
        let below = tree.advance(insight, &ctx, &mut report).expect("an event");
        assert_ne!(tree.node(below).unwrap().event, Event::Insight);
        assert!(tree.visible(below));
        assert_eq!(tree.visible_parent(below), Some(1));
    }

    #[test]
    fn comparison_measures_every_node_against_the_first_pick() {
        let (state, ctx) = state();
        let mut tree = Tree::from_state(&state);
        let mut report = no_report();
        let child = tree.advance(1, &ctx, &mut report).expect("an event");
        tree.toggle_compare(1);
        tree.toggle_compare(child);
        let rows = tree.comparison(&ctx);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].hours_delta, 0.0);
        assert!((rows[1].hours_delta - tree.total_hours(child)).abs() < 1e-9);
        assert_eq!(
            rows[1].level_delta,
            i64::from(tree.node(child).unwrap().state.research_level) - 60
        );
    }

    #[test]
    fn the_layout_orders_nodes_by_time_and_rows_by_branch() {
        let (state, ctx) = state();
        let mut tree = Tree::from_state(&state);
        let mut report = no_report();
        let first = tree.advance(1, &ctx, &mut report).expect("an event");
        let _ = tree.branch(first, &ctx, &mut report).expect("a fork");
        let layout = tree.layout();
        assert_eq!(layout.nodes.len(), 4);
        let root = layout.nodes.iter().find(|node| node.id == 1).unwrap();
        assert!(layout.nodes.iter().all(|node| node.x >= root.x));
        assert!(layout.width > 0.0 && layout.height > 0.0);
    }

    #[test]
    fn an_edit_reports_against_its_parent() {
        let (state, ctx) = state();
        let mut tree = Tree::from_state(&state);
        let mut report = no_report();
        let child = tree.advance(1, &ctx, &mut report).expect("an event");
        assert!(tree.changes(child).is_empty());
        tree.bump_grid(child, 31, -1);
        let changes = tree.changes(child);
        assert!(
            changes.iter().any(|change| change.starts_with("Grid:")),
            "no grid change reported: {changes:?}"
        );
    }

    #[test]
    fn auto_mags_and_shapes_repoint_every_magnifier_and_placement() {
        let (state, ctx) = state();
        let mut tree = Tree::from_state(&state);
        assert!(tree.auto_mags(1, &ctx) > 0);
        let mut report = no_report();
        assert!(tree.auto_shapes(1, &ctx, &mut report).is_some());
        assert!(!tree.node(1).unwrap().state.mags.is_empty());
    }
}
