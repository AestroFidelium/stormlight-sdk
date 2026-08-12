//! Animation notify points (stormlight/server#76) — feedback timed to the
//! *animation* rather than to the game event that started it.
//!
//! An ability's burst fired at cast time goes off while the character is still
//! winding up; a footstep played on a move order goes off while the foot is still
//! in the air. A notify moves that decision into the clip: the mod marks the
//! frame the swing connects, and the effect is spawned when playback crosses it —
//! at whatever rate the clip is running, however many times it loops.
//!
//! ## Strictly cosmetic
//!
//! A notify may spawn a visual and it may wake the mod's own client-side code. It
//! may **not** touch simulation state, and nothing about it is replicated: it
//! fires on each client from that client's own playback, so two machines running
//! at different frame rates never disagree about anything that matters. A notify
//! that is missed (an interrupted clip, art that failed to load) costs a puff of
//! smoke and nothing else — which is what makes it safe to skip one rather than
//! stall a frame trying.
//!
//! ## Naming what it spawns
//!
//! An effect notify names a **cosmetic effect key** — a string the same mod
//! declared a [`VisualModel`](crate::visuals::VisualModel) for, the way a
//! [`ClipRef`](crate::animation::ClipRef) names a clip inside a container. Not the
//! `(ability, role)` key ability feedback uses
//! ([`EffectVisualDescriptor`](crate::visuals::EffectVisualDescriptor)): a
//! footstep, a weapon trail or a landing puff belongs to no ability, and keying
//! them by one would mean inventing gameplay content to hang a cosmetic on. The
//! host qualifies each key with its declaring mod at adoption, so two packages may
//! both call their own effect `footstep`.
//!
//! Pure, serializable data like the rest of the ABI. The mod-defined
//! [`EventId`](crate::ids::EventId) a trigger notify names is authored in the mod's
//! **local** id space and remapped to global at adoption (see [`crate::remap`]).

use alloc::string::String;

use serde::{Deserialize, Serialize};

use crate::animation::BonePath;
use crate::ids::EventId;
use crate::remap::{IdMap, RemapIds};

/// When during a clip a notify fires.
///
/// Both forms exist because both are authored, and they answer differently when
/// the art changes: [`Self::Normalized`] rides along with a re-timed clip (a swing
/// re-exported 20% slower still connects at the same *pose*), while
/// [`Self::Seconds`] pins an offset that must not stretch — a lead-in that has to
/// stay a quarter second whatever the clip does.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum NotifyTime {
    /// A fraction of the clip's authored length, `0.0..=1.0`.
    Normalized(f32),
    /// Seconds from the clip's start.
    Seconds(f32),
}

impl NotifyTime {
    /// Where this lands in a clip of `duration` seconds.
    ///
    /// A fraction is resolved against the length the art turned out to have; an
    /// absolute time is itself. Total — a time past the end of the clip resolves
    /// to a moment playback never reaches, which the runtime reports rather than
    /// treating as an error here (the length is not knowable until the art loads).
    #[must_use]
    pub fn seconds(self, duration: f32) -> f32 {
        match self {
            Self::Normalized(f) => f * duration,
            Self::Seconds(s) => s,
        }
    }

    /// Whether this is a time a clip could actually reach: a finite, non-negative
    /// offset, or a fraction inside the clip.
    #[must_use]
    pub fn is_reachable(self) -> bool {
        match self {
            Self::Normalized(f) => f.is_finite() && (0.0..=1.0).contains(&f),
            Self::Seconds(s) => s.is_finite() && s >= 0.0,
        }
    }
}

/// Where a notify's effect is spawned.
///
/// The socket is the point of the whole feature for anything held or worn: a
/// weapon trail or a hand glow parented to the character's root slides off the
/// hand the moment the arm moves, because the root is exactly the one transform
/// the animation does *not* change.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum NotifyAttach {
    /// On the character itself, at its origin.
    Root,
    /// On a named bone, so the effect is carried by the animation.
    Socket {
        /// The bone, by the same name path a [`MaskGroup`](crate::animation::MaskGroup)
        /// uses — as the exporter wrote it.
        bone: BonePath,
        /// Offset from that bone, in the bone's own space.
        offset: [f32; 3],
    },
}

/// What a notify does when playback reaches it.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum NotifyAction {
    /// Spawn the cosmetic effect declared under `key`
    /// ([`NamedEffect`](crate::visuals::NamedEffect)), attached as `attach` says
    /// and retired after `lifetime` seconds.
    Effect {
        /// The declaring mod's own name for the effect.
        key: String,
        /// Where it is spawned.
        attach: NotifyAttach,
        /// How long it lives, in seconds. Non-positive means the host's default.
        lifetime: f32,
    },
    /// Announce a mod-defined event to that mod's own client-side code.
    ///
    /// Declared now and dispatched to nothing yet: cosmetic mods are registered
    /// but never invoked at runtime (there is no client-side guest entry point —
    /// see [`crate::runtime`] for the server's). The runtime reports each declared
    /// trigger so a mod author is told it is inert instead of watching for an
    /// effect that can never arrive.
    Trigger {
        /// The mod-defined event to announce.
        event: EventId,
    },
}

/// One notify: a time on a clip and what happens there.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct NotifyPoint {
    /// When during the clip it fires.
    pub at: NotifyTime,
    /// What it does.
    pub action: NotifyAction,
}

impl NotifyAction {
    /// Whether every number this action carries is finite. A non-finite lifetime
    /// would spawn an effect that never retires — one leak per swing.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        match self {
            Self::Effect { lifetime, .. } => lifetime.is_finite(),
            Self::Trigger { .. } => true,
        }
    }
}

impl RemapIds for NotifyAction {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        // The effect key is a string and the socket is a bone name — content, not
        // handles. The event is the one interned thing a notify carries.
        if let Self::Trigger { event } = self {
            *event = m.event(*event)?;
        }
        Ok(())
    }
}

impl RemapIds for NotifyPoint {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.action.remap_ids(m)
    }
}
