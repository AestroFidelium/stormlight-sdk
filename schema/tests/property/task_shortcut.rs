//! A task can be finished by a condition instead of by its count
//! (stormlight/server#137).
//!
//! Every task was finished by grinding its counter. Real designs also offer a
//! **shortcut**: one moment of play worth the whole ladder — *"hit four enemy
//! heroes at once"* finishes every remaining rung immediately and grants something
//! extra on top.
//!
//! What is pinned here is the half both ends share, which is the *eligibility*
//! rule rather than the payout:
//!
//!   - a shortcut fires **at most once**, like a rung;
//!   - it cannot fire once the task is already **finished**. That is the rule this
//!     issue existed to settle, and it is stated rather than left to fall out of
//!     ordering: a shortcut is payment for skipping, so once nothing is left to
//!     skip there is nothing it is paying for;
//!   - a task with a shortcut and **no rungs** is legal, is accepted at load, and
//!     is **not** finished before it starts — an empty ladder would otherwise be
//!     vacuously complete and the one thing it waits for would never be allowed to
//!     happen;
//!   - **finished** and **every rung reached** are different questions. The first
//!     is a fact about the simulation and the second about the count, and a
//!     shortcut is exactly where they part company.
//!
//! The condition itself is not evaluated here. It reads world state, which this
//! crate has no access to — the spec says *whether the answer matters* and the
//! simulation asks.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::conditions::{CmpOp, Condition};
use stormlight_mod_abi::ids::StackId;
use stormlight_mod_abi::math::{Value, Var, Who};
use stormlight_mod_abi::tasks::{
    QuestError, QuestPayout, QuestShortcut, QuestSpec, QuestStage, TaskProgress,
};

extern crate alloc;
use alloc::vec::Vec;

const COUNTER: StackId = StackId(9);

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// The gaps between rungs, in whole counts. Empty is the shortcut-only task.
    gaps: Vec<u8>,
    /// Whether the task declares a shortcut at all.
    has_shortcut: bool,
    /// How many rungs have been paid.
    paid: u8,
    /// Whether the shortcut has already fired.
    fired: bool,
}

impl Scenario {
    fn spec(&self) -> QuestSpec {
        let mut at = 0.0_f32;
        let stages = self
            .gaps
            .iter()
            .take(5)
            .map(|gap| {
                at += f32::from(*gap) + 1.0;
                QuestStage { threshold: at, reward: Vec::new() }
            })
            .collect();
        QuestSpec {
            counter: COUNTER,
            payout: QuestPayout::Stages(stages),
            shortcut: self.has_shortcut.then(|| QuestShortcut {
                // The shape the example needs: several counts arriving at once.
                when: Condition::Cmp(
                    CmpOp::Ge,
                    Value::Read(Var::StackGain(COUNTER, Who::Caster)),
                    Value::Const(4.0),
                ),
                reward: Vec::new(),
            }),
        }
    }

    fn progress(&self, spec: &QuestSpec) -> TaskProgress {
        TaskProgress {
            rungs: u32::from(self.paid).min(spec.rungs()),
            shortcut: self.fired && self.has_shortcut,
        }
    }
}

/// An objective only ever finished the interesting way. The issue names it as legal
/// and it has to survive the loader, which refuses an empty ladder on its own.
#[test]
fn a_shortcut_with_no_rungs_is_a_legal_task() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = QuestSpec { payout: QuestPayout::Stages(Vec::new()), ..s.spec() };
        if s.has_shortcut {
            spec.validate().expect("a shortcut-only task must load");
            assert!(
                !spec.done(TaskProgress::default()),
                "a task with nothing but a shortcut called itself finished before it started",
            );
            assert!(
                spec.shortcut_open(TaskProgress::default()),
                "a task with nothing but a shortcut would never let it fire",
            );
        } else {
            assert_eq!(
                spec.validate(),
                Err(QuestError::NoStages),
                "a task with neither rungs nor a shortcut can never be finished",
            );
        }
    });
}

/// Once, ever — the same rule a rung answers to, and for the same reason: "the
/// condition held" stays true of the past forever.
#[test]
fn a_shortcut_that_has_fired_never_opens_again() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        let fired = TaskProgress { shortcut: true, ..s.progress(&spec) };
        assert!(!spec.shortcut_open(fired), "a shortcut opened a second time");
        assert!(spec.done(fired), "a fired shortcut left the task unfinished");
    });
}

/// The rule this issue existed to settle, stated as a property rather than left to
/// the order systems happen to run in.
#[test]
fn a_shortcut_cannot_fire_once_the_ladder_is_already_paid() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        if spec.rungs() == 0 {
            return;
        }
        let ground = TaskProgress { rungs: spec.rungs(), shortcut: false };
        assert!(spec.done(ground), "every rung paid and the task was not finished");
        assert!(
            !spec.shortcut_open(ground),
            "a shortcut opened after the ladder it exists to skip was already paid",
        );
    });
}

/// A task with no shortcut is untouched by any of it, which is almost all of them.
#[test]
fn a_task_with_no_shortcut_never_opens_one() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = QuestSpec { shortcut: None, ..s.spec() };
        assert!(
            !spec.shortcut_open(s.progress(&spec)),
            "a task that declared no shortcut opened one",
        );
    });
}

/// The whole of the eligibility rule, in one place: declared, unfired, unfinished.
#[test]
fn a_shortcut_is_open_exactly_when_all_three_hold() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        let progress = s.progress(&spec);
        let expected = spec.shortcut.is_some() && !progress.shortcut && !spec.done(progress);
        assert_eq!(
            spec.shortcut_open(progress),
            expected,
            "the shortcut's eligibility did not follow its own three conditions",
        );
    });
}

/// "Finished" and "counted to the top" are different questions, and a shortcut is
/// where they part company. A client that derived the first from the second would
/// disagree with the simulation the moment one fired.
#[test]
fn finished_is_not_the_same_question_as_every_rung_reached() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        // Nothing counted at all, and the shortcut has fired.
        let shortcut = TaskProgress { rungs: 0, shortcut: true };
        if s.has_shortcut {
            assert!(spec.done(shortcut), "a fired shortcut did not finish the task");
            assert!(!spec.complete(0.0), "a count of nothing reached every rung");
        }
        // And the other way: a count at the top with nothing paid yet is complete
        // by the count and not yet finished by the simulation.
        if let Some(goal) = spec.goal() {
            assert!(spec.complete(goal), "a count at the top did not reach every rung");
            assert!(
                !spec.done(TaskProgress::default()),
                "a task nobody has been paid for called itself finished",
            );
        }
    });
}

#[test]
fn a_declared_shortcut_survives_the_wire() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        let bytes = postcard::to_allocvec(&spec).expect("a task must serialize");
        let back: QuestSpec = postcard::from_bytes(&bytes).expect("and deserialize");
        assert_eq!(back, spec, "the shortcut did not survive the trip");
    });
}

/// A client never fires a shortcut, so the bonus is dropped with the rungs' rewards
/// — but the condition stays, because a client may well want to say what a task is
/// asking for.
#[test]
fn stripping_keeps_what_a_shortcut_asks_and_drops_what_it_pays() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        let stripped = spec.stripped();
        assert_eq!(
            stripped.shortcut.as_ref().map(|shortcut| &shortcut.when),
            spec.shortcut.as_ref().map(|shortcut| &shortcut.when),
            "stripping lost what the shortcut asks for",
        );
        assert!(
            stripped.shortcut.is_none_or(|shortcut| shortcut.reward.is_empty()),
            "a shortcut's bonus survived stripping",
        );
    });
}
