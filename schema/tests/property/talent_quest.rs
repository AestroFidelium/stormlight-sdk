//! A talent that sets a task rather than handing over a step
//! (stormlight/server#132, the declaration half).
//!
//! Every talent this ABI could express was a **step**: taking it changes what an
//! ability does, once, for the rest of the match. A quest is the other axis — a
//! talent a player plays *toward* — and which of the two a row is has to be legible
//! at the moment of choosing. A quest row that looked identical to the ones around
//! it until forty minutes in would be the panel hiding the single most important
//! thing about that choice.
//!
//! What is declared here is the **mark and the target**, not the counting. That is a
//! deliberate split rather than a partial one: counting and paying out are the
//! effect system's — a reaction adjusting a counter, a condition, an `Impact` tree —
//! and the running count is not on the wire yet. What an interface needs in order to
//! *say* "this is a quest" is here, and it is all here.
//!
//!   - **declaring nothing is the good case.** Almost every talent is an ordinary
//!     one, and none of them should have to say so;
//!   - **a goal nobody can reach reads as no goal.** Zero is a quest complete before
//!     it starts; negative and `NaN` are targets no count reaches. All three print no
//!     figure rather than a nonsense one, because nothing else validates this number;
//!   - **it survives the wire**, like every other declaration.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{StackId, TalentId};
use stormlight_mod_abi::talents::{AbilitySelector, QuestSpec, TalentDescriptor};

extern crate alloc;
use alloc::vec::Vec;

#[derive(Debug, TypeGenerator)]
struct Scenario {
    counter: u16,
    /// The declared goal, as a signed count of tenths — so zero and negative are
    /// both reachable without generating raw floats.
    #[generator(-50..=500)]
    tenths: i32,
    /// Whether the goal is one of the values no arithmetic produces.
    broken: Option<bool>,
}

impl Scenario {
    fn goal(&self) -> f32 {
        match self.broken {
            Some(true) => f32::NAN,
            Some(false) => f32::INFINITY,
            #[allow(clippy::cast_precision_loss)] // A small integer count of tenths.
            None => self.tenths as f32 / 10.0,
        }
    }

    fn spec(&self) -> QuestSpec {
        QuestSpec { counter: StackId(self.counter), goal: self.goal() }
    }

    fn talent(&self, quest: Option<QuestSpec>) -> TalentDescriptor {
        TalentDescriptor {
            id: TalentId(1),
            selector: AbilitySelector::Any,
            patches: Vec::new(),
            riders: Vec::new(),
            add_reactions: Vec::new(),
            grants: Vec::new(),
            modifiers: Vec::new(),
            tags: Vec::new(),
            quest,
        }
    }
}

/// The overwhelming majority of talents. A field every one of them had to fill in
/// would be a field every one of them fills in with the same thing.
#[test]
fn an_ordinary_talent_sets_no_task() {
    check!().with_type::<Scenario>().for_each(|s| {
        assert_eq!(s.talent(None).quest, None, "a talent that declared no task carries one");
    });
}

/// Nothing else checks this number, so the reader does. All three unreachable
/// targets read the same way: no figure at all, rather than "0", "-4" or "NaN"
/// printed on a card.
#[test]
fn a_target_no_count_can_reach_reads_as_no_target() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec();
        let goal = spec.goal();
        let reachable = s.goal().is_finite() && s.goal() > 0.0;
        assert_eq!(
            goal.is_some(),
            reachable,
            "a goal of {} read as {goal:?}",
            s.goal(),
        );
        if let Some(read) = goal {
            assert_eq!(read, s.goal(), "a reachable goal was not read back as declared");
        }
    });
}

#[test]
fn a_declared_task_survives_the_wire() {
    check!().with_type::<Scenario>().for_each(|s| {
        let talent = s.talent(Some(s.spec()));
        let bytes = postcard::to_allocvec(&talent).expect("a talent must serialize");
        let back: TalentDescriptor = postcard::from_bytes(&bytes).expect("and deserialize");
        assert_eq!(back.quest.map(|q| q.counter), Some(StackId(s.counter)), "the counter moved");
        // The goal is compared through the reader, because `NaN != NaN` and the
        // reader is the only thing that ever looks at this number anyway.
        assert_eq!(
            back.quest.and_then(|q| q.goal()),
            s.spec().goal(),
            "the declared target did not survive the trip",
        );
    });
}
