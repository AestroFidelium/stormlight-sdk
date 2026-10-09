//! A sound, as one more thing a cosmetic declaration can lay down
//! (stormlight/server#176).
//!
//! A sound is placed, lives and dies exactly as a drawing does: hung on what it
//! dresses, kept for the effect's life, gone with the thing that held it. So it is
//! not a parallel system with its own placement and lifetime rules but another
//! payload of [`VisualModel`](crate::visuals::VisualModel) — declared wherever a
//! drawing can be, on any moment a drawing can mark.
//!
//! A MOBA's sound is positional: what happens across the field is quieter than
//! what happens in front of the player. How quickly is the sound's own business —
//! a footstep is gone in a few steps, a siege engine's shot carries across a lane —
//! so each sound declares its own [`SoundFalloff`].

use serde::{Deserialize, Serialize};

/// Whether a sound plays through once or for as long as it lasts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SoundPlayback {
    /// Once, to its end, even if what made it is gone by then — a blow is heard
    /// out whatever became of the shot.
    #[default]
    Once,
    /// Over and over for as long as the thing that holds it lasts, and not a moment
    /// longer: a looping hum stops with what hums.
    Loop,
}

/// How a sound fades with its distance from the listener, in world units.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct SoundFalloff {
    /// Heard at full volume within this distance.
    pub near: f32,
    /// Not heard at all past this one; fading evenly between the two.
    pub far: f32,
}

impl SoundFalloff {
    /// Whether a mixer can apply it: finite, the near edge at zero or beyond, the
    /// far edge no nearer than the near one, and something heard somewhere.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.near.is_finite()
            && self.far.is_finite()
            && self.near >= 0.0
            && self.far >= self.near
            && self.far > 0.0
    }

    /// The share of its volume a sound keeps `distance` world units from the
    /// listener: `1` within the near edge, `0` past the far one, linear between.
    #[must_use]
    pub fn gain_at(&self, distance: f32) -> f32 {
        if distance <= self.near {
            return 1.0;
        }
        if distance >= self.far {
            return 0.0;
        }
        ((self.far - distance) / (self.far - self.near)).clamp(0.0, 1.0)
    }
}
