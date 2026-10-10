//! Bodies — what an ability puts into the world — spelled short.

use alloc::vec::Vec;

use stormlight_mod_abi::common::TargetFilter;
use stormlight_mod_abi::impacts::Impact;
use stormlight_mod_abi::math::Value;
use stormlight_mod_abi::missiles::{BodyDescriptor, BodyFlags, BodyKind, CollisionSpec};

use super::target::enemies;
use super::value::IntoValue;

/// A body being spelled — see [`missile`].
#[derive(Clone, Debug, PartialEq)]
#[must_use = "a body does nothing until something spawns it"]
pub struct BodySpec(BodyDescriptor);

/// A straight missile flying `speed` units a second for `range` units. It hits
/// the first enemy it touches, stops there, flies at its anchor's height and does
/// nothing until it is given a payload.
pub fn missile(speed: impl IntoValue, range: impl IntoValue) -> BodySpec {
    BodySpec(BodyDescriptor {
        kind: BodyKind::Missile {
            speed: speed.into_value(),
            range: range.into_value(),
            homing: false,
            pierce: Value::Const(0.0),
        },
        on_spawn: Vec::new(),
        on_hit: Vec::new(),
        on_expire: Vec::new(),
        collision: CollisionSpec {
            filter: enemies(),
            pierce: Value::Const(0.0),
            through_walls: false,
        },
        flags: BodyFlags::default(),
        height: Value::Const(0.0),
    })
}

impl BodySpec {
    /// Colliding with whoever `filter` keeps instead.
    pub fn hits(mut self, filter: TargetFilter) -> Self {
        self.0.collision.filter = filter;
        self
    }

    /// Doing `impact` to whatever it hits — the payload travels with the body.
    pub fn on_hit(mut self, impact: impl Into<Impact>) -> Self {
        self.0.on_hit.push(impact.into());
        self
    }

    /// Doing `impact` the moment it appears.
    pub fn on_spawn(mut self, impact: impl Into<Impact>) -> Self {
        self.0.on_spawn.push(impact.into());
        self
    }

    /// Doing `impact` where it ends without hitting anything more.
    pub fn on_expire(mut self, impact: impl Into<Impact>) -> Self {
        self.0.on_expire.push(impact.into());
        self
    }

    /// Flying this far above its anchor.
    pub fn height(mut self, height: impl IntoValue) -> Self {
        self.0.height = height.into_value();
        self
    }

    /// Passing through this many targets before it stops. A missile's pierce is
    /// carried by its flight and by its collision both, and they say the same.
    pub fn pierce(mut self, count: impl IntoValue) -> Self {
        let count = count.into_value();
        if let BodyKind::Missile { pierce, .. } = &mut self.0.kind {
            *pierce = count.clone();
        }
        self.0.collision.pierce = count;
        self
    }

    /// Following its target rather than flying straight.
    pub fn homing(mut self) -> Self {
        if let BodyKind::Missile { homing, .. } = &mut self.0.kind {
            *homing = true;
        }
        self
    }

    /// Flying through walls.
    pub fn through_walls(mut self) -> Self {
        self.0.collision.through_walls = true;
        self
    }

    /// With `flags`.
    pub fn flags(mut self, flags: BodyFlags) -> Self {
        self.0.flags = flags;
        self
    }
}

impl From<BodySpec> for BodyDescriptor {
    fn from(spec: BodySpec) -> Self {
        spec.0
    }
}
