//! The `Value` expression language — the main anti-growth lever of the ISA.
//!
//! Numbers are *expressions*, not enum variants. "Deal a % of target max hp",
//! "heal scaling with caster missing hp", "effectiveness per stack up to a cap",
//! "% chance" — all are formulas over a small, generic, read-only projection of
//! state ([`Var`]), evaluated **once per resolution** against a [`ValueCtx`]
//! snapshot. The only thing that ever grows is `Var` (a generic projection);
//! the vocabulary of *operations* is fixed.
//!
//! Evaluation is **total** (never panics: guarded division, order-free clamp)
//! and **deterministic** given the same context — a hard requirement for replay.

use alloc::boxed::Box;

use serde::{Deserialize, Serialize};

use crate::ids::{BuffId, CurveId, ResourceId, Slot, StackId, StatId};

/// Whose state a [`Var`] projects. `Source` is the effect origin (e.g. the owner
/// of an in-flight missile), distinct from the immediate caster.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Who {
    Caster,
    Target,
    Source,
}

/// The fixed set of binary operators. Division is guarded (see [`Value::eval`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Min,
    Max,
}

/// A generic, read-only projection of simulation state. The only vocabulary
/// permitted to grow — and only with *projections*, never operations.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Var {
    Level,
    MaxHp(Who),
    CurHp(Who),
    MissingHp(Who),
    HpRatio(Who),
    MissingHpRatio(Who),
    Stat(StatId, Who),
    Resource(ResourceId, Who),
    StackCount(StackId, Who),
    BuffStacks(BuffId, Who),
    ChargesOf(Slot, Who),
    CooldownOf(Slot, Who),
    AllyCount,
    EnemyCount,
    DistanceToTarget,
    /// Channel completion in `[0, 1]`.
    ChannelProgress,
    /// A seeded, deterministic PRNG read in `[0, 1]` (fair rolls, "% chance").
    Rand01,
    /// How much a stack counter has gained **this tick** (stormlight/server#137).
    ///
    /// [`Self::StackCount`] answers *how many times, ever*; this answers *how many
    /// just now*, which is a different question and the only one that can express a
    /// burst. A rider adding one per hit turns "four enemy heroes at once" into a
    /// gain of four on one tick — a fact about an event, stated with an ordinary
    /// read rather than with a new condition kind tied to one hook.
    ///
    /// Counters are folded **once** per tick, so this is zero before that fold and
    /// the tick's whole gain after it, and it is cleared at the end of the tick. A
    /// decrease reads as a negative number: a mod that spends a counter is doing
    /// something, and rounding that to zero would hide it.
    ///
    /// **Appended, never inserted**: the variant order is the wire tag.
    StackGain(StackId, Who),
}

/// A numeric expression. Composed of a fixed operator vocabulary over [`Var`]
/// projections; mods express unbounded scalings without new ISA primitives.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Value {
    Const(f32),
    Read(Var),
    Bin(BinOp, Box<Value>, Box<Value>),
    Clamp {
        v: Box<Value>,
        lo: Box<Value>,
        hi: Box<Value>,
    },
    /// Table lookup (e.g. a level-scaling curve) applied to a sub-expression.
    Curve(CurveId, Box<Value>),
    /// The ambient `value_scale` multiplier threaded through nested casts.
    ScaleCtx,
}

/// The single place the evaluator reads world state. Concrete impls (the engine)
/// snapshot ECS queries here; tests supply a mock. Keeping every read behind this
/// trait is what lets `Value` stay a pure, side-effect-free expression.
pub trait ValueCtx {
    fn level(&self) -> f32;
    fn max_hp(&self, who: Who) -> f32;
    fn cur_hp(&self, who: Who) -> f32;
    fn stat(&self, stat: StatId, who: Who) -> f32;
    fn resource(&self, resource: ResourceId, who: Who) -> f32;
    fn stack_count(&self, stack: StackId, who: Who) -> f32;
    /// What `stack` gained this tick. Zero before the tick's fold — see
    /// [`Var::StackGain`].
    fn stack_gain(&self, stack: StackId, who: Who) -> f32;
    fn buff_stacks(&self, buff: BuffId, who: Who) -> f32;
    fn charges_of(&self, slot: Slot, who: Who) -> f32;
    fn cooldown_of(&self, slot: Slot, who: Who) -> f32;
    fn ally_count(&self) -> f32;
    fn enemy_count(&self) -> f32;
    fn distance_to_target(&self) -> f32;
    fn channel_progress(&self) -> f32;
    fn rand01(&self) -> f32;
    /// The ambient `value_scale` for `ScaleCtx`.
    fn scale(&self) -> f32;
    /// Evaluate curve `curve` at `x`.
    fn curve(&self, curve: CurveId, x: f32) -> f32;
}

/// Division that is total: `x / 0` is defined as `0` rather than `inf`/`NaN`,
/// keeping downstream aggregation finite and deterministic.
fn safe_div(a: f32, b: f32) -> f32 {
    if b == 0.0 { 0.0 } else { a / b }
}

impl Value {
    /// Whether this expression is already a bare constant (e.g. after freezing).
    #[must_use]
    pub fn is_const(&self) -> bool {
        matches!(self, Value::Const(_))
    }

    /// Evaluate against `ctx`. Total (never panics) and deterministic given the
    /// same `ctx`.
    pub fn eval<C: ValueCtx + ?Sized>(&self, ctx: &C) -> f32 {
        match self {
            Value::Const(c) => *c,
            Value::Read(var) => eval_var(var, ctx),
            Value::Bin(op, a, b) => {
                let (a, b) = (a.eval(ctx), b.eval(ctx));
                match op {
                    BinOp::Add => a + b,
                    BinOp::Sub => a - b,
                    BinOp::Mul => a * b,
                    BinOp::Div => safe_div(a, b),
                    BinOp::Min => a.min(b),
                    BinOp::Max => a.max(b),
                }
            }
            Value::Clamp { v, lo, hi } => {
                let (v, lo, hi) = (v.eval(ctx), lo.eval(ctx), hi.eval(ctx));
                // Order-free: total even when lo > hi (std `clamp` would panic).
                v.max(lo).min(hi)
            }
            Value::Curve(curve, x) => ctx.curve(*curve, x.eval(ctx)),
            Value::ScaleCtx => ctx.scale(),
        }
    }
}

fn eval_var<C: ValueCtx + ?Sized>(var: &Var, ctx: &C) -> f32 {
    match var {
        Var::Level => ctx.level(),
        Var::MaxHp(w) => ctx.max_hp(*w),
        Var::CurHp(w) => ctx.cur_hp(*w),
        // Derived hp projections keep the ctx surface minimal and always finite.
        Var::MissingHp(w) => (ctx.max_hp(*w) - ctx.cur_hp(*w)).max(0.0),
        Var::HpRatio(w) => safe_div(ctx.cur_hp(*w), ctx.max_hp(*w)),
        Var::MissingHpRatio(w) => {
            safe_div((ctx.max_hp(*w) - ctx.cur_hp(*w)).max(0.0), ctx.max_hp(*w))
        }
        Var::Stat(s, w) => ctx.stat(*s, *w),
        Var::Resource(r, w) => ctx.resource(*r, *w),
        Var::StackCount(s, w) => ctx.stack_count(*s, *w),
        Var::StackGain(s, w) => ctx.stack_gain(*s, *w),
        Var::BuffStacks(b, w) => ctx.buff_stacks(*b, *w),
        Var::ChargesOf(sl, w) => ctx.charges_of(*sl, *w),
        Var::CooldownOf(sl, w) => ctx.cooldown_of(*sl, *w),
        Var::AllyCount => ctx.ally_count(),
        Var::EnemyCount => ctx.enemy_count(),
        Var::DistanceToTarget => ctx.distance_to_target(),
        Var::ChannelProgress => ctx.channel_progress(),
        Var::Rand01 => ctx.rand01(),
    }
}
