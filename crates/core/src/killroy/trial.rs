//! One deterministic Killroy encounter trial.
//!
//! Ports the encounter in `js/stats/systems/common/combat-sim-killroy.js`: the
//! 300 arrivals armed 50 ms in on the `0.05 + 0.15 * i^1.3` schedule, the 60%
//! queue on every kill, the respawn bonus, the 100 ms spawn tick, the `100 +
//! Time` duration and the per-kill skull, talent-point, dungeon-credit and pearl
//! rolls. The stream is the combat simulator's `createRng` (mulberry32), so a
//! trial is a pure function of its inputs and seed.
//!
//! The player's damage is the one thing not ported: it belongs to the combat
//! simulator, which is a later section. [`TrialInputs::kills_per_second`] stands
//! in for it, so the trial models the encounter's supply and drop maths rather
//! than the character's power.

/// Monsters the encounter arms 50 ms into the run.
const ARRIVALS: usize = 300;
/// The upstream simulation step, in milliseconds.
const STEP_MS: f64 = 10.0;
/// The spawn tick: at most one queued arrival enters the map per 100 ms.
const SPAWN_MS: f64 = 100.0;
/// How many upgrade rows the game exposes.
pub const UPGRADES: usize = 6;

/// The save-derived levels and stand-in rate one trial runs with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrialInputs {
    /// `OptLacc[106..=111]`: Time, talent points, skulls, respawns, credits and
    /// pearls, in the game's own order.
    pub levels: [f64; UPGRADES],
    /// `OptLacc[114]`: the talent-drop counter the talent roll reads.
    pub talent_drops: f64,
    /// Meritoc vote 18's skull bonus, in percent. The voter tree lands with the
    /// W7 sections, so a save outside it runs at the base factor.
    pub meritoc: f64,
    /// Stand-in for the combat simulator's kill rate.
    pub kills_per_second: f64,
}

impl Default for TrialInputs {
    fn default() -> Self {
        Self {
            levels: [0.0; UPGRADES],
            talent_drops: 0.0,
            meritoc: 0.0,
            kills_per_second: 3.0,
        }
    }
}

/// What one trial produced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub seed: u64,
    /// Monsters killed in the run.
    pub kills: u64,
    /// Skulls the kills dropped.
    pub skulls_dropped: u64,
    /// Skulls collected. The game auto-loots within the run and its 2 s tail,
    /// so every drop is picked up while inventory space lasts.
    pub skulls_picked: u64,
    pub talent_points: u64,
    pub dungeon_credits: u64,
    pub pearls: u64,
    /// Monsters the spawn tick put on the map.
    pub spawned: u64,
    /// Seconds from the run's start to its results boundary.
    pub seconds: f64,
}

/// The combat simulator's stream: mulberry32 with a call counter, from
/// `createRng` in `js/stats/systems/common/combat-sim-core.js`.
struct Random {
    state: u32,
}

impl Random {
    fn new(seed: u64) -> Self {
        let state = seed as u32;
        Self {
            state: if state == 0 { 0x9e37_79b9 } else { state },
        }
    }

    fn float(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6d2b_79f5);
        let mut value = self.state;
        value = (value ^ (value >> 15)).wrapping_mul(value | 1);
        value ^= value.wrapping_add((value ^ (value >> 7)).wrapping_mul(value | 61));
        f64::from(value ^ (value >> 14)) / 4_294_967_296.0
    }

    /// The upstream `randomInt`, including its integer coercion and bounds.
    fn int(&mut self, low: i64, high: i64) -> i64 {
        let roll = self.float();
        if low <= high {
            low + (roll * (high - low + 1) as f64) as i64
        } else {
            high + (roll * (low - high + 1) as f64) as i64
        }
    }
}

/// One run's mutable counters.
struct Run {
    levels: [f64; UPGRADES],
    talent_drops: f64,
    meritoc: f64,
    kills_per_second: f64,
    random: Random,
    kills: u64,
    skulls_dropped: u64,
    spawned: u64,
    talent_points: u64,
    dungeon_credits: u64,
    pearls: u64,
    queued: f64,
    live: u64,
    budget: f64,
}

impl Run {
    /// One kill, and the upstream drop rolls in their own order. Each drop
    /// burns the same stream calls the game does, so the rolls stay in step.
    fn kill(&mut self) {
        if self.random.float() < 0.6 {
            self.queued += 1.0;
        }
        self.kills += 1;
        // Source 63627: the bonus-skull short circuit changes the stream.
        let factor = (1.0 + self.meritoc / 100.0)
            * (0.25
                * (1.0 + self.levels[2] / 100.0)
                * (1.0 - 0.995 * self.kills as f64 / (self.kills as f64 + 50.0)).max(0.1))
            .min(0.25);
        if self.random.float() < factor {
            self.skulls_dropped += 1;
            let kills = self.kills;
            let bonus = kills < 150
                && ((kills < 50 && self.random.float() < 0.16) || self.random.float() < 0.06);
            if bonus {
                self.skulls_dropped += 1;
            }
        }
        let talents = self.levels[1];
        if talents > 0.5
            && self.random.float()
                < (1.0 + 0.05 * talents)
                    / (self.talent_drops.powi(2) + 10.0 * (self.talent_drops + 1.0)).max(10.0)
        {
            self.talent_points += 1;
            self.talent_drops += 1.0;
        }
        let credits = self.levels[4];
        if credits > 0.5 {
            let dropped = self.random.float() < 0.01 * (1.0 + credits / 100.0);
            if dropped {
                let _ = self.random.int(1, 2);
                self.dungeon_credits += 1;
            }
        }
        let pearls = self.levels[5];
        // The name roll consumes one stream call, or two when it falls through.
        let dropped = pearls > 0.5 && self.random.float() < 0.002 * (1.0 + pearls / 100.0);
        if dropped && (self.random.float() < 0.5 || self.random.float() < 0.9) {
            self.pearls += 1;
        }
    }

    /// Kill up to `kills_per_second` monsters over one step.
    fn fight(&mut self) {
        self.budget += self.kills_per_second * STEP_MS / 1000.0;
        while self.budget >= 1.0 && self.live > 0 {
            self.budget -= 1.0;
            self.live -= 1;
            self.kill();
        }
    }
}

/// Run one trial for `seed`, following the upstream timers tick for tick.
pub fn trial(inputs: &TrialInputs, seed: u64) -> Record {
    let mut run = Run {
        levels: inputs.levels,
        talent_drops: inputs.talent_drops,
        meritoc: inputs.meritoc,
        kills_per_second: inputs.kills_per_second.max(0.0),
        random: Random::new(seed),
        kills: 0,
        skulls_dropped: 0,
        spawned: 0,
        talent_points: 0,
        dungeon_credits: 0,
        pearls: 0,
        queued: 0.0,
        live: 0,
        budget: 0.0,
    };
    // AE481 init (84268): the 300 arrivals are armed 50 ms into the run.
    let arrivals: Vec<f64> = (0..ARRIVALS)
        .map(|index| 50.0 + 1_000.0 * (0.05 + 0.15 * (index as f64).powf(1.3)))
        .collect();
    let duration = 100.0 + run.levels[0];
    // AE481 _event_01sec (83423): the counter starts at 100 + Time and drops a
    // tenth every tick, so the results boundary is one `100 + Time`-long run.
    let ticks_per_spawn = (SPAWN_MS / STEP_MS) as u64;
    let total_ticks = (duration * 1_000.0 / SPAWN_MS).round() as u64;
    let mut arrivals_left = 0;
    let mut step = 0u64;
    let mut finished_at = None;
    loop {
        let now = step as f64 * STEP_MS;
        while arrivals_left < arrivals.len() && arrivals[arrivals_left] <= now {
            arrivals_left += 1;
            run.queued += 1.0;
            let respawns = run.levels[3];
            if respawns > 0.0 {
                run.queued += (run.random.float() + respawns / 100.0).floor();
            }
        }
        // The spawn tick runs from 100 ms on; the first sim step is at 0 ms.
        if step > 0 && step.is_multiple_of(ticks_per_spawn) {
            if run.queued >= 1.0 {
                run.queued -= 1.0;
                run.spawned += 1;
                run.live += 1;
            }
            // The results boundary is the tick `100 + Time` seconds in.
            if step / ticks_per_spawn == total_ticks {
                finished_at = Some(now);
            }
        }
        if finished_at.is_some() {
            break;
        }
        run.fight();
        step += 1;
    }

    Record {
        seed,
        kills: run.kills,
        skulls_dropped: run.skulls_dropped,
        skulls_picked: run.skulls_dropped,
        talent_points: run.talent_points,
        dungeon_credits: run.dungeon_credits,
        pearls: run.pearls,
        spawned: run.spawned,
        seconds: finished_at.map_or(duration, |at| at / 1_000.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture's killroy levels: `OptLacc[106..=111]` and `[114]`.
    fn fixture_levels() -> TrialInputs {
        TrialInputs {
            levels: [155.0, 11.0, 281.0, 28.0, 1.0, 2.0],
            talent_drops: 137.0,
            meritoc: 0.0,
            kills_per_second: 3.0,
        }
    }

    #[test]
    fn a_seed_always_produces_the_same_record() {
        let inputs = fixture_levels();
        let first = trial(&inputs, 7);
        assert_eq!(first, trial(&inputs, 7));
        assert_ne!(first, trial(&inputs, 8));
    }

    #[test]
    fn the_run_lasts_one_hundred_plus_the_time_upgrade() {
        let mut inputs = fixture_levels();
        inputs.kills_per_second = 0.0;
        assert!((trial(&inputs, 1).seconds - 255.0).abs() < 1e-9);
        inputs.levels[0] += 1.0;
        assert!((trial(&inputs, 1).seconds - 256.0).abs() < 1e-9);
    }

    #[test]
    fn the_arrival_schedule_matches_upstream() {
        // node -e "console.log(JSON.stringify([0,1,2,299].map(i => 50 + 1000*(0.05 + 0.15*i**1.3))))"
        let arrivals: Vec<f64> = [0, 1, 2, 299]
            .iter()
            .map(|index| 50.0 + 1_000.0 * (0.05 + 0.15 * (*index as f64).powf(1.3)))
            .collect();
        for (got, want) in arrivals
            .iter()
            .zip([100.0, 250.0, 469.343_324, 248_106.922_857])
        {
            assert!((got - want).abs() < 0.5, "{got} != {want}");
        }
    }

    #[test]
    fn a_faster_character_kills_more() {
        let mut slow = fixture_levels();
        slow.levels = [103.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        slow.kills_per_second = 1.0;
        let mut fast = slow;
        fast.kills_per_second = 4.0;
        let one = trial(&slow, 3);
        let four = trial(&fast, 3);
        assert!(four.kills > one.kills, "{} != {}", four.kills, one.kills);
        assert!(four.kills <= four.spawned, "kills cannot exceed spawns");
    }

    #[test]
    fn an_empty_encounter_still_ends_on_time() {
        let mut inputs = fixture_levels();
        inputs.kills_per_second = 0.0;
        let record = trial(&inputs, 11);
        assert_eq!(record.kills, 0);
        assert_eq!(record.skulls_dropped, 0);
        assert!((record.seconds - 255.0).abs() < 1e-9);
    }

    #[test]
    fn the_talent_credit_and_pearl_unlocks_need_level_one() {
        let mut inputs = fixture_levels();
        inputs.levels = [155.0, 0.0, 0.0, 0.0, 1.0, 1.0];
        inputs.talent_drops = 0.0;
        inputs.kills_per_second = 10.0;
        let record = trial(&inputs, 5);
        assert_eq!(record.talent_points, 0, "talent points need level 1");
        assert!(record.dungeon_credits > 0);
        assert!(record.pearls > 0);
    }

    #[test]
    fn the_respawn_bonus_adds_queued_arrivals() {
        let mut inputs = fixture_levels();
        inputs.levels = [155.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        inputs.kills_per_second = 20.0;
        let none = trial(&inputs, 5);
        inputs.levels[3] = 100.0;
        let bonus = trial(&inputs, 5);
        assert!(bonus.spawned > none.spawned);
    }
}
