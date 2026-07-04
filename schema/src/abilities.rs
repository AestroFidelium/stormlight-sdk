//! `AbilityDescriptor` — a fully generic, data-defined ability. Its **named
//! parameters** (`ParamId -> Value`: cooldown, charges, cast time, range, radius,
//! target count, costs) are exactly what talents patch. The dispatch path is the
//! same for every ability regardless of hero/slot.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::common::TargetFilter;
use crate::conditions::Condition;
use crate::ids::{AbilityId, ParamId, ResourceId, TagId};
use crate::impacts::Impact;
use crate::math::Value;

/// Named ability parameters, resolved as `Value`s. A sorted assoc-list keeps the
/// ABI simple and iteration deterministic; adoption may index it for speed.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Params(pub Vec<(ParamId, Value)>);

impl Params {
    /// The expression bound to `param`, if present.
    #[must_use]
    pub fn get(&self, param: ParamId) -> Option<&Value> {
        self.0.iter().find(|(p, _)| *p == param).map(|(_, v)| v)
    }
}

/// How the ability is aimed.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Targeting {
    NoTarget,
    Unit { filter: TargetFilter },
    Point,
    Vector,
    SelfCast,
}

/// The cast/channel window.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum CastSpec {
    Instant,
    Cast { time: Value, movable: bool },
    Channel { time: Value, movable: bool, tick: Option<Value> },
}

/// A resource the cast consumes.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Cost {
    Resource { res: ResourceId, amount: Value },
    Charge,
    Health { amount: Value },
}

/// A generic ability. Cast flow: `cast_gate` + cost check → pay → `on_cast_start`
/// → cast/channel window → `on_cast` → emit `OnCast`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AbilityDescriptor {
    pub id: AbilityId,
    pub params: Params,
    pub targeting: Targeting,
    pub cast: CastSpec,
    pub cost: Vec<Cost>,
    /// Extra can-cast predicate (silence is handled generically by a tag class).
    pub cast_gate: Condition,
    pub on_cast_start: Vec<Impact>,
    pub on_cast: Vec<Impact>,
    /// Ability categories for talent selectors (e.g. an "ultimate" tag).
    pub tags: Vec<TagId>,
}
