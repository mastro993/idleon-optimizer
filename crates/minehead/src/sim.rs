//! The Depth Charge floor simulation, ported from `js/minehead/sim.js`.
//!
//! The engine is one state machine: [`Game`] holds a game's grid, lives and
//! per-turn state, and both the Monte Carlo search and the Play tab drive it
//! through the same `reveal`/`golden`/`attack` calls. The random stream is the
//! upstream `mulberry32`, consumed in the same order, so a seeded run lands on
//! the same numbers as the JS calculator.

use crate::data::{GameData, game};
use crate::math::{self, DamageInputs, bonus_dmg_per_tile_pct, current_outgoing_dmg};

/// A decision the strategy returns for one turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Reveal,
    Golden,
    Attack,
}

impl Decision {
    pub fn id(self) -> &'static str {
        match self {
            Self::Reveal => "reveal",
            Self::Golden => "golden",
            Self::Attack => "attack",
        }
    }
}

/// The context one decision reads, mirroring upstream `tunableStrategy`'s.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ctx {
    pub safe_revealed_this_turn: u32,
    pub mines_total: f64,
    pub mines_remaining: f64,
    pub unrevealed_count: f64,
    pub lives_left: f64,
    pub total_dmg_so_far: f64,
    pub boss_hp: f64,
    pub turns_played: u32,
    pub blocks_left: f64,
    pub goldens_on_grid: f64,
    pub current_turn_dmg: f64,
    pub remaining_hp: f64,
    pub per_tile_pct: f64,
    pub tile_count: u32,
    pub crown_progress: u32,
    pub last_turn_mine_hit: bool,
}

/// Every knob of the parameterized EV strategy, with upstream's defaults.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Params {
    pub ev_multiplier: f64,
    pub min_reveal: f64,
    pub mine_cap_pct: f64,
    pub golden_mine_pct: f64,
    pub hp_threshold: f64,
    pub block_aggro: bool,
    pub commit_min: f64,
    pub life_aggro: f64,
    pub turn1_ev_mul: f64,
    pub crown_chase: f64,
    pub commit_turns: u32,
    pub insta_mode: f64,
    pub post_mine_panic: f64,
    pub last_turn_aggro: f64,
    pub target_reveals: f64,
    pub golden_first: bool,
    pub crown_or_die: f64,
    pub hot_streak: f64,
    pub block_hoard: f64,
    pub corner_bias: f64,
    pub edge_bias: f64,
    pub cluster_bias: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            ev_multiplier: 1.0,
            min_reveal: 0.0,
            mine_cap_pct: 1.0,
            golden_mine_pct: 1.0,
            hp_threshold: 1.0,
            block_aggro: true,
            commit_min: 0.0,
            life_aggro: 1.0,
            turn1_ev_mul: 0.0,
            crown_chase: 0.0,
            commit_turns: 0,
            insta_mode: 0.0,
            post_mine_panic: 0.0,
            last_turn_aggro: 0.0,
            target_reveals: 0.0,
            golden_first: false,
            crown_or_die: 0.0,
            hot_streak: 0.0,
            block_hoard: 0.0,
            corner_bias: 1.0,
            edge_bias: 1.0,
            cluster_bias: 1.0,
        }
    }
}

/// The strategy a set of knobs defines.
pub fn decide(params: &Params, ctx: &Ctx) -> Decision {
    let reveal_or_golden = |ctx: &Ctx, mine: f64| {
        if params.golden_first && ctx.goldens_on_grid > 0.0 {
            return Decision::Golden;
        }
        if ctx.goldens_on_grid > 0.0 && mine >= params.golden_mine_pct {
            return Decision::Golden;
        }
        Decision::Reveal
    };
    let golden_or_attack = |ctx: &Ctx| {
        if ctx.goldens_on_grid > 0.0 {
            Decision::Golden
        } else {
            Decision::Attack
        }
    };

    if params.commit_turns > 0
        && ctx.turns_played >= params.commit_turns
        && ctx.safe_revealed_this_turn > 0
        && ctx.current_turn_dmg > 0.0
    {
        return golden_or_attack(ctx);
    }
    if params.hp_threshold > 0.0
        && ctx.current_turn_dmg >= params.hp_threshold * ctx.remaining_hp
        && ctx.current_turn_dmg > 0.0
    {
        return golden_or_attack(ctx);
    }
    if params.target_reveals > 0.0
        && f64::from(ctx.safe_revealed_this_turn) >= params.target_reveals
        && ctx.current_turn_dmg > 0.0
    {
        return golden_or_attack(ctx);
    }
    let p_mine = if ctx.unrevealed_count > 0.0 {
        ctx.mines_remaining / ctx.unrevealed_count
    } else {
        0.0
    };
    if ctx.safe_revealed_this_turn == 0 && ctx.unrevealed_count > 0.0 {
        return reveal_or_golden(ctx, p_mine);
    }
    if ctx.unrevealed_count == 0.0 {
        return golden_or_attack(ctx);
    }
    if params.crown_or_die > 0.0 && ctx.crown_progress >= 2 && p_mine <= params.crown_or_die {
        return reveal_or_golden(ctx, p_mine);
    }
    if p_mine >= params.mine_cap_pct {
        return golden_or_attack(ctx);
    }
    if f64::from(ctx.safe_revealed_this_turn) < params.min_reveal {
        return reveal_or_golden(ctx, p_mine);
    }

    let p_safe = 1.0 - p_mine;
    let tile_count = if ctx.tile_count == 0 {
        1.0
    } else {
        f64::from(ctx.tile_count)
    };
    let per_tile = ctx.per_tile_pct;
    let stage5 = 1.0 + tile_count * per_tile / 100.0;
    let next_stage5 = 1.0 + (tile_count + 1.0) * per_tile / 100.0;
    let dmg_after = if stage5 > 0.0 {
        ctx.current_turn_dmg * (next_stage5 / stage5)
    } else {
        ctx.current_turn_dmg
    };

    let ev = if params.turn1_ev_mul > 0.0 && ctx.total_dmg_so_far == 0.0 {
        params.turn1_ev_mul
    } else {
        params.ev_multiplier
    };
    let mut effective = if ctx.lives_left > 1.0 {
        ev * params.life_aggro
    } else {
        ev
    };
    if params.crown_chase > 0.0 && ctx.crown_progress >= 2 {
        effective *= 1.0 - params.crown_chase;
    }
    if params.post_mine_panic > 0.0 && ctx.last_turn_mine_hit {
        effective *= 1.0 + params.post_mine_panic;
    }
    if params.last_turn_aggro > 0.0
        && ctx.boss_hp > 0.0
        && ctx.remaining_hp / ctx.boss_hp < params.last_turn_aggro
    {
        effective *= 0.5;
    }
    if params.hot_streak > 0.0 && ctx.safe_revealed_this_turn >= 3 {
        effective *= 1.0 - params.hot_streak;
    }
    if params.block_hoard > 0.0 && ctx.blocks_left <= 1.0 {
        effective *= 1.0 + params.block_hoard;
    }
    let ev_reveal = if params.block_aggro && ctx.blocks_left > 0.0 {
        p_safe * dmg_after + p_mine * ctx.current_turn_dmg
    } else {
        p_safe * dmg_after
    };
    if ev_reveal > effective * ctx.current_turn_dmg {
        return Decision::Reveal;
    }
    if params.commit_min > 0.0
        && ctx.remaining_hp > 0.0
        && ctx.current_turn_dmg / ctx.remaining_hp < params.commit_min
    {
        return reveal_or_golden(ctx, p_mine);
    }
    golden_or_attack(ctx)
}

/// What one reveal did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reveal {
    Already,
    Safe,
    Jackpot,
    MineBlocked,
    MineHit,
}

/// How a turn ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    Attack,
    Mine,
}

/// Everything a game is played with.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub floor: usize,
    /// The raw upgrade levels; the grid size reads these, every quantity the
    /// doubled ones.
    pub levels: Vec<f64>,
    pub doublers: Vec<usize>,
    pub grid_bonus_166_1: f64,
    pub damage: DamageInputs,
    pub svar_hp: f64,
    /// Zero means no cap, as upstream.
    pub max_turns: u32,
    pub mine_reduction: f64,
    pub legend40_pts: f64,
    pub jelly_gold_tiles45: f64,
}

impl Options {
    /// The options a save's inputs define.
    pub fn from_inputs(inputs: &crate::Inputs) -> Self {
        Self {
            floor: inputs.floor,
            levels: inputs.levels.clone(),
            doublers: inputs.doublers.clone(),
            grid_bonus_166_1: inputs.grid[3],
            damage: inputs.damage_inputs(),
            svar_hp: inputs.server_var_hp,
            max_turns: 200,
            mine_reduction: inputs.reduction,
            legend40_pts: inputs.legend40_pts,
            jelly_gold_tiles45: inputs.jelly_gold_tiles45,
        }
    }
}

/// The values that stay the same across a game.
struct Setup {
    data: &'static GameData,
    cols: usize,
    rows: usize,
    num_tiles: usize,
    mines: f64,
    hp: f64,
    max_lives: f64,
    max_goldens: f64,
    blocks: f64,
    instas: f64,
    crown_odds: f64,
    jackpot_tiles: f64,
    gold_upg_total: f64,
    revival: bool,
    /// The QTY-equivalent levels.
    levels: Vec<f64>,
    damage: DamageInputs,
    max_turns: u32,
}

/// One game of Depth Charge, played a call at a time.
pub struct Game {
    setup: Setup,
    /// 0 is a depth charge, 1–29 a damage tile, 30 the jackpot, 40–49 currency.
    pub grid: Vec<u32>,
    pub crowns: Vec<bool>,
    pub golden: Vec<usize>,
    pub revealed: Vec<bool>,
    /// The damage-contributing tile values revealed this turn, in click order.
    pub values: Vec<u32>,
    pub crown_progress: u32,
    pub crown_sets: u32,
    pub safe_revealed: u32,
    pub manual_reveals: u32,
    insta_attempts: u32,
    insta_locked: bool,
    pub lives: f64,
    pub goldens_remaining: f64,
    pub blocks_left: f64,
    pub insta_left: f64,
    pub total_dmg: f64,
    pub turns_played: u32,
    pub commits: u32,
    pub mine_hits: u32,
    pub blocks_used: u32,
    pub first_click_mines: u32,
    pub first_turn_dmg: f64,
    pub last_turn_mine_hit: bool,
    pub outcome: Outcome,
    pub turret: bool,
    pub finished: bool,
}

impl Game {
    /// Build a game from the options, before the first grid is drawn.
    pub fn new(options: &Options) -> Self {
        let data = game();
        let (cols, rows) =
            math::grid_dims(data, options.levels.get(2).copied().unwrap_or(0.0) as usize);
        let num_tiles = cols * rows;
        let levels = math::qty_levels(&options.levels, &options.doublers);
        let setup = Setup {
            data,
            cols,
            rows,
            num_tiles,
            mines: math::mines_on_floor(
                options.floor,
                options.mine_reduction,
                options.legend40_pts,
            ),
            hp: math::floor_hp(options.floor, options.svar_hp),
            max_lives: math::max_hp_you(data, &levels),
            max_goldens: math::gold_tiles_total(data, &levels, options.jelly_gold_tiles45),
            blocks: math::blocks_total(data, &levels),
            instas: math::insta_reveals_total(data, &levels),
            crown_odds: math::bluecrown_odds(data, &levels),
            jackpot_tiles: math::jackpot_tiles(data, &levels),
            gold_upg_total: math::gold_tiles_total(data, &levels, options.jelly_gold_tiles45),
            revival: math::upgrade_qty(data, 19, levels.get(19).copied().unwrap_or(0.0), 1.0)
                >= 1.0,
            levels,
            damage: options.damage,
            max_turns: options.max_turns,
        };
        let num_tiles = setup.num_tiles;
        Self {
            lives: setup.max_lives,
            goldens_remaining: setup.max_goldens,
            blocks_left: setup.blocks,
            insta_left: setup.instas,
            grid: vec![0; num_tiles],
            crowns: vec![false; num_tiles],
            golden: Vec::new(),
            revealed: vec![false; num_tiles],
            values: Vec::new(),
            crown_progress: 0,
            crown_sets: 0,
            safe_revealed: 0,
            manual_reveals: 0,
            insta_attempts: 0,
            insta_locked: false,
            total_dmg: 0.0,
            turns_played: 0,
            commits: 0,
            mine_hits: 0,
            blocks_used: 0,
            first_click_mines: 0,
            first_turn_dmg: 0.0,
            last_turn_mine_hit: false,
            outcome: Outcome::None,
            turret: true,
            finished: false,
            setup,
        }
    }

    /// The grid's dimensions.
    pub fn dims(&self) -> (usize, usize) {
        (self.setup.cols, self.setup.rows)
    }

    /// The depth charges on the grid.
    pub fn mines(&self) -> f64 {
        self.setup.mines
    }

    /// The boss's health.
    pub fn boss_hp(&self) -> f64 {
        self.setup.hp
    }

    /// The lives one game starts with.
    pub fn max_lives(&self) -> f64 {
        self.setup.max_lives
    }

    /// The maximum number of depth charges one game starts with.
    pub fn max_goldens(&self) -> f64 {
        self.setup.max_goldens
    }

    /// The blocks one game starts with.
    pub fn max_blocks(&self) -> f64 {
        self.setup.blocks
    }

    /// The insta-reveals one game starts with.
    pub fn max_instas(&self) -> f64 {
        self.setup.instas
    }

    /// Draw a fresh grid and start a turn.
    pub fn start_turn(&mut self, rng: &mut Rng) {
        if self.finished {
            return;
        }
        self.turns_played += 1;
        let (grid, crowns) = generate_grid(
            self.setup.num_tiles,
            self.setup.mines,
            &self.setup.levels,
            self.setup.crown_odds,
            rng,
        );
        self.golden = place_goldens(
            &grid,
            self.setup.num_tiles,
            self.goldens_remaining,
            self.setup.gold_upg_total,
            rng,
        );
        self.grid = grid;
        self.crowns = crowns;
        self.revealed = vec![false; self.setup.num_tiles];
        self.values.clear();
        self.crown_progress = 0;
        self.crown_sets = 0;
        self.safe_revealed = 0;
        self.manual_reveals = 0;
        self.insta_attempts = 0;
        self.insta_locked = false;
        self.outcome = Outcome::None;
        self.turret = true;
    }

    /// Spend the insta-reveals this turn's strategy allows.
    pub fn use_instas(&mut self, rng: &mut Rng, threshold: f64) {
        let p_mine = self.setup.mines / self.setup.num_tiles as f64;
        while p_mine >= threshold && self.use_insta(rng) {}
    }

    /// Spend one insta-reveal on an unrevealed depth charge. Returns whether one
    /// was available; the Play tab's button calls this directly.
    pub fn use_insta(&mut self, rng: &mut Rng) -> bool {
        if self.insta_left <= 0.0 || self.insta_locked {
            return false;
        }
        let unrevealed: Vec<usize> = (0..self.setup.num_tiles)
            .filter(|index| self.grid[*index] == 0 && !self.revealed[*index])
            .collect();
        let Some(target) = unrevealed
            .get((rng.draw() * unrevealed.len() as f64).floor() as usize)
            .copied()
        else {
            return false;
        };
        if rng.draw() < (0.2 + 0.15 * self.insta_attempts as f64).min(0.7) {
            self.insta_locked = true;
        }
        self.insta_attempts += 1;
        self.insta_left -= 1.0;
        self.reveal(target, true, rng);
        true
    }

    /// How many mines, safe tiles and unclicked goldens remain.
    pub fn remaining(&self) -> (f64, f64, f64) {
        let mut mines = 0.0;
        let mut safe = 0.0;
        let mut goldens = 0.0;
        for index in 0..self.setup.num_tiles {
            if self.revealed[index] {
                continue;
            }
            if self.grid[index] == 0 {
                mines += 1.0;
            } else {
                safe += 1.0;
                if self.golden.contains(&index) {
                    goldens += 1.0;
                }
            }
        }
        (mines, safe, goldens)
    }

    /// The damage this turn would commit right now.
    pub fn turn_damage(&self) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }
        current_outgoing_dmg(
            self.setup.data,
            &self.values,
            self.crown_sets,
            self.lives <= 1.0,
            &self.setup.levels,
            &self.setup.damage,
        )
    }

    /// The context the strategy reads.
    pub fn context(&self) -> Ctx {
        let (mines, safe, goldens) = self.remaining();
        Ctx {
            safe_revealed_this_turn: self.safe_revealed,
            mines_total: self.setup.mines,
            mines_remaining: mines,
            unrevealed_count: mines + safe,
            lives_left: self.lives,
            total_dmg_so_far: self.total_dmg,
            boss_hp: self.setup.hp,
            turns_played: self.turns_played,
            blocks_left: self.blocks_left,
            goldens_on_grid: goldens,
            current_turn_dmg: self.turn_damage(),
            remaining_hp: self.setup.hp - self.total_dmg,
            per_tile_pct: bonus_dmg_per_tile_pct(
                self.setup.data,
                &self.setup.levels,
                self.setup.damage.grid_146,
            ),
            tile_count: self.values.len() as u32,
            crown_progress: self.crown_progress,
            last_turn_mine_hit: self.last_turn_mine_hit,
        }
    }

    /// Reveal one tile.
    pub fn reveal(&mut self, position: usize, is_insta: bool, rng: &mut Rng) -> Reveal {
        if position >= self.setup.num_tiles || self.revealed[position] {
            return Reveal::Already;
        }
        self.revealed[position] = true;
        if self.crowns[position] {
            self.crown_progress += 1;
            if self.crown_progress >= 3 {
                self.crown_progress = 0;
                self.crown_sets += 1;
            }
        }
        if self.golden.contains(&position) {
            self.goldens_remaining -= 1.0;
        }
        if self.grid[position] == 0 {
            if is_insta {
                return Reveal::Safe;
            }
            self.mine_hits += 1;
            if self.blocks_left > 0.0 {
                self.blocks_left -= 1.0;
                self.blocks_used += 1;
                return Reveal::MineBlocked;
            }
            self.lives -= 1.0;
            if self.lives == 1.0 && self.setup.revival {
                self.blocks_left = 1.0;
            }
            return Reveal::MineHit;
        }
        if self.grid[position] == 30 {
            let mut left = self.setup.jackpot_tiles;
            let mut attempts = 0;
            while attempts < 1000 && left > 0.0 {
                attempts += 1;
                let target = (rng.draw() * self.setup.num_tiles as f64).floor() as usize;
                if self.grid[target] != 0 && !self.revealed[target] {
                    self.reveal(target, false, rng);
                    left -= 1.0;
                }
            }
            return Reveal::Jackpot;
        }
        let value = self.grid[position];
        if (1..=29).contains(&value) {
            self.values.push(value);
            self.safe_revealed += 1;
        } else if (40..50).contains(&value) {
            self.safe_revealed += 1;
        }
        Reveal::Safe
    }

    /// Click one unclicked golden tile, if one is left.
    pub fn click_golden(&mut self, rng: &mut Rng) -> Option<Reveal> {
        let position = self
            .golden
            .iter()
            .copied()
            .find(|position| !self.revealed[*position])?;
        Some(self.reveal(position, false, rng))
    }

    /// Reveal a random candidate tile, weighted by the strategy's spatial knobs.
    pub fn reveal_candidate(&mut self, params: &Params, rng: &mut Rng) -> Option<Reveal> {
        self.manual_reveals += 1;
        let candidates: Vec<usize> = (0..self.setup.num_tiles)
            .filter(|index| !self.revealed[*index] && !self.golden.contains(index))
            .collect();
        if candidates.is_empty() {
            self.click_golden(rng);
            return None;
        }
        let spatial =
            params.corner_bias != 1.0 || params.edge_bias != 1.0 || params.cluster_bias != 1.0;
        let pick = if !spatial || candidates.len() == 1 {
            candidates[(rng.draw() * candidates.len() as f64).floor() as usize]
        } else {
            let mut weights = Vec::with_capacity(candidates.len());
            let mut total = 0.0;
            for index in &candidates {
                let row = index / self.setup.cols;
                let col = index % self.setup.cols;
                let edge_row = row == 0 || row + 1 == self.setup.rows;
                let edge_col = col == 0 || col + 1 == self.setup.cols;
                let corner = edge_row && edge_col;
                let edge = (edge_row || edge_col) && !corner;
                let mut weight = 1.0;
                if corner {
                    weight *= params.corner_bias;
                } else if edge {
                    weight *= params.edge_bias;
                }
                if params.cluster_bias != 1.0 {
                    let mut adjacent = false;
                    for dr in -1i32..=1 {
                        for dc in -1i32..=1 {
                            if dr == 0 && dc == 0 {
                                continue;
                            }
                            let nr = row as i32 + dr;
                            let nc = col as i32 + dc;
                            if nr < 0
                                || nc < 0
                                || nr as usize >= self.setup.rows
                                || nc as usize >= self.setup.cols
                            {
                                continue;
                            }
                            let neighbour = nr as usize * self.setup.cols + nc as usize;
                            if self.revealed[neighbour] && self.grid[neighbour] != 0 {
                                adjacent = true;
                            }
                        }
                    }
                    if adjacent {
                        weight *= params.cluster_bias;
                    }
                }
                weights.push(weight);
                total += weight;
            }
            let mut dart = rng.draw() * total;
            let mut pick = *candidates.last().expect("a candidate");
            for (index, weight) in candidates.iter().zip(&weights) {
                dart -= weight;
                if dart <= 0.0 {
                    pick = *index;
                    break;
                }
            }
            pick
        };
        let result = self.reveal(pick, false, rng);
        if result == Reveal::MineHit {
            if self.manual_reveals == 1 {
                self.first_click_mines += 1;
            }
            self.end_turn(Outcome::Mine);
        } else if result == Reveal::MineBlocked && self.manual_reveals == 1 {
            self.first_click_mines += 1;
        }
        Some(result)
    }

    /// Commit this turn's damage and end it.
    pub fn attack(&mut self) {
        self.end_turn(Outcome::Attack);
    }

    fn end_turn(&mut self, outcome: Outcome) {
        if self.outcome == Outcome::Mine {
            return;
        }
        if outcome != Outcome::Mine && !self.values.is_empty() {
            let dmg = self.turn_damage();
            if dmg > 0.0 {
                if self.commits == 0 {
                    self.first_turn_dmg = dmg;
                }
                self.total_dmg += dmg;
                self.commits += 1;
            }
        }
        self.outcome = outcome;
        self.last_turn_mine_hit = outcome == Outcome::Mine;
        self.turret = false;
        if self.lives <= 0.0 || self.total_dmg >= self.setup.hp {
            self.finished = true;
        }
    }

    /// End the turn with no more tiles left to reveal.
    pub fn clear_turn(&mut self) {
        self.end_turn(Outcome::None);
    }

    /// Whether the game is over.
    pub fn finished(&self) -> bool {
        self.finished || (self.setup.max_turns > 0 && self.turns_played >= self.setup.max_turns)
    }

    /// The result a finished game reports.
    pub fn result(&self) -> GameResult {
        GameResult {
            won: self.total_dmg >= self.setup.hp,
            total_dmg: self.total_dmg,
            hp_remaining: (self.setup.hp - self.total_dmg).max(0.0),
            lives_left: self.lives.max(0.0),
            turns_played: self.turns_played,
            commits: self.commits,
            mine_hits: self.mine_hits,
            blocks_used: self.blocks_used,
            first_click_mines: self.first_click_mines,
            first_turn_dmg: self.first_turn_dmg,
            timed_out: self.setup.max_turns > 0
                && self.turns_played >= self.setup.max_turns
                && self.total_dmg < self.setup.hp
                && self.lives > 0.0,
            floor_hp: self.setup.hp,
        }
    }
}

/// One finished game.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameResult {
    pub won: bool,
    pub total_dmg: f64,
    pub hp_remaining: f64,
    pub lives_left: f64,
    pub turns_played: u32,
    pub commits: u32,
    pub mine_hits: u32,
    pub blocks_used: u32,
    pub first_click_mines: u32,
    pub first_turn_dmg: f64,
    pub timed_out: bool,
    pub floor_hp: f64,
}

/// Play one game to its end with the strategy.
pub fn simulate_game(options: &Options, params: &Params, rng: &mut Rng) -> GameResult {
    let mut game = Game::new(options);
    while !game.finished() {
        game.start_turn(rng);
        game.use_instas(rng, params.insta_mode);
        loop {
            // `safe <= 0` covers both upstream exits: every tile revealed, and
            // only depth charges left.
            if game.remaining().1 <= 0.0 {
                game.attack();
                break;
            }
            let ctx = game.context();
            match decide(params, &ctx) {
                Decision::Attack => {
                    game.attack();
                    break;
                }
                Decision::Golden => {
                    if game.click_golden(rng).is_none() {
                        game.attack();
                        break;
                    }
                }
                Decision::Reveal => {
                    game.reveal_candidate(params, rng);
                }
            }
            if !game.turret {
                break;
            }
        }
    }
    game.result()
}

/// The aggregate one Monte Carlo run reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonteCarlo {
    pub win_rate: f64,
    pub avg_dmg: f64,
    pub avg_turns: f64,
    pub avg_commits: f64,
    pub avg_mine_hits: f64,
    pub avg_blocks_used: f64,
    pub avg_first_click_mines: f64,
    pub avg_first_turn_dmg: f64,
    pub avg_dmg_per_commit: f64,
    pub timeout_rate: f64,
    pub trials: u32,
}

/// Run `trials` games on one random stream.
pub fn monte_carlo(options: &Options, params: &Params, trials: u32, seed: u32) -> MonteCarlo {
    let mut rng = Rng::new(seed);
    let mut wins = 0.0;
    let mut dmg = 0.0;
    let mut turns = 0.0;
    let mut commits = 0.0;
    let mut mine_hits = 0.0;
    let mut blocks = 0.0;
    let mut first_click = 0.0;
    let mut first_turn = 0.0;
    let mut timeouts = 0.0;
    for _ in 0..trials {
        let result = simulate_game(options, params, &mut rng);
        if result.won {
            wins += 1.0;
        }
        if result.timed_out {
            timeouts += 1.0;
        }
        dmg += result.total_dmg;
        turns += f64::from(result.turns_played);
        commits += f64::from(result.commits);
        mine_hits += f64::from(result.mine_hits);
        blocks += f64::from(result.blocks_used);
        first_click += f64::from(result.first_click_mines);
        first_turn += result.first_turn_dmg;
    }
    let trials = trials as f64;
    MonteCarlo {
        win_rate: wins / trials,
        avg_dmg: dmg / trials,
        avg_turns: turns / trials,
        avg_commits: commits / trials,
        avg_mine_hits: mine_hits / trials,
        avg_blocks_used: blocks / trials,
        avg_first_click_mines: first_click / trials,
        avg_first_turn_dmg: first_turn / trials,
        avg_dmg_per_commit: if commits > 0.0 { dmg / commits } else { 0.0 },
        timeout_rate: timeouts / trials,
        trials: trials as u32,
    }
}

/// `generateGrid`: the game's own tile rolls.
pub fn generate_grid(
    num_tiles: usize,
    mines: f64,
    levels: &[f64],
    crown_odds: f64,
    rng: &mut Rng,
) -> (Vec<u32>, Vec<bool>) {
    let data = game();
    let level = |t: usize| levels.get(t).copied().unwrap_or(0.0);
    let numbahs = math::upgrade_qty(data, 1, level(1), 1.0);
    let bettah = math::upgrade_qty(data, 3, level(3), 1.0);
    let multi_upg = math::upgrade_qty(data, 12, level(12), 1.0);
    let multi_skew = math::upgrade_qty(data, 13, level(13), 1.0);
    let add_upg = math::upgrade_qty(data, 17, level(17), 1.0);
    let add_skew = math::upgrade_qty(data, 18, level(18), 1.0);
    let jp_odds = math::jackpot_odds(data, levels);

    let mut grid = vec![0u32; num_tiles];
    let mut crowns = vec![false; num_tiles];

    // Step 1: the base tile values. The game rolls a fixed 72 values.
    for slot in 0..72 {
        let mut value = (1.0 + js_min(js_min(12.0, bettah / 150.0) + rng.draw(), numbahs)).floor();
        let incr_prob =
            0.14 + (bettah / 2000.0).min(0.06) + (bettah / (bettah + 1500.0) * 0.5).min(0.5);
        for _ in 0..17 {
            if js_round(value) > js_round(numbahs) {
                break;
            }
            if rng.draw() >= incr_prob {
                break;
            }
            value = js_round(value + 1.0);
        }
        if slot < grid.len() {
            grid[slot] = value as u32;
            if bettah > 10.0 && (1.0 + rng.draw() * 5000.0).floor() == 1.0 {
                grid[slot] = 19;
            }
        } else if bettah > 10.0 {
            // The rare subtract roll still consumes the stream.
            let _ = rng.draw();
        }
    }

    // Step 2: multiplier, jackpot and currency tiles.
    let mut has_jackpot = false;
    for cell in grid.iter_mut().take(num_tiles) {
        let multi_chance = 0.05 + multi_skew / (multi_skew + 1000.0) * 0.08;
        if rng.draw() < multi_chance && multi_upg > 0.0 {
            let mut value = (js_min(3.0, multi_skew / 400.0) + rng.draw()).floor();
            let incr_prob =
                0.18 + (multi_skew / 2000.0).min(0.07) + multi_skew / (multi_skew + 1200.0) * 0.3;
            for _ in 0..9 {
                if js_round(value + 1.0) >= js_round(multi_upg) {
                    break;
                }
                if rng.draw() >= incr_prob {
                    break;
                }
                value = js_round(value + 1.0);
            }
            *cell = js_round(js_min(29.0, 20.0 + value)) as u32;
        }
        if !has_jackpot && rng.draw() < jp_odds {
            *cell = 30;
            has_jackpot = true;
        }
        let add_chance = 0.07 + add_skew / (add_skew + 1000.0) * 0.1;
        if rng.draw() < add_chance && add_upg > 0.0 {
            let mut value = (js_min(3.0, add_skew / 400.0) + rng.draw()).floor();
            let incr_prob =
                0.14 + (add_skew / 2000.0).min(0.06) + add_skew / (add_skew + 1200.0) * 0.25;
            for _ in 0..9 {
                if js_round(value + 1.0) >= js_round(add_upg) {
                    break;
                }
                if rng.draw() >= incr_prob {
                    break;
                }
                value = js_round(value + 1.0);
            }
            *cell = js_round(js_min(49.0, 40.0 + value)) as u32;
        }
    }

    // Step 3: the depth charges overwrite random tiles.
    let mine_count = (mines.min(num_tiles as f64)) as usize;
    let mut placed = 0;
    let mut guard = 0usize;
    while placed < mine_count && guard < num_tiles * 1000 {
        guard += 1;
        let position = (rng.draw() * num_tiles as f64).floor() as usize;
        if grid[position] == 0 {
            continue;
        }
        grid[position] = 0;
        placed += 1;
    }

    // Step 4: blue crowns sit on safe tiles only.
    for (value, crown) in grid.iter().zip(crowns.iter_mut()).take(num_tiles) {
        if *value != 0 && rng.draw() < crown_odds {
            *crown = true;
        }
    }
    (grid, crowns)
}

/// `_placeGoldens`: golden tiles on safe positions, up to the budget.
pub fn place_goldens(
    grid: &[u32],
    num_tiles: usize,
    budget: f64,
    gold_upg_total: f64,
    rng: &mut Rng,
) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut remaining = budget;
    for (slot, value) in grid.iter().enumerate().take(num_tiles) {
        if remaining <= 0.0 {
            break;
        }
        if *value == 0 {
            continue;
        }
        let probability =
            js_max(2.0, (rng.draw() * 0.28 + 0.12) * gold_upg_total) / num_tiles as f64;
        if rng.draw() < probability {
            positions.push(slot);
            remaining -= 1.0;
        }
    }
    positions
}

/// The upstream seeded PRNG, `mulberry32`.
pub struct Rng {
    state: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    /// The next value in `[0, 1)`.
    pub fn draw(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6D2B_79F5);
        let a = self.state ^ (self.state >> 15);
        let t = a.wrapping_mul(self.state | 1);
        let t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

/// JS `Math.round`.
fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

fn js_min(left: f64, right: f64) -> f64 {
    if left < right { left } else { right }
}

fn js_max(left: f64, right: f64) -> f64 {
    if left > right { left } else { right }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seeded_stream_matches_mulberry32() {
        let mut rng = Rng::new(42);
        let values: Vec<f64> = (0..4).map(|_| rng.draw()).collect();
        assert!(
            (values[0] - 0.601_103_751_920_163_6).abs() < 1e-12,
            "{values:?}"
        );
        assert!(
            (values[3] - 0.669_734_041_439_369_3).abs() < 1e-12,
            "{values:?}"
        );
    }

    #[test]
    fn a_flat_grid_rolls_one_mine_per_depth_charge() {
        let mut rng = Rng::new(7);
        let (grid, crowns) = generate_grid(9, 2.0, &[0.0; 30], 0.0, &mut rng);
        assert_eq!(grid.len(), 9);
        assert_eq!(grid.iter().filter(|value| **value == 0).count(), 2);
        assert!(crowns.iter().all(|crown| !crown));
    }
}
