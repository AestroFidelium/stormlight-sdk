//! Predicates over the world — the gate layer of the ISA. Conditions reuse
//! [`Value`] and *the same* evaluation context, so anything a number can read,
//! a predicate can test. Because the ctx is assembled once and passed in, the
//! prototype's "always-false because the query can't be threaded" bug cannot
//! recur.
//!
//! "is a hero" = `HasTag(hero, Target)`; "channel ≥ 50%" =
//! `Cmp(Ge, Read(ChannelProgress), Const(0.5))`. New gates are new *data*, never
//! new primitives.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::ids::{BuffId, TagId, TalentId};
use crate::math::{Origin, Value, ValueCtx, Who};

/// Total order comparison operators for [`Condition::Cmp`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum CmpOp {
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}

/// A boolean predicate over the resolution context.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Condition {
    Always,
    Cmp(CmpOp, Value, Value),
    HasTag(TagId, Who),
    HasBuff(BuffId, Who),
    HasTalent(TalentId),
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Not(Box<Condition>),
    /// Whether `who` carries the buff **and** [`Origin`] is who applied it
    /// (stormlight/server#150).
    ///
    /// The sourced twin of [`Self::HasBuff`], with the same relationship to it
    /// that [`Var::BuffStacksFrom`] has to [`Var::BuffStacks`]:
    /// [`Origin::Anyone`] is routed to the unsourced read, so the global question
    /// has one meaning however it is spelled.
    ///
    /// A tag convention is the workaround this replaces, and it has the hole one
    /// level down — another unit's identically-tagged debuff satisfies it too.
    /// Tags stay global on purpose: a tag records no origin, a buff instance does.
    ///
    /// **Appended, never inserted**: the variant order is the wire tag.
    ///
    /// [`Var::BuffStacks`]: crate::math::Var::BuffStacks
    /// [`Var::BuffStacksFrom`]: crate::math::Var::BuffStacksFrom
    HasBuffFrom(BuffId, Who, Origin),
}

/// The context conditions read. Extends [`ValueCtx`] with the membership queries
/// numbers don't need — one ctx object implements both at runtime.
pub trait ConditionCtx: ValueCtx {
    fn has_tag(&self, tag: TagId, who: Who) -> bool;
    fn has_buff(&self, buff: BuffId, who: Who) -> bool;
    /// Whether `who` carries `buff` applied by `from`. Only ever asked with a
    /// concrete origin — [`Origin::Anyone`] is answered by [`Self::has_buff`].
    fn has_buff_from(&self, buff: BuffId, who: Who, from: Who) -> bool;
    fn has_talent(&self, talent: TalentId) -> bool;
}

impl Condition {
    /// Evaluate against `ctx`. Total and deterministic given the same `ctx`.
    /// `And([])` is vacuously `true`; `Or([])` is vacuously `false`.
    pub fn eval<C: ConditionCtx + ?Sized>(&self, ctx: &C) -> bool {
        match self {
            Condition::Always => true,
            Condition::Cmp(op, a, b) => {
                let (a, b) = (a.eval(ctx), b.eval(ctx));
                match op {
                    CmpOp::Lt => a < b,
                    CmpOp::Le => a <= b,
                    CmpOp::Eq => a == b,
                    CmpOp::Ge => a >= b,
                    CmpOp::Gt => a > b,
                }
            }
            Condition::HasTag(tag, who) => ctx.has_tag(*tag, *who),
            Condition::HasBuff(buff, who) => ctx.has_buff(*buff, *who),
            Condition::HasTalent(talent) => ctx.has_talent(*talent),
            Condition::And(cs) => cs.iter().all(|c| c.eval(ctx)),
            Condition::Or(cs) => cs.iter().any(|c| c.eval(ctx)),
            Condition::Not(c) => !c.eval(ctx),
            // `Anyone` is the unsourced question (see the variant's docs).
            Condition::HasBuffFrom(buff, who, Origin::Anyone) => ctx.has_buff(*buff, *who),
            Condition::HasBuffFrom(buff, who, Origin::By(from)) => {
                ctx.has_buff_from(*buff, *who, *from)
            }
        }
    }
}
