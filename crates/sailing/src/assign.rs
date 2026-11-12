//! One captain per boat: the saved pairings and the maximum-weight matching.

use super::{AssignmentMode, Fleet, Goal, RouteContext, RouteMetrics};

/// One fleet on one island, scored by one goal.
#[derive(Clone, Debug)]
pub struct FleetPlan {
    pub island: usize,
    pub goal: Goal,
    pub mode: AssignmentMode,
    pub routes: Vec<RouteMetrics>,
}

impl FleetPlan {
    pub fn generated_chests_per_hour(&self) -> f64 {
        self.routes
            .iter()
            .map(|route| route.generated_chests_per_hour)
            .sum()
    }

    pub fn kept_chests_per_hour(&self) -> f64 {
        self.routes
            .iter()
            .map(|route| route.kept_chests_per_hour)
            .sum()
    }

    pub fn treasure_per_hour(&self) -> f64 {
        self.routes
            .iter()
            .map(|route| route.treasure_per_hour)
            .sum()
    }

    pub fn artifact_finds_per_hour(&self) -> f64 {
        self.routes
            .iter()
            .map(|route| route.artifact_finds_per_hour)
            .sum()
    }

    /// The plan's total score for its goal.
    pub fn score(&self) -> f64 {
        self.routes.iter().map(|route| route.score(self.goal)).sum()
    }
}

/// The saved fleet and the optimizer's fleet on the same island and goal.
#[derive(Clone, Debug)]
pub struct FleetComparison {
    pub saved: FleetPlan,
    pub optimized: FleetPlan,
}

impl FleetComparison {
    /// Percentage gain of the optimized plan over the saved one.
    pub fn gain_pct(&self) -> f64 {
        let before = self.saved.score();
        if before <= 0.0 {
            return 0.0;
        }
        (self.optimized.score() / before - 1.0) * 100.0
    }

    /// Whether the optimizer matched or beat the save, the acceptance check.
    pub fn optimized_wins(&self) -> bool {
        self.optimized.score() + f64::EPSILON >= self.saved.score()
    }
}

/// The save's pairings, keeping only boats whose captain exists.
pub fn saved(fleet: &Fleet, island: usize, goal: Goal, context: &RouteContext) -> FleetPlan {
    let mut routes = Vec::new();
    for (boat, entry) in fleet.boats.iter().enumerate() {
        let Some(captain) = entry.captain.filter(|idx| *idx < fleet.captains.len()) else {
            continue;
        };
        routes.push(super::route_metrics(
            fleet,
            boat,
            island,
            Some(captain),
            context,
        ));
    }
    FleetPlan {
        island,
        goal,
        mode: AssignmentMode::Saved,
        routes,
    }
}

/// The maximum-weight one-captain-per-boat assignment.
pub fn optimize(fleet: &Fleet, island: usize, goal: Goal, context: &RouteContext) -> FleetPlan {
    let boats = fleet.boats.len();
    if boats == 0 {
        return FleetPlan {
            island,
            goal,
            mode: AssignmentMode::Optimized,
            routes: Vec::new(),
        };
    }
    let mut candidates: Vec<Option<usize>> = (0..fleet.captains.len()).map(Some).collect();
    while candidates.len() < boats {
        candidates.push(None);
    }
    let mut weights = vec![vec![0.0; candidates.len()]; boats];
    let mut routes = vec![Vec::with_capacity(candidates.len()); boats];
    for boat in 0..boats {
        for (column, captain) in candidates.iter().enumerate() {
            let route = super::route_metrics(fleet, boat, island, *captain, context);
            weights[boat][column] = route.score(goal);
            routes[boat].push(route);
        }
    }
    let assignment = hungarian_max(&weights);
    let mut plan = Vec::with_capacity(boats);
    for (boat, column) in assignment.into_iter().enumerate() {
        if column < routes[boat].len() {
            plan.push(routes[boat][column].clone());
        }
    }
    plan.sort_by_key(|route| route.boat);
    FleetPlan {
        island,
        goal,
        mode: AssignmentMode::Optimized,
        routes: plan,
    }
}

/// Both plans, so the UI can show current versus optimized.
pub fn compare(
    fleet: &Fleet,
    island: usize,
    goal: Goal,
    context: &RouteContext,
) -> FleetComparison {
    FleetComparison {
        saved: saved(fleet, island, goal, context),
        optimized: optimize(fleet, island, goal, context),
    }
}

/// Maximum-weight rectangular assignment, a port of the upstream Hungarian
/// solver. The result maps each row to a column index.
fn hungarian_max(weights: &[Vec<f64>]) -> Vec<usize> {
    let rows = weights.len();
    if rows == 0 {
        return Vec::new();
    }
    let mut cols = weights.first().map(Vec::len).unwrap_or(0);
    if cols == 0 {
        return Vec::new();
    }
    let mut weights = weights.to_vec();
    while cols < rows {
        for row in &mut weights {
            row.push(0.0);
        }
        cols += 1;
    }
    let max_weight = weights
        .iter()
        .flat_map(|row| row.iter())
        .copied()
        .fold(0.0_f64, f64::max);
    let mut u = vec![0.0; rows + 1];
    let mut v = vec![0.0; cols + 1];
    let mut p = vec![0usize; cols + 1];
    let mut way = vec![0usize; cols + 1];

    for row in 1..=rows {
        p[0] = row;
        let mut j0 = 0usize;
        let mut minv = vec![f64::INFINITY; cols + 1];
        let mut used = vec![false; cols + 1];
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let mut delta = f64::INFINITY;
            let mut j1 = 0usize;
            for col in 1..=cols {
                if used[col] {
                    continue;
                }
                let current = (max_weight - weights[i0 - 1][col - 1]) - u[i0] - v[col];
                if current < minv[col] {
                    minv[col] = current;
                    way[col] = j0;
                }
                if minv[col] < delta {
                    delta = minv[col];
                    j1 = col;
                }
            }
            for update in 0..=cols {
                if used[update] {
                    u[p[update]] += delta;
                    v[update] -= delta;
                } else {
                    minv[update] -= delta;
                }
            }
            j0 = j1;
            if p[j0] == 0 {
                break;
            }
        }
        loop {
            let previous = way[j0];
            p[j0] = p[previous];
            j0 = previous;
            if j0 == 0 {
                break;
            }
        }
    }

    let mut assignment = vec![usize::MAX; rows];
    for column in 1..=cols {
        if p[column] > 0 && p[column] <= rows {
            assignment[p[column] - 1] = column - 1;
        }
    }
    assignment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_matching_picks_the_best_distinct_captains() {
        let weights = vec![
            vec![10.0, 19.0, 8.0],
            vec![10.0, 18.0, 7.0],
            vec![13.0, 16.0, 9.0],
        ];
        let assignment = hungarian_max(&weights);
        let total: f64 = assignment
            .iter()
            .enumerate()
            .map(|(row, column)| weights[row][*column])
            .sum();
        assert_eq!(total, 19.0 + 7.0 + 13.0);
        let mut columns = assignment.clone();
        columns.sort_unstable();
        columns.dedup();
        assert_eq!(columns.len(), 3);
    }

    #[test]
    fn fewer_captains_than_boats_pads_with_empty_seats() {
        let weights = vec![vec![5.0], vec![5.0], vec![5.0]];
        let assignment = hungarian_max(&weights);
        assert_eq!(assignment.len(), 3);
        let mut columns = assignment;
        columns.sort_unstable();
        assert_eq!(columns, [0, 1, 2]);
    }
}
