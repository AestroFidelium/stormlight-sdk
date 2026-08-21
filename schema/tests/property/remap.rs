//! Invariants of the generic id-remap walk (`RemapIds` + `IdMap`) — the traversal
//! that rewrites every interned local handle buried in a descriptor tree to its
//! global equivalent at adoption. Directional/structural only:
//!   - **Identity is a no-op**: remapping through a map that returns every id
//!     unchanged leaves the tree byte-identical (the walk never corrupts).
//!   - **Totality**: a map that fails on any id makes the walk return `Err`,
//!     never panic; a total map always returns `Ok`.
//!   - **Determinism**: remapping two clones through the same map is identical.
//!
//! Completeness of the walk (that *no* embedded local id is left behind) is the
//! server-side adoption equivalence test's job — a missed field there shows up as
//! a globalized descriptor that differs from the same descriptor authored in
//! global ids. Here we only pin that the traversal is total and faithful.

use core::cell::Cell;

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::abilities::{AbilityDescriptor, CastSpec, Cost, Params, Targeting};
use stormlight_mod_abi::behaviors::BuffSpec;
use stormlight_mod_abi::behaviors::{ModOp, Modifier, Reapply, StackScope, Stacking};
use stormlight_mod_abi::common::{Affiliation, Direction, ImpactTarget, NumOp, TargetFilter};
use stormlight_mod_abi::conditions::{CmpOp, Condition};
use stormlight_mod_abi::descriptors::{Curve, Names, Registration};
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, Slot, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::impacts::{
    AbilityTarget, BuffSelector, CostMode, DamageFlags, HealFlags, Impact, LoopKind, PendingFilter,
    PoolRef, SpawnAnchor, SpawnPattern, TargetShape, TeleportDest,
};
use stormlight_mod_abi::manifest::ABI_VERSION;
use stormlight_mod_abi::math::{BinOp, Value, Var, Who};
use stormlight_mod_abi::missiles::{BodyDescriptor, BodyFlags, BodyKind, CollisionSpec};
use stormlight_mod_abi::navmesh::NavMeshDescriptor;
use stormlight_mod_abi::placement::UnitPlacement;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::talents::{
    AbilityHook, AbilitySelector, GrantAbility, ParamPatch, QuestSpec, Rider, TalentDescriptor,
};
use stormlight_mod_abi::triggers::{EventFilter, EventKind, Reaction};
use stormlight_mod_abi::units::{ResourcePool, UnitDescriptor};

/// A map that returns every id unchanged and never fails, while counting how many
/// ids the walk visited — so we know whether a given tree carried any handles.
#[derive(Default)]
struct Counting {
    seen: Cell<u32>,
}

impl Counting {
    fn bump<H>(&self, id: H) -> Result<H, ()> {
        self.seen.set(self.seen.get() + 1);
        Ok(id)
    }
}

impl IdMap for Counting {
    type Error = ();
    fn stat(&self, id: StatId) -> Result<StatId, ()> {
        self.bump(id)
    }
    fn resource(&self, id: ResourceId) -> Result<ResourceId, ()> {
        self.bump(id)
    }
    fn stack(&self, id: StackId) -> Result<StackId, ()> {
        self.bump(id)
    }
    fn tag(&self, id: TagId) -> Result<TagId, ()> {
        self.bump(id)
    }
    fn tag_class(&self, id: TagClassId) -> Result<TagClassId, ()> {
        self.bump(id)
    }
    fn param(&self, id: ParamId) -> Result<ParamId, ()> {
        self.bump(id)
    }
    fn event(&self, id: EventId) -> Result<EventId, ()> {
        self.bump(id)
    }
    fn buff(&self, id: BuffId) -> Result<BuffId, ()> {
        self.bump(id)
    }
    fn curve(&self, id: CurveId) -> Result<CurveId, ()> {
        self.bump(id)
    }
    fn damage_type(&self, id: DamageTypeId) -> Result<DamageTypeId, ()> {
        self.bump(id)
    }
    fn ability(&self, id: AbilityId) -> Result<AbilityId, ()> {
        self.bump(id)
    }
    fn talent(&self, id: TalentId) -> Result<TalentId, ()> {
        self.bump(id)
    }
    fn handler(&self, id: HandlerId) -> Result<HandlerId, ()> {
        self.bump(id)
    }
    fn unit(&self, id: UnitId) -> Result<UnitId, ()> {
        self.bump(id)
    }
    fn navmesh(&self, id: NavMeshId) -> Result<NavMeshId, ()> {
        self.bump(id)
    }
    fn anim_state(&self, id: AnimStateId) -> Result<AnimStateId, ()> {
        self.bump(id)
    }
}

/// A map that fails on the very first id it is asked to translate — models a
/// dangling reference. The walk must surface the error, never panic.
struct FailAll;

impl IdMap for FailAll {
    type Error = ();
    fn stat(&self, _: StatId) -> Result<StatId, ()> {
        Err(())
    }
    fn resource(&self, _: ResourceId) -> Result<ResourceId, ()> {
        Err(())
    }
    fn stack(&self, _: StackId) -> Result<StackId, ()> {
        Err(())
    }
    fn tag(&self, _: TagId) -> Result<TagId, ()> {
        Err(())
    }
    fn tag_class(&self, _: TagClassId) -> Result<TagClassId, ()> {
        Err(())
    }
    fn param(&self, _: ParamId) -> Result<ParamId, ()> {
        Err(())
    }
    fn event(&self, _: EventId) -> Result<EventId, ()> {
        Err(())
    }
    fn buff(&self, _: BuffId) -> Result<BuffId, ()> {
        Err(())
    }
    fn curve(&self, _: CurveId) -> Result<CurveId, ()> {
        Err(())
    }
    fn damage_type(&self, _: DamageTypeId) -> Result<DamageTypeId, ()> {
        Err(())
    }
    fn ability(&self, _: AbilityId) -> Result<AbilityId, ()> {
        Err(())
    }
    fn talent(&self, _: TalentId) -> Result<TalentId, ()> {
        Err(())
    }
    fn handler(&self, _: HandlerId) -> Result<HandlerId, ()> {
        Err(())
    }
    fn unit(&self, _: UnitId) -> Result<UnitId, ()> {
        Err(())
    }
    fn navmesh(&self, _: NavMeshId) -> Result<NavMeshId, ()> {
        Err(())
    }
    fn anim_state(&self, _: AnimStateId) -> Result<AnimStateId, ()> {
        Err(())
    }
}

/// Interprets a flat seed stream into a bounded `Registration` covering every
/// descriptor family and the id-bearing tree types beneath them.
struct Gen<'a> {
    ops: &'a [u16],
    pos: usize,
}

impl Gen<'_> {
    fn next(&mut self) -> u16 {
        let v = self.ops.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        v
    }
    fn f32(&mut self) -> f32 {
        f32::from(self.next()) / f32::from(u16::MAX) * 200.0 - 100.0
    }
    fn point(&mut self) -> [f32; 3] {
        [self.f32(), self.f32(), self.f32()]
    }
    fn who(&mut self) -> Who {
        match self.next() % 3 {
            0 => Who::Caster,
            1 => Who::Target,
            _ => Who::Source,
        }
    }
    fn slot(&mut self) -> Slot {
        Slot(self.next() as u8)
    }
    fn var(&mut self) -> Var {
        let w = self.who();
        match self.next() % 17 {
            0 => Var::Level,
            1 => Var::MaxHp(w),
            2 => Var::CurHp(w),
            3 => Var::MissingHp(w),
            4 => Var::HpRatio(w),
            5 => Var::MissingHpRatio(w),
            6 => Var::Stat(StatId(self.next()), w),
            7 => Var::Resource(ResourceId(self.next()), w),
            8 => Var::StackCount(StackId(self.next()), w),
            9 => Var::BuffStacks(BuffId(self.next()), w),
            10 => Var::ChargesOf(self.slot(), w),
            11 => Var::CooldownOf(self.slot(), w),
            12 => Var::AllyCount,
            13 => Var::EnemyCount,
            14 => Var::DistanceToTarget,
            15 => Var::ChannelProgress,
            _ => Var::Rand01,
        }
    }
    fn binop(&mut self) -> BinOp {
        match self.next() % 6 {
            0 => BinOp::Add,
            1 => BinOp::Sub,
            2 => BinOp::Mul,
            3 => BinOp::Div,
            4 => BinOp::Min,
            _ => BinOp::Max,
        }
    }
    fn value(&mut self, depth: u8) -> Value {
        if depth == 0 {
            return Value::Const(self.f32());
        }
        match self.next() % 6 {
            0 => Value::Const(self.f32()),
            1 => Value::Read(self.var()),
            2 => Value::Bin(
                self.binop(),
                Box::new(self.value(depth - 1)),
                Box::new(self.value(depth - 1)),
            ),
            3 => Value::Clamp {
                v: Box::new(self.value(depth - 1)),
                lo: Box::new(self.value(depth - 1)),
                hi: Box::new(self.value(depth - 1)),
            },
            4 => Value::Curve(CurveId(self.next()), Box::new(self.value(depth - 1))),
            _ => Value::ScaleCtx,
        }
    }
    fn cmpop(&mut self) -> CmpOp {
        match self.next() % 5 {
            0 => CmpOp::Lt,
            1 => CmpOp::Le,
            2 => CmpOp::Eq,
            3 => CmpOp::Ge,
            _ => CmpOp::Gt,
        }
    }
    fn cond(&mut self, depth: u8) -> Condition {
        if depth == 0 {
            return Condition::Always;
        }
        match self.next() % 8 {
            0 => Condition::Always,
            1 => Condition::Cmp(self.cmpop(), self.value(1), self.value(1)),
            2 => Condition::HasTag(TagId(self.next()), self.who()),
            3 => Condition::HasBuff(BuffId(self.next()), self.who()),
            4 => Condition::HasTalent(TalentId(u32::from(self.next()))),
            5 => Condition::And((0..self.next() % 3).map(|_| self.cond(depth - 1)).collect()),
            6 => Condition::Or((0..self.next() % 3).map(|_| self.cond(depth - 1)).collect()),
            _ => Condition::Not(Box::new(self.cond(depth - 1))),
        }
    }
    fn direction(&mut self) -> Direction {
        match self.next() % 7 {
            0 => Direction::Forward,
            1 => Direction::Backward,
            2 => Direction::FromCaster,
            3 => Direction::TowardCaster,
            4 => Direction::ToTarget,
            5 => Direction::Up,
            _ => Direction::Custom(self.point()),
        }
    }
    fn filter(&mut self) -> TargetFilter {
        let affiliation = match self.next() % 3 {
            0 => Affiliation::Enemies,
            1 => Affiliation::Allies,
            _ => Affiliation::All,
        };
        TargetFilter {
            affiliation,
            require_tags: (0..self.next() % 3).map(|_| TagId(self.next())).collect(),
            exclude_tags: (0..self.next() % 3).map(|_| TagId(self.next())).collect(),
            include_dead: self.next().is_multiple_of(2),
        }
    }
    fn target(&mut self) -> ImpactTarget {
        match self.next() % 5 {
            0 => ImpactTarget::Caster,
            1 => ImpactTarget::PrimaryTarget,
            2 => ImpactTarget::ResolvedTarget,
            3 => ImpactTarget::Source,
            _ => ImpactTarget::AtPoint(self.point()),
        }
    }
    fn body(&mut self, depth: u8) -> BodyDescriptor {
        let kind = match self.next() % 3 {
            0 => BodyKind::Missile {
                speed: self.value(1),
                range: self.value(1),
                homing: self.next().is_multiple_of(2),
                pierce: self.value(1),
            },
            1 => BodyKind::Unit {
                health: self.value(1),
                duration: self.next().is_multiple_of(2).then(|| self.value(1)),
            },
            _ => BodyKind::Zone {
                radius: self.value(1),
                duration: self.value(1),
                tick: self.value(1),
            },
        };
        BodyDescriptor {
            kind,
            on_spawn: self.impacts(depth),
            on_hit: self.impacts(depth),
            on_expire: self.impacts(depth),
            collision: CollisionSpec {
                filter: self.filter(),
                pierce: self.value(1),
                through_walls: self.next().is_multiple_of(2),
            },
            flags: BodyFlags {
                reflectable: self.next().is_multiple_of(2),
                cross_dimension: self.next().is_multiple_of(2),
                live_values: self.next().is_multiple_of(2),
            },
        }
    }
    fn impacts(&mut self, depth: u8) -> Vec<Impact> {
        (0..self.next() % 3).map(|_| self.impact(depth)).collect()
    }
    fn impact(&mut self, depth: u8) -> Impact {
        let pick = if depth == 0 { 4 + self.next() % 14 } else { self.next() % 18 };
        match pick {
            0 => Impact::Retarget {
                shape: self.shape(),
                filter: self.filter(),
                max_targets: self.value(1),
                exclude_primary: self.next().is_multiple_of(2),
                inner: self.impacts(depth - 1),
            },
            1 => Impact::If {
                cond: self.cond(2),
                then: self.impacts(depth - 1),
                els: self.impacts(depth - 1),
            },
            2 => Impact::Loop { kind: self.loopkind(), inner: self.impacts(depth - 1) },
            3 => Impact::Delay { secs: self.value(1), inner: self.impacts(depth - 1) },
            4 => Impact::Damage {
                amount: self.value(2),
                dtype: DamageTypeId(self.next()),
                target: self.target(),
                flags: DamageFlags {
                    can_crit: self.next().is_multiple_of(2),
                    lifesteal: self.next().is_multiple_of(2),
                },
            },
            5 => Impact::Heal {
                amount: self.value(2),
                target: self.target(),
                flags: HealFlags { can_overheal: self.next().is_multiple_of(2) },
            },
            6 => Impact::AdjustPool {
                pool: self.pool(),
                op: self.numop(),
                amount: self.value(2),
                target: self.target(),
            },
            7 => Impact::ApplyModifiers {
                buff: BuffId(self.next()),
                stacks: self.value(1),
                duration_override: self.next().is_multiple_of(2).then(|| self.value(1)),
                target: self.target(),
            },
            8 => Impact::RemoveModifiers { sel: self.selector(), target: self.target() },
            9 => Impact::Dash {
                dir: self.direction(),
                dist: self.value(1),
                speed: self.next().is_multiple_of(2).then(|| self.value(1)),
                on_collision: if depth == 0 { Vec::new() } else { self.impacts(depth - 1) },
                target: self.target(),
            },
            10 => Impact::Knockback {
                dir: self.direction(),
                force: self.value(1),
                target: self.target(),
            },
            11 => Impact::Teleport {
                dest: self.teleport(),
                target: self.target(),
                record: self.next().is_multiple_of(2),
            },
            12 => Impact::Spawn {
                body: self.body(if depth == 0 { 0 } else { depth - 1 }),
                at: self.anchor(),
                count: self.value(1),
                pattern: self.pattern(),
            },
            13 => Impact::CastAbility {
                slot: self.slot(),
                target: self.abilitytarget(),
                value_scale: self.value(1),
                cost: if self.next().is_multiple_of(2) { CostMode::Normal } else { CostMode::Free },
            },
            14 => Impact::Interrupt { target: self.target() },
            15 => Impact::ResolvePending { filter: self.pending() },
            16 => Impact::Emit {
                event: EventId(self.next()),
                target: self.target(),
                payload: self.value(1),
            },
            _ => Impact::Custom {
                handler: HandlerId(u32::from(self.next())),
                params: (0..self.next() % 4).map(|_| self.next() as u8).collect(),
                target: self.target(),
            },
        }
    }
    fn shape(&mut self) -> TargetShape {
        match self.next() % 7 {
            0 => TargetShape::SelfOnly,
            1 => TargetShape::Circle { at: self.target(), radius: self.value(1) },
            2 => TargetShape::Cone { radius: self.value(1), angle: self.value(1) },
            3 => TargetShape::Chain { jumps: self.value(1), range: self.value(1) },
            4 => TargetShape::Line { length: self.value(1), width: self.value(1) },
            5 => TargetShape::AllAllies,
            _ => TargetShape::AllEnemies,
        }
    }
    fn loopkind(&mut self) -> LoopKind {
        if self.next().is_multiple_of(2) {
            LoopKind::Times { count: self.value(1), gap: self.value(1) }
        } else {
            LoopKind::Interval { period: self.value(1), ticks: self.value(1) }
        }
    }
    fn pool(&mut self) -> PoolRef {
        match self.next() % 5 {
            0 => PoolRef::Shield,
            1 => PoolRef::Resource(ResourceId(self.next())),
            2 => PoolRef::Stacks(StackId(self.next())),
            3 => PoolRef::Cooldown(self.slot()),
            _ => PoolRef::Charges(self.slot()),
        }
    }
    fn numop(&mut self) -> NumOp {
        match self.next() % 4 {
            0 => NumOp::Set,
            1 => NumOp::Add,
            2 => NumOp::Sub,
            _ => NumOp::Mul,
        }
    }
    fn selector(&mut self) -> BuffSelector {
        if self.next().is_multiple_of(2) {
            BuffSelector::Id(BuffId(self.next()))
        } else {
            BuffSelector::Class(TagClassId(self.next()))
        }
    }
    fn teleport(&mut self) -> TeleportDest {
        match self.next() % 4 {
            0 => TeleportDest::ToTarget,
            1 => TeleportDest::ToPoint(self.point()),
            2 => TeleportDest::Offset(self.direction(), self.value(1)),
            _ => TeleportDest::Home,
        }
    }
    fn anchor(&mut self) -> SpawnAnchor {
        match self.next() % 4 {
            0 => SpawnAnchor::Caster,
            1 => SpawnAnchor::Target,
            2 => SpawnAnchor::Source,
            _ => SpawnAnchor::Point(self.point()),
        }
    }
    fn pattern(&mut self) -> SpawnPattern {
        match self.next() % 3 {
            0 => SpawnPattern::Single,
            1 => SpawnPattern::Radial { count: self.value(1) },
            _ => SpawnPattern::Arc { count: self.value(1), spread: self.value(1) },
        }
    }
    fn abilitytarget(&mut self) -> AbilityTarget {
        match self.next() % 4 {
            0 => AbilityTarget::Inherit,
            1 => AbilityTarget::PrimaryTarget,
            2 => AbilityTarget::Caster,
            _ => AbilityTarget::Point(self.point()),
        }
    }
    fn pending(&mut self) -> PendingFilter {
        if self.next().is_multiple_of(2) {
            PendingFilter::FromCaster
        } else {
            PendingFilter::OriginTag(TagId(self.next()))
        }
    }
    fn modifier(&mut self) -> Modifier {
        let op = match self.next() % 4 {
            0 => ModOp::AddFlat,
            1 => ModOp::AddPct,
            2 => ModOp::Mul,
            _ => ModOp::Override,
        };
        Modifier { stat: StatId(self.next()), op, value: self.value(2) }
    }
    fn event_kind(&mut self) -> EventKind {
        match self.next() % 3 {
            0 => EventKind::OnHit,
            1 => EventKind::OnKill,
            _ => EventKind::Custom(EventId(self.next())),
        }
    }
    fn reaction(&mut self) -> Reaction {
        Reaction {
            on: self.event_kind(),
            filter: EventFilter {
                source_slot: self.next().is_multiple_of(2).then(|| self.slot()),
                every_nth: self.next().is_multiple_of(2).then(|| self.next()),
                require_tag_on_target: self.next().is_multiple_of(2).then(|| TagId(self.next())),
            },
            cond: self.cond(2),
            effects: self.impacts(2),
            target: self.target(),
            internal_cd: self.next().is_multiple_of(2).then(|| self.value(1)),
            charges: self.next().is_multiple_of(2).then(|| self.next()),
        }
    }
    fn ability(&mut self) -> AbilityDescriptor {
        let targeting = match self.next() % 5 {
            0 => Targeting::NoTarget,
            1 => Targeting::Unit { filter: self.filter() },
            2 => Targeting::Point,
            3 => Targeting::Vector,
            _ => Targeting::SelfCast,
        };
        let cast = match self.next() % 3 {
            0 => CastSpec::Instant,
            1 => CastSpec::Cast { time: self.value(1), movable: self.next().is_multiple_of(2) },
            _ => CastSpec::Channel {
                time: self.value(1),
                movable: self.next().is_multiple_of(2),
                tick: self.next().is_multiple_of(2).then(|| self.value(1)),
            },
        };
        let cost = (0..self.next() % 3)
            .map(|_| match self.next() % 3 {
                0 => Cost::Resource { res: ResourceId(self.next()), amount: self.value(1) },
                1 => Cost::Charge,
                _ => Cost::Health { amount: self.value(1) },
            })
            .collect();
        AbilityDescriptor {
            id: AbilityId(u32::from(self.next())),
            params: Params(
                (0..self.next() % 3).map(|_| (ParamId(self.next()), self.value(1))).collect(),
            ),
            targeting,
            cast,
            cost,
            cast_gate: self.cond(2),
            on_cast_start: self.impacts(2),
            on_cast: self.impacts(2),
            tags: (0..self.next() % 3).map(|_| TagId(self.next())).collect(),
        }
    }
    fn talent(&mut self) -> TalentDescriptor {
        let selector = match self.next() % 5 {
            0 => AbilitySelector::Slot(self.slot()),
            1 => AbilitySelector::Tag(TagId(self.next())),
            2 => AbilitySelector::Ability(AbilityId(u32::from(self.next()))),
            3 => AbilitySelector::Any,
            _ => AbilitySelector::SelfUnit,
        };
        let hook = |n: u16| match n % 3 {
            0 => AbilityHook::OnCastStart,
            1 => AbilityHook::OnCast,
            _ => AbilityHook::OnHit,
        };
        TalentDescriptor {
            id: TalentId(u32::from(self.next())),
            selector,
            patches: (0..self.next() % 3)
                .map(|_| ParamPatch {
                    param: ParamId(self.next()),
                    op: self.numop(),
                    value: self.value(1),
                })
                .collect(),
            riders: (0..self.next() % 2)
                .map(|_| Rider { hook: hook(self.next()), effects: self.impacts(2) })
                .collect(),
            add_reactions: (0..self.next() % 2).map(|_| self.reaction()).collect(),
            grants: (0..self.next() % 2)
                .map(|_| GrantAbility {
                    slot: self.slot(),
                    ability: AbilityId(u32::from(self.next())),
                })
                .collect(),
            modifiers: (0..self.next() % 3).map(|_| self.modifier()).collect(),
            tags: (0..self.next() % 3).map(|_| TagId(self.next())).collect(),
            // A task names a counter, which is an interned handle like any other —
            // so the walk has to reach it (server#132).
            quest: self
                .next()
                .is_multiple_of(2)
                .then(|| QuestSpec { counter: StackId(self.next()), goal: f32::from(self.next()) }),
        }
    }
    fn buff(&mut self) -> BuffSpec {
        BuffSpec {
            id: BuffId(self.next()),
            duration: self.next().is_multiple_of(2).then(|| self.value(1)),
            stacking: Stacking {
                on_reapply: match self.next() % 4 {
                    0 => Reapply::RefreshDuration,
                    1 => Reapply::AddDuration,
                    2 => Reapply::Independent,
                    _ => Reapply::Ignore,
                },
                scope: if self.next().is_multiple_of(2) {
                    StackScope::PerSource
                } else {
                    StackScope::Global
                },
            },
            max_stacks: self.next(),
            modifiers: (0..self.next() % 3).map(|_| self.modifier()).collect(),
            tags: (0..self.next() % 3).map(|_| TagId(self.next())).collect(),
            reactions: (0..self.next() % 2).map(|_| self.reaction()).collect(),
            on_apply: self.impacts(2),
            on_expire: self.impacts(2),
            on_remove: self.impacts(2),
            drop_on_death: self.next().is_multiple_of(2),
        }
    }
    fn navmesh(&mut self) -> NavMeshDescriptor {
        NavMeshDescriptor {
            id: NavMeshId(u32::from(self.next())),
            outline: (0..self.next() % 4 + 3).map(|_| [self.f32(), self.f32()]).collect(),
            obstacles: (0..self.next() % 2)
                .map(|_| (0..self.next() % 3 + 3).map(|_| [self.f32(), self.f32()]).collect())
                .collect(),
            agent_radius: self.f32().abs(),
            placements: (0..self.next() % 3)
                .map(|_| UnitPlacement {
                    unit: UnitId(u32::from(self.next())),
                    team: u32::from(self.next() % 4),
                    at: [self.f32(), self.f32()],
                    facing: self.f32(),
                    respawn: self.next().is_multiple_of(2).then(|| self.f32().abs()),
                })
                .collect(),
        }
    }
    fn unit(&mut self) -> UnitDescriptor {
        UnitDescriptor {
            id: UnitId(u32::from(self.next())),
            health: self.value(1),
            stats: (0..self.next() % 3).map(|_| (StatId(self.next()), self.value(1))).collect(),
            tags: (0..self.next() % 3).map(|_| TagId(self.next())).collect(),
            abilities: (0..self.next() % 3)
                .map(|_| (self.slot(), AbilityId(u32::from(self.next()))))
                .collect(),
            resources: (0..self.next() % 3)
                .map(|_| ResourcePool {
                    id: ResourceId(self.next()),
                    max: self.value(1),
                    regen: self.value(1),
                })
                .collect(),
            talents: (0..self.next() % 3).map(|_| TalentId(u32::from(self.next()))).collect(),
            talent_tree: None,
            respawn: None,
            progression: None,
            turn_rate: None,
        }
    }
    fn registration(&mut self) -> Registration {
        Registration {
            abi: ABI_VERSION,
            names: Names::default(),
            abilities: (0..self.next() % 3).map(|_| self.ability()).collect(),
            talents: (0..self.next() % 3).map(|_| self.talent()).collect(),
            buffs: (0..self.next() % 3).map(|_| self.buff()).collect(),
            tag_classes: (0..self.next() % 3)
                .map(|_| (TagId(self.next()), TagClassId(self.next())))
                .collect(),
            curves: (0..self.next() % 2)
                .map(|_| Curve {
                    points: (0..self.next() % 3).map(|_| [self.f32(), self.f32()]).collect(),
                })
                .collect(),
            units: (0..self.next() % 3).map(|_| self.unit()).collect(),
            navmeshes: (0..self.next() % 2).map(|_| self.navmesh()).collect(),
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    ops: Vec<u16>,
}

fn build(s: &Scenario) -> Registration {
    Gen { ops: &s.ops, pos: 0 }.registration()
}

#[test]
fn identity_remap_is_a_noop() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = build(s);
        let mut out = reg.clone();
        out.remap_ids(&Counting::default()).expect("identity map never fails");
        // Byte-identical: an id-returning-unchanged map must leave the tree exactly
        // as authored — the walk rewrites ids and touches nothing else.
        let a = postcard::to_allocvec(&reg).expect("serialize");
        let b = postcard::to_allocvec(&out).expect("serialize");
        assert_eq!(a, b, "identity remap changed the tree");
    });
}

#[test]
fn remap_is_deterministic() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = build(s);
        let (mut a, mut b) = (reg.clone(), reg.clone());
        a.remap_ids(&Counting::default()).unwrap();
        b.remap_ids(&Counting::default()).unwrap();
        assert_eq!(
            postcard::to_allocvec(&a).unwrap(),
            postcard::to_allocvec(&b).unwrap(),
            "remap is non-deterministic",
        );
    });
}

#[test]
fn a_failing_map_errors_without_panicking() {
    check!().with_type::<Scenario>().for_each(|s| {
        // Count the ids the walk visits through a total map.
        let counter = Counting::default();
        let mut probe = build(s);
        probe.remap_ids(&counter).expect("total map succeeds");
        let had_ids = counter.seen.get() > 0;

        // A map that fails on the first id must surface Err (never panic) exactly
        // when the tree carried at least one interned handle.
        let mut reg = build(s);
        let result = reg.remap_ids(&FailAll);
        assert_eq!(result.is_err(), had_ids, "error propagation disagrees with id presence");
    });
}
