//! The Den's hound-wave attempt (CS-06).
//!
//! Ported from the `The Den (map 216, cavern 2)` section of
//! `js/stats/systems/common/combat-sim-special.js`: `ampDebuff`, `ampMulti`
//! and `houndNextOpalScoreReq` are the amplifier formulas, and
//! [`project`] replays the cavern's attempt — `startDen`, the `_denFrame`
//! scene updater, the hounds' 100 ms timer (`_caveAInit`), the kill's score
//! and Skull chain (`_denHoundKill`) and the fear-maximum end (`_denEnd`).
//!
//! The projection runs the attempt on its own: the map's other monsters, the
//! reward stream and the player's movement are not part of the censused
//! outcome, so the attempt is driven by the run's kill interval instead. The
//! Den panel says so.
//!
//! `crates/combat/data/den.json` and `fixtures/combat-den.json` are written by
//! `script/combat-den-oracle.mjs` from one upstream checkout. The fixture
//! drives upstream's own Den functions on a fabricated sim, so the wave, fear,
//! score and opal formulas cannot drift from their source.

use idleon_common::Rng;
use serde::Deserialize;
use std::sync::OnceLock;

const DEN_JSON: &str = include_str!("../data/den.json");

/// `DEN_FEAR_MAX`: the fear at which the attempt ends.
pub const FEAR_MAX: f64 = 50.0;
/// The game's fixed update step, in milliseconds.
const STEP_MS: u64 = 10;
/// How long the attempt's "ending" state lasts after the fear maximum.
const RESET_MS: u64 = 7_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    hound: HoundRow,
    /// `HolesInfo[70]`: the Bolaia bonus per level.
    bolaia: Vec<f64>,
    /// The amplifier names the panel shows.
    amp_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HoundRow {
    name: String,
    hp: f64,
    respawn: f64,
}

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(DEN_JSON).expect("den.json is valid"))
}

/// The hound's base `MonsterHPTotal`.
pub fn hound_hp() -> f64 {
    data().hound.hp
}

/// The hound's display name.
pub fn hound_name() -> &'static str {
    &data().hound.name
}

/// The hound's `RespawnTime`, for the panel's note.
pub fn hound_respawn() -> f64 {
    data().hound.respawn
}

/// `holesBolaiaPerLv(2)`: the Skull-chain score bonus per Bolaia level.
pub fn bolaia_per_level() -> f64 {
    data().bolaia.get(2).copied().unwrap_or(0.0)
}

/// The eight amplifier labels, in `Holes[12]` order.
pub fn amp_names() -> &'static [String] {
    &data().amp_names
}

/// `ampDebuff(Holes[12], t)`: what each amplifier adds to the attempt.
pub fn amp_debuff(levels: &[f64; 8], kind: usize) -> f64 {
    let h = levels.get(kind).copied().unwrap_or(0.0);
    match kind {
        0 => (5e3 + 800.0 * h) * 1.25f64.powf(h),
        1 => (6e3 + 4e3 * h) * 1.3f64.powf(h),
        2 => 5.0 * h,
        3 => (1.0 + h) * 1.05f64.powf(h),
        4 => (1.0 + h) * 1.07f64.powf(h),
        5 => 1.0 + h,
        6 => 4.0 * h,
        7 => h / 10.0,
        _ => 0.0,
    }
}

/// `ampMulti`: the Den score multiplier `AmpMulti`.
pub fn amp_multi(levels: &[f64; 8], b_upg23: f64) -> f64 {
    let total: f64 = levels.iter().sum();
    0.5 * b_upg23 * (total / 10.0).floor()
        + (11.0 * levels[0]
            + (9.0 * levels[1]
                + (8.0 * levels[2]
                    + (10.0 * levels[3]
                        + (8.0 * levels[4]
                            + (7.0 * levels[5] + (10.0 * levels[6] + 7.0 * levels[7])))))))
            / 100.0
        + 1.0
}

/// `houndNextOpalScoreReq(Holes[7][2])`: the score the next opal needs.
pub fn next_opal_score(level: f64) -> f64 {
    12.0 * (150.0 + (30.0 + level) * level) * 1.5f64.powf(level)
}

/// The saved state the attempt reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// `Holes[12]`, the eight amplifier levels (or the edited ones).
    pub amps: [f64; 8],
    /// `computeBUpg(23, 1)`: the amplifier-multi upgrade built.
    pub b_upg23: f64,
    /// `Holes[26][2]`: the Bolaia level that scales the Skull chain.
    pub bolaia_level: f64,
    /// `Holes[11][8]`: the account's best Den score.
    pub best_score: f64,
    /// `Holes[7][2]`: the opal level the next score requirement reads.
    pub opal_level: f64,
    /// `caveA`'s `MonsterHPTotal`.
    pub hound_hp: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            amps: [0.0; 8],
            b_upg23: 0.0,
            bolaia_level: 0.0,
            best_score: 0.0,
            opal_level: 0.0,
            hound_hp: data().hound.hp,
        }
    }
}

/// The Den panel's rows; `denSnapshot`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// `1` in progress, `0.5` ending, `0` ended or not started.
    pub active: f64,
    pub fear: f64,
    pub fear_max: f64,
    pub score: f64,
    /// Hound kills, `GenINFO[130]`.
    pub kills: f64,
    pub wave: f64,
    /// Incoming fear tokens.
    pub tokens: f64,
    pub amp_multi: f64,
    /// Skull hounds killed this attempt.
    pub chain: f64,
    pub runs: f64,
    pub opals: f64,
}

/// The attempt's totals; the `special.den` block.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Totals {
    pub runs: f64,
    pub hounds_spawned: f64,
    pub hounds_killed: f64,
    pub opals: f64,
    pub fear_gained: f64,
    pub player_kills: f64,
}

/// When one hound wave started, on the run's own clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wave {
    /// `den.wave`: the wave number, from one.
    pub wave: u32,
    /// The simulated milliseconds the wave spawned at.
    pub t_ms: u64,
}

/// What the panel shows for one projected attempt.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    pub snapshot: Snapshot,
    pub totals: Totals,
    /// Every wave's start, so a replay can step between them.
    pub waves: Vec<Wave>,
}

impl Outcome {
    /// The status line the panel prints under the score.
    pub fn status(&self) -> &'static str {
        if self.snapshot.active == 1.0 {
            "In progress"
        } else if self.snapshot.active == 0.5 {
            "Ending"
        } else if self.snapshot.runs > 0.0 {
            "Ended"
        } else {
            "Not started"
        }
    }
}

/// One live hound.
#[derive(Clone, Copy, Debug)]
struct Hound {
    hp_tot: f64,
    hp: f64,
    /// `persNum`: the 100 ms timer's frame counter.
    pers: f64,
    /// `persNum2`: the wave the hound spawned on, which its score reads.
    pers2: f64,
    /// `StatusSkull5`: the hound pays the amplifier's token count.
    skull5: bool,
    /// `StatusSkull3`: the kill restarts the Skull chain.
    skull3: bool,
    alive: bool,
    /// Steps until the hound's 100 ms timer fires.
    tick: u64,
    /// `-69.42069` marks a hound the attempt removed without rewards.
    removed: bool,
}

/// Replay one Den attempt at the kill interval and duration a run measured.
pub fn project(config: &Config, kill_every_ms: u64, duration_ms: u64, seed: u64) -> Outcome {
    let amp = |kind: usize| amp_debuff(&config.amps, kind);
    let amp_multi = amp_multi(&config.amps, config.b_upg23);
    let sbb20 = config.bolaia_level * bolaia_per_level();
    let chain_pending = config.bolaia_level >= 1.0;
    let mut rng = Rng::new(seed);
    let steps = (duration_ms + STEP_MS / 2) / STEP_MS;

    let mut snapshot = Snapshot {
        fear_max: FEAR_MAX,
        amp_multi,
        runs: 1.0,
        ..Snapshot::default()
    };
    let mut totals = Totals {
        runs: 1.0,
        ..Totals::default()
    };
    snapshot.active = 1.0;
    // `startDen`: the first hound carries the Bolaia Skull chain.
    let mut chain_pending = chain_pending;
    snapshot.fear = (FEAR_MAX * amp(6) / 100.0).min(FEAR_MAX - 1.0);
    let mut countdown = 0.0;
    let mut flash: f64 = 0.0;
    let mut best = config.best_score;
    let mut opal_level = config.opal_level;
    let mut tokens: Vec<f64> = Vec::new();
    let mut hounds: Vec<Hound> = Vec::new();
    let mut kill_in = kill_every_ms as i64;
    let mut reset_in: Option<u64> = None;
    let mut player_kill_in: Option<u64> = None;
    let mut player_kills = 0.0;
    let mut waves: Vec<Wave> = Vec::new();

    for step in 0..steps {
        // The scheduler's due tasks, in creation order: the stand-in's kill
        // tick first, then the hounds' 100 ms timers, then the attempt's tail.
        kill_in -= STEP_MS as i64;
        if kill_in <= 0 {
            kill_in += kill_every_ms as i64;
            if let Some(hound) = hounds
                .iter_mut()
                .find(|hound| hound.alive && hound.hp > 0.0 && !hound.removed)
            {
                hound.hp = hound.hp.min(0.0) - 1.0;
                hound.alive = false;
                let pers2 = hound.pers2;
                let skull3 = hound.skull3;
                snapshot.score += 10.0
                    * 1.1f64.powf((pers2 / 100.0).floor())
                    * (1.0 + 20.0 * pers2 / (pers2 + 1e3))
                    * amp_multi.max(1.0)
                    * (1.0 + snapshot.chain * sbb20 / 100.0);
                snapshot.kills += 1.0;
                if skull3 {
                    chain_pending = true;
                    snapshot.chain += 1.0;
                    countdown = 1.0;
                }
                totals.hounds_killed += 1.0;
            }
        }
        for hound in &mut hounds {
            if !hound.alive || hound.removed {
                continue;
            }
            hound.tick -= STEP_MS;
            if hound.tick > 0 {
                continue;
            }
            hound.tick += 100;
            // `_caveAInit`, the per-hound 100 ms task.
            let p = hound.pers.max(0.0);
            if p % 30.0 == 29.0 && amp(2) > 0.0 {
                hound.hp = hound.hp_tot.min(hound.hp + hound.hp_tot * amp(2) / 100.0);
            }
            let period = 50.0 - amp(7);
            if p % period == (49.0 - 10.0 * amp(7)).round() {
                let count = if hound.skull5 { amp(5) } else { 1.0 };
                for _ in 0..(count as i64) {
                    rng.int(-55, 55);
                    rng.int(-40, -78);
                }
                tokens.extend(std::iter::repeat_n(0.0, count as usize));
            }
            hound.pers = (hound.pers + 1.0).round();
        }
        if let Some(remaining) = reset_in.as_mut() {
            *remaining = remaining.saturating_sub(STEP_MS);
            if *remaining == 0 {
                snapshot.active = 0.0;
                reset_in = None;
            }
        }
        if let Some(remaining) = player_kill_in.as_mut() {
            *remaining = remaining.saturating_sub(STEP_MS);
            if *remaining == 0 {
                player_kills += 1.0;
                player_kill_in = None;
            }
        }

        // `_denFrame`, the scene updater.
        if flash > 0.0 {
            flash = (flash - 9.0).max(0.0);
        }
        if snapshot.active == 1.0 && snapshot.fear >= FEAR_MAX {
            // `_denEnd`.
            snapshot.active = 0.5;
            if snapshot.score > best {
                best = snapshot.score;
                let mut gained = 0.0;
                while best >= next_opal_score(opal_level) {
                    opal_level += 1.0;
                    gained += 1.0;
                }
                snapshot.opals += gained;
                totals.opals += gained;
            }
            reset_in = Some(RESET_MS);
            // `later(1e3, () => later(2200, …))`: the inner task is created by
            // a scheduler callback, so it fires one step earlier than 2.2 s.
            player_kill_in = Some(1_000 + 2_190);
            for hound in &mut hounds {
                if hound.alive {
                    hound.hp = -69.42069;
                    hound.alive = false;
                    hound.removed = true;
                }
            }
            tokens.clear();
            snapshot.tokens = 0.0;
        }
        if snapshot.active != 1.0 {
            continue;
        }
        countdown -= 1.0;
        if !hounds
            .iter()
            .any(|hound| hound.alive && hound.hp > 0.0 && !hound.removed)
        {
            countdown = -1.0;
        }
        if countdown < 0.0 {
            let amped = if rng.float() < 0.1 && amp(3) > 1.0 {
                1.0
            } else {
                0.0
            };
            snapshot.wave += 1.0;
            waves.push(Wave {
                wave: snapshot.wave as u32,
                t_ms: step * STEP_MS,
            });
            countdown = (snapshot.wave + 200.0) / (5.0 * snapshot.wave + 80.0) * 150.0;
            let hp = config.hound_hp * (amp(3) * amped).max(1.0) * 1.08f64.powf(snapshot.wave);
            // `createMonster(sim, 'caveA', 1, randomInt(99, 861), 'CaveHound')`:
            // the x roll, `_init`'s velocity roll, then `_caveHound`'s size roll.
            rng.int(99, 861);
            rng.int(-1, 1);
            rng.int(0, 70);
            let mut hound = Hound {
                hp_tot: hp,
                hp,
                pers: -10.0,
                pers2: snapshot.wave,
                skull5: false,
                skull3: false,
                alive: true,
                tick: 100,
                removed: false,
            };
            if chain_pending {
                hound.skull3 = true;
                chain_pending = false;
            } else if rng.float() < 0.05 && amp(5) > 1.0 {
                hound.skull5 = true;
            }
            totals.hounds_spawned += 1.0;
            hounds.push(hound);
        }
        // Fear tokens: +1 fear 345 steps after creation.
        let mut index = 0;
        while index < tokens.len() {
            tokens[index] += 1.0;
            if 250.0 < tokens[index] && tokens[index] < 1e5 {
                tokens[index] = 1e5;
            }
            if tokens[index] > 100_093.0 {
                snapshot.fear += 1.0;
                flash = 180.0;
                totals.fear_gained += 1.0;
                tokens.remove(index);
            } else {
                index += 1;
            }
        }
        snapshot.tokens = tokens.len() as f64;
    }
    totals.player_kills = player_kills;
    Outcome {
        snapshot,
        totals,
        waves,
    }
}
