//! "Is this effect *mine*" (stormlight/server#150, item 2).
//!
//! [`Condition::HasBuff`] and [`Var::BuffStacks`] ask a deliberately global
//! question: does this unit carry that effect, no matter who put it there. That is
//! often exactly right — a cleanse does not care whose slow it is — and sometimes
//! precisely wrong: a talent that pays out on "the target carrying *my* mark" fires
//! on an ally's identical mark, and the only workaround is a tag convention that
//! has the same hole one level down.
//!
//! The origin half is an [`Origin`], and the thing it must *not* become is a
//! general comparison of arbitrary entities. It cannot: the only actors it can name
//! are the ones the resolution already names ([`Who`]), so the widest thing a mod
//! can write is still a question about the caster, the target, or the effect's
//! source.
//!
//! The two invariants that keep the vocabulary honest are both laws of the
//! *evaluator*, not promises each context impl has to keep:
//!
//!   - [`Origin::Anyone`] is exactly the old question — it reads the same context
//!     method the unsourced form does, so a global gate and a global read can never
//!     drift apart from their sourced spelling;
//!   - [`Origin::By`] carries its origin through unchanged, and it is a *separate*
//!     axis from the `Who` being asked about: "the mark I put on them" and "the mark
//!     they put on me" are different questions, and swapping the two must change
//!     the answer.
//!
//! The mock below answers the two context methods with deliberately *different*
//! numbers, so an evaluator that routed one form into the other's method fails
//! rather than silently agreeing.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::conditions::{Condition, ConditionCtx};
use stormlight_mod_abi::ids::{BuffId, CurveId, ResourceId, Slot, StackId, StatId, TalentId};
use stormlight_mod_abi::math::{Origin, Value, ValueCtx, Var, Who};

extern crate alloc;

/// A context whose two buff reads are told apart by construction: the *global*
/// answer is a round hundred per buff, the *sourced* one a small number keyed on
/// both the holder and the origin. Nothing here is a plausible real distribution —
/// it is a fingerprint, so a misrouted read is a failed assertion.
struct MockCtx;

impl MockCtx {
    const fn global(buff: BuffId) -> f32 {
        100.0 + buff.0 as f32
    }
    const fn sourced(buff: BuffId, who: Who, from: Who) -> f32 {
        // Asymmetric in (who, from): swapping the holder and the origin changes
        // the answer, which is what makes "my mark on them" a different question
        // from "their mark on me".
        buff.0 as f32 + 3.0 * (who as u8 as f32) + 7.0 * (from as u8 as f32)
    }
}

impl ValueCtx for MockCtx {
    fn level(&self) -> f32 {
        1.0
    }
    fn max_hp(&self, _who: Who) -> f32 {
        100.0
    }
    fn cur_hp(&self, _who: Who) -> f32 {
        50.0
    }
    fn stat(&self, _stat: StatId, _who: Who) -> f32 {
        0.0
    }
    fn resource(&self, _resource: ResourceId, _who: Who) -> f32 {
        0.0
    }
    fn stack_count(&self, _stack: StackId, _who: Who) -> f32 {
        0.0
    }
    fn stack_gain(&self, _stack: StackId, _who: Who) -> f32 {
        0.0
    }
    fn buff_stacks(&self, buff: BuffId, _who: Who) -> f32 {
        Self::global(buff)
    }
    fn buff_stacks_from(&self, buff: BuffId, who: Who, from: Who) -> f32 {
        Self::sourced(buff, who, from)
    }
    fn charges_of(&self, _slot: Slot, _who: Who) -> f32 {
        0.0
    }
    fn cooldown_of(&self, _slot: Slot, _who: Who) -> f32 {
        0.0
    }
    fn source_slot(&self) -> Option<Slot> {
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
        0.5
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
    fn curve(&self, _curve: CurveId, x: f32) -> f32 {
        x
    }
}

impl ConditionCtx for MockCtx {
    fn has_tag(&self, _tag: stormlight_mod_abi::ids::TagId, _who: Who) -> bool {
        false
    }
    fn has_buff(&self, buff: BuffId, _who: Who) -> bool {
        // Every even buff id is present globally — the global answer, unsourced.
        buff.0.is_multiple_of(2)
    }
    fn has_buff_from(&self, buff: BuffId, who: Who, from: Who) -> bool {
        // Deliberately unrelated to `has_buff`: only a buff whose id matches the
        // origin/holder pair counts as "put there by that one".
        u32::from(buff.0) == u32::from(who as u8) + u32::from(from as u8)
    }
    fn has_talent(&self, _talent: TalentId) -> bool {
        false
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    buff: u16,
    who: u8,
    from: u8,
}

fn who(seed: u8) -> Who {
    match seed % 3 {
        0 => Who::Caster,
        1 => Who::Target,
        _ => Who::Source,
    }
}

#[test]
fn anyone_is_exactly_the_unsourced_question() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (buff, holder) = (BuffId(s.buff), who(s.who));
        let ctx = MockCtx;

        // The read: asking "from anyone" is the same instruction as not asking.
        let global = Value::Read(Var::BuffStacks(buff, holder)).eval(&ctx);
        let anyone = Value::Read(Var::BuffStacksFrom(buff, holder, Origin::Anyone)).eval(&ctx);
        assert_eq!(anyone.to_bits(), global.to_bits(), "`Anyone` read a sourced total");

        // The gate: likewise, and the mock's two answers disagree for most ids, so
        // an evaluator routing this into `has_buff_from` is caught.
        let global = Condition::HasBuff(buff, holder).eval(&ctx);
        let anyone = Condition::HasBuffFrom(buff, holder, Origin::Anyone).eval(&ctx);
        assert_eq!(anyone, global, "`Anyone` gated on a sourced answer");
    });
}

#[test]
fn by_carries_its_origin_through_unchanged() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (buff, holder, origin) = (BuffId(s.buff), who(s.who), who(s.from));
        let ctx = MockCtx;

        let read = Value::Read(Var::BuffStacksFrom(buff, holder, Origin::By(origin))).eval(&ctx);
        assert_eq!(
            read.to_bits(),
            MockCtx::sourced(buff, holder, origin).to_bits(),
            "a sourced read lost or confused its origin",
        );

        let gate = Condition::HasBuffFrom(buff, holder, Origin::By(origin)).eval(&ctx);
        assert_eq!(gate, ctx.has_buff_from(buff, holder, origin), "a sourced gate misrouted");
    });
}

#[test]
fn holder_and_origin_are_separate_axes() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (buff, a, b) = (BuffId(s.buff), who(s.who), who(s.from));
        if a == b {
            return;
        }
        let ctx = MockCtx;

        // "The effect I put on them" and "the effect they put on me" are two
        // questions; a vocabulary that collapsed them would return one number.
        let mine_on_them = Value::Read(Var::BuffStacksFrom(buff, a, Origin::By(b))).eval(&ctx);
        let theirs_on_me = Value::Read(Var::BuffStacksFrom(buff, b, Origin::By(a))).eval(&ctx);
        assert_ne!(
            mine_on_them.to_bits(),
            theirs_on_me.to_bits(),
            "the holder and the origin collapsed into one axis",
        );
    });
}

#[test]
fn sourced_reads_are_total_and_deterministic() {
    check!().with_type::<Scenario>().for_each(|s| {
        let ctx = MockCtx;
        for origin in [Origin::Anyone, Origin::By(who(s.from))] {
            let v = Value::Read(Var::BuffStacksFrom(BuffId(s.buff), who(s.who), origin));
            let (first, second) = (v.eval(&ctx), v.eval(&ctx));
            assert_eq!(first.to_bits(), second.to_bits(), "a sourced read is non-deterministic");
            assert!(first.is_finite(), "a sourced read produced a non-finite number");
        }
    });
}
