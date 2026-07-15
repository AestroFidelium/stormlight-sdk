//! The ISA is a total, self-describing data model: **any** generated `Impact`
//! tree survives a postcard round-trip unchanged, and no variant needs a special
//! case. This is the data-layer form of the epic's fuzz invariant "any generated
//! `Impact` interprets without a special case" — nothing content-shaped can leak
//! into the vocabulary, because the whole vocabulary is fixed and serializable.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::behaviors::{ModOp, Modifier};
use stormlight_mod_abi::common::{Affiliation, Direction, ImpactTarget, NumOp, TargetFilter};
use stormlight_mod_abi::conditions::{CmpOp, Condition};
use stormlight_mod_abi::ids::{
    BuffId, CurveId, DamageTypeId, EventId, HandlerId, ResourceId, Slot, StackId, StatId,
    TagClassId, TagId, TalentId,
};
use stormlight_mod_abi::impacts::{
    AbilityTarget, BuffSelector, CostMode, DamageFlags, HealFlags, Impact, LoopKind, PendingFilter,
    PoolRef, SpawnAnchor, SpawnPattern, TargetShape, TeleportDest,
};
use stormlight_mod_abi::math::{BinOp, Value, Var, Who};
use stormlight_mod_abi::missiles::{BodyDescriptor, BodyFlags, BodyKind, CollisionSpec};

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
    fn who(&mut self) -> Who {
        match self.next() % 3 {
            0 => Who::Caster,
            1 => Who::Target,
            _ => Who::Source,
        }
    }
    fn target(&mut self) -> ImpactTarget {
        match self.next() % 5 {
            0 => ImpactTarget::Caster,
            1 => ImpactTarget::PrimaryTarget,
            2 => ImpactTarget::ResolvedTarget,
            3 => ImpactTarget::Source,
            _ => ImpactTarget::AtPoint([self.f32(), self.f32(), self.f32()]),
        }
    }
    fn value(&mut self, depth: u8) -> Value {
        if depth == 0 {
            return Value::Const(self.f32());
        }
        match self.next() % 7 {
            0 => Value::Const(self.f32()),
            1 => Value::Read(self.var()),
            2 => Value::Bin(self.binop(), Box::new(self.value(depth - 1)), Box::new(self.value(depth - 1))),
            3 => Value::Clamp {
                v: Box::new(self.value(depth - 1)),
                lo: Box::new(self.value(depth - 1)),
                hi: Box::new(self.value(depth - 1)),
            },
            4 => Value::Curve(CurveId(self.next()), Box::new(self.value(depth - 1))),
            5 => Value::ScaleCtx,
            _ => Value::Read(Var::Level),
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
            10 => Var::ChargesOf(Slot(self.next() as u8), w),
            11 => Var::CooldownOf(Slot(self.next() as u8), w),
            12 => Var::AllyCount,
            13 => Var::EnemyCount,
            14 => Var::DistanceToTarget,
            15 => Var::ChannelProgress,
            _ => Var::Rand01,
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
    fn cmpop(&mut self) -> CmpOp {
        match self.next() % 5 {
            0 => CmpOp::Lt,
            1 => CmpOp::Le,
            2 => CmpOp::Eq,
            3 => CmpOp::Ge,
            _ => CmpOp::Gt,
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
            _ => Direction::Custom([self.f32(), self.f32(), self.f32()]),
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
            _ => BodyKind::Zone { radius: self.value(1), duration: self.value(1), tick: self.value(1) },
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
        let n = self.next() % 3;
        (0..n).map(|_| self.impact(depth)).collect()
    }
    fn impact(&mut self, depth: u8) -> Impact {
        // At depth 0, only emit leaves (no further nesting).
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
                flags: DamageFlags { can_crit: self.next().is_multiple_of(2), lifesteal: self.next().is_multiple_of(2) },
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
            10 => Impact::Knockback { dir: self.direction(), force: self.value(1), target: self.target() },
            11 => Impact::Teleport { dest: self.teleport(), target: self.target(), record: self.next().is_multiple_of(2) },
            12 => Impact::Spawn {
                body: self.body(if depth == 0 { 0 } else { depth - 1 }),
                at: self.anchor(),
                count: self.value(1),
                pattern: self.pattern(),
            },
            13 => Impact::CastAbility {
                slot: Slot(self.next() as u8),
                target: self.abilitytarget(),
                value_scale: self.value(1),
                cost: if self.next().is_multiple_of(2) { CostMode::Normal } else { CostMode::Free },
            },
            14 => Impact::Interrupt { target: self.target() },
            15 => Impact::ResolvePending { filter: self.pending() },
            16 => Impact::Emit { event: EventId(self.next()), target: self.target(), payload: self.value(1) },
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
            1 => TargetShape::Circle { radius: self.value(1) },
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
            3 => PoolRef::Cooldown(Slot(self.next() as u8)),
            _ => PoolRef::Charges(Slot(self.next() as u8)),
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
            1 => TeleportDest::ToPoint([self.f32(), self.f32(), self.f32()]),
            2 => TeleportDest::Offset(self.direction(), self.value(1)),
            _ => TeleportDest::Home,
        }
    }
    fn anchor(&mut self) -> SpawnAnchor {
        match self.next() % 4 {
            0 => SpawnAnchor::Caster,
            1 => SpawnAnchor::Target,
            2 => SpawnAnchor::Source,
            _ => SpawnAnchor::Point([self.f32(), self.f32(), self.f32()]),
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
            _ => AbilityTarget::Point([self.f32(), self.f32(), self.f32()]),
        }
    }
    fn pending(&mut self) -> PendingFilter {
        if self.next().is_multiple_of(2) {
            PendingFilter::FromCaster
        } else {
            PendingFilter::OriginTag(TagId(self.next()))
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    ops: Vec<u16>,
}

#[test]
fn any_impact_survives_a_postcard_round_trip() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut g = Gen { ops: &s.ops, pos: 0 };
        let tree = g.impact(4);

        // Serialize -> deserialize -> reserialize. The two byte strings must be
        // identical: every variant (de)serializes with no special case, and the
        // representation is stable (a wire-format requirement for the ABI).
        let bytes = postcard::to_allocvec(&tree).expect("serialize");
        let back: Impact = postcard::from_bytes(&bytes).expect("deserialize");
        let again = postcard::to_allocvec(&back).expect("reserialize");
        assert_eq!(bytes, again, "Impact did not round-trip through postcard");
    });
}

#[test]
fn combinator_partition_is_total() {
    // The walker relies on exactly the four control combinators reporting as
    // combinators and every leaf reporting as a leaf.
    let combinators = [
        Impact::Retarget {
            shape: TargetShape::SelfOnly,
            filter: TargetFilter {
                affiliation: Affiliation::All,
                require_tags: Vec::new(),
                exclude_tags: Vec::new(),
                include_dead: false,
            },
            max_targets: Value::Const(1.0),
            exclude_primary: false,
            inner: Vec::new(),
        },
        Impact::If { cond: Condition::Always, then: Vec::new(), els: Vec::new() },
        Impact::Loop {
            kind: LoopKind::Times { count: Value::Const(1.0), gap: Value::Const(0.0) },
            inner: Vec::new(),
        },
        Impact::Delay { secs: Value::Const(1.0), inner: Vec::new() },
    ];
    for c in &combinators {
        assert!(c.is_combinator(), "{c:?} should be a combinator");
    }

    let leaves = [
        Impact::Interrupt { target: ImpactTarget::default() },
        Impact::Damage {
            amount: Value::Const(1.0),
            dtype: DamageTypeId(0),
            target: ImpactTarget::default(),
            flags: DamageFlags::default(),
        },
        Impact::Emit { event: EventId(0), target: ImpactTarget::default(), payload: Value::Const(0.0) },
    ];
    for l in &leaves {
        assert!(!l.is_combinator(), "{l:?} should be a leaf");
    }

    // A modifier is not part of the impact vocabulary — it lives beside it, and
    // the ISA references buffs by id, keeping magnitude data out of the tree.
    let _m = Modifier { stat: StatId(0), op: ModOp::AddFlat, value: Value::Const(1.0) };
}
