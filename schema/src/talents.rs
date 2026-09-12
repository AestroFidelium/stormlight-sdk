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
use crate::tasks::QuestSpec;
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

/// Which single ability of the caster's a talent is *about*, when there is one
/// (stormlight/server#129).
///
/// Derived from what the talent already declares rather than stated beside it. A
/// talent says what it applies to and what it hands over; a second field for a HUD
/// to print would be a mod's words free to disagree with the mod's mechanism, which
/// is the worst kind of label — wrong rather than missing.
///
/// Two shapes because the declaration has two, and the difference matters to
/// whoever is drawing it: a slot is a position on the caster's bar and is the answer
/// straight away, while an ability has to be looked up in whatever loadout the
/// caster is carrying right now — the same ability may sit under a different key on
/// a different unit, or under none at all.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum AbilityFocus {
    /// The ability bound in this slot, whichever one that is.
    Slot(Slot),
    /// This ability, wherever the caster happens to be carrying it.
    Ability(AbilityId),
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
    /// The task this talent sets, if it sets one (stormlight/server#132).
    ///
    /// Declaration only — see [`QuestSpec`]. A talent that declares none is an
    /// ordinary talent and is untouched by any of it, which is almost all of them.
    ///
    /// A talent carrying one is the *ordinary* case of a task rather than the only
    /// one: a unit declares its own the same way ([`UnitDescriptor::tasks`]), and
    /// neither is a different kind of task to the simulation.
    ///
    /// [`UnitDescriptor::tasks`]: crate::units::UnitDescriptor::tasks
    #[serde(default)]
    pub quest: Option<QuestSpec>,
}

impl TalentDescriptor {
    /// Which one of the caster's abilities this talent changes, if that question
    /// has a single answer (stormlight/server#129).
    ///
    /// Read off two halves of the declaration, and which of them speaks is the whole
    /// of the rule:
    ///
    ///   - the **selector** governs [`patches`](Self::patches) and
    ///     [`riders`](Self::riders), and nothing else. A talent whose only content
    ///     is a unit-level reaction or a grant may carry any selector at all — that
    ///     selector applies to nothing, so a key read off it would point the player
    ///     at a button the talent never touches;
    ///   - a **lone grant** is about the slot its new ability appears in, which
    ///     covers the case a selector cannot: a talent whose whole point is a new
    ///     button.
    ///
    /// `None` when the honest answer is nothing. A talent selecting by tag, one
    /// selecting nothing at all, or one granting two abilities is about more than
    /// one of the player's buttons, and naming one of them would be a label that is
    /// confidently wrong.
    #[must_use]
    pub fn changes(&self) -> Option<AbilityFocus> {
        let governed = !self.patches.is_empty() || !self.riders.is_empty();
        let selected = governed
            .then_some(match self.selector {
                AbilitySelector::Slot(slot) => Some(AbilityFocus::Slot(slot)),
                AbilitySelector::Ability(id) => Some(AbilityFocus::Ability(id)),
                // A tag selects a set; "any" and "the unit itself" select no ability
                // at all. None of the three is one button.
                AbilitySelector::Tag(_) | AbilitySelector::Any | AbilitySelector::SelfUnit => None,
            })
            .flatten();
        selected.or(match self.grants.as_slice() {
            [only] => Some(AbilityFocus::Slot(only.slot)),
            _ => None,
        })
    }
}
