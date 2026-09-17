//! `SlotRef` — how an effect names an ability slot (stormlight/server#187).
//!
//! A talent used to state its slot **twice, in two coordinate systems**. Its
//! patches are relative by construction ([`ParamPatch`] names no slot at all; it
//! lands in whichever slot matched), while everything in *effect* position — the
//! cooldown an [`AdjustPool`] pays back, the slot a [`CastAbility`] re-fires, the
//! number a [`Var::CooldownOf`] reads — was a literal `Slot` written down by the
//! author. So a rider **triggered** relative to the matched slot and **acted** on
//! a typed-in one.
//!
//! A literal is right wherever the author knows the number and means it: "hitting
//! with this shortens *that*" is a deliberate absolute reference, and players build
//! around slot-shaped cooldown effects. [`SlotRef::At`] keeps saying exactly that
//! and is not deprecated. What was missing is the other half — the slot that is
//! **unknown when the talent is written**:
//!
//! - a selector matching several slots copies one rider into every one of them,
//!   literal included, so "each of your basics refunds *its own* cooldown" could
//!   not be said at all;
//! - `AbilitySelector::Ability(id)` exists *because* the slot is unknown; the rider
//!   attached to the right slot and paid out to the written-down one;
//! - a granted or replaced ability picks its slot at runtime, so its own tree could
//!   not refer to itself;
//! - a reaction has no matched slot, and attributing its payload to the ability
//!   that provoked it needs one.
//!
//! [`SlotRef::This`] says "the slot this effect is running from" and costs nothing
//! at runtime, because it is resolved wherever the slot is already known:
//!
//! - **at fold time**, in a talent's patches and riders — `resolve_loadout` is
//!   already looping over matched slots, so each copy is [bound](bind_slots) to its
//!   own. A multi-match selector produces several instantiations, each correct;
//! - **at freeze time**, in the payload a cast hands to a body — the projectile
//!   already remembers everything that shaped it, and which button fired it is one
//!   more such fact;
//! - **at resolution time** otherwise, from the running context's source slot: the
//!   casting slot for an ability's own tree, the event's slot for a reaction's.
//!
//! Where none of the three can answer, the read is a documented zero and the effect
//! is skipped — never a silent fall back to slot 0. The declarations where that is
//! *certain* rather than possible are refused at adoption instead
//! ([`require_bound`]).
//!
//! [`ParamPatch`]: crate::talents::ParamPatch
//! [`AdjustPool`]: crate::impacts::Impact::AdjustPool
//! [`CastAbility`]: crate::impacts::Impact::CastAbility
//! [`Var::CooldownOf`]: crate::math::Var::CooldownOf

use serde::{Deserialize, Serialize};

use crate::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, Slot, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use crate::remap::{IdMap, RemapIds};

/// Which ability slot an effect means: a written-down one, or the one it is
/// running from. See the module docs.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SlotRef {
    /// This exact slot, whatever is bound in it — an absolute reference, and a
    /// first-class one: "hitting with this ability shortens the cooldown of *that*
    /// slot" is a thing content deliberately says.
    At(Slot),
    /// The slot this effect is running from, whichever that turns out to be.
    This,
}

impl SlotRef {
    /// The slot this names, given the slot the effect is running from (`None` when
    /// nothing is).
    ///
    /// The single place the rule lives, so no caller can invent a different one —
    /// and in particular none of them can invent slot 0.
    #[must_use]
    pub const fn resolve(self, running_from: Option<Slot>) -> Option<Slot> {
        match self {
            Self::At(slot) => Some(slot),
            Self::This => running_from,
        }
    }

    /// Whether this reference still needs a slot to be resolved against. False once
    /// it has been [bound](bind_slots), which is what makes binding idempotent.
    #[must_use]
    pub const fn is_relative(self) -> bool {
        matches!(self, Self::This)
    }
}

impl From<Slot> for SlotRef {
    fn from(slot: Slot) -> Self {
        Self::At(slot)
    }
}

/// One `IdMap` method that translates nothing — every id family, for a map whose
/// whole business is the slot reference.
macro_rules! identity_family {
    ($method:ident, $ty:ty) => {
        fn $method(&self, id: $ty) -> Result<$ty, Self::Error> {
            Ok(id)
        }
    };
}

/// Every id family left alone, for a map that only rewrites [`SlotRef`]s.
macro_rules! identity_ids {
    () => {
        identity_family!(stat, StatId);
        identity_family!(resource, ResourceId);
        identity_family!(stack, StackId);
        identity_family!(tag, TagId);
        identity_family!(tag_class, TagClassId);
        identity_family!(param, ParamId);
        identity_family!(event, EventId);
        identity_family!(buff, BuffId);
        identity_family!(curve, CurveId);
        identity_family!(damage_type, DamageTypeId);
        identity_family!(ability, AbilityId);
        identity_family!(talent, TalentId);
        identity_family!(handler, HandlerId);
        identity_family!(unit, UnitId);
        identity_family!(navmesh, NavMeshId);
        identity_family!(anim_state, AnimStateId);
    };
}

/// Rewrites every [`SlotRef::This`] in a tree to one concrete slot.
struct BindThis(Slot);

impl IdMap for BindThis {
    type Error = core::convert::Infallible;
    identity_ids!();

    fn slot_ref(&self, slot: SlotRef) -> Result<SlotRef, Self::Error> {
        Ok(match slot {
            // Already absolute: left exactly as written, which is what makes
            // binding idempotent and keeps `At` first-class.
            SlotRef::At(_) => slot,
            SlotRef::This => SlotRef::At(self.0),
        })
    }
}

/// Refuses any [`SlotRef::This`] it finds.
struct RejectThis;

impl IdMap for RejectThis {
    type Error = UnboundSlot;
    identity_ids!();

    fn slot_ref(&self, slot: SlotRef) -> Result<SlotRef, Self::Error> {
        if slot.is_relative() { Err(UnboundSlot) } else { Ok(slot) }
    }
}

/// A declaration that says "the slot I am running from" where nothing can ever say
/// which slot that is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct UnboundSlot;

impl core::fmt::Display for UnboundSlot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("`This` slot reference in a position with no slot to resolve against")
    }
}

/// Bind every relative slot reference anywhere in `tree` to `slot`.
///
/// Rides the id-remap traversal rather than a second one of its own: a `SlotRef` is
/// a leaf reference buried in exactly the same descriptor shapes an interned handle
/// is, and one exhaustive walk that the compiler forces to stay complete is worth
/// more than two that can silently disagree about where a `CastAbility` hides.
///
/// **Idempotent**: an already-bound [`SlotRef::At`] is left alone, so binding a
/// tree twice (a rider bound at fold time and frozen again at spawn) says the same
/// thing as binding it once.
pub fn bind_slots<T: RemapIds + ?Sized>(tree: &mut T, slot: Slot) {
    match tree.remap_ids(&BindThis(slot)) {
        Ok(()) => {}
        Err(never) => match never {},
    }
}

/// Error unless every slot reference in `tree` is already absolute.
///
/// Takes `&mut` because it rides the in-place remap walk; [`RejectThis`] rewrites
/// nothing, so a tree that passes is byte-for-byte what it was and one that fails
/// is too.
pub fn require_bound<T: RemapIds + ?Sized>(tree: &mut T) -> Result<(), UnboundSlot> {
    tree.remap_ids(&RejectThis)
}
