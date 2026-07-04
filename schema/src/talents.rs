//! `TalentDescriptor` — a fully generic talent: a **selector** (by slot or tag,
//! never unit identity) plus generic patches/riders/reactions/grants. This is
//! what delivers "any talent on any ability, any ability on any unit".

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::behaviors::Modifier;
use crate::common::NumOp;
use crate::ids::{AbilityId, ParamId, Slot, TagId, TalentId};
use crate::impacts::Impact;
use crate::math::Value;
use crate::triggers::Reaction;

/// Which abilities a talent applies to — keyed on slot/tag/id, never identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum AbilitySelector {
    Slot(Slot),
    Tag(TagId),
    Ability(AbilityId),
    Any,
    SelfUnit,
}

/// A generic parameter tweak: the whole "+radius / −cooldown / +charge /
/// +targets" class is one patch.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ParamPatch {
    pub param: ParamId,
    pub op: NumOp,
    pub value: Value,
}

/// Which of a selected ability's own hooks a rider appends into.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum AbilityHook {
    OnCastStart,
    OnCast,
    OnHit,
}

/// Extra effects appended into a selected ability's payload — they travel with
/// its missiles and respect `value_scale` (contrast with unit-level reactions).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Rider {
    pub hook: AbilityHook,
    pub effects: Vec<Impact>,
}

/// Grant a new ability into a free slot.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct GrantAbility {
    pub slot: Slot,
    pub ability: AbilityId,
}

/// A generic talent.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TalentDescriptor {
    pub id: TalentId,
    pub selector: AbilitySelector,
    pub patches: Vec<ParamPatch>,
    pub riders: Vec<Rider>,
    pub add_reactions: Vec<Reaction>,
    pub grants: Vec<GrantAbility>,
    pub modifiers: Vec<Modifier>,
    pub tags: Vec<TagId>,
}
