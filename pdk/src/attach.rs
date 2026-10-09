//! Asking a rig for a point to hang a visual on (stormlight/server#160) — the
//! authoring half.
//!
//! ```ignore
//! use stormlight_mod_sdk::attach::{AttachExt, point};
//!
//! // The weapon if the rig has one, else the chest; level however the staff swings.
//! ctx.effect_visual_at("bolt", EffectRole::CastIndicator, flare, point("Ref_Weapon").or("Ref_Chest").upright());
//! ```

use alloc::string::ToString;

use stormlight_mod_abi::attach::AttachPoint;

/// A request for the attachment point `name`, with no fallback, no offset, and the
/// point's full rotation.
#[must_use]
pub fn point(name: &str) -> AttachPoint {
    AttachPoint::new(name)
}

/// The chaining half of a request.
pub trait AttachExt {
    /// Ask for `fallback` where the rig lacks the point.
    #[must_use]
    fn or(self, fallback: &str) -> Self;
    /// Offset from the point, in its own space, in world units.
    #[must_use]
    fn offset(self, offset: [f32; 3]) -> Self;
    /// Keep only the point's heading: level however the bone pitches or rolls.
    #[must_use]
    fn upright(self) -> Self;
}

impl AttachExt for AttachPoint {
    fn or(mut self, fallback: &str) -> Self {
        self.fallback = Some(fallback.to_string());
        self
    }
    fn offset(mut self, offset: [f32; 3]) -> Self {
        self.offset = offset;
        self
    }
    fn upright(mut self) -> Self {
        self.upright = true;
        self
    }
}
