//! `AttackDescriptor` — the **basic attack**, declared as content (server#89).
//!
//! An attack is the thing a player does most of a match, and it is deliberately
//! *not* a private combat loop inside the engine. It is an ordinary payload of
//! the same effect ISA an ability uses ([`Impact`]), delivered on the same
//! timeline vocabulary, resolved through the same phases. That constraint is the
//! whole point: "attacks apply a stack", "every third one cleaves", "attacks heal
//! you", "the ranged shot bounces" are then *content* — a payload a mod or a
//! talent authors — instead of an engine change each.
//!
//! What the engine reads generically is only the **timeline**: how long the
//! wind-up is, when the swing point falls, how long the recovery runs, and how
//! often the cycle may repeat. Everything the attack *does* is in `payload`.
//!
//! A unit that declares no attack simply has none — the engine ships no default
//! weapon (see [`UnitDescriptor::attack`](crate::units::UnitDescriptor::attack)).

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::common::TargetFilter;
use crate::impacts::Impact;
use crate::math::Value;
use crate::missiles::BodyDescriptor;

/// How an attack's payload reaches its target.
///
/// The two shapes a basic attack comes in, and the only thing that differs
/// between them is *when* the payload resolves — not what it is, and not what
/// runs it.
// One variant carries a whole body descriptor and the other carries nothing,
// which is the intended vocabulary shape rather than an oversight: melee *is* the
// absence of a body. Collapsing it into an `Option<BodyDescriptor>` field would
// hide that behind a mode flag, and a unit descriptor holds at most one attack.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum AttackDelivery {
    /// The payload resolves **on the target, at the swing point**. Nothing
    /// travels; range is checked when the swing starts.
    Melee,
    /// A body is launched at the swing point and the payload rides it, resolving
    /// **on arrival** — the existing projectile path, entered through the same
    /// [`Impact::Spawn`](crate::impacts::Impact::Spawn) verb every ability uses.
    ///
    /// The engine appends the attack's `payload` to this body's `on_hit`, so an
    /// `OnHit` talent rider lands on the shot exactly as it does on an ability's
    /// missile — payload travels with the body, frozen at launch.
    Ranged { body: BodyDescriptor },
}

/// A unit's basic attack: a payload, whom it may be pointed at, how it is
/// delivered, and the timeline the engine drives it on.
///
/// Every number is a [`Value`], so it can be scaled off a stat rather than fixed
/// at authoring time — which is what makes an attack-speed buff, a range
/// increase, or a level-scaled hit ordinary modifier-stack arithmetic instead of
/// an attack-specific feature. See [`crate::stats`] for the reserved stat names
/// the engine and a mod's HUD agree to spell the same way.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AttackDescriptor {
    /// What the attack does. An `Impact` tree like an ability's `on_cast`,
    /// resolved with the attacker as caster and the attacked unit as target.
    pub payload: Vec<Impact>,
    /// Whom this unit may attack. Checked when a target is ordered *and* when one
    /// is acquired, so an attack-move never picks up something an explicit order
    /// would have been refused.
    pub filter: TargetFilter,
    /// Melee or ranged — when the payload resolves.
    pub delivery: AttackDelivery,
    /// Seconds from the start of a swing to its **swing point** — the moment the
    /// payload resolves (melee) or the shot leaves (ranged). This is also the
    /// attack's point of no return: a new order during the wind-up cancels the
    /// swing outright and costs the attacker nothing.
    pub windup: Value,
    /// Seconds after the swing point before the unit may begin another swing.
    /// Distinct from `period`: recovery is the commitment the *animation* owes,
    /// and it gates the next swing even when an attack-speed buff has pulled the
    /// period below it.
    pub recovery: Value,
    /// Base seconds between two consecutive swing **starts**, before the
    /// `attack_speed` stat divides it. The attack interval, not the wind-up.
    pub period: Value,
    /// How far the target may be, in world units — the fallback when the unit
    /// aggregates no `attack_range` stat, exactly as `MoveSpeed` is the fallback
    /// for `move_speed`.
    pub range: Value,
}
