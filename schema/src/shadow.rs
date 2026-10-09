//! How a drawn unit sits on the ground (stormlight/server#179).
//!
//! A character with nothing under it reads as pasted onto the ground rather than
//! standing on it. Two things anchor it: a soft **contact shadow** directly beneath
//! it, which moves with it and costs one quad, and — where the map declares a
//! shadowing light ([`crate::environment::Shadow`]) — the shadow its model casts.
//! Neither is automatic for every drawing: a flier, a ghost or a projection wants
//! none, and that has to be expressible.
//!
//! The engine ships no number here. A contact shadow sized to the drawing is
//! measured off the drawing; one of a declared size is the mod's.

use serde::{Deserialize, Serialize};

/// The shadow a drawn unit casts.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum ModelShadow {
    /// A contact shadow as wide as the drawing itself, and the model casts in the
    /// map's shadowing light. What a unit gets by saying nothing.
    #[default]
    Fit,
    /// A contact shadow of exactly this radius, world units — for art whose bounds
    /// overstate its footprint (a long cape, a raised weapon) — and the model
    /// casts.
    Radius(f32),
    /// No shadow at all: nothing beneath it, and its model casts nothing.
    None,
}

impl ModelShadow {
    /// Whether a renderer can draw it: a declared radius must be finite and
    /// positive.
    #[must_use]
    pub fn is_valid(self) -> bool {
        match self {
            Self::Radius(radius) => radius.is_finite() && radius > 0.0,
            Self::Fit | Self::None => true,
        }
    }

    /// Whether the model casts a shadow in a shadowing light.
    #[must_use]
    pub fn casts(self) -> bool {
        !matches!(self, Self::None)
    }
}
