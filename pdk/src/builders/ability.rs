//! Abilities spelled short.

use alloc::vec::Vec;

use stormlight_mod_abi::abilities::{AbilityDescriptor, CastSpec, Cost, Params, Targeting};
use stormlight_mod_abi::conditions::Condition;
use stormlight_mod_abi::ids::{AbilityId, ParamId, ResourceId, TagId};
use stormlight_mod_abi::impacts::Impact;

use super::value::IntoValue;

/// An ability being spelled — see [`ability`].
#[derive(Clone, Debug, PartialEq)]
#[must_use = "an ability does nothing until the context declares it"]
pub struct AbilitySpec(AbilityDescriptor);

/// An ability aimed as `targeting` says: instant, free, ungated, doing nothing
/// until it is given something to do on cast.
pub fn ability(targeting: Targeting) -> AbilitySpec {
    AbilitySpec(AbilityDescriptor {
        id: AbilityId(0),
        params: Params(Vec::new()),
        targeting,
        cast: CastSpec::Instant,
        cost: Vec::new(),
        cast_gate: Condition::Always,
        on_cast_start: Vec::new(),
        on_cast: Vec::new(),
        tags: Vec::new(),
    })
}

impl AbilitySpec {
    /// Set the parameter `param` — its cooldown, its range, any number its effects
    /// read by name.
    pub fn param(mut self, param: ParamId, value: impl IntoValue) -> Self {
        self.0.params.0.push((param, value.into_value()));
        self
    }

    /// Wound up for `seconds`, rooting the caster.
    pub fn cast(mut self, seconds: impl IntoValue) -> Self {
        self.0.cast = CastSpec::Cast { time: seconds.into_value(), movable: false };
        self
    }

    /// Wound up for `seconds`, the caster free to walk through it.
    pub fn movable_cast(mut self, seconds: impl IntoValue) -> Self {
        self.0.cast = CastSpec::Cast { time: seconds.into_value(), movable: true };
        self
    }

    /// Channelled for `seconds`, rooting the caster, its effects every `tick`
    /// seconds if it has one.
    pub fn channel(mut self, seconds: impl IntoValue, tick: Option<f32>) -> Self {
        self.0.cast = CastSpec::Channel {
            time: seconds.into_value(),
            movable: false,
            tick: tick.map(IntoValue::into_value),
        };
        self
    }

    /// Carried in a slot and never pressed: a trait whose effect lives elsewhere.
    pub fn passive(mut self) -> Self {
        self.0.cast = CastSpec::Passive;
        self
    }

    /// Costing `amount` of `resource`.
    pub fn cost(mut self, resource: ResourceId, amount: impl IntoValue) -> Self {
        self.0.cost.push(Cost::Resource { res: resource, amount: amount.into_value() });
        self
    }

    /// Costing `amount` of the caster's health.
    pub fn costs_health(mut self, amount: impl IntoValue) -> Self {
        self.0.cost.push(Cost::Health { amount: amount.into_value() });
        self
    }

    /// Spending a charge.
    pub fn costs_charge(mut self) -> Self {
        self.0.cost.push(Cost::Charge);
        self
    }

    /// Castable only while `gate` holds.
    pub fn gate(mut self, gate: Condition) -> Self {
        self.0.cast_gate = gate;
        self
    }

    /// Doing `impact` when the cast begins.
    pub fn on_cast_start(mut self, impact: impl Into<Impact>) -> Self {
        self.0.on_cast_start.push(impact.into());
        self
    }

    /// Doing `impact` when the cast goes off.
    pub fn on_cast(mut self, impact: impl Into<Impact>) -> Self {
        self.0.on_cast.push(impact.into());
        self
    }

    /// Tagged `tag`.
    pub fn tag(mut self, tag: TagId) -> Self {
        self.0.tags.push(tag);
        self
    }
}

impl From<AbilitySpec> for AbilityDescriptor {
    fn from(spec: AbilitySpec) -> Self {
        spec.0
    }
}
