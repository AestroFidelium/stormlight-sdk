//! **Quantity roles** — what a number in an ability *is* (stormlight/server#190).
//!
//! A talent that says "+10% damage" has to mean damage on every ability it is ever
//! given, including ones written later and ones from another mod. Patching a param
//! by *name* cannot promise that: a name is a mod's private convention, and the same
//! patch would read as damage on one ability and as a charge count on another. So a
//! talent patches a **role** instead, from this closed vocabulary, and the engine
//! knows where every role lives:
//!
//! - most roles are **positions in the effect ISA** — the `amount` of a `Damage` is
//!   damage, the `force` of a `Knockback` is knockback, the `radius` of a zone is a
//!   radius. Nothing has to be annotated, so nothing can be annotated wrongly, and a
//!   literal deep inside a missile's payload is as reachable as a top-level one;
//! - the rest are numbers the cast pipeline reads off the ability itself — its
//!   cooldown, its aim range — which were already reserved params.
//!
//! The two are one table: **every role is a reserved param name**, in the same id
//! order ([`crate::params::RESERVED`]), so a [`ParamPatch`] naming `"damage"` is a
//! role patch and the old `"cooldown"`/`"range"` patches were role patches all along.
//! A role an ability does not carry is a well-defined no-op — "+10% damage" on a
//! pure heal changes nothing — and the engine reports such a patch rather than
//! leaving the author to discover it.
//!
//! **Append only.** A role's index is its param id and its wire identity.
//!
//! [`ParamPatch`]: crate::talents::ParamPatch

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::common::NumOp;
use crate::ids::{BuffId, ParamId};
use crate::impacts::{Impact, PoolRef, TargetShape};
use crate::math::{BinOp, Value};
use crate::missiles::{BodyDescriptor, BodyKind};
use crate::motion::{Leg, Motion};
use crate::params;

/// What a quantity is. See the module docs.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum QuantityRole {
    /// Seconds before the ability's slot can be used again. Read off the ability.
    Cooldown,
    /// How far the ability reaches: its aim range, the distance its missiles fly,
    /// the length of its lines and of its dashes, and a basic attack's reach.
    Range,
    /// The size of an area: a blast's circle, a cone's reach, a zone's radius, and
    /// the area the ability's aim draws.
    Radius,
    /// How wide a cone opens, in radians from its centre line — the aimed cone and
    /// a cone the ability resolves.
    Spread,
    /// The point of no return of the ability's cast. Read off the ability.
    Commit,
    /// The amount of every `Damage` the ability deals.
    Damage,
    /// The amount of every `Heal` the ability does.
    Healing,
    /// Every shield the ability grants (an `AdjustPool` adding to a shield).
    Shielding,
    /// How long what the ability leaves behind lasts: the buffs it applies, the
    /// zones, summons and items it puts down.
    Duration,
    /// The force of every knockback the ability inflicts.
    Knockback,
    /// How fast what the ability moves goes: its missiles and its dashes.
    Speed,
}

impl QuantityRole {
    /// Every role, in reserved-param order: `ALL[i].param()` is `ParamId(i)`.
    pub const ALL: [Self; 11] = [
        Self::Cooldown,
        Self::Range,
        Self::Radius,
        Self::Spread,
        Self::Commit,
        Self::Damage,
        Self::Healing,
        Self::Shielding,
        Self::Duration,
        Self::Knockback,
        Self::Speed,
    ];

    /// The role's printable identity — and the reserved param name a mod spells to
    /// patch it. One word for both, so what an interface prints about a talent can
    /// never name a different quantity than the one the talent changes.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cooldown => params::COOLDOWN,
            Self::Range => params::RANGE,
            Self::Radius => params::RADIUS,
            Self::Spread => params::SPREAD,
            Self::Commit => params::COMMIT,
            Self::Damage => params::DAMAGE,
            Self::Healing => params::HEALING,
            Self::Shielding => params::SHIELDING,
            Self::Duration => params::DURATION,
            Self::Knockback => params::KNOCKBACK,
            Self::Speed => params::SPEED,
        }
    }

    /// The reserved param id this role is patched under.
    #[must_use]
    pub const fn param(self) -> ParamId {
        ParamId(self as u16)
    }

    /// The role a param id names, or `None` for a mod's own param.
    #[must_use]
    pub fn of_param(param: ParamId) -> Option<Self> {
        Self::ALL.get(usize::from(param.0)).copied()
    }
}

/// One stacked talent patch, as the fold reads it.
pub type Patch<T> = (NumOp, T);

/// Fold `patches` onto `base` — the one rule every patched number follows.
///
/// **Order-free**: the last `Set` (in talent order) replaces the base, every `Add`
/// and `Sub` is summed onto that, and every `Mul` scales the result. So "+10" and
/// "+20%" from two talents give the same number whichever was picked first, which a
/// left-to-right fold could not promise — and "everything stacks" is a rule a player
/// can predict rather than an accident of talent ids.
#[must_use]
pub fn fold_patches(base: f32, patches: &[Patch<f32>]) -> f32 {
    let start = patches.iter().rev().find(|(op, _)| *op == NumOp::Set).map_or(base, |p| p.1);
    let summed = patches.iter().fold(start, |v, (op, a)| match op {
        NumOp::Add => v + a,
        NumOp::Sub => v - a,
        NumOp::Set | NumOp::Mul => v,
    });
    patches.iter().filter(|(op, _)| *op == NumOp::Mul).fold(summed, |v, (_, a)| v * a)
}

/// [`fold_patches`] as an expression over `base`, for a quantity that is evaluated
/// later (when its effect lands). Builds the very operations, in the very order,
/// the numeric fold performs, so the two agree bit for bit. No patches is `base`
/// itself, untouched.
#[must_use]
pub fn patched_value(base: Value, patches: &[Patch<Value>]) -> Value {
    let start =
        patches.iter().rev().find(|(op, _)| *op == NumOp::Set).map_or(base, |p| p.1.clone());
    let bin = |op, a, b: &Value| Value::Bin(op, Box::new(a), Box::new(b.clone()));
    let summed = patches.iter().fold(start, |v, (op, a)| match op {
        NumOp::Add => bin(BinOp::Add, v, a),
        NumOp::Sub => bin(BinOp::Sub, v, a),
        NumOp::Set | NumOp::Mul => v,
    });
    patches
        .iter()
        .filter(|(op, _)| *op == NumOp::Mul)
        .fold(summed, |v, (_, a)| bin(BinOp::Mul, v, a))
}

/// Every role-bearing quantity in `program`, in tree order, with its current
/// expression. A buff applied without a duration of its own carries none here —
/// see [`materialize_durations`].
#[must_use]
pub fn quantities(program: &[Impact]) -> Vec<(QuantityRole, Value)> {
    let mut out = Vec::new();
    let mut copy = program.to_vec();
    visit_quantities(&mut copy, &mut |role, value| out.push((role, value.clone())));
    out
}

/// Whether `program` carries any quantity of `role` — with buffs' own durations
/// counted, so "lasts longer" on an ability that applies a timed buff is not
/// reported as matching nothing.
#[must_use]
pub fn carries(
    program: &[Impact],
    role: QuantityRole,
    buff_duration: &dyn Fn(BuffId) -> Option<Value>,
) -> bool {
    let mut copy = program.to_vec();
    materialize_durations(&mut copy, buff_duration);
    let mut found = false;
    visit_quantities(&mut copy, &mut |r, _| found |= r == role);
    found
}

/// Make every applied buff's duration explicit: a buff applied with no override
/// lasts what its own declaration says, so that number is written in as the
/// override. A permanent buff stays permanent — there is no duration to lengthen.
///
/// Needed only before patching durations: the override is the one place a
/// duration-patch can land on, and the value written is exactly the one the buff
/// would have used, so nothing else about the program changes.
pub fn materialize_durations(
    program: &mut [Impact],
    buff_duration: &dyn Fn(BuffId) -> Option<Value>,
) {
    walk_impacts(program, &mut |impact| {
        if let Impact::ApplyModifiers { buff, duration_override: slot @ None, .. } = impact {
            *slot = buff_duration(*buff);
        }
    });
}

/// Rewrite every role-bearing quantity in `program` by the patches stacked on its
/// role. `patches` is a slot's whole patch table; entries for a mod's own params
/// are not roles and are ignored here (the cast pipeline reads those).
///
/// The program's shape never changes: only the expressions at role positions do.
/// A role with no entry is untouched, and a program carrying no quantity of a
/// patched role is left exactly as it was.
pub fn patch_quantities(
    program: &mut [Impact],
    patches: &BTreeMap<ParamId, Vec<Patch<Value>>>,
    buff_duration: &dyn Fn(BuffId) -> Option<Value>,
) {
    let by_role: BTreeMap<QuantityRole, &[Patch<Value>]> = patches
        .iter()
        .filter_map(|(param, list)| QuantityRole::of_param(*param).map(|r| (r, list.as_slice())))
        .filter(|(_, list)| !list.is_empty())
        .collect();
    if by_role.is_empty() {
        return;
    }
    if by_role.contains_key(&QuantityRole::Duration) {
        materialize_durations(program, buff_duration);
    }
    visit_quantities(program, &mut |role, value| {
        if let Some(list) = by_role.get(&role) {
            let base = core::mem::replace(value, Value::Const(0.0));
            *value = patched_value(base, list);
        }
    });
}

/// Visit every quantity of `program` with its role — **the** table of where each
/// role lives in the ISA. Descends into everything the ability itself causes: the
/// combinators' sub-programs, its bodies' payloads (and the bodies those spawn), a
/// zone's entry effects and a dash's hooks. Not into a `CastAbility`, which runs a
/// different ability with that ability's own patches.
pub fn visit_quantities(program: &mut [Impact], f: &mut dyn FnMut(QuantityRole, &mut Value)) {
    for impact in program {
        match impact {
            Impact::Retarget { shape, inner, .. } => {
                shape_quantities(shape, f);
                visit_quantities(inner, f);
            }
            Impact::If { then, els, .. } => {
                visit_quantities(then, f);
                visit_quantities(els, f);
            }
            Impact::Loop { inner, .. } | Impact::Delay { inner, .. } => visit_quantities(inner, f),
            Impact::Damage { amount, .. } => f(QuantityRole::Damage, amount),
            Impact::Heal { amount, .. } => f(QuantityRole::Healing, amount),
            // Only a grant is shielding; stripping a shield is the opposite of it.
            Impact::AdjustPool { pool: PoolRef::Shield, op: NumOp::Add, amount, .. } => {
                f(QuantityRole::Shielding, amount);
            }
            Impact::ApplyModifiers { duration_override: Some(d), .. } => {
                f(QuantityRole::Duration, d);
            }
            Impact::Knockback { force, .. } => f(QuantityRole::Knockback, force),
            Impact::Dash { motion, .. } => motion_quantities(motion, f),
            Impact::Spawn { body, .. } => body_quantities(body, f),
            Impact::AdjustPool { .. }
            | Impact::ApplyModifiers { .. }
            | Impact::RemoveModifiers { .. }
            | Impact::Teleport { .. }
            | Impact::CastAbility { .. }
            | Impact::Interrupt { .. }
            | Impact::ResolvePending { .. }
            | Impact::Emit { .. }
            | Impact::Custom { .. } => {}
        }
    }
}

fn shape_quantities(shape: &mut TargetShape, f: &mut dyn FnMut(QuantityRole, &mut Value)) {
    match shape {
        TargetShape::Circle { radius, .. } => f(QuantityRole::Radius, radius),
        TargetShape::Cone { radius, angle } => {
            f(QuantityRole::Radius, radius);
            f(QuantityRole::Spread, angle);
        }
        TargetShape::Line { length, .. } => f(QuantityRole::Range, length),
        TargetShape::SelfOnly
        | TargetShape::Chain { .. }
        | TargetShape::AllAllies
        | TargetShape::AllEnemies => {}
    }
}

fn body_quantities(body: &mut BodyDescriptor, f: &mut dyn FnMut(QuantityRole, &mut Value)) {
    match &mut body.kind {
        BodyKind::Missile { speed, range, .. } => {
            f(QuantityRole::Speed, speed);
            f(QuantityRole::Range, range);
        }
        BodyKind::Unit { duration, .. } => {
            if let Some(d) = duration {
                f(QuantityRole::Duration, d);
            }
        }
        BodyKind::Zone { radius, duration, on_enter, .. } => {
            f(QuantityRole::Radius, radius);
            f(QuantityRole::Duration, duration);
            visit_quantities(on_enter, f);
        }
        BodyKind::Pickup { duration, .. } => {
            if let Some(d) = duration {
                f(QuantityRole::Duration, d);
            }
        }
    }
    visit_quantities(&mut body.on_spawn, f);
    visit_quantities(&mut body.on_hit, f);
    visit_quantities(&mut body.on_expire, f);
}

fn motion_quantities(motion: &mut Motion, f: &mut dyn FnMut(QuantityRole, &mut Value)) {
    f(QuantityRole::Speed, &mut motion.speed);
    for leg in &mut motion.legs {
        match leg {
            Leg::Straight { dist, .. } | Leg::Arc { dist, .. } => f(QuantityRole::Range, dist),
            Leg::Back | Leg::ToTarget => {}
        }
    }
    visit_quantities(&mut motion.on_hit, f);
    visit_quantities(&mut motion.on_collision, f);
    visit_quantities(&mut motion.on_end, f);
}

/// Visit every impact in `program`, at every depth [`visit_quantities`] reaches.
fn walk_impacts(program: &mut [Impact], f: &mut dyn FnMut(&mut Impact)) {
    for impact in program {
        f(impact);
        match impact {
            Impact::Retarget { inner, .. }
            | Impact::Loop { inner, .. }
            | Impact::Delay { inner, .. } => {
                walk_impacts(inner, f);
            }
            Impact::If { then, els, .. } => {
                walk_impacts(then, f);
                walk_impacts(els, f);
            }
            Impact::Dash { motion, .. } => {
                walk_impacts(&mut motion.on_hit, f);
                walk_impacts(&mut motion.on_collision, f);
                walk_impacts(&mut motion.on_end, f);
            }
            Impact::Spawn { body, .. } => {
                if let BodyKind::Zone { on_enter, .. } = &mut body.kind {
                    walk_impacts(on_enter, f);
                }
                walk_impacts(&mut body.on_spawn, f);
                walk_impacts(&mut body.on_hit, f);
                walk_impacts(&mut body.on_expire, f);
            }
            _ => {}
        }
    }
}
