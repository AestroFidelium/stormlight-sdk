//! Naming the slot an effect is running from (stormlight/server#187).
//!
//! `SlotRef` exists because a talent stated its slot twice in two coordinate
//! systems: its patches were relative by construction, its riders' payloads
//! absolute. The type closes that, and the whole of it rests on one resolution
//! rule plus the promise that the rule lives in exactly one place.
//!
//! The invariants are directional and each one is a way the feature could be
//! quietly wrong rather than loudly broken:
//!
//!   - **an absolute reference is context-free** — `At(s)` reads slot `s` whatever
//!     the resolution is running from. This is the half the issue insists stays
//!     first-class: "hitting with this shortens *that*" must not start following
//!     the caster around;
//!   - **a relative reference follows the context** — `This` reads whatever
//!     `source_slot` says, so the same rider copied onto two slots asks two
//!     different questions;
//!   - **nothing to resolve against reads zero, not slot 0.** The mock below makes
//!     every slot's answer distinctive *and* makes slot 0's answer loudly nonzero,
//!     so an evaluator that quietly fell back to `Slot(0)` fails here rather than
//!     paying out somebody else's ability forty minutes into a match;
//!   - **the rule is the evaluator's, not the context's.** The mock's
//!     `cooldown_of`/`charges_of` take a concrete `Slot` and can therefore not see
//!     a `SlotRef` at all — if resolution ever moved into the impls, this file
//!     stops compiling.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{BuffId, CurveId, ResourceId, Slot, StackId, StatId};
use stormlight_mod_abi::math::{Value, ValueCtx, Var, Who};
use stormlight_mod_abi::slot_ref::SlotRef;

/// A context whose slot reads are a fingerprint: every slot answers with its own
/// number, **including slot 0**, which answers with a number nothing else does.
struct MockCtx {
    running_from: Option<Slot>,
}

impl MockCtx {
    /// Distinct per (slot, who) and never zero — so "read nothing" and "read slot
    /// anything" can never be confused for one another.
    const fn cooldown(slot: Slot, who: Who) -> f32 {
        1_000.0 + slot.0 as f32 * 10.0 + who as u8 as f32
    }
    const fn charges(slot: Slot, who: Who) -> f32 {
        -1_000.0 - slot.0 as f32 * 10.0 - who as u8 as f32
    }
}

impl ValueCtx for MockCtx {
    fn level(&self) -> f32 {
        1.0
    }
    fn max_hp(&self, _: Who) -> f32 {
        100.0
    }
    fn cur_hp(&self, _: Who) -> f32 {
        50.0
    }
    fn stat(&self, _: StatId, _: Who) -> f32 {
        0.0
    }
    fn resource(&self, _: ResourceId, _: Who) -> f32 {
        0.0
    }
    fn stack_count(&self, _: StackId, _: Who) -> f32 {
        0.0
    }
    fn stack_gain(&self, _: StackId, _: Who) -> f32 {
        0.0
    }
    fn buff_stacks(&self, _: BuffId, _: Who) -> f32 {
        0.0
    }
    fn buff_stacks_from(&self, _: BuffId, _: Who, _: Who) -> f32 {
        0.0
    }
    fn charges_of(&self, slot: Slot, who: Who) -> f32 {
        Self::charges(slot, who)
    }
    fn cooldown_of(&self, slot: Slot, who: Who) -> f32 {
        Self::cooldown(slot, who)
    }
    fn source_slot(&self) -> Option<Slot> {
        self.running_from
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
    fn curve(&self, _: CurveId, x: f32) -> f32 {
        x
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    written: u8,
    running_from: Option<u8>,
    who: u8,
}

impl Scenario {
    fn who(&self) -> Who {
        match self.who % 3 {
            0 => Who::Caster,
            1 => Who::Target,
            _ => Who::Source,
        }
    }
    fn ctx(&self) -> MockCtx {
        MockCtx { running_from: self.running_from.map(Slot) }
    }
}

#[test]
fn an_absolute_reference_ignores_the_running_slot() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (slot, who) = (Slot(s.written), s.who());
        let here = Value::Read(Var::CooldownOf(SlotRef::At(slot), who)).eval(&s.ctx());
        // The same expression evaluated somewhere that is running from no slot at
        // all: an absolute reference has to answer identically, or a talent aimed
        // at one button would start drifting onto whichever one triggered it.
        let nowhere = Value::Read(Var::CooldownOf(SlotRef::At(slot), who))
            .eval(&MockCtx { running_from: None });
        assert_eq!(here, nowhere, "an absolute slot reference moved with the context");
        assert_eq!(here, MockCtx::cooldown(slot, who), "absolute reference read the wrong slot");
    });
}

#[test]
fn a_relative_reference_follows_the_running_slot() {
    check!().with_type::<Scenario>().for_each(|s| {
        let who = s.who();
        let read = Value::Read(Var::ChargesOf(SlotRef::This, who)).eval(&s.ctx());
        match s.running_from.map(Slot) {
            // Running from a slot: `This` is that slot's own answer — the same
            // number the absolute spelling of it gives, so the two cannot drift.
            Some(slot) => {
                assert_eq!(read, MockCtx::charges(slot, who), "`This` read the wrong slot");
                let spelled_out =
                    Value::Read(Var::ChargesOf(SlotRef::At(slot), who)).eval(&s.ctx());
                assert_eq!(read, spelled_out, "`This` and its absolute spelling disagree");
            }
            // Running from nothing: a documented zero. Emphatically *not* slot 0's
            // answer, which the mock makes impossible to mistake for one.
            None => {
                assert_eq!(read, 0.0, "an unresolvable slot reference did not read zero");
                assert_ne!(
                    read,
                    MockCtx::charges(Slot(0), who),
                    "an unresolvable slot reference fell back to slot 0",
                );
            }
        }
    });
}

#[test]
fn resolution_is_the_one_rule() {
    check!().with_type::<Scenario>().for_each(|s| {
        let running_from = s.running_from.map(Slot);
        // `At` is total and context-free; `This` is exactly the context.
        assert_eq!(SlotRef::At(Slot(s.written)).resolve(running_from), Some(Slot(s.written)));
        assert_eq!(SlotRef::This.resolve(running_from), running_from);
        // And the two forms are told apart by the same predicate binding uses, so
        // "has this been bound yet?" and "does this need a slot?" are one question.
        assert!(!SlotRef::At(Slot(s.written)).is_relative());
        assert!(SlotRef::This.is_relative());
        assert_eq!(
            SlotRef::This.resolve(running_from).is_some(),
            running_from.is_some(),
            "a relative reference resolved without a slot to resolve against",
        );
    });
}
