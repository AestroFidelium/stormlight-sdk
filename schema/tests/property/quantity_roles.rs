//! Quantity roles (stormlight/server#190): a talent patches **what a number is** —
//! damage, healing, a duration, a radius — not what a mod happened to call it.
//!
//! Laws:
//!   - **one table**: every role is a reserved param name, in reserved-id order, so
//!     a mod spelling `"damage"` lands on the id the engine patches by role;
//!   - **order-free fold**: the same patches in any order give the same number —
//!     two talents never need to know which was picked first;
//!   - **expression = number**: the patched `Value` the tree carries evaluates to
//!     exactly what the numeric fold gives;
//!   - **only that role moves**: patching a role rewrites every quantity of that
//!     role, wherever in the program it sits (a missile's payload, a zone's, a
//!     blast's), and leaves every other quantity and the program's shape alone;
//!   - **absent is a no-op**: a role the program does not carry changes nothing.

use std::collections::BTreeMap;

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::common::{Affiliation, ImpactTarget, NumOp, TargetFilter};
use stormlight_mod_abi::ids::{BuffId, DamageTypeId, ParamId};
use stormlight_mod_abi::impacts::{
    DamageFlags, HealFlags, Impact, PoolRef, SpawnAnchor, SpawnPattern, TargetShape,
};
use stormlight_mod_abi::math::Value;
use stormlight_mod_abi::missiles::{BodyDescriptor, BodyFlags, BodyKind, CollisionSpec};
use stormlight_mod_abi::params;
use stormlight_mod_abi::roles::{
    QuantityRole, fold_patches, materialize_durations, patch_quantities, patched_value, quantities,
};

/// A small whole number, so sums and products stay exact in `f32` and "the same
/// in any order" can be asserted bit for bit.
#[derive(Debug, Clone, Copy, TypeGenerator)]
struct Small(#[generator(1u8..=12)] u8);

impl Small {
    fn f(self) -> f32 {
        f32::from(self.0)
    }
    fn v(self) -> Value {
        Value::Const(self.f())
    }
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Op {
    Set,
    Add,
    Sub,
    Mul,
}

impl Op {
    fn num(self) -> NumOp {
        match self {
            Op::Set => NumOp::Set,
            Op::Add => NumOp::Add,
            Op::Sub => NumOp::Sub,
            Op::Mul => NumOp::Mul,
        }
    }
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
struct Patch {
    op: Op,
    amount: Small,
}

/// Which role a scenario patches, as an index into [`QuantityRole::ALL`].
#[derive(Debug, Clone, Copy, TypeGenerator)]
struct RolePick(#[generator(0u8..=10)] u8);

impl RolePick {
    fn role(self) -> QuantityRole {
        let all = QuantityRole::ALL;
        all[usize::from(self.0) % all.len()]
    }
}

/// A leaf effect that carries (or does not carry) a quantity.
#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Leaf {
    Hit(Small),
    Mend(Small),
    /// A shield granted (`add`) or stripped — only the grant is shielding.
    Ward {
        amount: Small,
        grant: bool,
    },
    /// A buff applied with or without its own duration. Without one, the buff's
    /// declared duration is what the effect lasts — an even buff id declares one,
    /// an odd one is permanent.
    Mark {
        buff: u8,
        stacks: Small,
        duration: Option<Small>,
    },
    Shove(Small),
    /// A leaf with no quantity at all.
    Interrupt,
}

/// One top-level piece of an ability's program.
#[derive(Debug, Clone, TypeGenerator)]
enum Piece {
    Leaf(Leaf),
    Missile {
        speed: Small,
        range: Small,
        #[generator(bolero::produce::<Vec<Leaf>>().with().len(0usize..=3))]
        payload: Vec<Leaf>,
    },
    Zone {
        radius: Small,
        duration: Small,
        #[generator(bolero::produce::<Vec<Leaf>>().with().len(0usize..=3))]
        payload: Vec<Leaf>,
    },
    Blast {
        radius: Small,
        #[generator(bolero::produce::<Vec<Leaf>>().with().len(0usize..=3))]
        inner: Vec<Leaf>,
    },
}

/// A base and the patches stacked on it, few enough that every product stays exact.
#[derive(Debug, Clone, TypeGenerator)]
struct Stack {
    base: Small,
    #[generator(bolero::produce::<Vec<Patch>>().with().len(0usize..=4))]
    patches: Vec<Patch>,
}

#[derive(Debug, Clone, TypeGenerator)]
struct Scenario {
    #[generator(bolero::produce::<Vec<Piece>>().with().len(0usize..=4))]
    program: Vec<Piece>,
    role: RolePick,
    #[generator(bolero::produce::<Vec<Patch>>().with().len(1usize..=4))]
    patches: Vec<Patch>,
}

fn leaf(l: Leaf) -> Impact {
    let target = ImpactTarget::ResolvedTarget;
    match l {
        Leaf::Hit(a) => Impact::Damage {
            amount: a.v(),
            dtype: DamageTypeId(0),
            target,
            flags: DamageFlags::default(),
        },
        Leaf::Mend(a) => Impact::Heal { amount: a.v(), target, flags: HealFlags::default() },
        Leaf::Ward { amount, grant } => Impact::AdjustPool {
            pool: PoolRef::Shield,
            op: if grant { NumOp::Add } else { NumOp::Sub },
            amount: amount.v(),
            target,
        },
        Leaf::Mark { buff, stacks, duration } => Impact::ApplyModifiers {
            buff: BuffId(u16::from(buff)),
            stacks: stacks.v(),
            duration_override: duration.map(Small::v),
            target,
        },
        Leaf::Shove(f) => Impact::Knockback {
            dir: stormlight_mod_abi::common::Direction::FromCaster,
            force: f.v(),
            target,
        },
        Leaf::Interrupt => Impact::Interrupt { target },
    }
}

fn body(kind: BodyKind, payload: &[Leaf]) -> BodyDescriptor {
    BodyDescriptor {
        kind,
        on_spawn: Vec::new(),
        on_hit: payload.iter().copied().map(leaf).collect(),
        on_expire: Vec::new(),
        collision: CollisionSpec {
            filter: TargetFilter::of(Affiliation::Enemies),
            pierce: Value::Const(0.0),
            through_walls: false,
        },
        flags: BodyFlags::default(),
        height: Value::Const(0.0),
    }
}

fn spawn(body: BodyDescriptor) -> Impact {
    Impact::Spawn {
        body,
        at: SpawnAnchor::Caster,
        count: Value::Const(1.0),
        pattern: SpawnPattern::Single,
    }
}

fn program(pieces: &[Piece]) -> Vec<Impact> {
    pieces
        .iter()
        .map(|p| match p {
            Piece::Leaf(l) => leaf(*l),
            Piece::Missile { speed, range, payload } => spawn(body(
                BodyKind::Missile {
                    speed: speed.v(),
                    range: range.v(),
                    homing: false,
                    pierce: Value::Const(0.0),
                },
                payload,
            )),
            Piece::Zone { radius, duration, payload } => spawn(body(
                BodyKind::Zone {
                    radius: radius.v(),
                    duration: duration.v(),
                    tick: Value::Const(1.0),
                    on_enter: Vec::new(),
                    time_scale: None,
                },
                payload,
            )),
            Piece::Blast { radius, inner } => Impact::Retarget {
                shape: TargetShape::Circle { at: ImpactTarget::Caster, radius: radius.v() },
                filter: TargetFilter::of(Affiliation::Enemies),
                max_targets: Value::Const(5.0),
                exclude_primary: false,
                inner: inner.iter().copied().map(leaf).collect(),
            },
        })
        .collect()
}

/// An even buff declares a duration of its own; an odd one lasts until removed.
fn buff_duration(buff: BuffId) -> Option<Value> {
    buff.0.is_multiple_of(2).then_some(Value::Const(7.0))
}

fn as_num(patches: &[Patch]) -> Vec<(NumOp, f32)> {
    patches.iter().map(|p| (p.op.num(), p.amount.f())).collect()
}

fn as_value(patches: &[Patch]) -> Vec<(NumOp, Value)> {
    patches.iter().map(|p| (p.op.num(), p.amount.v())).collect()
}

/// The reference model: the last `Set` replaces the base, the additions are summed
/// onto it, and the multipliers scale the result.
fn model(base: f32, patches: &[(NumOp, f32)]) -> f32 {
    let start = patches.iter().rev().find(|(op, _)| *op == NumOp::Set).map_or(base, |p| p.1);
    let sum: f32 = patches
        .iter()
        .map(|(op, a)| match op {
            NumOp::Add => *a,
            NumOp::Sub => -*a,
            _ => 0.0,
        })
        .sum();
    let product: f32 =
        patches.iter().filter(|(op, _)| *op == NumOp::Mul).map(|(_, a)| *a).product();
    (start + sum) * product
}

/// No context is needed to evaluate a tree of constants.
struct NoCtx;

impl stormlight_mod_abi::math::ValueCtx for NoCtx {
    fn level(&self) -> f32 {
        0.0
    }
    fn max_hp(&self, _: stormlight_mod_abi::math::Who) -> f32 {
        0.0
    }
    fn cur_hp(&self, _: stormlight_mod_abi::math::Who) -> f32 {
        0.0
    }
    fn stat(&self, _: stormlight_mod_abi::ids::StatId, _: stormlight_mod_abi::math::Who) -> f32 {
        0.0
    }
    fn resource(
        &self,
        _: stormlight_mod_abi::ids::ResourceId,
        _: stormlight_mod_abi::math::Who,
    ) -> f32 {
        0.0
    }
    fn stack_count(
        &self,
        _: stormlight_mod_abi::ids::StackId,
        _: stormlight_mod_abi::math::Who,
    ) -> f32 {
        0.0
    }
    fn stack_gain(
        &self,
        _: stormlight_mod_abi::ids::StackId,
        _: stormlight_mod_abi::math::Who,
    ) -> f32 {
        0.0
    }
    fn buff_stacks(&self, _: BuffId, _: stormlight_mod_abi::math::Who) -> f32 {
        0.0
    }
    fn buff_stacks_from(
        &self,
        _: BuffId,
        _: stormlight_mod_abi::math::Who,
        _: stormlight_mod_abi::math::Who,
    ) -> f32 {
        0.0
    }
    fn charges_of(
        &self,
        _: stormlight_mod_abi::ids::Slot,
        _: stormlight_mod_abi::math::Who,
    ) -> f32 {
        0.0
    }
    fn cooldown_of(
        &self,
        _: stormlight_mod_abi::ids::Slot,
        _: stormlight_mod_abi::math::Who,
    ) -> f32 {
        0.0
    }
    fn source_slot(&self) -> Option<stormlight_mod_abi::ids::Slot> {
        None
    }
    fn ally_count(&self) -> f32 {
        0.0
    }
    fn enemy_count(&self) -> f32 {
        0.0
    }
    fn distance_to_target(&self) -> f32 {
        0.0
    }
    fn channel_progress(&self) -> f32 {
        0.0
    }
    fn rand01(&self) -> f32 {
        0.0
    }
    fn event_magnitude(&self) -> f32 {
        0.0
    }
    fn loop_index(&self) -> f32 {
        0.0
    }
    fn scale(&self) -> f32 {
        1.0
    }
    fn curve(&self, _: stormlight_mod_abi::ids::CurveId, x: f32) -> f32 {
        x
    }
}

#[test]
fn every_role_is_a_reserved_param_in_id_order() {
    let all = QuantityRole::ALL;
    for (i, role) in all.iter().enumerate() {
        let id = role.param();
        let name = params::RESERVED.get(usize::from(id.0)).copied();
        assert_eq!(name, Some(role.name()), "{role:?} must sit on its own reserved name");
        assert_eq!(QuantityRole::of_param(id), Some(*role));
        assert_eq!(params::reserved_index(role.name()), Some(usize::from(id.0)));
        assert!(all.iter().skip(i + 1).all(|other| other.name() != role.name()));
    }
    // Every reserved param is a role: there is one vocabulary, not two.
    assert_eq!(all.len(), params::RESERVED.len());
    // A mod's own param above the reserved table is nobody's role.
    let first_free = ParamId(u16::try_from(params::RESERVED.len()).unwrap_or(u16::MAX));
    assert_eq!(QuantityRole::of_param(first_free), None);
}

#[test]
fn the_fold_does_not_care_which_talent_came_first() {
    check!().with_type::<Stack>().for_each(|Stack { base, patches }| {
        let forward = as_num(patches);
        let mut backward = forward.clone();
        backward.reverse();
        // Reversal moves a `Set`; the last `Set` in *talent* order is the one that
        // wins, so only permutations that keep the Sets' relative order compare.
        let sets: Vec<_> = forward.iter().filter(|(op, _)| *op == NumOp::Set).collect();
        let got = fold_patches(base.f(), &forward);
        assert_eq!(got.to_bits(), model(base.f(), &forward).to_bits());
        if sets.len() <= 1 {
            assert_eq!(got.to_bits(), fold_patches(base.f(), &backward).to_bits());
        }
        assert_eq!(fold_patches(base.f(), &[]).to_bits(), base.f().to_bits());
    });
}

#[test]
fn the_patched_expression_evaluates_to_the_fold() {
    check!().with_type::<Stack>().for_each(|Stack { base, patches }| {
        let base = base.f() / 7.0;
        let expr = patched_value(Value::Const(base), &as_value(patches));
        assert_eq!(expr.eval(&NoCtx).to_bits(), fold_patches(base, &as_num(patches)).to_bits());
        assert_eq!(patched_value(Value::Const(base), &[]), Value::Const(base));
    });
}

#[test]
fn a_patch_moves_its_own_role_everywhere_and_nothing_else() {
    check!().with_type::<Scenario>().for_each(|s| {
        let role = s.role.role();
        let original = program(&s.program);
        let mut materialized = original.clone();
        materialize_durations(&mut materialized, &buff_duration);
        let before = quantities(&materialized);

        let mut table = BTreeMap::new();
        table.insert(role.param(), as_value(&s.patches));
        let mut patched = original.clone();
        patch_quantities(&mut patched, &table, &buff_duration);
        let after = quantities(&patched);

        // Same shape: the same quantities, of the same roles, in the same order —
        // with a buff's own duration made explicit only when durations were patched.
        let reference =
            if role == QuantityRole::Duration { &before } else { &quantities(&original) };
        assert_eq!(after.len(), reference.len());
        for ((r_before, v_before), (r_after, v_after)) in reference.iter().zip(&after) {
            assert_eq!(r_before, r_after);
            if *r_before == role {
                assert_eq!(*v_after, patched_value(v_before.clone(), &as_value(&s.patches)));
            } else {
                assert_eq!(v_after, v_before);
            }
        }
        // A role the program does not carry is a no-op, byte for byte.
        if !before.iter().any(|(r, _)| *r == role) {
            assert_eq!(patched, original);
        }
    });
}

#[test]
fn an_empty_table_leaves_the_program_untouched() {
    check!().with_type::<Vec<Piece>>().for_each(|pieces| {
        let original = program(pieces);
        let mut patched = original.clone();
        patch_quantities(&mut patched, &BTreeMap::new(), &buff_duration);
        assert_eq!(patched, original);
        // A mod's own param is not a role either.
        let mut table = BTreeMap::new();
        let mods_own = ParamId(u16::try_from(params::RESERVED.len()).unwrap_or(u16::MAX));
        table.insert(mods_own, vec![(NumOp::Mul, Value::Const(3.0))]);
        patch_quantities(&mut patched, &table, &buff_duration);
        assert_eq!(patched, original);
    });
}
