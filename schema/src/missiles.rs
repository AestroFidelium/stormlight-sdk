//! `BodyDescriptor` — the one `Spawn` verb's payload. A missile, a summoned
//! unit, and a ground zone are all "a body with a shape and hooks"; the shape
//! lives in [`BodyKind`] (data), never in a new primitive.
//!
//! A spawned body carries its own effect payload, **frozen** at launch unless
//! `flags.live_values`, so later buffs to the caster don't retro-change an
//! in-flight shot and reflection re-targets the same self-contained entity
//! (§6.1).

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::common::TargetFilter;
use crate::impacts::Impact;
use crate::math::Value;

/// The shape of a spawned body; all parameters are `Value` expressions.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum BodyKind {
    Missile { speed: Value, range: Value, homing: bool, pierce: Value },
    Unit { health: Value, duration: Option<Value> },
    Zone { radius: Value, duration: Value, tick: Value },
}

/// How a body collides with the world.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct CollisionSpec {
    pub filter: TargetFilter,
    pub pierce: Value,
    pub through_walls: bool,
}

/// Cross-cutting body switches (freezing, reflection, dimensions).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct BodyFlags {
    pub reflectable: bool,
    pub cross_dimension: bool,
    /// Keep payload `Value`s live (re-evaluated) instead of frozen at spawn.
    pub live_values: bool,
}

/// A spawnable body plus its lifecycle effect hooks.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct BodyDescriptor {
    pub kind: BodyKind,
    pub on_spawn: Vec<Impact>,
    /// The payload — frozen at spawn unless [`BodyFlags::live_values`].
    pub on_hit: Vec<Impact>,
    pub on_expire: Vec<Impact>,
    pub collision: CollisionSpec,
    pub flags: BodyFlags,
    /// How high above its spawn anchor the body travels, in world units
    /// (stormlight/server#152).
    ///
    /// A unit's `Transform` is its **ground** origin, so a body anchored to one
    /// and given no height leaves from the floor and skims it for the whole
    /// flight — passing under the model that fired it and under the one it is
    /// aimed at. Where a shot leaves a character is a property of the art, so it
    /// is content: the engine adds this to the anchor and asks nothing else.
    ///
    /// Purely a vertical offset. Contact is decided on the ground plane every
    /// other distance question in the simulation is asked on, so raising a body
    /// changes what it looks like and never what it hits.
    pub height: Value,
}
