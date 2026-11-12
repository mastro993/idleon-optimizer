//! The overview's counters: what the save holds across every world.

use super::*;

impl RoyalGuardian {
    /// The overview's counters.
    pub fn counters(&self) -> Counters {
        let outposts: Vec<Outpost> = (0..WORLDS)
            .flat_map(|world| self.world_outposts(world))
            .collect();
        let built: Vec<&Outpost> = outposts.iter().filter(|outpost| outpost.built).collect();
        let mut militia_clearing = 0;
        let mut militia_clears_per_hour = 0.0;
        for outpost in outposts.iter().filter(|outpost| !outpost.built) {
            let clear = self.clear(outpost.map);
            if clear.state == ClearState::Clearing {
                militia_clearing += 1;
                militia_clears_per_hour += clear.rate_per_hour;
            }
        }
        Counters {
            built_outposts: built.len(),
            outposts: outposts.len(),
            worlds_unlocked: self.worlds_unlocked(),
            armory_levels: self.armory_levels(),
            units: built
                .iter()
                .map(|outpost| self.total_units(outpost.map))
                .sum(),
            purified: built
                .iter()
                .filter(|outpost| self.purified(outpost.map))
                .count(),
            support: built
                .iter()
                .filter(|outpost| self.outpost_type(outpost.map) == 1.0)
                .count(),
            savage: built
                .iter()
                .filter(|outpost| self.outpost_type(outpost.map) == 2.0)
                .count(),
            militia_clearing,
            militia_clears_per_hour,
        }
    }
}
