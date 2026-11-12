//! The outposts' saved connections: which resource a Normal or Savage outpost
//! drains, which outpost a Support outpost links, and what each one reaches.
//!
//! Ported from `parseConnectionEndpoint`, `outpostConnections`, `outpostSupports`,
//! `outpostSupportLinks`, `supportCount`, `outpostDistance`,
//! `reachableResourcesForOutpost`, `reachableOutpostsForResource` and
//! `supportTargetsForOutpost`.
//!
//! A saved cell encodes both endpoint kinds: a value from 1,000 up is a map
//! (minus the offset), a smaller non-negative value is a resource index, and
//! anything else - including the `-1` the game writes for an empty slot - is no
//! connection at all.

use super::*;

/// How much reach a Support outpost needs beyond the target's anchor.
const SUPPORT_ALLOWANCE: f64 = 8.0;

/// What one saved connection endpoint points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endpoint {
    /// The slot holds no connection.
    Empty,
    Resource(usize),
    Map(usize),
}

impl Endpoint {
    /// The upstream endpoint kind, for the oracle comparison.
    pub fn kind(self) -> &'static str {
        match self {
            Endpoint::Empty => "empty",
            Endpoint::Resource(_) => "resource",
            Endpoint::Map(_) => "map",
        }
    }

    /// The raw endpoint value the game writes back for this target.
    pub fn encode(self) -> i64 {
        match self {
            Endpoint::Empty => -1,
            Endpoint::Resource(index) => index as i64,
            Endpoint::Map(map) => 1_000 + map as i64,
        }
    }
}

/// One of an outpost's two saved connection slots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Connection {
    pub slot: usize,
    /// The saved cell, when the row carries one.
    pub raw: Option<f64>,
    pub endpoint: Endpoint,
}

/// One Support outpost's link to a target map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupportLink {
    pub map: usize,
    /// The slot the link sits in; the second slot is the legacy one.
    pub slot: usize,
    pub legacy: bool,
}

/// One resource node a built outpost can link, with the distance it measures.
#[derive(Clone, Debug, PartialEq)]
pub struct ReachableResource {
    pub resource: usize,
    pub distance: f64,
    /// The outpost's own range, before the resource allowance.
    pub range: f64,
    /// How many slots already link a resource.
    pub connected_slots: usize,
    pub reachable: bool,
}

/// One built outpost that can link a resource node.
#[derive(Clone, Debug, PartialEq)]
pub struct ReachableOutpost {
    pub map: usize,
    pub distance: f64,
    pub range: f64,
    pub slots_used: usize,
    pub slots_free: usize,
    /// How many of its slots already link this node.
    pub connected_count: usize,
    pub reachable: bool,
}

/// One map a Support outpost can link.
#[derive(Clone, Debug, PartialEq)]
pub struct SupportTarget {
    pub map: usize,
    pub distance: f64,
    pub range: f64,
    /// The reach a Support link needs at this distance.
    pub radius: f64,
    pub built: bool,
    /// Whether the target's row is far enough along to take a link.
    pub linkable: bool,
    /// How many of the Support outpost's slots already link this target.
    pub linked_count: usize,
    pub reachable: bool,
}

impl RoyalGuardian {
    /// Decode one saved endpoint: upstream `parseConnectionEndpoint`.
    pub fn parse_endpoint(value: Option<f64>) -> Endpoint {
        let Some(value) = value else {
            return Endpoint::Empty;
        };
        if !value.is_finite() || value < 0.0 {
            return Endpoint::Empty;
        }
        if value >= 1_000.0 {
            Endpoint::Map((value - 1_000.0) as usize)
        } else {
            Endpoint::Resource(value as usize)
        }
    }

    /// An outpost's two saved connections: upstream `outpostConnections`.
    pub fn outpost_connections(&self, map: usize) -> [Connection; 2] {
        std::array::from_fn(|slot| {
            let raw = self.map_saved(map, cell::LINKS + slot);
            Connection {
                slot,
                raw,
                endpoint: Self::parse_endpoint(raw),
            }
        })
    }

    /// Whether either slot carries a connection: upstream `outpostSupports`.
    pub fn outpost_supports(&self, map: usize) -> bool {
        self.outpost_connections(map)
            .iter()
            .any(|connection| connection.endpoint != Endpoint::Empty)
    }

    /// Every Support outpost that links `target`: upstream `outpostSupportLinks`.
    pub fn support_links(&self, target: usize) -> Vec<SupportLink> {
        let mut links = Vec::new();
        for map in 0..self.maps.len() {
            if !self.outpost(map).built || self.outpost_type(map) != 1.0 {
                continue;
            }
            for connection in self.outpost_connections(map) {
                if connection.endpoint == Endpoint::Map(target) {
                    links.push(SupportLink {
                        map,
                        slot: connection.slot,
                        legacy: connection.slot == 1,
                    });
                }
            }
        }
        links
    }

    /// How many Support outposts link `target`: upstream `supportCount`.
    pub fn support_count(&self, target: usize) -> usize {
        self.support_links(target).len()
    }

    /// How far two map anchors sit apart: upstream `outpostDistance`.
    pub fn outpost_distance(from: usize, to: usize) -> f64 {
        let (Some((x1, y1)), Some((x2, y2))) =
            (Self::outpost_position(from), Self::outpost_position(to))
        else {
            return f64::INFINITY;
        };
        ((x1 - x2).powi(2) + (y1 - y2).powi(2)).sqrt()
    }

    /// The resource nodes an outpost can link, nearest first: upstream
    /// `reachableResourcesForOutpost`.
    pub fn reachable_resources(&self, map: usize) -> Vec<ReachableResource> {
        let range = self.range(map);
        let connected_slots = self
            .outpost_connections(map)
            .iter()
            .filter(|connection| matches!(connection.endpoint, Endpoint::Resource(_)))
            .count();
        let world = Self::world_of(map);
        let mut nodes: Vec<ReachableResource> = (0..data::resource_count())
            .filter(|resource| Self::resource_world(*resource) == world)
            .map(|resource| ReachableResource {
                resource,
                distance: Self::resource_distance(map, resource),
                range,
                connected_slots,
                reachable: self.resource_reachable(map, resource),
            })
            .collect();
        nodes.sort_by(|left, right| left.distance.total_cmp(&right.distance));
        nodes
    }

    /// The built outposts that can link a resource node, nearest first:
    /// upstream `reachableOutpostsForResource`.
    pub fn reachable_outposts(&self, resource: usize) -> Vec<ReachableOutpost> {
        let world = Self::resource_world(resource);
        let mut outposts: Vec<ReachableOutpost> = (0..self.maps.len())
            .filter(|map| {
                Self::world_of(*map) == world
                    && self.outpost(*map).built
                    && matches!(self.outpost_type(*map), 0.0 | 2.0)
            })
            .map(|map| {
                let connections = self.outpost_connections(map);
                let used = connections
                    .iter()
                    .filter(|connection| connection.endpoint != Endpoint::Empty)
                    .count();
                ReachableOutpost {
                    map,
                    distance: Self::resource_distance(map, resource),
                    range: self.range(map),
                    slots_used: used,
                    slots_free: 2usize.saturating_sub(used),
                    connected_count: connections
                        .iter()
                        .filter(|connection| connection.endpoint == Endpoint::Resource(resource))
                        .count(),
                    reachable: self.resource_reachable(map, resource),
                }
            })
            .collect();
        outposts.sort_by(|left, right| left.distance.total_cmp(&right.distance));
        outposts
    }

    /// The maps a Support outpost can link: upstream `supportTargetsForOutpost`.
    ///
    /// The connection editor accepts a target whose row has grown to its second
    /// cell - in progress or built - within `floor(range) + 8` of the anchors.
    pub fn support_targets(&self, map: usize) -> Vec<SupportTarget> {
        if !self.outpost(map).built || self.outpost_type(map) != 1.0 {
            return Vec::new();
        }
        let range = self.range(map);
        let linked: Vec<usize> = self
            .outpost_connections(map)
            .iter()
            .filter_map(|connection| match connection.endpoint {
                Endpoint::Map(target) => Some(target),
                _ => None,
            })
            .collect();
        let world = Self::world_of(map);
        let mut targets: Vec<SupportTarget> = (world * MAPS_PER_WORLD
            ..(world + 1) * MAPS_PER_WORLD)
            .filter(|target| *target != map && Self::outpost_position(*target).is_some())
            .map(|target| {
                let distance = Self::outpost_distance(map, target);
                let linkable = self.map_row(target).len() >= 2;
                SupportTarget {
                    map: target,
                    distance,
                    range,
                    radius: range + SUPPORT_ALLOWANCE,
                    built: self.outpost(target).built,
                    linkable,
                    linked_count: linked.iter().filter(|linked| **linked == target).count(),
                    reachable: linkable && distance <= range + SUPPORT_ALLOWANCE,
                }
            })
            .collect();
        targets.sort_by(|left, right| left.distance.total_cmp(&right.distance));
        targets
    }
}
