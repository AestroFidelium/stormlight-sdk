//! Map scenery — the static art a map is dressed in.
//!
//! A map's *geometry* ([`crate::navmesh`]) says where units may stand, and its
//! *placements* ([`crate::placement`]) say who stands there. Neither says what the
//! place looks like: the ground, the walls and buildings around the lanes, the
//! trees and clutter between them. That is this — purely cosmetic, declared by a
//! map's client half, and never seen by the simulation.
//!
//! The engine learns nothing about what a piece *is*. It draws an asset at each
//! of its placements, and loops a clip on it if one is named. Two shapes of
//! content fit that one description, and a map is expected to use both:
//!
//! - **Merged art** — geometry the map's authoring pipeline has already moved into
//!   world space and combined, one piece per region of the map. Placed once, at
//!   [`SceneryPlacement::IDENTITY`], and cheap to draw because it arrives merged.
//! - **Instanced art** — one model standing in many places: something that
//!   animates in place, or carries effects, and so cannot be merged without losing
//!   what makes it that model. Placed many times; every placement shares the one
//!   loaded asset.
//!
//! Which is which is the mod's call. The engine draws both the same way.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::visuals::ModelClips;

/// Where one copy of a piece of scenery stands, in world space (`Y` up, `-Z`
/// forward, like every other placement the engine draws).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct SceneryPlacement {
    /// World position of the asset's origin.
    pub translation: [f32; 3],
    /// Orientation as a unit quaternion `[x, y, z, w]`. A full rotation rather
    /// than a yaw: map art is tilted as often as it is turned — a fallen barrel,
    /// a leaning post.
    pub rotation: [f32; 4],
    /// Per-axis scale.
    pub scale: [f32; 3],
}

impl SceneryPlacement {
    /// The neutral placement: art already authored in world space.
    pub const IDENTITY: Self =
        Self { translation: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3] };
}

/// One piece of scenery: an asset and every place it stands.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SceneryPiece {
    /// The art, as a `mod://<id>/<path>` URL.
    pub asset: String,
    /// What the art plays on its own; empty for still geometry.
    pub clips: ModelClips,
    /// Every place it stands. Empty draws nothing, which is an ordinary answer
    /// for a piece a map declares and then has no room for.
    pub placements: Vec<SceneryPlacement>,
}

/// The height a map's units are **drawn** standing at, as a regular grid of samples
/// on the ground plane (world `XZ`).
///
/// The simulation is planar and stays so; this only lifts the art. Without it every
/// unit is drawn at `y = 0`, which is right on open ground and wrong anywhere the
/// walkable surface is not the ground — a bridge deck, a raised platform — where a
/// unit would walk *inside* the art. Sampled bilinearly; a point off the grid reads
/// the nearest edge.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct HeightField {
    /// World `XZ` of sample `(0, 0)`.
    pub origin: [f32; 2],
    /// Spacing between neighbouring samples, world units.
    pub cell: f32,
    /// Samples along `X` and along `Z`.
    pub size: [u32; 2],
    /// Heights, row-major: `heights[j * size[0] + i]` is at `origin + (i, j) · cell`.
    pub heights: Vec<f32>,
}

impl HeightField {
    /// Whether the field is usable: a positive finite spacing, at least one
    /// sample, a sample count that matches its size, and every height finite.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let [w, h] = self.size;
        self.cell.is_finite()
            && self.cell > 0.0
            && self.origin.iter().all(|v| v.is_finite())
            && w > 0
            && h > 0
            && (w as usize).checked_mul(h as usize) == Some(self.heights.len())
            && self.heights.iter().all(|v| v.is_finite())
    }

    /// The drawn ground height at world point `p` (`[x, z]`). `0.0` for an
    /// invalid field.
    #[must_use]
    pub fn sample(&self, p: [f32; 2]) -> f32 {
        if !self.is_valid() {
            return 0.0;
        }
        let [w, h] = self.size;
        let fx = ((p[0] - self.origin[0]) / self.cell).clamp(0.0, (w - 1) as f32);
        let fz = ((p[1] - self.origin[1]) / self.cell).clamp(0.0, (h - 1) as f32);
        let (i0, j0) = (fx as u32, fz as u32);
        let (i1, j1) = ((i0 + 1).min(w - 1), (j0 + 1).min(h - 1));
        let (tx, tz) = (fx - i0 as f32, fz - j0 as f32);
        let at = |i: u32, j: u32| self.heights[(j * w + i) as usize];
        let a = at(i0, j0) + (at(i1, j0) - at(i0, j0)) * tx;
        let b = at(i0, j1) + (at(i1, j1) - at(i0, j1)) * tx;
        a + (b - a) * tz
    }
}
