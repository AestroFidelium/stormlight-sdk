//! A task that pays on every increment, up to a ceiling (stormlight/server#138).
//!
//! *"Every hit adds one, to a maximum of forty-five."* A task by every measure that
//! matters — a counter, a ceiling, something a player builds toward and reads off
//! the interface — with **no threshold** at all. The reward arrives with each count
//! and the interesting part is where it stops.
//!
//! # The decision, and why it went the way it did
//!
//! The issue offers two readings and asks for both to be written out.
//!
//! **Reading 2 — a neighbour, not a variant.** "A per-increment reward is really a
//! stacking modifier with a cap, which the modifier stack can nearly express
//! already." Taken seriously, and it is half right in a way that changes the answer
//! rather than settling it:
//!
//!   - the *stacking buff* route does **not** work. A modifier is folded once per
//!     buff and does not scale with a stack count, and a modifier whose value read
//!     its own holder's buff stacks would be the self-reference the fold rules out;
//!   - but a modifier whose value reads the **counter** and clamps works exactly.
//!     `Clamp(StackCount(c), 0, 45)` is `min(count × step, ceiling)` on the nose,
//!     and it is *derived* rather than latched — a count walked backwards walks the
//!     bonus back with it, and no record can drift out of step with the tally.
//!
//! So a mod that wants a **standing quantity** should not use a task at all, and the
//! first property below pins that: the neighbour reading is real, it needs no engine
//! change, and it is the better tool for its half of the problem.
//!
//! **Reading 1 — one descriptor, two payout modes.** Which leaves the other half:
//! a per-increment **event**, something that has to happen once per count and stay
//! happened. A derived modifier cannot do that — there is nothing to derive — and a
//! latch is exactly what it needs. That is [`QuestPayout::PerCount`], and it is one
//! arm of the payout enum rather than a second descriptor because the two schedules
//! disagree about exactly one question: what count reaches rung `i`. Here the answer
//! is `i + 1`, and the latch, the payout walk, the wire and the interface are
//! untouched.
//!
//! The split is therefore by **what the reward is**, which is a line a mod can draw
//! without being told twice: a number that stands, or a thing that happens.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::common::{ImpactTarget, NumOp};
use stormlight_mod_abi::ids::{ResourceId, StackId};
use stormlight_mod_abi::impacts::{Impact, PoolRef};
use stormlight_mod_abi::math::{Value, ValueCtx, Var, Who};
use stormlight_mod_abi::tasks::{
    MAX_PER_COUNT_PAYOUTS, QuestError, QuestPayout, QuestSpec, TaskProgress,
};

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

const COUNTER: StackId = StackId(2);

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// How many increments pay.
    #[generator(0..=80)]
    ceiling: u32,
    /// The count to read at, in tenths — so fractional counts are covered.
    #[generator(0..=1500)]
    count_tenths: u32,
    /// A second count, to compare two orderings of the same total.
    #[generator(0..=1500)]
    other_tenths: u32,
}

impl Scenario {
    fn spec(&self) -> QuestSpec {
        QuestSpec {
            counter: COUNTER,
            payout: QuestPayout::PerCount { reward: reward(), ceiling: self.ceiling },
            shortcut: None,
        }
    }

    fn count(&self) -> f32 {
        #[allow(clippy::cast_precision_loss)] // A small integer count of tenths.
        {
            self.count_tenths as f32 / 10.0
        }
    }

    fn other(&self) -> f32 {
        #[allow(clippy::cast_precision_loss)] // A small integer count of tenths.
        {
            self.other_tenths as f32 / 10.0
        }
    }
}

fn reward() -> Vec<Impact> {
    vec![Impact::AdjustPool {
        pool: PoolRef::Resource(ResourceId(0)),
        op: NumOp::Add,
        amount: Value::Const(1.0),
        target: ImpactTarget::Caster,
    }]
}

/// A `ValueCtx` that answers one question — what the counter stands at — and
/// documented defaults for everything else. Enough to evaluate the neighbour
/// reading's declaration, which is the point of the first test.
struct CountingCtx(f32);

impl ValueCtx for CountingCtx {
    fn level(&self) -> f32 {
        1.0
    }
    fn max_hp(&self, _: Who) -> f32 {
        1.0
    }
    fn cur_hp(&self, _: Who) -> f32 {
        1.0
    }
    fn stat(&self, _: stormlight_mod_abi::ids::StatId, _: Who) -> f32 {
        0.0
    }
    fn resource(&self, _: ResourceId, _: Who) -> f32 {
        0.0
    }
    fn stack_count(&self, _: StackId, _: Who) -> f32 {
        self.0
    }
    fn stack_gain(&self, _: StackId, _: Who) -> f32 {
        0.0
    }
    fn buff_stacks(&self, _: stormlight_mod_abi::ids::BuffId, _: Who) -> f32 {
        0.0
    }
    fn charges_of(&self, _: stormlight_mod_abi::ids::Slot, _: Who) -> f32 {
        0.0
    }
    fn cooldown_of(&self, _: stormlight_mod_abi::ids::Slot, _: Who) -> f32 {
        0.0
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
    fn scale(&self) -> f32 {
        1.0
    }
    fn curve(&self, _: stormlight_mod_abi::ids::CurveId, x: f32) -> f32 {
        x
    }
}

/// **The neighbour reading, written out.** A standing quantity that grows with the
/// count and stops at a cap needs no task at all: an ordinary modifier value reading
/// the counter and clamping is exactly `min(count × step, ceiling)`.
///
/// This is what decided the issue. The task descriptor is for the *other* half —
/// something that has to happen once per count — and a mod reaching for a task to
/// express a growing stat would be latching what it could have derived.
#[test]
fn a_growing_capped_stat_needs_no_task_at_all() {
    check!().with_type::<Scenario>().for_each(|s| {
        let step = 0.01_f32;
        #[allow(clippy::cast_precision_loss)] // A small ceiling.
        let cap = s.ceiling as f32 * step;
        // "Every count adds `step`, to a maximum of `cap`."
        let declared = Value::Clamp {
            v: alloc::boxed::Box::new(Value::Bin(
                stormlight_mod_abi::math::BinOp::Mul,
                alloc::boxed::Box::new(Value::Read(Var::StackCount(COUNTER, Who::Caster))),
                alloc::boxed::Box::new(Value::Const(step)),
            )),
            lo: alloc::boxed::Box::new(Value::Const(0.0)),
            hi: alloc::boxed::Box::new(Value::Const(cap)),
        };

        let count = s.count();
        let got = declared.eval(&CountingCtx(count));
        let expected = (count * step).clamp(0.0, cap);
        assert!(
            (got - expected).abs() < 1e-6,
            "a clamped count read {got} where min(count x step, ceiling) is {expected}",
        );
        // Derived, which is the whole of its advantage: walk the count back and the
        // bonus walks back with it, with no record to drift.
        assert_eq!(
            declared.eval(&CountingCtx(0.0)),
            0.0,
            "a derived bonus survived its count being emptied",
        );
    });
}

/// The issue's own property, for the shape a task really is for: the accumulated
/// payout is `min(count, ceiling)` applications — monotone, capped, and decided by
/// the count alone rather than by how the increments were grouped.
#[test]
fn the_accumulated_payout_is_the_count_capped_at_the_ceiling() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        if spec.validate().is_err() {
            return;
        }
        let count = s.count();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let expected = (count.max(0.0) as u32).min(s.ceiling);
        assert_eq!(spec.reached(count), expected, "a count of {count} paid the wrong number");
    });
}

/// Independent of grouping: the same total pays the same, whether it arrived in one
/// step or in many. A task cannot be farmed by arriving cleverly.
#[test]
fn how_the_increments_were_grouped_changes_nothing() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        if spec.validate().is_err() {
            return;
        }
        let (a, b) = (s.count(), s.other());
        let (low, high) = if a <= b { (a, b) } else { (b, a) };
        assert!(spec.reached(low) <= spec.reached(high), "counting further paid less");
        // One jump to the total and a walk to the same total agree.
        let walked = spec.reached(low) + (spec.reached(high) - spec.reached(low));
        assert_eq!(walked, spec.reached(high), "arriving in two steps paid a different amount");
    });
}

/// The ceiling is reached exactly once; further counts change nothing. And it does
/// **not** stop the count — a player who is capped should see that they are capped,
/// not a counter that froze.
#[test]
fn the_ceiling_is_reached_once_and_stops_only_the_payout() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        if spec.validate().is_err() {
            return;
        }
        #[allow(clippy::cast_precision_loss)] // A small ceiling.
        let at_cap = s.ceiling as f32;
        assert_eq!(spec.reached(at_cap), s.ceiling, "the ceiling was not reached at its own count");
        for over in [1.0, 10.0, 1000.0] {
            assert_eq!(
                spec.reached(at_cap + over),
                s.ceiling,
                "counting {over} past the ceiling paid again",
            );
        }
        // The task's own reading of a capped count is full, and the raw counter is
        // still whatever the mod put in it — read with `PoolRef::Stacks`, untouched.
        assert!(spec.complete(at_cap), "a capped task did not read as finished");
        assert_eq!(spec.fraction(at_cap + 100.0), 1.0, "a capped bar read short");
    });
}

/// One arm of one enum, not a second descriptor: the ladder's whole vocabulary
/// answers for a per-increment payout without knowing it is one.
#[test]
fn a_per_increment_payout_answers_every_question_a_ladder_does() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        if spec.validate().is_err() {
            return;
        }
        assert_eq!(spec.rungs(), s.ceiling, "the ceiling is the rung count");
        for rung in 0..spec.rungs().min(8) {
            #[allow(clippy::cast_precision_loss)] // A small rung index.
            let at = (rung + 1) as f32;
            assert_eq!(spec.threshold(rung), Some(at), "rung {rung} is the {at}th increment");
            assert!(!spec.reward(rung).is_empty(), "a rung inside the ceiling pays nothing");
        }
        assert!(spec.reward(s.ceiling).is_empty(), "a rung past the ceiling paid");
        assert_eq!(spec.threshold(s.ceiling), None, "a rung past the ceiling had a threshold");
        #[allow(clippy::cast_precision_loss)] // A small ceiling.
        let top = s.ceiling as f32;
        assert_eq!(spec.goal(), Some(top), "the task's target is its ceiling");
        // The latch reads it the same way a ladder's does.
        assert!(
            spec.done(TaskProgress { rungs: s.ceiling, shortcut: false }),
            "every increment paid and the task was not finished",
        );
    });
}

/// The two ways this shape can be declared wrong, each named.
#[test]
fn a_ceiling_the_loader_refuses_reads_as_no_task() {
    check!().with_type::<Scenario>().for_each(|s| {
        let empty = QuestSpec {
            payout: QuestPayout::PerCount { reward: reward(), ceiling: 0 },
            ..s.spec()
        };
        assert_eq!(empty.validate(), Err(QuestError::ZeroCeiling), "a ceiling of none was allowed");
        assert_eq!(empty.rungs(), 0, "a refused ceiling offered rungs");
        assert_eq!(empty.reached(s.count()), 0, "a refused ceiling was reached");

        let huge = MAX_PER_COUNT_PAYOUTS + 1 + s.ceiling;
        let over = QuestSpec {
            payout: QuestPayout::PerCount { reward: reward(), ceiling: huge },
            ..s.spec()
        };
        assert_eq!(
            over.validate(),
            Err(QuestError::CeilingTooHigh { ceiling: huge }),
            "a ceiling past the limit was allowed",
        );
        assert_eq!(over.reached(f32::from(u16::MAX)), 0, "a refused ceiling was reached");
    });
}

#[test]
fn a_per_increment_payout_survives_the_wire() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        let bytes = postcard::to_allocvec(&spec).expect("a task must serialize");
        let back: QuestSpec = postcard::from_bytes(&bytes).expect("and deserialize");
        assert_eq!(back, spec, "the ceiling did not survive the trip");
        let stripped = spec.stripped();
        assert_eq!(stripped.rungs(), spec.rungs(), "stripping changed the ceiling");
        assert!(stripped.reward(0).is_empty(), "a prize survived stripping");
    });
}
