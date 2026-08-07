//! Generic id-remap walk — rewrites every interned handle buried in a descriptor
//! tree from one id space to another. A mod authors descriptors in its **local**
//! id space (0-based per family); at adoption the host translates every handle to
//! the engine's **global** space via an [`IdMap`]. This module owns the traversal
//! (Layer A, so the exhaustive matches sit beside the ISA and the compiler forces
//! coverage as it grows); the host owns the mapping policy (the local→global
//! tables) by implementing [`IdMap`].
//!
//! Only **name-interned** handles are rewritten. [`crate::ids::Slot`] and raw
//! numeric/geometry data are pure mod convention or coordinates and pass through
//! untouched. The walk is fallible: a dangling local handle surfaces as the map's
//! error rather than a panic (see [`IdMap::Error`]).

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::abilities::{AbilityDescriptor, CastSpec, Cost, Params, Targeting};
use crate::behaviors::{BuffSpec, Modifier};
use crate::common::TargetFilter;
use crate::conditions::Condition;
use crate::descriptors::Registration;
use crate::ids::{
    AbilityId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId, ResourceId,
    StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use crate::impacts::{
    BuffSelector, Impact, LoopKind, PendingFilter, PoolRef, SpawnPattern, TargetShape, TeleportDest,
};
use crate::math::{Value, Var};
use crate::missiles::{BodyDescriptor, BodyKind, CollisionSpec};
use crate::talents::{AbilitySelector, GrantAbility, ParamPatch, Rider, TalentDescriptor};
use crate::triggers::{EventFilter, EventKind, Reaction};
use crate::units::{ResourcePool, UnitDescriptor};
use crate::visuals::{ClientRegistration, EffectVisualDescriptor, VisualDescriptor};

/// Translates one interned handle to another, one method per id family. A total
/// map (identity, or a complete local→global table) never errors; a map missing a
/// referenced handle returns [`Self::Error`] so the walk can abort cleanly.
pub trait IdMap {
    /// Why a handle could not be translated (e.g. a dangling local reference).
    type Error;

    fn stat(&self, id: StatId) -> Result<StatId, Self::Error>;
    fn resource(&self, id: ResourceId) -> Result<ResourceId, Self::Error>;
    fn stack(&self, id: StackId) -> Result<StackId, Self::Error>;
    fn tag(&self, id: TagId) -> Result<TagId, Self::Error>;
    fn tag_class(&self, id: TagClassId) -> Result<TagClassId, Self::Error>;
    fn param(&self, id: ParamId) -> Result<ParamId, Self::Error>;
    fn event(&self, id: EventId) -> Result<EventId, Self::Error>;
    fn buff(&self, id: BuffId) -> Result<BuffId, Self::Error>;
    fn curve(&self, id: CurveId) -> Result<CurveId, Self::Error>;
    fn damage_type(&self, id: DamageTypeId) -> Result<DamageTypeId, Self::Error>;
    fn ability(&self, id: AbilityId) -> Result<AbilityId, Self::Error>;
    fn talent(&self, id: TalentId) -> Result<TalentId, Self::Error>;
    fn handler(&self, id: HandlerId) -> Result<HandlerId, Self::Error>;
    fn unit(&self, id: UnitId) -> Result<UnitId, Self::Error>;
    fn navmesh(&self, id: NavMeshId) -> Result<NavMeshId, Self::Error>;
}

/// In-place translation of every interned handle inside `self` through `m`.
/// Returns the map's first error (leaving `self` partially rewritten — callers
/// that need atomicity remap a clone).
pub trait RemapIds {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error>;
}

impl<T: RemapIds> RemapIds for Vec<T> {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        for x in self.iter_mut() {
            x.remap_ids(m)?;
        }
        Ok(())
    }
}

impl<T: RemapIds> RemapIds for Option<T> {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        if let Some(x) = self {
            x.remap_ids(m)?;
        }
        Ok(())
    }
}

impl<T: RemapIds> RemapIds for Box<T> {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        (**self).remap_ids(m)
    }
}

impl RemapIds for Var {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            // No interned handle (or a non-interned `Slot`): nothing to rewrite.
            Var::Level
            | Var::MaxHp(_)
            | Var::CurHp(_)
            | Var::MissingHp(_)
            | Var::HpRatio(_)
            | Var::MissingHpRatio(_)
            | Var::ChargesOf(_, _)
            | Var::CooldownOf(_, _)
            | Var::AllyCount
            | Var::EnemyCount
            | Var::DistanceToTarget
            | Var::ChannelProgress
            | Var::Rand01 => {}
            Var::Stat(id, _) => *id = m.stat(*id)?,
            Var::Resource(id, _) => *id = m.resource(*id)?,
            Var::StackCount(id, _) => *id = m.stack(*id)?,
            Var::BuffStacks(id, _) => *id = m.buff(*id)?,
        }
        Ok(())
    }
}

impl RemapIds for Value {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            Value::Const(_) | Value::ScaleCtx => {}
            Value::Read(var) => var.remap_ids(m)?,
            Value::Bin(_, a, b) => {
                a.remap_ids(m)?;
                b.remap_ids(m)?;
            }
            Value::Clamp { v, lo, hi } => {
                v.remap_ids(m)?;
                lo.remap_ids(m)?;
                hi.remap_ids(m)?;
            }
            Value::Curve(id, x) => {
                *id = m.curve(*id)?;
                x.remap_ids(m)?;
            }
        }
        Ok(())
    }
}

impl RemapIds for Condition {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            Condition::Always => {}
            Condition::Cmp(_, a, b) => {
                a.remap_ids(m)?;
                b.remap_ids(m)?;
            }
            Condition::HasTag(id, _) => *id = m.tag(*id)?,
            Condition::HasBuff(id, _) => *id = m.buff(*id)?,
            Condition::HasTalent(id) => *id = m.talent(*id)?,
            Condition::And(cs) | Condition::Or(cs) => cs.remap_ids(m)?,
            Condition::Not(c) => c.remap_ids(m)?,
        }
        Ok(())
    }
}

/// A bare `Vec<TagId>` — an id family carried directly rather than through a
/// `RemapIds` type, so it is rewritten via the helper below.
fn remap_tags<M: IdMap>(tags: &mut [TagId], m: &M) -> Result<(), M::Error> {
    for t in tags.iter_mut() {
        *t = m.tag(*t)?;
    }
    Ok(())
}

impl RemapIds for TargetFilter {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        remap_tags(&mut self.require_tags, m)?;
        remap_tags(&mut self.exclude_tags, m)?;
        Ok(())
    }
}

impl RemapIds for TargetShape {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            TargetShape::SelfOnly | TargetShape::AllAllies | TargetShape::AllEnemies => {}
            // The origin is relational (or a literal point) — it carries no id.
            TargetShape::Circle { at: _, radius } => radius.remap_ids(m)?,
            TargetShape::Cone { radius, angle } => {
                radius.remap_ids(m)?;
                angle.remap_ids(m)?;
            }
            TargetShape::Chain { jumps, range } => {
                jumps.remap_ids(m)?;
                range.remap_ids(m)?;
            }
            TargetShape::Line { length, width } => {
                length.remap_ids(m)?;
                width.remap_ids(m)?;
            }
        }
        Ok(())
    }
}

impl RemapIds for LoopKind {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            LoopKind::Times { count, gap } => {
                count.remap_ids(m)?;
                gap.remap_ids(m)?;
            }
            LoopKind::Interval { period, ticks } => {
                period.remap_ids(m)?;
                ticks.remap_ids(m)?;
            }
        }
        Ok(())
    }
}

impl RemapIds for PoolRef {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            // `Shield` has no id; `Cooldown`/`Charges` key on a non-interned `Slot`.
            PoolRef::Shield | PoolRef::Cooldown(_) | PoolRef::Charges(_) => {}
            PoolRef::Resource(id) => *id = m.resource(*id)?,
            PoolRef::Stacks(id) => *id = m.stack(*id)?,
        }
        Ok(())
    }
}

impl RemapIds for BuffSelector {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            BuffSelector::Id(id) => *id = m.buff(*id)?,
            BuffSelector::Class(id) => *id = m.tag_class(*id)?,
        }
        Ok(())
    }
}

impl RemapIds for TeleportDest {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            // `Direction` carries only a coordinate, never an id.
            TeleportDest::ToTarget | TeleportDest::ToPoint(_) | TeleportDest::Home => {}
            TeleportDest::Offset(_, v) => v.remap_ids(m)?,
        }
        Ok(())
    }
}

impl RemapIds for SpawnPattern {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            SpawnPattern::Single => {}
            SpawnPattern::Radial { count } => count.remap_ids(m)?,
            SpawnPattern::Arc { count, spread } => {
                count.remap_ids(m)?;
                spread.remap_ids(m)?;
            }
        }
        Ok(())
    }
}

impl RemapIds for PendingFilter {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            PendingFilter::FromCaster => {}
            PendingFilter::OriginTag(id) => *id = m.tag(*id)?,
        }
        Ok(())
    }
}

impl RemapIds for Impact {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        // `target`/`flags`/`dir`/`at`/`slot`/`cost` fields below carry no interned
        // handle (a resolved-target selector, switches, a coordinate direction, a
        // spawn anchor, a non-interned `Slot`, or a cost mode) and are skipped.
        match self {
            Impact::Retarget { shape, filter, max_targets, exclude_primary: _, inner } => {
                shape.remap_ids(m)?;
                filter.remap_ids(m)?;
                max_targets.remap_ids(m)?;
                inner.remap_ids(m)?;
            }
            Impact::If { cond, then, els } => {
                cond.remap_ids(m)?;
                then.remap_ids(m)?;
                els.remap_ids(m)?;
            }
            Impact::Loop { kind, inner } => {
                kind.remap_ids(m)?;
                inner.remap_ids(m)?;
            }
            Impact::Delay { secs, inner } => {
                secs.remap_ids(m)?;
                inner.remap_ids(m)?;
            }
            Impact::Damage { amount, dtype, target: _, flags: _ } => {
                amount.remap_ids(m)?;
                *dtype = m.damage_type(*dtype)?;
            }
            Impact::Heal { amount, target: _, flags: _ } => amount.remap_ids(m)?,
            Impact::AdjustPool { pool, op: _, amount, target: _ } => {
                pool.remap_ids(m)?;
                amount.remap_ids(m)?;
            }
            Impact::ApplyModifiers { buff, stacks, duration_override, target: _ } => {
                *buff = m.buff(*buff)?;
                stacks.remap_ids(m)?;
                duration_override.remap_ids(m)?;
            }
            Impact::RemoveModifiers { sel, target: _ } => sel.remap_ids(m)?,
            Impact::Dash { dir: _, dist, speed, on_collision, target: _ } => {
                dist.remap_ids(m)?;
                speed.remap_ids(m)?;
                on_collision.remap_ids(m)?;
            }
            Impact::Knockback { dir: _, force, target: _ } => force.remap_ids(m)?,
            Impact::Teleport { dest, target: _, record: _ } => dest.remap_ids(m)?,
            Impact::Spawn { body, at: _, count, pattern } => {
                body.remap_ids(m)?;
                count.remap_ids(m)?;
                pattern.remap_ids(m)?;
            }
            Impact::CastAbility { slot: _, target: _, value_scale, cost: _ } => {
                value_scale.remap_ids(m)?;
            }
            Impact::Interrupt { target: _ } => {}
            Impact::ResolvePending { filter } => filter.remap_ids(m)?,
            Impact::Emit { event, target: _, payload } => {
                *event = m.event(*event)?;
                payload.remap_ids(m)?;
            }
            Impact::Custom { handler, params: _, target: _ } => *handler = m.handler(*handler)?,
        }
        Ok(())
    }
}

impl RemapIds for BodyKind {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            BodyKind::Missile { speed, range, homing: _, pierce } => {
                speed.remap_ids(m)?;
                range.remap_ids(m)?;
                pierce.remap_ids(m)?;
            }
            BodyKind::Unit { health, duration } => {
                health.remap_ids(m)?;
                duration.remap_ids(m)?;
            }
            BodyKind::Zone { radius, duration, tick } => {
                radius.remap_ids(m)?;
                duration.remap_ids(m)?;
                tick.remap_ids(m)?;
            }
        }
        Ok(())
    }
}

impl RemapIds for CollisionSpec {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.filter.remap_ids(m)?;
        self.pierce.remap_ids(m)?;
        Ok(())
    }
}

impl RemapIds for BodyDescriptor {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.kind.remap_ids(m)?;
        self.on_spawn.remap_ids(m)?;
        self.on_hit.remap_ids(m)?;
        self.on_expire.remap_ids(m)?;
        self.collision.remap_ids(m)?;
        // `flags` are booleans only.
        Ok(())
    }
}

impl RemapIds for Modifier {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.stat = m.stat(self.stat)?;
        self.value.remap_ids(m)?;
        Ok(())
    }
}

impl RemapIds for BuffSpec {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.id = m.buff(self.id)?;
        self.duration.remap_ids(m)?;
        self.modifiers.remap_ids(m)?;
        remap_tags(&mut self.tags, m)?;
        self.reactions.remap_ids(m)?;
        self.on_apply.remap_ids(m)?;
        self.on_expire.remap_ids(m)?;
        self.on_remove.remap_ids(m)?;
        Ok(())
    }
}

impl RemapIds for EventKind {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        if let EventKind::Custom(id) = self {
            *id = m.event(*id)?;
        }
        Ok(())
    }
}

impl RemapIds for EventFilter {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        if let Some(t) = &mut self.require_tag_on_target {
            *t = m.tag(*t)?;
        }
        Ok(())
    }
}

impl RemapIds for Reaction {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.on.remap_ids(m)?;
        self.filter.remap_ids(m)?;
        self.cond.remap_ids(m)?;
        self.effects.remap_ids(m)?;
        self.internal_cd.remap_ids(m)?;
        Ok(())
    }
}

impl RemapIds for Params {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        for (p, v) in self.0.iter_mut() {
            *p = m.param(*p)?;
            v.remap_ids(m)?;
        }
        Ok(())
    }
}

impl RemapIds for Targeting {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        if let Targeting::Unit { filter } = self {
            filter.remap_ids(m)?;
        }
        Ok(())
    }
}

impl RemapIds for CastSpec {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            CastSpec::Instant => {}
            CastSpec::Cast { time, movable: _ } => time.remap_ids(m)?,
            CastSpec::Channel { time, movable: _, tick } => {
                time.remap_ids(m)?;
                tick.remap_ids(m)?;
            }
        }
        Ok(())
    }
}

impl RemapIds for Cost {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            Cost::Resource { res, amount } => {
                *res = m.resource(*res)?;
                amount.remap_ids(m)?;
            }
            Cost::Charge => {}
            Cost::Health { amount } => amount.remap_ids(m)?,
        }
        Ok(())
    }
}

impl RemapIds for AbilityDescriptor {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.id = m.ability(self.id)?;
        self.params.remap_ids(m)?;
        self.targeting.remap_ids(m)?;
        self.cast.remap_ids(m)?;
        self.cost.remap_ids(m)?;
        self.cast_gate.remap_ids(m)?;
        self.on_cast_start.remap_ids(m)?;
        self.on_cast.remap_ids(m)?;
        remap_tags(&mut self.tags, m)?;
        Ok(())
    }
}

impl RemapIds for AbilitySelector {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            AbilitySelector::Slot(_) | AbilitySelector::Any | AbilitySelector::SelfUnit => {}
            AbilitySelector::Tag(id) => *id = m.tag(*id)?,
            AbilitySelector::Ability(id) => *id = m.ability(*id)?,
        }
        Ok(())
    }
}

impl RemapIds for ParamPatch {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.param = m.param(self.param)?;
        self.value.remap_ids(m)?;
        Ok(())
    }
}

impl RemapIds for Rider {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.effects.remap_ids(m)
    }
}

impl RemapIds for GrantAbility {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.ability = m.ability(self.ability)?;
        Ok(())
    }
}

impl RemapIds for TalentDescriptor {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.id = m.talent(self.id)?;
        self.selector.remap_ids(m)?;
        self.patches.remap_ids(m)?;
        self.riders.remap_ids(m)?;
        self.add_reactions.remap_ids(m)?;
        self.grants.remap_ids(m)?;
        self.modifiers.remap_ids(m)?;
        remap_tags(&mut self.tags, m)?;
        Ok(())
    }
}

impl RemapIds for ResourcePool {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.id = m.resource(self.id)?;
        self.max.remap_ids(m)?;
        self.regen.remap_ids(m)?;
        Ok(())
    }
}

impl RemapIds for UnitDescriptor {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.id = m.unit(self.id)?;
        self.health.remap_ids(m)?;
        for (s, v) in self.stats.iter_mut() {
            *s = m.stat(*s)?;
            v.remap_ids(m)?;
        }
        remap_tags(&mut self.tags, m)?;
        // Loadout handles: slots pass through (pure convention); the bound ability,
        // each pool's resource id, and each default talent are rewritten.
        for (_slot, ability) in self.abilities.iter_mut() {
            *ability = m.ability(*ability)?;
        }
        self.resources.remap_ids(m)?;
        for talent in self.talents.iter_mut() {
            *talent = m.talent(*talent)?;
        }
        Ok(())
    }
}

impl RemapIds for VisualDescriptor {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        // Only the unit key is an interned handle; `model` carries asset strings
        // and numeric geometry/color, never an id.
        self.unit = m.unit(self.unit)?;
        Ok(())
    }
}

impl RemapIds for EffectVisualDescriptor {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        // Only the ability key is an interned handle; `role` is an enum and
        // `model` carries asset strings and numeric geometry/color, never an id.
        self.ability = m.ability(self.ability)?;
        Ok(())
    }
}

impl RemapIds for ClientRegistration {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        // `abi` and `names` (the string tables) carry no interned handle.
        self.visuals.remap_ids(m)?;
        self.effects.remap_ids(m)
    }
}

impl RemapIds for Registration {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        // `abi`, `names`, and `curves` (positional `[f32; 2]` points, no embedded
        // handles) carry no interned id inside the tree.
        self.abilities.remap_ids(m)?;
        self.talents.remap_ids(m)?;
        self.buffs.remap_ids(m)?;
        for (tag, class) in self.tag_classes.iter_mut() {
            *tag = m.tag(*tag)?;
            *class = m.tag_class(*class)?;
        }
        self.units.remap_ids(m)?;
        self.navmeshes.remap_ids(m)?;
        Ok(())
    }
}
