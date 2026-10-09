//! Where on a host's rig a visual is hung (stormlight/server#160).
//!
//! A character's art already knows where things belong on it: the authored
//! attachment points of its skeleton — a weapon, a hand, the chest, the head. A
//! cosmetic mod hangs a visual on one **by asking for it**, not by indexing a bone:
//! [`AttachPoint`] names the point the visual wants and, optionally, a second one
//! to use where the rig carries no first. So one declaration serves rigs that name
//! their points differently or lack one entirely, which is what lets a visual
//! written for one character be worn by another.
//!
//! Strictly a drawing. The server has no skeleton and loads no art; a point moves
//! where a visual is *drawn*, never a hitbox, a range or a contact.
//!
//! A point the rig does not carry, and a fallback it does not carry either, puts
//! the visual on the host's own origin — the place every visual was hung before
//! points could be asked for — and the client says so once rather than on every
//! spawn.

use alloc::string::String;

use serde::{Deserialize, Serialize};

/// A request for a point on whatever rig a visual is hung on.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AttachPoint {
    /// The attachment point wanted, by the name the art authored — the name inside
    /// the point's own payload, as the converter wrote it.
    pub point: String,
    /// Asked for when the rig carries no `point`. `None` goes straight to the
    /// host's origin.
    pub fallback: Option<String>,
    /// Offset from the point, in the point's own space, in world units.
    pub offset: [f32; 3],
    /// Keep only the point's **heading**: the visual turns with the bone it hangs
    /// on but stays level however the bone is pitched or rolled. What a glow on a
    /// swung staff or a ring under a hand wants; a flare that should point down the
    /// shaft wants the whole rotation.
    pub upright: bool,
}

impl AttachPoint {
    /// A request for `point`, with no fallback, no offset, and the bone's full
    /// rotation.
    #[must_use]
    pub fn new(point: impl Into<String>) -> Self {
        Self { point: point.into(), fallback: None, offset: [0.0; 3], upright: false }
    }

    /// Whether the request can be asked of a rig at all: it names a point, its
    /// fallback (if any) names one too, and its offset is finite.
    ///
    /// A blank name is not a request for "no point" — that is a visual declared
    /// without an [`AttachPoint`] — so it is refused rather than read as the origin.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        !self.point.is_empty()
            && self.fallback.as_ref().is_none_or(|f| !f.is_empty())
            && self.offset.iter().all(|v| v.is_finite())
    }
}
