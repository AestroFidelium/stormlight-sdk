//! Tasks — a counted objective that pays out **while** it is being worked at
//! (stormlight/server#135, stormlight/server#136).
//!
//! A step is settled at the moment of choosing: take it and what it changes is
//! changed for the rest of the match. A task is the other shape — the player keeps
//! playing and the objective keeps paying — and it is the one the engine had no way
//! to express. [`QuestSpec`] is the whole of the declaration: a counter to watch,
//! and a schedule saying what the count buys.
//!
//! # Why this is not a talent field
//!
//! A task lived on [`TalentDescriptor`](crate::talents::TalentDescriptor) when it
//! arrived, because a talent was the only thing that wanted one. It is not a talent
//! *feature*: a hero's own baseline objective, an event's, a map's are all the same
//! declaration with nothing chosen. So the declaration sits in its own module and a
//! talent carries one the way a unit does — as a rider, not as a kind of thing.
//! Building it the other way round would mean building it twice.
//!
//! # The ladder, and why a rung is the unit of everything here
//!
//! One counter, an ordered list of thresholds, each with its own reward. A task
//! with a single goal is the one-rung case rather than a second shape, which is
//! what keeps the payout path, the latch, the wire and the interface from each
//! growing two branches.
//!
//! Everything downstream is phrased in **rungs** — [`QuestSpec::reached`] counts
//! them, the latch remembers how many have been paid, and a payout walks the ones
//! in between. That indirection is deliberate: it is what lets a second payout
//! *schedule* (stormlight/server#138) be a second way of answering
//! [`QuestSpec::threshold`] rather than a second engine path.
//!
//! # Counting is not here
//!
//! Nothing in this module counts. A count is an ordinary
//! [`AdjustPool`](crate::impacts::Impact::AdjustPool) on the named counter, from a
//! rider, a reaction, a mod's tick — whatever the mod writes. What the engine adds
//! is the **watch** (notice a threshold being crossed) and the **latch** (pay it
//! once), because those are the two halves a mod cannot express for itself, and
//! every mod that tried would invent them differently.
//!
//! Content-free: this knows a counter handle, some numbers, and effect trees it
//! never looks inside.

use alloc::vec::Vec;
use core::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::StackId;
use crate::impacts::Impact;

/// One rung of a task's ladder: the count that reaches it, and what reaching it
/// hands over.
///
/// The reward is an ordinary effect tree, which is the single decision that keeps
/// the engine from ever learning what a prize is. Everything the ISA can already do
/// — grant, patch, buff, spawn, damage — is already a reward, and no reward *kind*
/// will ever need an engine change.
///
/// A stage carries **no target**. A reward is dispatched against the task's owner
/// as caster, source and target alike, and a stage that means to pay somebody else
/// says so the way every other effect tree does: with an impact that selects
/// ([`Impact::Area`](crate::impacts::Impact::Area) and its neighbours). A target
/// field here would be a second, weaker selection vocabulary beside the one the ISA
/// already has, and it could not express "my allies in a circle" however it was
/// spelled.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct QuestStage {
    /// The count at which this rung pays. Strictly greater than the rung before it
    /// — see [`QuestSpec::validate`].
    pub threshold: f32,
    /// What reaching it hands over. Empty is legal and means a rung that only marks
    /// progress, which is a thing an interface may well want.
    #[serde(default)]
    pub reward: Vec<Impact>,
}

/// How a task pays for the counts it is given.
///
/// One enum rather than one shape, because the two schedules a design actually
/// asks for are genuinely different questions — *what does crossing this number
/// buy* and *what does one more of these buy* — and collapsing either into the
/// other costs a mod either the ladder or the cap.
///
/// They are the same machinery underneath: both answer "how many rungs does a
/// count of `n` reach" and "what does rung `i` pay", and every consumer — the
/// latch, the payout walk, the wire, the interface — is written against those two
/// questions alone. See [`QuestSpec::threshold`].
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum QuestPayout {
    /// An ordered ladder: rung `i` pays when the count reaches `stages[i].threshold`.
    ///
    /// The single-goal task is the one-element case.
    Stages(Vec<QuestStage>),
}

/// A task: a counter, and what the counts in it buy.
///
/// **The whole objective in one declaration.** The counter and the schedule are
/// halves of one sentence — a prize with no counter to watch is not a task, and a
/// counter with nothing to buy is a tally a mod keeps for its own reasons and
/// declares with nothing at all.
///
/// Which of a step and a task something is has to be legible **at the moment of
/// choosing**, before any of it has happened: a quest row that looked identical to
/// the ones around it until forty minutes in would be the panel hiding the single
/// most important thing about that choice. So the declaration carries what an
/// interface needs in order to *say* it is a task as well as what the simulation
/// needs to run one.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct QuestSpec {
    /// The stack counter the task is counted in.
    pub counter: StackId,
    /// What the counts buy, and when.
    pub payout: QuestPayout,
}

/// Why a [`QuestSpec`] cannot be run as declared.
///
/// Each variant is a break that would otherwise be absorbed silently, leaving the
/// author with a task that never pays and no explanation — or, worse, one that pays
/// everything on the first tick of the match. Indices point at the rung at fault.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QuestError {
    /// A ladder with no rungs: an objective that can never be finished, and nothing
    /// for an interface to draw a target against.
    NoStages,
    /// A threshold no count can reach — zero, negative, or not a number. Zero is a
    /// task already complete before it starts; the other two are targets no tally
    /// ever arrives at.
    UnreachableThreshold { stage: u16 },
    /// A rung that does not sit above the one before it. Refused rather than sorted:
    /// a mod that wrote its ladder out of order meant something, and quietly
    /// reordering it would pay rewards in an order the author never wrote.
    UnorderedStages { stage: u16 },
}

impl fmt::Display for QuestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoStages => f.write_str("task declares no stages"),
            Self::UnreachableThreshold { stage } => {
                write!(f, "task stage {stage} sets a threshold no count can reach")
            }
            Self::UnorderedStages { stage } => {
                write!(f, "task stage {stage} does not sit above the stage before it")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for QuestError {}

impl QuestSpec {
    /// A single-goal task: reach `goal`, get `reward`. The shape every task had
    /// before the ladder existed, kept as the constructor it always was.
    #[must_use]
    pub fn single(counter: StackId, goal: f32, reward: Vec<Impact>) -> Self {
        Self {
            counter,
            payout: QuestPayout::Stages(alloc::vec![QuestStage { threshold: goal, reward }]),
        }
    }

    /// Why this task cannot be run, or `Ok` if it can.
    ///
    /// Called at adoption, so a malformed declaration is a **named refusal at load**
    /// rather than a task that silently never pays. Every reader below is
    /// nonetheless total over a spec that would fail this, because the ABI is also
    /// what a client holds and a reader that panicked on content would be a worse
    /// failure than one that draws nothing.
    ///
    /// # Errors
    ///
    /// [`QuestError`] naming the rung at fault.
    pub fn validate(&self) -> Result<(), QuestError> {
        match &self.payout {
            QuestPayout::Stages(stages) => {
                if stages.is_empty() {
                    return Err(QuestError::NoStages);
                }
                let mut previous = 0.0_f32;
                for (index, stage) in stages.iter().enumerate() {
                    let at = u16::try_from(index).unwrap_or(u16::MAX);
                    if !stage.threshold.is_finite() || stage.threshold <= 0.0 {
                        return Err(QuestError::UnreachableThreshold { stage: at });
                    }
                    if stage.threshold <= previous {
                        return Err(QuestError::UnorderedStages { stage: at });
                    }
                    previous = stage.threshold;
                }
                Ok(())
            }
        }
    }

    /// Whether this task is one the engine will run at all.
    ///
    /// The one gate every reader below passes through, so a declaration the loader
    /// would refuse reads as *no task* everywhere rather than as a different task in
    /// each place that looks at it.
    #[must_use]
    pub fn well_formed(&self) -> bool {
        self.validate().is_ok()
    }

    /// How many rungs this task has. Zero for a declaration nothing can run.
    #[must_use]
    pub fn rungs(&self) -> u32 {
        if !self.well_formed() {
            return 0;
        }
        match &self.payout {
            QuestPayout::Stages(stages) => u32::try_from(stages.len()).unwrap_or(u32::MAX),
        }
    }

    /// The count at which rung `rung` pays, or `None` past the end of the ladder.
    ///
    /// The whole of what a payout schedule *is*. Everything else here — reaching,
    /// paying, the bar an interface fills — is written against this one question, so
    /// a second schedule is a second arm of this match and nothing else.
    #[must_use]
    pub fn threshold(&self, rung: u32) -> Option<f32> {
        if !self.well_formed() {
            return None;
        }
        match &self.payout {
            QuestPayout::Stages(stages) => {
                stages.get(usize::try_from(rung).ok()?).map(|stage| stage.threshold)
            }
        }
    }

    /// What rung `rung` hands over. Empty past the end of the ladder, which is the
    /// same thing a rung that pays nothing hands over — both dispatch nothing.
    #[must_use]
    pub fn reward(&self, rung: u32) -> &[Impact] {
        if !self.well_formed() {
            return &[];
        }
        match &self.payout {
            QuestPayout::Stages(stages) => usize::try_from(rung)
                .ok()
                .and_then(|at| stages.get(at))
                .map_or(&[][..], |stage| &stage.reward),
        }
    }

    /// How many rungs a count of `count` has reached.
    ///
    /// The rule the simulation pays on and the interface draws from, held in one
    /// place so the two can never disagree about what "done" means.
    ///
    /// The count must be a **number**. `inf >= threshold` is arithmetically true and
    /// means nothing: a counter that has left the finite numbers has lost whatever
    /// it was counting, and paying a prize for it — or telling a player they have
    /// finished — would be rewarding a broken tally.
    ///
    /// Because the thresholds are strictly increasing ([`Self::validate`]), the
    /// rungs a count reaches are always a **prefix** of the ladder. That is the fact
    /// the latch is built on: a high-water mark and a set of paid rungs are the same
    /// record here, and the cheap one is the honest one.
    #[must_use]
    pub fn reached(&self, count: f32) -> u32 {
        if !count.is_finite() {
            return 0;
        }
        (0..self.rungs())
            .take_while(|&rung| self.threshold(rung).is_some_and(|at| count >= at))
            .count()
            .try_into()
            .unwrap_or(u32::MAX)
    }

    /// The task's final target — the last rung's threshold, and the figure an
    /// interface prints as the whole objective's goal.
    ///
    /// `None` for a declaration nothing can run, so a malformed task prints no
    /// figure rather than a nonsense one.
    #[must_use]
    pub fn goal(&self) -> Option<f32> {
        self.rungs().checked_sub(1).and_then(|last| self.threshold(last))
    }

    /// Whether a count of `count` has reached every rung.
    ///
    /// The *count's* answer, which is what both ends can derive from content alone.
    /// It is not the same question as "has this task been paid out in full" once a
    /// shortcut can finish a task early (stormlight/server#137) — that one is a fact
    /// about the simulation and has to be told rather than derived.
    #[must_use]
    pub fn complete(&self, count: f32) -> bool {
        self.rungs() > 0 && self.reached(count) == self.rungs()
    }

    /// How far along the task a count of `count` is, **capped at the final target**
    /// — the figure an interface prints.
    ///
    /// Capped because the counter is not the task. A stack counter is an ordinary
    /// reserve and a mod may go on adjusting it long after the last rung is passed —
    /// the same counter may feed two tasks, or be spent and re-earned — so the raw
    /// number climbing past the target says nothing a player wants to read. "40 of
    /// 40" is the end of a task; "57 of 40" is a bug in the eyes of everyone who
    /// sees it.
    ///
    /// A HUD that genuinely wants the raw tally still has it, by naming the counter
    /// directly with [`PoolRef::Stacks`](crate::impacts::PoolRef::Stacks).
    #[must_use]
    pub fn progress(&self, count: f32) -> f32 {
        match self.goal() {
            Some(goal) if count.is_finite() => count.clamp(0.0, goal),
            _ => 0.0,
        }
    }

    /// How far along a count of `count` is, in `0.0..=1.0` — what a bar fills to.
    ///
    /// A task with no reachable target reads **empty** rather than full: there is no
    /// progress to be made toward a goal no count reaches, and a full bar over a
    /// task nobody can finish is the confident wrong answer.
    #[must_use]
    pub fn fraction(&self, count: f32) -> f32 {
        match self.goal() {
            Some(goal) if count.is_finite() && goal > 0.0 => (count / goal).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// This task with every reward tree emptied.
    ///
    /// What a **client** is allowed to hold: the counter, the thresholds and the
    /// shape, and none of the prizes. A client never pays a task out, so a prize
    /// reaching one would be handles it has not translated and cannot act on —
    /// dropped at the door rather than carried around unused.
    #[must_use]
    pub fn stripped(&self) -> Self {
        Self {
            counter: self.counter,
            payout: match &self.payout {
                QuestPayout::Stages(stages) => QuestPayout::Stages(
                    stages
                        .iter()
                        .map(|stage| QuestStage { threshold: stage.threshold, reward: Vec::new() })
                        .collect(),
                ),
            },
        }
    }
}
