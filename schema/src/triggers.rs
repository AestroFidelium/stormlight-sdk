//! Events + reactions — the single hook path. Everything hookable flows through
//! **event → reaction → impact**. The event set is bounded and grows only when
//! the sim gains a genuinely new thing to announce; a [`Reaction`] is pure data,
//! so mods add behavior without new engine code.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::common::ImpactTarget;
use crate::conditions::Condition;
use crate::ids::{EventId, Slot, TagId};
use crate::impacts::Impact;
use crate::math::Value;

/// The bounded lifecycle-event set. `Custom` carries a mod-defined [`EventId`]
/// for `Emit`, so mods hook novel moments without extending this enum.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum EventKind {
    OnCastStart,
    OnCast,
    OnHit,
    OnDamageDealt,
    OnDamageTaken,
    OnLethalDamage,
    OnKill,
    OnDeath,
    OnCcApplied,
    OnCcEnded,
    OnPickup,
    OnTick,
    OnMove,
    OnChannelProgress,
    OnHeal,
    OnShieldChanged,
    OnStacksChanged,
    OnAutoAttack,
    OnAbilityReady,
    OnBuffApplied,
    OnBuffExpired,
    OnSpawn,
    OnDespawn,
    Custom(EventId),
}

/// Cheap structural narrowing applied before the (more expensive) [`Condition`].
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct EventFilter {
    pub source_slot: Option<Slot>,
    /// Fire only every Nth matching event (e.g. "every 3rd attack").
    pub every_nth: Option<u16>,
    pub require_tag_on_target: Option<TagId>,
}

/// A data-defined response to an event: when it matches, enqueue `effects` as a
/// fresh resolution against `target`. Reactions come from talents (unit-level)
/// and buffs (scoped, removed with the buff).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Reaction {
    pub on: EventKind,
    pub filter: EventFilter,
    pub cond: Condition,
    pub effects: Vec<Impact>,
    pub target: ImpactTarget,
    /// Internal cooldown, e.g. "not more than once per 120s".
    pub internal_cd: Option<Value>,
    /// Finite-use reactions (None = unlimited).
    pub charges: Option<u16>,
}
