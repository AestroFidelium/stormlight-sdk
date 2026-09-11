//! A task stops at its goal, and says so (stormlight/server#132).
//!
//! The first cut of the running count had two holes a player found immediately, and
//! both are the same mistake: it published the *counter* where it meant to publish
//! the **task**.
//!
//! A stack counter is an ordinary reserve. Nothing stops it at the goal, and nothing
//! should — a mod may go on adjusting it, two talents may share it, it may be spent
//! and re-earned. So a count read straight out of it climbs past the target forever,
//! and a player who has done what was asked watches the number keep going and has no
//! way to tell whether it counted.
//!
//! Two rules fix it, and both belong on the spec rather than in whichever interface
//! happens to be drawing:
//!
//!   - [`progress`](QuestSpec::progress) is the count **capped at the goal** — "40 of
//!     40" is the end of a task, and "57 of 40" is a bug in the eyes of everyone who
//!     sees it. The raw tally is still reachable by naming the counter directly, for
//!     a HUD that genuinely wants it;
//!   - [`complete`](QuestSpec::complete) is the same predicate the **server pays out
//!     on**, so an interface saying "done" and an engine having paid can never
//!     disagree — which is the whole reason the rule is here and not in two places.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::StackId;
use stormlight_mod_abi::talents::QuestSpec;

extern crate alloc;
use alloc::vec::Vec;

/// How a generated goal is chosen — a real target plus the three no count reaches.
#[derive(Debug, TypeGenerator)]
enum Goal {
    Wanted(u16),
    Zero,
    Negative(u16),
    NotANumber,
}

impl Goal {
    fn value(&self) -> f32 {
        match self {
            Goal::Wanted(v) => f32::from(*v),
            Goal::Zero => 0.0,
            Goal::Negative(v) => -f32::from(*v),
            Goal::NotANumber => f32::NAN,
        }
    }
}

/// And how a count is — including the ones a counter should never hold, because the
/// reader is the last thing between them and a rendered width.
#[derive(Debug, TypeGenerator)]
enum Count {
    Counted(u16),
    Zero,
    Negative(u16),
    Infinite,
    NotANumber,
}

impl Count {
    fn value(&self) -> f32 {
        match self {
            Count::Counted(v) => f32::from(*v),
            Count::Zero => 0.0,
            Count::Negative(v) => -f32::from(*v),
            Count::Infinite => f32::INFINITY,
            Count::NotANumber => f32::NAN,
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    goal: Goal,
    count: Count,
}

fn spec(s: &Scenario) -> QuestSpec {
    QuestSpec { counter: StackId(0), goal: s.goal.value(), reward: Vec::new() }
}

/// The figure a player reads. It never runs past what was asked, whatever the
/// counter behind it is doing.
#[test]
fn the_printed_progress_never_passes_the_goal() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = spec(s);
        let shown = spec.progress(s.count.value());
        assert!(shown.is_finite(), "a task's progress left the numbers a reader can use");
        assert!(shown >= 0.0, "a task's progress ran below empty");
        if let Some(goal) = spec.goal() {
            assert!(shown <= goal, "a task showed {shown} against a goal of {goal}");
        } else {
            assert_eq!(shown, 0.0, "a task nobody can finish showed progress toward it");
        }
    });
}

/// Under the goal it is the count itself — capping must not cost the reading its
/// only job, which is to say how far along you actually are.
#[test]
fn below_the_goal_the_progress_is_the_count() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = spec(s);
        let (Some(goal), Count::Counted(count)) = (spec.goal(), &s.count) else { return };
        let count = f32::from(*count);
        if count <= goal {
            assert_eq!(spec.progress(count), count, "a count short of the goal was altered");
        }
    });
}

/// The load-bearing agreement. The interface says "finished" from the same
/// predicate the server pays out on, so a player cannot be shown a finished task
/// that was never paid, nor a running one that already was.
#[test]
fn finished_is_exactly_the_progress_having_reached_the_goal() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = spec(s);
        let count = s.count.value();
        let done = spec.complete(count);
        match spec.goal() {
            Some(goal) => {
                // A count must be a *number* to have finished anything: `inf >= goal`
                // is arithmetically true and means only that the tally broke.
                assert_eq!(
                    done,
                    count.is_finite() && count >= goal,
                    "done disagreed with the count against the goal",
                );
                if done {
                    assert_eq!(
                        spec.progress(count),
                        goal,
                        "a finished task showed something other than its goal",
                    );
                    assert_eq!(spec.fraction(count), 1.0, "a finished task's bar was not full");
                }
            }
            // A target no count can reach is never finished, however high the tally
            // goes — a malformed declaration must not read as an achievement.
            None => assert!(!done, "a task nobody can finish reported itself finished"),
        }
    });
}

/// The two rules must never disagree, and there is one shape in particular they must
/// never produce between them: a task that reads "0 of 40" and calls itself finished.
#[test]
fn nothing_reads_as_finished_while_showing_no_progress() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = spec(s);
        let count = s.count.value();
        if spec.complete(count) {
            assert!(
                spec.progress(count) > 0.0,
                "a task called itself finished while showing no progress at all",
            );
        }
    });
}

/// Counting **past** the goal changes nothing a player sees. This is the case that
/// sent the report: a task of three, cast four times.
#[test]
fn counting_past_the_goal_changes_nothing_a_reader_sees() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = spec(s);
        let Some(goal) = spec.goal() else { return };
        let over = goal + f32::from(u8::from(matches!(s.count, Count::Counted(_)))) + 1.0;
        assert_eq!(spec.progress(goal), spec.progress(over), "the figure moved past the goal");
        assert_eq!(spec.fraction(goal), spec.fraction(over), "the bar moved past the goal");
        assert!(spec.complete(over), "a task past its goal stopped reading as finished");
    });
}
