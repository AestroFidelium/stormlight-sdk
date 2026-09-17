//! Events + reactions — the single hook path. Everything hookable flows through
//! **event → reaction → impact**. The event set is bounded and grows only when
//! the sim gains a genuinely new thing to announce; a [`Reaction`] is pure data,
//! so mods add behavior without new engine code.
//!
//! # When a reaction answers (stormlight/server#186)
//!
//! An event is *announced* by whatever part of the simulation did the thing, and
//! answered at the **first reaction phase after the announcement** — the one phase
//! that drains them, immediately before impacts resolve. What that means for an
//! author is one rule with two halves, and both are exact rather than "soon":
//!
//! - announced *before* that phase in the tick (a cast starting, a cast
//!   resolving, a slot coming off cooldown): answered **in the same tick**, so the
//!   reaction's own effects land with the cast that provoked them;
//! - announced *at or after* it (a body striking, damage committing, a death, a
//!   buff expiring): answered at the **start of the next tick**.
//!
//! The batch is taken once per phase, so effects a reaction runs can never be
//! answered by the same pass — an event they raise is heard on the next tick, and
//! a reaction that provokes itself costs one dispatch per tick rather than
//! recursing. That is the whole re-entrancy rule; [`Reaction::internal_cd`] and
//! [`Reaction::charges`] are how content bounds it further.
//!
//! Ordering within one event is fixed: a unit's own reactions (from its talents)
//! answer first, in talent order, then the ones its buffs lend it, by buff. So a
//! replay fires the same reactions in the same order.
//!
//! # Which kinds the simulation announces
//!
//! Every variant of [`EventKind`] is a legal declaration; each one's doc says what
//! announces it, and the handful with no occasion yet say so outright rather than
//! silently never firing. `Custom` is raised by
//! [`Impact::Emit`], so a mod's own moments are on
//! exactly the same footing as the engine's.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::common::ImpactTarget;
use crate::conditions::Condition;
use crate::ids::{EventId, Slot, TagId};
use crate::impacts::Impact;
use crate::math::Value;

/// The bounded lifecycle-event set. `Custom` carries a mod-defined [`EventId`]
/// for `Emit`, so mods hook novel moments without extending this enum.
///
/// Each variant names **who hears it** (the unit whose reactions are matched) and
/// **who the other party is** (what an [`ImpactTarget::PrimaryTarget`] effect acts
/// on, and whose tags [`EventFilter::require_tag_on_target`] reads).
///
/// [`ImpactTarget::PrimaryTarget`]: crate::common::ImpactTarget::PrimaryTarget
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum EventKind {
    /// A cast has begun. Heard by the caster, about what it is aimed at; carries
    /// the cast's slot.
    OnCastStart,
    /// A cast resolved its `on_cast` — once for an instant or timed cast, once per
    /// tick of a channel. Heard by the caster, about its target; carries the slot.
    OnCast,
    /// A payload of mine landed on a unit: a missile striking, a zone applying, a
    /// melee swing connecting. Heard by whoever owns the payload, about the unit
    /// it landed on. This is the attacker's side of a hit — the unit struck hears
    /// [`EventKind::OnDamageTaken`] if the payload actually hurt it.
    OnHit,
    /// Damage was committed. Heard by the dealer, about the victim; the magnitude
    /// is what actually landed, after mitigation.
    OnDamageDealt,
    /// The same blow, heard by the victim, about the dealer.
    OnDamageTaken,
    /// A blow took its victim to zero. Heard by the victim, about the dealer,
    /// before death is finalized.
    OnLethalDamage,
    /// A unit died and this unit struck it last. Heard by the killer, about the
    /// victim.
    OnKill,
    /// A unit died. Heard by the unit that died, about whoever struck it last (or
    /// about itself, when nothing is on record).
    OnDeath,
    /// **No occasion yet.** Crowd control is an ordinary tag in a capability
    /// class, not a distinguished moment the simulation announces; saying *which*
    /// control was applied needs the condition vocabulary of
    /// stormlight/server#150.
    OnCcApplied,
    /// **No occasion yet** — see [`EventKind::OnCcApplied`].
    OnCcEnded,
    /// **No occasion yet.** Nothing in the simulation can be picked up.
    OnPickup,
    /// The heartbeat: announced once per tick to every **living** unit that holds a
    /// reaction at all, about itself. `every_nth` and [`Reaction::internal_cd`] are
    /// how a periodic passive states its period.
    ///
    /// The dead are left out of this one, and only this one: a corpse does nothing
    /// of its own accord, so a passive on a timer stops while its owner is down
    /// rather than making lying dead the fastest way to work at something. Events
    /// announced *to* a dead unit still reach it.
    OnTick,
    /// **No occasion yet.** Movement is continuous rather than a moment, and
    /// "moved" carries no magnitude a reaction could read.
    OnMove,
    /// A channel advanced by one of its ticks. Heard by the caster, about its
    /// target; the magnitude is the channel's completion in `[0, 1]`.
    OnChannelProgress,
    /// Healing was committed. Heard by the healed unit about the healer, and by
    /// the healer about the healed; the magnitude is the health actually restored.
    OnHeal,
    /// **No occasion yet.** A shield moves inside the damage and heal rules rather
    /// than at a moment of its own.
    OnShieldChanged,
    /// One of a unit's stack counters moved. Heard by the unit, about itself; the
    /// magnitude is the change. Which counter moved is not yet expressible in a
    /// filter (stormlight/server#150).
    OnStacksChanged,
    /// A basic attack resolved its swing. Heard by the attacker, about what it
    /// swung at — for a ranged attack this is the launch, and the arrival is an
    /// [`EventKind::OnHit`].
    OnAutoAttack,
    /// A slot finished its cooldown. Heard by its owner, about itself; carries the
    /// slot, so `source_slot` narrows it to one ability.
    OnAbilityReady,
    /// A buff was applied. Heard by the holder, about whoever applied it.
    OnBuffApplied,
    /// A buff reached the end of its duration. Heard by the former holder, about
    /// whoever applied it.
    OnBuffExpired,
    /// **No occasion yet.** A unit entering the world is announced to nothing,
    /// because it cannot yet be carrying a reaction when it does.
    OnSpawn,
    /// **No occasion yet** — see [`EventKind::OnSpawn`].
    OnDespawn,
    /// A mod's own moment, raised by [`Impact::Emit`].
    /// Heard by the unit the emitting effect targeted, about the emitter; the
    /// magnitude is the payload the emit carried.
    Custom(EventId),
}

/// Cheap structural narrowing applied before the (more expensive) [`Condition`].
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct EventFilter {
    /// Only events the reacting unit's own slot produced — the cast, the channel
    /// tick, the slot that came off cooldown. `None` hears every slot, and an
    /// event that names no slot (a hit, a death, the heartbeat) is never matched
    /// by a filter that names one.
    pub source_slot: Option<Slot>,
    /// Fire only every Nth matching event (e.g. "every 3rd attack").
    ///
    /// Counted over the events that got past **both** this filter and the
    /// [`Condition`] — the count is of the occasions this reaction would otherwise
    /// have fired on, not of everything that happened nearby. The tally is kept
    /// per reaction and survives an unrelated talent pick.
    pub every_nth: Option<u16>,
    /// Only events whose other party carries this tag — the unit hit, the dealer
    /// of the blow, the applier of the buff (see [`EventKind`] for which is which).
    pub require_tag_on_target: Option<TagId>,
}

/// A data-defined response to an event: when it matches, enqueue `effects` as a
/// fresh resolution against `target`. Reactions come from talents (unit-level)
/// and buffs (scoped, removed with the buff).
///
/// This is the only way a **passive** can be expressed, and the only way an effect
/// can reach the unit that something was just done to. The resolution runs with the
/// reacting unit as caster and source, and the event's other party as the primary
/// target — so `target: PrimaryTarget` is "whatever I just hit" / "whoever just hit
/// me", and `target: Caster` is "me".
///
/// Two of the event's own facts are inherited by the resolution:
/// [`Var::EventMagnitude`](crate::math::Var::EventMagnitude) reads what the event
/// carried (the damage dealt, the healing done), and the source slot is the one the
/// event named — so an effect answering a cast can still say which ability is
/// answering.
///
/// Its own numbers are evaluated **when it fires**, against the state of the world
/// at that moment, not frozen when the talent was picked or the buff applied.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Reaction {
    pub on: EventKind,
    pub filter: EventFilter,
    pub cond: Condition,
    pub effects: Vec<Impact>,
    pub target: ImpactTarget,
    /// Internal cooldown, e.g. "not more than once per 120s". Counted from the
    /// last time this reaction actually fired, in seconds evaluated against the
    /// reacting unit.
    pub internal_cd: Option<Value>,
    /// Finite-use reactions (None = unlimited).
    ///
    /// Spent charges are remembered per reaction and survive an unrelated talent
    /// pick; a buff's are restored when the buff is re-applied, because that is a
    /// fresh application of the same effect.
    pub charges: Option<u16>,
}
