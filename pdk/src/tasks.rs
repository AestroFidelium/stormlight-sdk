//! Declaring a task (stormlight/server#136) — constructors for the ladder a
//! counted objective pays along.
//!
//! The ABI itself is plain data: a [`QuestSpec`] is a counter and a list of
//! `(threshold, effects)`. Written out literally that is four lines of punctuation
//! per rung, and the interesting part — the numbers and what they buy — is the
//! smallest thing on the page. So the pdk supplies the two constructors that let a
//! ladder read as a ladder:
//!
//! ```ignore
//! use stormlight_mod_sdk::tasks::{ladder, rung};
//!
//! // Hit heroes with this ability: something at 15, something better at 30 and 45.
//! let quest = ladder(hits, vec![
//!     rung(15.0, vec![gain_energy(energy, 20.0)]),
//!     rung(30.0, vec![apply_buff(honed, ImpactTarget::Caster)]),
//!     rung(45.0, vec![apply_buff(mastered, ImpactTarget::Caster)]),
//! ]);
//! ```
//!
//! The single-goal task keeps its own name ([`goal`]) rather than being spelled as
//! a one-element ladder at every call site. It is the same declaration — the ABI
//! has one shape — but *"do this forty times"* is a sentence a mod should be able
//! to write as a sentence.
//!
//! Nothing here counts. A count is an ordinary `AdjustPool` on the named counter,
//! authored wherever the mod wants the counting to happen: a rider on a cast, a
//! reaction to a hit, a mod's own tick.

use alloc::vec;
use alloc::vec::Vec;

use stormlight_mod_abi::ids::StackId;
use stormlight_mod_abi::impacts::Impact;
use stormlight_mod_abi::tasks::{QuestPayout, QuestSpec, QuestStage};

/// One rung: the count that reaches it, and what reaching it hands over.
///
/// An empty `reward` is legal and means a rung that only marks progress — a thing
/// an interface may well want and the simulation costs nothing for.
#[must_use]
pub fn rung(threshold: f32, reward: Vec<Impact>) -> QuestStage {
    QuestStage { threshold, reward }
}

/// A task counted in `counter` that pays along `stages`.
///
/// The thresholds must climb; a ladder that goes backwards is refused at load with
/// a named error rather than quietly sorted, because a mod that wrote its rungs out
/// of order meant something.
#[must_use]
pub fn ladder(counter: StackId, stages: Vec<QuestStage>) -> QuestSpec {
    QuestSpec { counter, payout: QuestPayout::Stages(stages) }
}

/// A task with a single target: reach `goal` in `counter`, get `reward`.
///
/// The one-rung ladder, named after the sentence it is written to express.
#[must_use]
pub fn goal(counter: StackId, goal: f32, reward: Vec<Impact>) -> QuestSpec {
    ladder(counter, vec![rung(goal, reward)])
}
