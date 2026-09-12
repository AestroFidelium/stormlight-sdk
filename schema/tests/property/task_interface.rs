//! What an interface may say about a task (stormlight/server#139) — the ABI half.
//!
//! Three additions, and each answers a question the issue asks rather than assuming
//! one:
//!
//!   - **the binding is its own, and it grew a span.** #132 already settled that a
//!     task's progress is not a pool by another name: which counter a task is
//!     counted in is the gameplay mod's declaration, so an interface that named one
//!     would be authoring content. What was missing is that a staged task has *two*
//!     honest targets — the whole objective, and the rung being worked at — and
//!     neither is derivable from the other without the client deciding something it
//!     should be told. So [`QuestSpan`] is a field;
//!   - **the states are their own family.** [`OptionState`] answers questions about
//!     an *option*; [`TaskState`] answers questions about a *task*. The practical
//!     proof they are different: a unit carries tasks with nothing chosen, and those
//!     have no coordinate for an `OptionState` to name them by;
//!   - **per-coordinate and per-unit are answerable separately**, which the issue
//!     asks for in as many words. Two gates and two bindings, one enum each.
//!
//! The **mark** stays an [`OptionState`]. "This row sets a task" is a fact about the
//! option, true before anything has happened, and it is what a player choosing
//! needs.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::ui::{
    OptionState, QuestSpan, RootVisibility, Shown, Strip, SummonGate, TaskState, UiRoot, UiSubject,
    ValueBinding, ValuePart, Widget, WidgetKind,
};

extern crate alloc;
use alloc::string::ToString;
use alloc::vec;

#[derive(Debug, TypeGenerator)]
struct Scenario {
    tier: u8,
    option: u8,
    index: u8,
    stage: bool,
    state: u8,
    /// Whether the widget under test is about a talent's task or the unit's own.
    on_unit: bool,
}

impl Scenario {
    fn span(&self) -> QuestSpan {
        if self.stage { QuestSpan::Stage } else { QuestSpan::Whole }
    }

    fn state(&self) -> TaskState {
        match self.state % 3 {
            0 => TaskState::Running,
            1 => TaskState::Underway,
            _ => TaskState::Done,
        }
    }

    fn gate(&self) -> Shown {
        if self.on_unit {
            Shown::WhileUnitTask { index: self.index, is: self.state() }
        } else {
            Shown::WhileTalentTask { tier: self.tier, option: self.option, is: self.state() }
        }
    }

    fn binding(&self) -> ValueBinding {
        if self.on_unit {
            ValueBinding::UnitTask { index: self.index, span: self.span() }
        } else {
            ValueBinding::TalentQuest { tier: self.tier, option: self.option, span: self.span() }
        }
    }
}

/// A map that renames every handle it is given, so a binding that secretly carried
/// one would be caught by the identity property below.
struct Shift(u16);

macro_rules! shifted {
    ($($method:ident, $ty:path);* $(;)?) => {
        $(fn $method(&self, id: $ty) -> Result<$ty, ()> {
            Ok($ty(id.0.wrapping_add(self.0.into())))
        })*
    };
}

impl IdMap for Shift {
    type Error = ();
    shifted!(
        stat, stormlight_mod_abi::ids::StatId;
        resource, stormlight_mod_abi::ids::ResourceId;
        stack, stormlight_mod_abi::ids::StackId;
        tag, stormlight_mod_abi::ids::TagId;
        tag_class, stormlight_mod_abi::ids::TagClassId;
        param, stormlight_mod_abi::ids::ParamId;
        event, stormlight_mod_abi::ids::EventId;
        buff, stormlight_mod_abi::ids::BuffId;
        curve, stormlight_mod_abi::ids::CurveId;
        damage_type, stormlight_mod_abi::ids::DamageTypeId;
        anim_state, stormlight_mod_abi::ids::AnimStateId;
    );
    fn ability(
        &self,
        id: stormlight_mod_abi::ids::AbilityId,
    ) -> Result<stormlight_mod_abi::ids::AbilityId, ()> {
        Ok(stormlight_mod_abi::ids::AbilityId(id.0.wrapping_add(u32::from(self.0))))
    }
    fn talent(
        &self,
        id: stormlight_mod_abi::ids::TalentId,
    ) -> Result<stormlight_mod_abi::ids::TalentId, ()> {
        Ok(stormlight_mod_abi::ids::TalentId(id.0.wrapping_add(u32::from(self.0))))
    }
    fn handler(
        &self,
        id: stormlight_mod_abi::ids::HandlerId,
    ) -> Result<stormlight_mod_abi::ids::HandlerId, ()> {
        Ok(stormlight_mod_abi::ids::HandlerId(id.0.wrapping_add(u32::from(self.0))))
    }
    fn unit(
        &self,
        id: stormlight_mod_abi::ids::UnitId,
    ) -> Result<stormlight_mod_abi::ids::UnitId, ()> {
        Ok(stormlight_mod_abi::ids::UnitId(id.0.wrapping_add(u32::from(self.0))))
    }
    fn navmesh(
        &self,
        id: stormlight_mod_abi::ids::NavMeshId,
    ) -> Result<stormlight_mod_abi::ids::NavMeshId, ()> {
        Ok(stormlight_mod_abi::ids::NavMeshId(id.0.wrapping_add(u32::from(self.0))))
    }
}

fn root(s: &Scenario) -> UiRoot {
    let mut bar = Widget {
        name: "ring".to_string(),
        layout: stormlight_mod_abi::ui::Layout::default(),
        style: stormlight_mod_abi::ui::Style::default(),
        kind: WidgetKind::Bar { value: s.binding() },
    };
    bar.layout.shown = s.gate();
    UiRoot {
        name: "tasks".to_string(),
        when: RootVisibility::Always,
        summon: SummonGate::default(),
        subject: UiSubject::LocalPlayer,
        strip: Strip::default(),
        root: Widget {
            name: "tasks_root".to_string(),
            layout: stormlight_mod_abi::ui::Layout::default(),
            style: stormlight_mod_abi::ui::Style::default(),
            kind: WidgetKind::Panel { children: vec![bar] },
        },
    }
}

/// The whole objective is what a task with nothing staged should be drawn against,
/// so it is what an author gets by saying nothing.
#[test]
fn the_whole_objective_is_the_default_span() {
    check!().with_type::<Scenario>().for_each(|_| {
        assert_eq!(QuestSpan::default(), QuestSpan::Whole, "the default span moved");
        assert_eq!(TaskState::default(), TaskState::Running, "the default task state moved");
    });
}

/// Both gates and both bindings are ordinary members of their families: a tree
/// carrying them validates exactly as one carrying anything else does.
#[test]
fn a_tree_gated_on_a_task_validates() {
    check!().with_type::<Scenario>().for_each(|s| {
        root(s).validate().expect("a task gate is an ordinary gate");
    });
}

/// A unit's own task belongs to no tree at all, which is most of the reason it
/// needed a gate of its own — and the one gate in the family that is about no tier.
#[test]
fn only_the_coordinate_gate_is_about_a_tier() {
    check!().with_type::<Scenario>().for_each(|s| {
        let talent = Shown::WhileTalentTask { tier: s.tier, option: s.option, is: s.state() };
        assert_eq!(talent.tier(), Some(s.tier), "a coordinate gate lost its tier");
        let unit = Shown::WhileUnitTask { index: s.index, is: s.state() };
        assert_eq!(unit.tier(), None, "a unit's own task claimed a tier");
    });
}

/// Neither binding names an interned handle. Which counter a task is counted in is
/// the gameplay mod's declaration, resolved at read time through the tree or the
/// unit's own list — an interface that carried one would be authoring content.
#[test]
fn a_task_binding_names_no_interned_handle() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut binding = s.binding();
        binding.remap_ids(&Shift(7)).expect("a total map never fails");
        assert_eq!(binding, s.binding(), "adoption rewrote a task binding");
    });
}

/// All three parts are real and different for both spans, which is why this is one
/// binding rather than two texts.
#[test]
fn a_task_binding_survives_the_wire_with_every_part() {
    check!().with_type::<Scenario>().for_each(|s| {
        let bytes = postcard::to_allocvec(&s.binding()).expect("a binding must serialize");
        let back: ValueBinding = postcard::from_bytes(&bytes).expect("and deserialize");
        assert_eq!(back, s.binding(), "a task binding did not survive the trip");

        let gate = postcard::to_allocvec(&s.gate()).expect("a gate must serialize");
        let back: Shown = postcard::from_bytes(&gate).expect("and deserialize");
        assert_eq!(back, s.gate(), "a task gate did not survive the trip");

        for part in [ValuePart::Current, ValuePart::Max, ValuePart::Fraction] {
            let bytes = postcard::to_allocvec(&part).expect("a part must serialize");
            let back: ValuePart = postcard::from_bytes(&bytes).expect("and deserialize");
            assert_eq!(back, part, "a value part did not survive the trip");
        }
    });
}

/// The mark is still a fact about the **option**, and it is still where it was: a
/// player choosing needs it before any progress exists, which is exactly when the
/// task family has nothing to say.
#[test]
fn the_mark_is_still_an_option_state() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mark = Shown::WhileOption { tier: s.tier, option: s.option, is: OptionState::Quest };
        assert_eq!(mark.tier(), Some(s.tier), "the mark stopped being about a tier");
        let bytes = postcard::to_allocvec(&mark).expect("a gate must serialize");
        let back: Shown = postcard::from_bytes(&bytes).expect("and deserialize");
        assert_eq!(back, mark, "the mark did not survive the trip");
    });
}
