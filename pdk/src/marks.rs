//! Declaring what lies on the ground under a unit because of what it is to the
//! player looking at it (stormlight/server#181) — the authoring half.
//!
//! ```ignore
//! use stormlight_mod_sdk::abi::unit_mark::{MarkRole, Relation};
//! use stormlight_mod_sdk::marks::mark;
//!
//! // A ring with a gap under the hero the player drives, pointing where it faces.
//! ctx.mark_units(mark(MarkRole::Driven, asset("ui/own_ring.png")).scale(1.3).turning());
//! // The specific case before the general one: declared order is precedence.
//! ctx.mark_units(mark(MarkRole::Target, asset("ui/target.png")).only(Relation::Ally).tint(GREEN));
//! ctx.mark_units(mark(MarkRole::Target, asset("ui/target.png")).tint(ORANGE));
//! ```

use alloc::string::String;

use stormlight_mod_abi::decal::DecalBlend;
use stormlight_mod_abi::unit_mark::{MarkLook, MarkRole, Relation, UnitMark};

/// A mark being declared — see [`mark`], and
/// [`ClientContext::mark_units`](crate::client::ClientContext::mark_units) to
/// declare it.
#[derive(Clone, Debug)]
#[must_use = "a mark is declared only once it is handed to the context"]
pub struct MarkSpec(pub(crate) UnitMark);

/// A mark under every unit playing `role`, whatever its relation: `asset` laid
/// flat, untinted and painted over, exactly the body's width, keeping the world's
/// orientation with the picture's top toward the top of the screen.
pub fn mark(role: MarkRole, asset: impl Into<String>) -> MarkSpec {
    MarkSpec(UnitMark {
        role,
        relation: None,
        look: MarkLook {
            asset: asset.into(),
            tint: [1.0; 4],
            blend: DecalBlend::Blend,
            scale: 1.0,
            turns: false,
            yaw_offset: 0.0,
        },
    })
}

impl MarkSpec {
    /// Only under units that are `relation` to the viewer.
    pub fn only(mut self, relation: Relation) -> Self {
        self.0.relation = Some(relation);
        self
    }

    /// A linear RGBA multiplier over the picture.
    pub fn tint(mut self, tint: [f32; 4]) -> Self {
        self.0.look.tint = tint;
        self
    }

    /// Added onto the ground rather than painted over it.
    pub fn additive(mut self) -> Self {
        self.0.look.blend = DecalBlend::Add;
        self
    }

    /// Its width, as a multiple of the unit's body width.
    pub fn scale(mut self, scale: f32) -> Self {
        self.0.look.scale = scale;
        self
    }

    /// Turning with the unit's drawn heading.
    pub fn turning(mut self) -> Self {
        self.0.look.turns = true;
        self
    }

    /// The picture turned by `radians` about the vertical — see
    /// [`MarkLook::yaw_offset`].
    pub fn turned(mut self, radians: f32) -> Self {
        self.0.look.yaw_offset = radians;
        self
    }
}
