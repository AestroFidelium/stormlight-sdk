//! How long a transient cosmetic effect lives, and what it does when the unit
//! carrying it goes (stormlight/server#167).
//!
//! A burst where a shot lands, or a puff an animation notify spawns, is a drawing
//! with nothing in the simulation behind it: no server despawn ends it, so the
//! client has to decide when it is over. That decision used to be two constants in
//! the client, which a mod could not change, and an effect whose art ran longer
//! than its constant was cut off mid-play. It is a declaration now.
//!
//! How the effect *ends* is a different question, and it belongs to the art: a
//! [`ModelClips::death`](crate::visuals::ModelClips::death) clip is played when the
//! effect retires, whatever retired it. Nothing here decides that.

use serde::{Deserialize, Serialize};

/// How long a transient effect lives before it retires.
///
/// Read by the effects nothing else ends: an ability's
/// [`Impact`](crate::visuals::EffectRole::Impact) and
/// [`Miss`](crate::visuals::EffectRole::Miss) bursts, and a
/// [`NamedEffect`](crate::visuals::NamedEffect) an animation notify spawns. A shot
/// in flight, a cast indicator and a body on the ground live exactly as long as the
/// thing they draw, and ignore it.
///
/// Retiring is not vanishing: art with a closing clip plays it once its life is up
/// and is gone when the clip is.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum EffectLifetime {
    /// The client's own number for the kind of drawing: short for the engine's
    /// tinted placeholder, long enough for a typical burst of art to play out for
    /// art a mod supplied. What every effect lived before this could be declared.
    #[default]
    Default,
    /// Exactly this many seconds.
    Seconds(f32),
    /// As long as the art plays: until its opening clip has finished **and** the
    /// last particle its one-shot emitters fired has died. A container with
    /// neither — one that only loops, or that carries nothing the client can
    /// measure — lives the default.
    ///
    /// The art's own length rather than a number copied out of it, so re-exporting
    /// the art longer or shorter never leaves the declaration cutting it off.
    Art,
}

impl EffectLifetime {
    /// Whether a clock can run this lifetime: an explicit duration must be a
    /// positive, finite number of seconds. A zero would retire the effect on the
    /// frame it appeared, and an infinite one would never retire it — one leak per
    /// hit.
    #[must_use]
    pub fn is_runnable(self) -> bool {
        match self {
            Self::Seconds(seconds) => seconds.is_finite() && seconds > 0.0,
            Self::Default | Self::Art => true,
        }
    }

    /// The seconds this lifetime comes to, given the client's `default` for the
    /// drawing and the art's measured length, if it could be measured.
    #[must_use]
    pub fn seconds(self, default: f32, art: Option<f32>) -> f32 {
        match self {
            Self::Default => default,
            Self::Seconds(seconds) => seconds,
            Self::Art => art.unwrap_or(default),
        }
    }
}

/// What an effect hung on a unit does when that unit goes down, or is removed, while
/// the effect still has life of its own left.
///
/// Only an effect that outlives the moment it was spawned has the question to
/// answer — today a [`NamedEffect`](crate::visuals::NamedEffect) an animation
/// notify hangs on a character. Neither answer is right for every effect: a glow
/// on a weapon belongs to the weapon and goes where it goes, and a puff of dust
/// kicked up by a step belongs to the ground the step was on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum HostEnd {
    /// Stay on it: ride the body down as it falls, and retire with it when it is
    /// removed. What every effect did before this could be declared.
    #[default]
    Follow,
    /// Leave it: stay where it was in the world the moment the unit went down or was
    /// removed, and finish its own life there.
    Detach,
}
