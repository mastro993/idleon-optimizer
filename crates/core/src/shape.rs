//! Shape polygons and the cell-coverage lattice.
//!
//! The game places a shape anywhere on a pixel canvas and marks every grid
//! cell whose corner falls inside the polygon. Cells are 30×30 with their
//! centres on a fixed lattice, so coverage repeats every 30 px in x and y; the
//! lookup table here encodes that periodicity once and then answers a placement
//! in a handful of additions.

use crate::data::{game, ShapeDef};
use crate::model::Placement;
use std::sync::OnceLock;

/// Pixels per grid cell.
pub const CELL: f64 = 30.0;
/// First cell's centre, in grid pixels.
pub const ORIGIN: [f64; 2] = [15.0, 24.0];
/// Rotation steps the game allows, in degrees.
pub const ROT_STEP: f64 = 5.0;
/// Pixel lattice the optimizer snaps placements to, keeping coverage exact.
pub const PHASE_STEP: i32 = 5;
/// Distinct pixel phases per axis.
pub const PHASES: i32 = (CELL as i32) / PHASE_STEP;
/// Number of rotation steps in a full turn.
pub const ROT_STEPS: usize = 72;

/// The shape's polygon for a placement, in grid pixels.
pub fn polygon_at(shape: &ShapeDef, x: f64, y: f64, rot: f64) -> Vec<[f64; 2]> {
    let (cx, cy) = (shape.dims[0] / 2.0, shape.dims[1] / 2.0);
    let angle = rot.to_radians();
    let (sin, cos) = angle.sin_cos();
    shape
        .vertices
        .iter()
        .map(|[vx, vy]| {
            let (dx, dy) = (vx - cx, vy - cy);
            [cx + x + dx * cos - dy * sin, cy + y + dx * sin + dy * cos]
        })
        .collect()
}

/// Ray-casting point-in-polygon, matching the game's own test.
pub fn point_in_polygon(px: f64, py: f64, polygon: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let [xi, yi] = polygon[i];
        let [xj, yj] = polygon[j];
        if (yi > py) != (yj > py) {
            let cross = (xj - xi) * ((py - yi) / (yj - yi)) + xi;
            if px < cross {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Cell centre for an index.
pub fn cell_origin(cell: usize) -> [f64; 2] {
    let cols = game().grid.cols;
    let col = cell % cols;
    let row = cell / cols;
    [
        ORIGIN[0] + CELL * col as f64,
        ORIGIN[1] + CELL * row as f64,
    ]
}

/// Cells a polygon covers: a cell counts when any of its corners is inside.
pub fn covered_cells(polygon: &[[f64; 2]]) -> Vec<usize> {
    let game = game();
    let mut covered = Vec::new();
    for cell in 0..game.cells() {
        if game.node_at(cell).is_none() {
            continue;
        }
        let [x, y] = cell_origin(cell);
        let hit = [
            [3.0, 3.0],
            [24.0, 3.0],
            [3.0, 24.0],
            [24.0, 24.0],
        ]
        .iter()
        .any(|[ox, oy]| point_in_polygon(x + ox, y + oy, polygon));
        if hit {
            covered.push(cell);
        }
    }
    covered
}

/// One distinct cell-offset pattern a shape makes, and the phases that draw it.
#[derive(Clone, Debug, Default)]
pub struct Pattern {
    /// Cell offsets relative to the base column and row.
    pub offsets: Vec<(i32, i32)>,
    /// `(r, s)` pixel phases, `r` in x and `s` in y, each in `0..30`.
    pub phases: Vec<(i32, i32)>,
}

/// Coverage lattice for every shape and rotation.
pub struct CoverageLut {
    /// Indexed by `shape * ROT_STEPS + rotation step`.
    entries: Vec<Vec<Pattern>>,
}

impl CoverageLut {
    /// Patterns for one shape at one rotation.
    pub fn patterns(&self, shape: usize, rot: f64) -> &[Pattern] {
        let step = rotation_step(rot);
        self.entries
            .get(shape * ROT_STEPS + step)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn build() -> Self {
        let game = game();
        let mut entries = Vec::with_capacity(game.shapes.len() * ROT_STEPS);
        for shape in &game.shapes {
            for step in 0..ROT_STEPS {
                entries.push(patterns_for(shape, step as f64 * ROT_STEP));
            }
        }
        Self { entries }
    }
}

/// Rotation step index for an angle in degrees.
pub fn rotation_step(rot: f64) -> usize {
    let normalised = ((rot % 360.0) + 360.0) % 360.0;
    ((normalised / ROT_STEP).round() as usize) % ROT_STEPS
}

/// The shared coverage lattice, built once.
pub fn lut() -> &'static CoverageLut {
    static LUT: OnceLock<CoverageLut> = OnceLock::new();
    LUT.get_or_init(CoverageLut::build)
}

fn patterns_for(shape: &ShapeDef, rot: f64) -> Vec<Pattern> {
    let polygon = polygon_at(shape, 0.0, 0.0, rot);
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for [x, y] in &polygon {
        min_x = min_x.min(*x);
        max_x = max_x.max(*x);
        min_y = min_y.min(*y);
        max_y = max_y.max(*y);
    }
    // Cell corners sit at 3 and 24 inside a cell whose centre is at 15 and 24.
    let col_min = ((min_x - (ORIGIN[0] + 24.0)) / CELL).floor() as i32 - 1;
    let col_max = ((max_x - (ORIGIN[0] + 3.0)) / CELL).ceil() as i32 + 1;
    let row_min = ((min_y - (ORIGIN[1] + 24.0)) / CELL).floor() as i32 - 1;
    let row_max = ((max_y - (ORIGIN[1] + 3.0)) / CELL).ceil() as i32 + 1;
    let corners = [[3.0, 3.0], [24.0, 3.0], [3.0, 24.0], [24.0, 24.0]];

    let mut patterns: Vec<Pattern> = Vec::new();
    for phase_row in 0..PHASES {
        for phase_col in 0..PHASES {
            let phase_x = (phase_col * PHASE_STEP) as f64;
            let phase_y = (phase_row * PHASE_STEP) as f64;
            let mut offsets = Vec::new();
            for row in row_min..=row_max {
                for col in col_min..=col_max {
                    let cell_x = ORIGIN[0] + CELL * col as f64;
                    let cell_y = ORIGIN[1] + CELL * row as f64;
                    let hit = corners.iter().any(|[ox, oy]| {
                        point_in_polygon(cell_x + ox - phase_x, cell_y + oy - phase_y, &polygon)
                    });
                    if hit {
                        offsets.push((col, row));
                    }
                }
            }
            if offsets.is_empty() {
                continue;
            }
            match patterns
                .iter_mut()
                .find(|pattern| pattern.offsets == offsets)
            {
                Some(pattern) => pattern.phases.push((phase_col, phase_row)),
                None => patterns.push(Pattern {
                    offsets,
                    phases: vec![(phase_col, phase_row)],
                }),
            }
        }
    }
    patterns
}

/// Snap a coordinate to the placement lattice.
pub fn snap(value: f64) -> f64 {
    (value / PHASE_STEP as f64).round() * PHASE_STEP as f64
}

/// Cells a placement covers, within the grid and on an existing node.
pub fn cells_at(placement: &Placement) -> Vec<usize> {
    let game = game();
    let Some(shape) = game.shapes.get(placement.shape) else {
        return Vec::new();
    };
    let x = snap(placement.x);
    let y = snap(placement.y);
    let base_col = (x / CELL).floor() as i32;
    let base_row = (y / CELL).floor() as i32;
    let phase_col = (x.rem_euclid(CELL) / PHASE_STEP as f64).round().max(0.0) as i32 % PHASES;
    let phase_row = (y.rem_euclid(CELL) / PHASE_STEP as f64).round().max(0.0) as i32 % PHASES;
    let patterns = lut().patterns(shape.id, placement.rot);
    let pattern = patterns
        .iter()
        .find(|pattern| pattern.phases.contains(&(phase_col, phase_row)))
        .or_else(|| patterns.first());
    let Some(pattern) = pattern else {
        return Vec::new();
    };
    cells_for(pattern, base_col, base_row)
}

/// Grid cells an offset pattern covers from a base.
pub fn cells_for(pattern: &Pattern, base_col: i32, base_row: i32) -> Vec<usize> {
    let game = game();
    let (cols, rows) = (game.grid.cols as i32, game.grid.rows as i32);
    let mut cells = Vec::with_capacity(pattern.offsets.len());
    for (dcol, drow) in &pattern.offsets {
        let col = base_col + dcol;
        let row = base_row + drow;
        if col < 0 || col >= cols || row < 0 || row >= rows {
            continue;
        }
        let cell = (row * cols + col) as usize;
        if game.node_at(cell).is_some() {
            cells.push(cell);
        }
    }
    cells
}

/// Shape index covering each cell, `-1` where no shape does. First placement wins.
pub fn overlay(placements: &[Placement]) -> Vec<i32> {
    let mut overlay = vec![-1; game().cells()];
    for placement in placements {
        for cell in cells_at(placement) {
            if overlay[cell] < 0 {
                overlay[cell] = placement.shape as i32;
            }
        }
    }
    overlay
}

/// Does a placement keep its polygon inside the canvas?
pub fn in_bounds(placement: &Placement) -> bool {
    let game = game();
    let Some(shape) = game.shapes.get(placement.shape) else {
        return false;
    };
    let width = CELL * game.grid.cols as f64;
    let height = CELL * game.grid.rows as f64;
    placement.x >= -shape.dims[0]
        && placement.y >= -shape.dims[1]
        && placement.x <= width
        && placement.y <= height
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A placement of `shape` that covers `cell`, on the snap lattice.
    fn covering(shape: usize, cell: usize) -> Placement {
        let [x, y] = cell_origin(cell);
        for dx in (-60..=60).step_by(PHASE_STEP as usize) {
            for dy in (-60..=60).step_by(PHASE_STEP as usize) {
                let placement = Placement {
                    shape,
                    x: snap(x + dx as f64 - 15.0),
                    y: snap(y + dy as f64 - 30.0),
                    rot: 0.0,
                };
                if cells_at(&placement).contains(&cell) {
                    return placement;
                }
            }
        }
        panic!("shape {shape} cannot cover cell {cell}");
    }

    #[test]
    fn a_polygon_covers_the_cells_it_sits_on() {
        let placement = covering(3, 129);
        let cells = cells_at(&placement);
        assert!(cells.contains(&129));
        assert!(cells.iter().all(|cell| game().node_at(*cell).is_some()));
    }

    #[test]
    fn lookup_agrees_with_exact_geometry() {
        for shape in &game().shapes {
            for placement in [
                covering(shape.id, 129),
                covering(shape.id, 50),
                covering(shape.id, 190),
            ] {
                let fast = cells_at(&placement);
                let exact = covered_cells(&polygon_at(
                    shape,
                    snap(placement.x),
                    snap(placement.y),
                    placement.rot,
                ));
                assert_eq!(fast, exact, "shape {} at {:?}", shape.id, placement);
            }
        }
    }

    #[test]
    fn rotation_changes_coverage() {
        let upright = covering(3, 129);
        let sideways = Placement {
            rot: 90.0,
            ..upright
        };
        assert_ne!(cells_at(&upright), cells_at(&sideways));
    }

    #[test]
    fn overlay_gives_the_first_shape_its_cells() {
        let placements = [
            covering(0, 129),
            Placement {
                shape: 1,
                ..covering(0, 129)
            },
        ];
        let overlay = overlay(&placements);
        assert!(overlay.contains(&0));
        assert!(!overlay.contains(&1));
    }

    #[test]
    fn placements_outside_the_canvas_are_rejected() {
        let inside = covering(0, 129);
        assert!(in_bounds(&inside));
        assert!(!in_bounds(&Placement {
            x: -400.0,
            ..inside
        }));
    }
}
