//! What lies on the ground under a unit because of what it is to the player
//! looking at it (stormlight/server#181).
//!
//! In a fight, a player keeps track of which of several similar units is theirs
//! and which one their orders are landing on by what is drawn under them: a ring
//! under the hero they drive, another under the unit they are attacking. Those
//! rings are presentation, so the engine draws none of its own. A cosmetic mod
//! declares, for each **role** a unit can play for the viewer, what lies under it
//! — and may vary that by the unit's **relation** to the viewer, so the ring
//! under an enemy target need not be the one under an ally's.
//!
//! A mark is a picture laid flat on the ground, sized to the unit's own body: the
//! simulation's body radius, which the server publishes for every unit, so a ring
//! fits a siege engine and a rat alike without a number per unit.

use alloc::string::String;

use serde::{Deserialize, Serialize};

use crate::decal::DecalBlend;

/// What a unit is to the player looking at it, for the purpose of being marked.
/// A unit may play several roles at once — the hero a player drives can also be
/// the one under the cursor — and wears one mark per role it plays.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum MarkRole {
    /// The unit this player drives.
    Driven,
    /// The unit this player last clicked on, to look at.
    Selected,
    /// The unit this player's own unit is attacking — the one its orders are
    /// landing on, whether ordered or picked up on its own.
    Target,
    /// The unit under the cursor.
    Hovered,
}

impl MarkRole {
    /// Every role, in declaration order.
    pub const ALL: [Self; 4] = [Self::Driven, Self::Selected, Self::Target, Self::Hovered];
}

/// What a unit is to the player looking at it, by side.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Relation {
    /// The unit this player drives.
    Own,
    /// On this player's side.
    Ally,
    /// On another side.
    Enemy,
    /// On no side at all — a camp, a training target.
    Neutral,
}

/// A mark a unit wears while it plays `role` for the viewer and is `relation` to
/// them.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct UnitMark {
    pub role: MarkRole,
    /// The relation it is for. `None`: every relation.
    pub relation: Option<Relation>,
    pub look: MarkLook,
}

/// What a mark looks like.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MarkLook {
    /// The picture laid flat on the ground (`mod://<id>/…`): a ring, usually,
    /// drawn to fill the square it is laid in.
    pub asset: String,
    /// A linear RGBA multiplier over the picture.
    pub tint: [f32; 4],
    pub blend: DecalBlend,
    /// How wide it is, as a multiple of the unit's body width: `1.0` lies exactly
    /// under the body, a little more shows around it.
    pub scale: f32,
    /// Whether it turns with the unit's drawn heading — a ring with a pointer
    /// then says which way the unit faces. When not, it keeps the world's own
    /// orientation, so a pointer stays where it was put however the unit turns.
    pub turns: bool,
    /// How far the picture is turned about the vertical, radians, from lying with
    /// its top toward the unit's front (one that turns) or toward the far edge of
    /// the field, the top of the screen (one that does not). A pointer drawn at
    /// the picture's top is put at the bottom of the screen by half a turn.
    pub yaw_offset: f32,
}

impl MarkLook {
    /// Whether a renderer can draw it: a picture, a finite tint, a positive,
    /// finite width and a finite turn.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.asset.is_empty()
            && self.tint.iter().all(|c| c.is_finite())
            && self.scale.is_finite()
            && self.scale > 0.0
            && self.yaw_offset.is_finite()
    }
}

/// The look a unit playing `role`, `relation` to the viewer, wears: the first of
/// `marks` that names the role and the relation (or every relation). Declared
/// order is precedence, so a mod states the specific case before the general one
/// and the first mod loaded speaks before the next.
#[must_use]
pub fn mark_for(marks: &[UnitMark], role: MarkRole, relation: Relation) -> Option<&MarkLook> {
    marks
        .iter()
        .find(|mark| mark.role == role && mark.relation.is_none_or(|r| r == relation))
        .map(|mark| &mark.look)
}
