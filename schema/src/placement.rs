//! Unit placement (stormlight/server#60) — how a map says what stands on it.
//!
//! Geometry ([`crate::navmesh`]) describes where a unit *may* stand. That is only
//! half a battlefield: a map with nothing on it is an empty field, and there is
//! nothing to aim at, walk around, or kill. A [`UnitPlacement`] is the other half
//! — the map naming one of its own units, a side, a spot, and a direction to face.
//!
//! It stays as declarative as the geometry beside it. The engine reads placements
//! at startup and spawns exactly what they say; it never invents one, and it never
//! learns what the placed unit *is*. A training dummy, a neutral camp, a
//! destructible objective and a lane's worth of creeps are all this one shape.
//!
//! The placed unit is named by [`UnitId`] — the map mod's own local handle, remapped
//! to global at adoption like every other id. That scopes a placement to the mod
//! that declares it: a map places units it ships, not another mod's.

use serde::{Deserialize, Serialize};

use crate::ids::UnitId;
use crate::navmesh::Point2;
use crate::remap::{IdMap, RemapIds};

/// One unit the map stands up when the match begins.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct UnitPlacement {
    /// Which unit to spawn. A handle the declaring mod interned; an id no loaded
    /// mod resolves is reported and skipped, never fatal.
    pub unit: UnitId,
    /// The side it fights for. A runtime allegiance, so it is stated here rather
    /// than in the unit descriptor — the same unit can stand on either side of a
    /// map, or on none (`0`, the neutral team).
    pub team: u32,
    /// Where it stands, on the ground plane (world `XZ`), like the geometry around
    /// it. A spot outside the walkable region is the map's mistake to make; the
    /// engine spawns what it is told.
    pub at: Point2,
    /// Which way it faces at spawn — a yaw in radians about the up axis, `0`
    /// looking down `-Z`. Cosmetic for a target dummy, load-bearing for anything
    /// with a firing arc.
    pub facing: f32,
    /// How long after dying it comes back, in seconds. `None` means it stays down
    /// once killed — the honest default for a one-shot objective. Declared here so
    /// a map can state the whole lifecycle in one place; consumed by the death and
    /// respawn phases.
    pub respawn: Option<f32>,
}

impl RemapIds for UnitPlacement {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        // The placed unit is the only interned handle here — team, position,
        // facing and the respawn delay are plain numbers.
        self.unit = m.unit(self.unit)?;
        Ok(())
    }
}
