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
use crate::math::{Value, ValueCtx, Who};

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
}

/// The context conditions read. Extends [`ValueCtx`] with the membership queries
/// numbers don't need — one ctx object implements both at runtime.
pub trait ConditionCtx: ValueCtx {
    fn has_tag(&self, tag: TagId, who: Who) -> bool;
    fn has_buff(&self, buff: BuffId, who: Who) -> bool;
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
        }
    }
}
