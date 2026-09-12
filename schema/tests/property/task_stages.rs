//! A task pays at several thresholds, not once at the end (stormlight/server#136).
//!
//! A task was a counter and a single goal, which expresses *"do this forty times,
//! then get a thing"* and nothing else. The design most objectives actually want is
//! a **ladder on one counter** — 15, 30, 45, each rung paying on its own — so the
//! objective is rewarding to work at rather than only to finish. A mod that wanted
//! one had to declare three tasks over three counters and keep them in step by
//! hand: three marks on an interface for one objective, and three chances for them
//! to disagree.
//!
//! What is pinned here is the **rule**, which is the half both ends share:
//!
//!   - the single-goal task is the **one-rung case**, not a second shape. Every
//!     property below is asserted over ladders of every length, one included;
//!   - the rungs a count reaches are a **prefix**, always. That is not a
//!     convenience — it is the fact the latch is built on, and it is what makes a
//!     high-water mark and a set of paid rungs the same record. It holds because
//!     the thresholds are strictly increasing, which is checked at load;
//!   - a malformed ladder reads as **no task at all**, uniformly. An interface that
//!     printed one figure for a broken declaration and a payout that used another
//!     would be two readers disagreeing about content neither of them can fix;
//!   - it **survives the wire**, like every other declaration.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::StackId;
use stormlight_mod_abi::tasks::{QuestError, QuestPayout, QuestSpec, QuestStage};

extern crate alloc;
use alloc::vec::Vec;

/// A generated ladder: gaps between consecutive thresholds, so an ordered
/// declaration is the common case and disorder has to be asked for.
#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// One entry per rung, each the gap above the rung below it, in tenths. Short
    /// ladders — six rungs is already more than any design writes.
    gaps: Vec<u8>,
    /// Optionally break one rung, and how.
    break_at: Option<(u8, Breakage)>,
    /// The count to read the ladder at, in tenths.
    #[generator(0..=900)]
    count_tenths: u32,
}

#[derive(Debug, TypeGenerator)]
enum Breakage {
    /// A threshold that sits at or below the one before it.
    OutOfOrder,
    /// A threshold no count reaches.
    NotANumber,
    /// A threshold of zero — a rung already crossed before the match starts.
    Zero,
}

impl Scenario {
    fn thresholds(&self) -> Vec<f32> {
        let mut at = 0.0_f32;
        let mut out = Vec::new();
        for gap in self.gaps.iter().take(6) {
            // +1 so consecutive thresholds are always strictly increasing before
            // a breakage is applied: a gap of zero would be a ladder that repeats.
            at += f32::from(*gap) / 10.0 + 0.1;
            out.push(at);
        }
        if let Some((index, how)) = &self.break_at
            && !out.is_empty()
        {
            let at = usize::from(*index) % out.len();
            out[at] = match how {
                Breakage::OutOfOrder => {
                    if at == 0 {
                        // Nothing sits below rung 0, so the only way to break its
                        // order is to make it unreachable instead.
                        -1.0
                    } else {
                        out[at - 1]
                    }
                }
                Breakage::NotANumber => f32::NAN,
                Breakage::Zero => 0.0,
            };
        }
        out
    }

    fn spec(&self) -> QuestSpec {
        QuestSpec {
            counter: StackId(7),
            shortcut: None,
            payout: QuestPayout::Stages(
                self.thresholds()
                    .into_iter()
                    .map(|threshold| QuestStage { threshold, reward: Vec::new() })
                    .collect(),
            ),
        }
    }

    fn count(&self) -> f32 {
        #[allow(clippy::cast_precision_loss)] // A small integer count of tenths.
        {
            self.count_tenths as f32 / 10.0
        }
    }
}

/// The acceptance the issue names first: the shape a task always had is the
/// one-rung case of the shape it has now, not a second branch anybody has to keep
/// in step.
#[test]
fn a_single_goal_task_is_the_one_rung_case() {
    check!().with_type::<Scenario>().for_each(|s| {
        let goal = s.count().max(0.1);
        let spec = QuestSpec::single(StackId(3), goal, Vec::new());
        assert_eq!(spec.rungs(), 1, "a single-goal task declared other than one rung");
        assert_eq!(spec.goal(), Some(goal), "its target is its only threshold");
        assert!(spec.complete(goal), "a count standing exactly on the goal is finished");
        assert_eq!(
            spec.complete(s.count()),
            s.count() >= goal,
            "the one-rung ladder disagreed with the single-goal rule it replaces",
        );
    });
}

/// The fact everything downstream leans on. A count reaches a *prefix* of the
/// ladder — never a hole in the middle — so "how many rungs have been paid" is a
/// complete record and the latch needs no set.
#[test]
fn the_rungs_a_count_reaches_are_a_prefix() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        let count = s.count();
        let reached = spec.reached(count);
        for rung in 0..spec.rungs() {
            let at = spec.threshold(rung).expect("a well-formed ladder answers every rung");
            assert_eq!(
                rung < reached,
                count >= at,
                "rung {rung} of {} was reached out of order at count {count}",
                spec.rungs(),
            );
        }
    });
}

/// Monotone in the count, which is what makes "the count fell and came back" a
/// question the latch alone has to answer rather than one the rule can get wrong.
#[test]
fn more_count_never_reaches_fewer_rungs() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        let lower = s.count();
        let higher = lower + 1.0;
        assert!(
            spec.reached(higher) >= spec.reached(lower),
            "counting further reached fewer rungs",
        );
        assert!(spec.reached(lower) <= spec.rungs(), "more rungs were reached than exist");
    });
}

/// A declaration the loader refuses reads as nothing, everywhere. The alternative
/// is an interface drawing a target the simulation will never pay.
#[test]
fn a_ladder_the_loader_refuses_reads_as_no_task() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        if spec.validate().is_ok() {
            assert!(spec.rungs() > 0, "a ladder the loader accepts has rungs to pay");
            return;
        }
        assert_eq!(spec.rungs(), 0, "a refused ladder offered rungs");
        assert_eq!(spec.goal(), None, "a refused ladder printed a target");
        assert_eq!(spec.reached(s.count()), 0, "a refused ladder was reached");
        assert!(!spec.complete(s.count()), "a refused ladder called itself finished");
        assert_eq!(spec.fraction(s.count()), 0.0, "a refused ladder filled a bar");
        assert!(spec.reward(0).is_empty(), "a refused ladder offered a prize");
    });
}

/// Each refusal names the rung at fault, because an author staring at a task that
/// never pays has nothing else to go on.
#[test]
fn a_refusal_names_what_is_wrong_and_where() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        let thresholds = s.thresholds();
        match spec.validate() {
            Ok(()) => assert!(!thresholds.is_empty(), "an empty ladder was accepted"),
            Err(QuestError::NoStages) => assert!(thresholds.is_empty(), "a ladder read as empty"),
            Err(QuestError::UnreachableThreshold { stage }) => {
                let at = thresholds[usize::from(stage)];
                assert!(!at.is_finite() || at <= 0.0, "a reachable threshold {at} was refused");
            }
            Err(QuestError::UnorderedStages { stage }) => {
                assert!(stage > 0, "rung 0 has nothing to sit above");
                let at = thresholds[usize::from(stage)];
                let below = thresholds[usize::from(stage) - 1];
                assert!(at <= below, "an ordered threshold {at} above {below} was refused");
            }
        }
    });
}

/// A bar is filled from the same declaration the payout walks, so a task that reads
/// full and does not pay is not expressible.
#[test]
fn the_bar_and_the_ladder_agree_about_finished() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        let count = s.count();
        let fraction = spec.fraction(count);
        assert!((0.0..=1.0).contains(&fraction), "a bar filled to {fraction}");
        if spec.complete(count) {
            assert_eq!(fraction, 1.0, "every rung reached and the bar was not full");
            assert_eq!(
                spec.progress(count),
                spec.goal().unwrap_or(0.0),
                "the figure stopped short"
            );
        }
        assert!(
            spec.progress(count) <= spec.goal().unwrap_or(0.0),
            "the printed figure climbed past the target",
        );
    });
}

/// A count that has left the finite numbers has lost whatever it was counting.
/// Paying a prize for it, or telling a player they have finished, would be
/// rewarding a broken tally.
#[test]
fn a_count_that_is_not_a_number_has_reached_nothing() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        for count in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(spec.reached(count), 0, "{count} reached a rung");
            assert!(!spec.complete(count), "{count} finished a task");
            assert_eq!(spec.fraction(count), 0.0, "{count} filled a bar");
        }
    });
}

#[test]
fn a_declared_ladder_survives_the_wire() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        let bytes = postcard::to_allocvec(&spec).expect("a task must serialize");
        let back: QuestSpec = postcard::from_bytes(&bytes).expect("and deserialize");
        assert_eq!(back.counter, spec.counter, "the counter moved");
        assert_eq!(back.rungs(), spec.rungs(), "the ladder changed length");
        for rung in 0..back.rungs() {
            assert_eq!(back.threshold(rung), spec.threshold(rung), "rung {rung} moved");
        }
    });
}

/// What a client is allowed to hold. It never pays a task out, so a prize reaching
/// one is handles it has not translated and cannot act on.
#[test]
fn stripping_a_task_keeps_its_shape_and_drops_its_prizes() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        let stripped = spec.stripped();
        assert_eq!(stripped.counter, spec.counter, "stripping moved the counter");
        assert_eq!(stripped.rungs(), spec.rungs(), "stripping changed the ladder");
        for rung in 0..spec.rungs() {
            assert_eq!(stripped.threshold(rung), spec.threshold(rung), "rung {rung} moved");
            assert!(stripped.reward(rung).is_empty(), "a prize survived stripping");
        }
    });
}
