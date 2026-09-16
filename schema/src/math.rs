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

/// Whose *doing* an effect was — the origin half of a membership question
/// (stormlight/server#150).
///
/// [`Var::BuffStacks`] and [`HasBuff`] ask a deliberately global question: does
/// this unit carry that effect, whoever put it there. Often right — a cleanse does
/// not care whose slow it is — and sometimes precisely wrong: a talent paying out
/// on "the target carrying *my* mark" should not fire on an ally's identical mark.
///
/// Closed by construction, which is the point: the only actors an origin can name
/// are the ones the resolution already names ([`Who`]), so a predicate gains
/// "mine" without the ISA gaining a general comparison of arbitrary entities.
///
/// [`HasBuff`]: crate::conditions::Condition::HasBuff
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Origin {
    /// Anyone at all — the same question the unsourced form asks. The evaluator
    /// routes it to the unsourced context read, so the two spellings cannot drift.
    Anyone,
    /// Applied by this resolution's caster, target or source.
    By(Who),
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
    /// Stacks of a buff on `who`, counting only the ones [`Origin`] applied
    /// (stormlight/server#150).
    ///
    /// The sourced twin of [`Self::BuffStacks`], and a strict superset of it:
    /// [`Origin::Anyone`] evaluates to the very same context read, so "how many
    /// marks are on them" and "how many of my marks are on them" are one
    /// instruction with one axis of difference rather than two vocabularies.
    ///
    /// The holder and the origin are **separate** axes. `(Target, By(Caster))` is
    /// the mark I put on them; `(Caster, By(Target))` is the one they put on me.
    ///
    /// **Appended, never inserted**: the variant order is the wire tag.
    BuffStacksFrom(BuffId, Who, Origin),
    /// The magnitude of the event a reaction is running from — the number that
    /// caused it (stormlight/server#150).
    ///
    /// The damage just dealt, the healing just done, the shield just lost: "deal a
    /// further half of that", "restore resource for a share of it" are the
    /// commonest shape a reaction has, and without this they are inexpressible —
    /// the event says *that* it happened and never *how much*.
    ///
    /// Zero outside a reaction, which is the honest reading: an ability's own
    /// payload is not running from an event, so there is no magnitude to quote.
    ///
    /// **Appended, never inserted**: the variant order is the wire tag.
    EventMagnitude,
    /// Which pass of the innermost enclosing [`Loop`] is running, counting from
    /// zero (stormlight/server#150).
    ///
    /// "Each following wave hits harder than the last" is a formula over this —
    /// and, crucially, a talent can *add* the ramp to an ability that never had
    /// one, which an unrolled table of constants can never be patched into.
    ///
    /// Zero outside any loop, and zero on a loop's first pass: the first wave is
    /// the unramped one.
    ///
    /// **Appended, never inserted**: the variant order is the wire tag.
    ///
    /// [`Loop`]: crate::impacts::Impact::Loop
    LoopIndex,
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
    /// Stacks of `buff` on `who` that `from` applied. Only ever asked with a
    /// concrete origin — [`Origin::Anyone`] is answered by [`Self::buff_stacks`]
    /// in the evaluator, so an impl cannot make the two disagree.
    fn buff_stacks_from(&self, buff: BuffId, who: Who, from: Who) -> f32;
    fn charges_of(&self, slot: Slot, who: Who) -> f32;
    fn cooldown_of(&self, slot: Slot, who: Who) -> f32;
    fn ally_count(&self) -> f32;
    fn enemy_count(&self) -> f32;
    fn distance_to_target(&self) -> f32;
    fn channel_progress(&self) -> f32;
    fn rand01(&self) -> f32;
    /// The magnitude of the event this resolution is reacting to — see
    /// [`Var::EventMagnitude`]. Zero when there is no event behind it.
    fn event_magnitude(&self) -> f32;
    /// The zero-based pass of the innermost enclosing loop — see
    /// [`Var::LoopIndex`]. Zero outside any loop.
    fn loop_index(&self) -> f32;
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
        // `Anyone` is the unsourced question, answered by the unsourced read: the
        // two spellings are one instruction, so no context impl can make the
        // global form mean something different from `BuffStacks`.
        Var::BuffStacksFrom(b, w, Origin::Anyone) => ctx.buff_stacks(*b, *w),
        Var::BuffStacksFrom(b, w, Origin::By(from)) => ctx.buff_stacks_from(*b, *w, *from),
        Var::ChargesOf(sl, w) => ctx.charges_of(*sl, *w),
        Var::CooldownOf(sl, w) => ctx.cooldown_of(*sl, *w),
        Var::AllyCount => ctx.ally_count(),
        Var::EnemyCount => ctx.enemy_count(),
        Var::DistanceToTarget => ctx.distance_to_target(),
        Var::ChannelProgress => ctx.channel_progress(),
        Var::Rand01 => ctx.rand01(),
        Var::EventMagnitude => ctx.event_magnitude(),
        Var::LoopIndex => ctx.loop_index(),
    }
}
