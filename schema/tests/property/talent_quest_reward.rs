//! What completing a task hands over (stormlight/server#132, the payout half).
//!
//! A quest declared a counter and a goal and nothing at the end of it, which made
//! it a talent that quietly counted. The reward is the third of the three things
//! the issue asks for, and it is the one that makes the row a quest rather than a
//! slow buff.
//!
//! It rides on the [`QuestSpec`] rather than beside it, and that is the whole
//! design decision:
//!
//!   - **one declaration says the whole task.** The counter, the target and the
//!     prize are three halves of one sentence, and a talent carrying a reward with
//!     no task — or a task whose prize sat in a second, unrelated field — would be
//!     two ways to write down something that only means anything together;
//!   - **paying once is the engine's guarantee**, not each mod's. A reward
//!     authored as an ordinary rider would fire on every cast past the goal, and
//!     every mod would have to invent the same latch;
//!   - **it is the same effect vocabulary as everything else.** A quest pays out
//!     in `Impact`s, so it can hand over anything an ability can, and the ids
//!     inside it are interned handles that travel through adoption like any other.
//!
//! Declaring none stays the good case: a task with no prize is a counter a mod is
//! keeping for its own reasons, which is a thing a mod is allowed to want.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::common::{ImpactTarget, NumOp};
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::impacts::{Impact, PoolRef};
use stormlight_mod_abi::math::Value;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::talents::{AbilitySelector, QuestSpec, TalentDescriptor};

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Debug, TypeGenerator)]
struct Scenario {
    counter: u16,
    #[generator(1..=500)]
    goal: u16,
    /// The buff a completed task hands over, as a local handle.
    buff: u16,
    /// The resource it also tops up — a second leaf, so the reward is a tree
    /// rather than a single effect.
    resource: u16,
    /// What the whole family is shifted by when the mod is adopted beside another.
    #[generator(0..=64)]
    shift: u16,
}

/// A shift-everything map: the crudest possible adoption, and enough to catch a
/// handle the reward tree forgot to rewrite.
struct Shift(u16);

macro_rules! shifted {
    ($($method:ident: $family:ident($raw:ty)),* $(,)?) => {
        impl IdMap for Shift {
            type Error = core::convert::Infallible;
            $(
                fn $method(&self, id: $family) -> Result<$family, Self::Error> {
                    Ok($family(id.0.wrapping_add(self.0 as $raw)))
                }
            )*
        }
    };
}

shifted! {
    stat: StatId(u16),
    resource: ResourceId(u16),
    stack: StackId(u16),
    tag: TagId(u16),
    tag_class: TagClassId(u16),
    param: ParamId(u16),
    event: EventId(u16),
    buff: BuffId(u16),
    curve: CurveId(u16),
    damage_type: DamageTypeId(u16),
    ability: AbilityId(u32),
    talent: TalentId(u32),
    handler: HandlerId(u32),
    unit: UnitId(u32),
    navmesh: NavMeshId(u32),
    anim_state: AnimStateId(u16),
}

impl Scenario {
    fn reward(&self) -> Vec<Impact> {
        vec![
            Impact::ApplyModifiers {
                buff: BuffId(self.buff),
                stacks: Value::Const(1.0),
                duration_override: None,
                target: ImpactTarget::Caster,
            },
            Impact::AdjustPool {
                pool: PoolRef::Resource(ResourceId(self.resource)),
                op: NumOp::Add,
                amount: Value::Const(10.0),
                target: ImpactTarget::Caster,
            },
        ]
    }

    fn spec(&self, reward: Vec<Impact>) -> QuestSpec {
        QuestSpec { counter: StackId(self.counter), goal: f32::from(self.goal), reward }
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

/// A counter a mod keeps for its own reasons is still a legal task. Nothing about
/// declaring a goal obliges a mod to declare a prize at the end of it.
#[test]
fn a_task_may_hand_over_nothing() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = s.spec(Vec::new());
        assert!(spec.reward.is_empty(), "an unrewarded task invented a prize");
        assert_eq!(spec.goal(), Some(f32::from(s.goal)), "and it is still a task");
    });
}

#[test]
fn a_declared_reward_survives_the_wire() {
    check!().with_type::<Scenario>().for_each(|s| {
        let talent = s.talent(Some(s.spec(s.reward())));
        let bytes = postcard::to_allocvec(&talent).expect("a talent must serialize");
        let back: TalentDescriptor = postcard::from_bytes(&bytes).expect("and deserialize");
        assert_eq!(back, talent, "the prize did not survive the trip");
    });
}

/// The load-bearing one. A reward is an `Impact` tree full of interned handles,
/// and adoption rewrites every handle a mod authored into the global space. One
/// that stayed local would pay out somebody else's buff — silently, forty minutes
/// into a match, which is the worst possible moment to find out.
#[test]
fn adoption_rewrites_the_handles_inside_a_reward() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut talent = s.talent(Some(s.spec(s.reward())));
        talent.remap_ids(&Shift(s.shift)).expect("a total map never fails");
        let quest = talent.quest.expect("the task survived adoption");

        assert_eq!(
            quest.counter,
            StackId(s.counter.wrapping_add(s.shift)),
            "the counter was not rewritten",
        );
        match quest.reward.as_slice() {
            [
                Impact::ApplyModifiers { buff, .. },
                Impact::AdjustPool { pool: PoolRef::Resource(res), .. },
            ] => {
                assert_eq!(
                    *buff,
                    BuffId(s.buff.wrapping_add(s.shift)),
                    "the prize's buff is local"
                );
                assert_eq!(
                    *res,
                    ResourceId(s.resource.wrapping_add(s.shift)),
                    "the prize's resource is local",
                );
            }
            other => panic!("the reward tree changed shape under adoption: {other:?}"),
        }
    });
}
