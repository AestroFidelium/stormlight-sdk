//! Navigation geometry (stormlight/server#52) — the walkable shape of a map, as
//! a mod ships it.
//!
//! The engine owns no map. It only knows how to *interpret* a walkable region:
//! content declares where units may stand, and the pathing stack turns that into
//! routes. So the ABI carries the smallest description a polygon navmesh can be
//! built from, and nothing about what the map *is*:
//!
//! - an **outline** — the walkable boundary, counter-clockwise;
//! - **obstacles** — holes punched in it (a building, a cliff, a wall);
//! - an **agent radius** — how far walkable space is pulled back from every edge,
//!   so a unit of that size never clips a corner.
//!
//! Geometry is `[f32; 2]` ground points (world `XZ`), plain arrays like the rest
//! of the ABI so the schema stays dependency-free for guest wasm. Height is not
//! part of it: the simulation is planar, and terrain elevation is cosmetic.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::ids::NavMeshId;
use crate::remap::{IdMap, RemapIds};

/// A ground point on the navigation plane (world `XZ`).
pub type Point2 = [f32; 2];

/// One walkable region of a map: an outline with holes, pre-inset for an agent
/// of `agent_radius`.
///
/// Winding is the usual polygon convention — the outline counter-clockwise, holes
/// the other way — but the engine's baker is tolerant of either, since a mod that
/// authors a wall backwards should get a wall, not a crash.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct NavMeshDescriptor {
    /// This mesh's interned handle, so units and abilities can name it.
    pub id: NavMeshId,
    /// The walkable boundary, in order. Fewer than three points is not a region
    /// and the engine rejects it at adoption rather than baking a degenerate mesh.
    pub outline: Vec<Point2>,
    /// Holes in the walkable region — each an ordered ring of ground points.
    pub obstacles: Vec<Vec<Point2>>,
    /// How far walkable space is pulled back from every edge, world units. The
    /// radius of the largest unit expected to path here; `0.0` leaves the region
    /// exactly as authored (units may then brush the geometry).
    pub agent_radius: f32,
}

impl RemapIds for NavMeshDescriptor {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        // Geometry holds no handles — the mesh's own id is the only one here.
        self.id = m.navmesh(self.id)?;
        Ok(())
    }
}
