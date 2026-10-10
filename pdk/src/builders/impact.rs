//! The impacts most descriptors are made of, spelled short.

use stormlight_mod_abi::common::ImpactTarget;
use stormlight_mod_abi::ids::{BuffId, DamageTypeId};
use stormlight_mod_abi::impacts::{DamageFlags, Impact, SpawnAnchor, SpawnPattern};
use stormlight_mod_abi::math::Value;
use stormlight_mod_abi::missiles::BodyDescriptor;

use super::value::IntoValue;

/// A damage impact being spelled — see [`damage`].
#[derive(Clone, Debug, PartialEq)]
#[must_use = "an impact does nothing until a descriptor carries it"]
pub struct DamageSpec {
    amount: Value,
    dtype: DamageTypeId,
    target: ImpactTarget,
    flags: DamageFlags,
}

/// `amount` of `dtype` damage to the unit the impact resolved against — the one a
/// body struck, or a cast was aimed at — with no flags.
pub fn damage(amount: impl IntoValue, dtype: DamageTypeId) -> DamageSpec {
    DamageSpec {
        amount: amount.into_value(),
        dtype,
        target: ImpactTarget::ResolvedTarget,
        flags: DamageFlags::default(),
    }
}

impl DamageSpec {
    /// Dealt to `target` instead.
    pub fn to(mut self, target: ImpactTarget) -> Self {
        self.target = target;
        self
    }

    /// With `flags`.
    pub fn flags(mut self, flags: DamageFlags) -> Self {
        self.flags = flags;
        self
    }
}

impl From<DamageSpec> for Impact {
    fn from(s: DamageSpec) -> Self {
        Impact::Damage { amount: s.amount, dtype: s.dtype, target: s.target, flags: s.flags }
    }
}

/// A buff application being spelled — see [`apply`].
#[derive(Clone, Debug, PartialEq)]
#[must_use = "an impact does nothing until a descriptor carries it"]
pub struct ApplySpec {
    buff: BuffId,
    stacks: Value,
    duration_override: Option<Value>,
    target: ImpactTarget,
}

/// One stack of `buff`, for its own duration, on the unit the impact resolved
/// against.
pub fn apply(buff: BuffId) -> ApplySpec {
    ApplySpec {
        buff,
        stacks: Value::Const(1.0),
        duration_override: None,
        target: ImpactTarget::ResolvedTarget,
    }
}

impl ApplySpec {
    /// On `target` instead.
    pub fn to(mut self, target: ImpactTarget) -> Self {
        self.target = target;
        self
    }

    /// On the caster.
    pub fn to_caster(self) -> Self {
        self.to(ImpactTarget::Caster)
    }

    /// This many stacks at once.
    pub fn stacks(mut self, stacks: impl IntoValue) -> Self {
        self.stacks = stacks.into_value();
        self
    }

    /// Lasting this long instead of the buff's own duration.
    pub fn lasting(mut self, seconds: impl IntoValue) -> Self {
        self.duration_override = Some(seconds.into_value());
        self
    }
}

impl From<ApplySpec> for Impact {
    fn from(s: ApplySpec) -> Self {
        Impact::ApplyModifiers {
            buff: s.buff,
            stacks: s.stacks,
            duration_override: s.duration_override,
            target: s.target,
        }
    }
}

/// A spawn being spelled — see [`spawn`].
#[derive(Clone, Debug, PartialEq)]
#[must_use = "an impact does nothing until a descriptor carries it"]
pub struct SpawnSpec {
    body: BodyDescriptor,
    at: SpawnAnchor,
    count: Value,
    pattern: SpawnPattern,
}

/// One `body`, from the caster.
pub fn spawn(body: impl Into<BodyDescriptor>) -> SpawnSpec {
    SpawnSpec {
        body: body.into(),
        at: SpawnAnchor::Caster,
        count: Value::Const(1.0),
        pattern: SpawnPattern::Single,
    }
}

impl SpawnSpec {
    /// From `anchor` instead.
    pub fn at(mut self, anchor: SpawnAnchor) -> Self {
        self.at = anchor;
        self
    }

    /// This many at once, laid out as `pattern`.
    pub fn several(mut self, count: impl IntoValue, pattern: SpawnPattern) -> Self {
        self.count = count.into_value();
        self.pattern = pattern;
        self
    }
}

impl From<SpawnSpec> for Impact {
    fn from(s: SpawnSpec) -> Self {
        Impact::Spawn { body: s.body, at: s.at, count: s.count, pattern: s.pattern }
    }
}
