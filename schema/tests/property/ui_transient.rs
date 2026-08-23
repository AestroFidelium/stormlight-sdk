//! The transient root — a tree spawned by an authoritative occurrence rather than
//! shown while a predicate holds (stormlight/server#93).
//!
//! Every condition a root could name before this was *a state that lasts*: the
//! player has a unit, a tier is waiting, the cursor is over something. Floating
//! combat text is none of those — it is one instance per hit, over the unit that
//! was hit, alive for a declared moment and then gone. These pin what that has to
//! mean in the ABI:
//!   - **The trigger is inert to structure.** A tree is well-formed for structural
//!     reasons only, so the same tree gets the same verdict whether it is shown
//!     always or spawned by a hit. The one thing an occurrence root adds is its
//!     lifetime, and that is the only new way it can be refused.
//!   - **A lifetime no clock can run is refused** — zero, negative or non-finite.
//!     A popup that lives no time is not a subtle popup, it is one the author will
//!     never see and never be told about.
//!   - **Round-trip**: the trigger, its lifetime and its merge rule survive the
//!     wasm boundary and re-serialize to identical bytes.
//!   - **Tag stability**: the ABI is unversioned, so the new variants may only be
//!     *appended* — `RootVisibility::OnEvent` after the three conditions,
//!     `ValueBinding::Event` after the five unit quantities. Everything a HUD
//!     authored before this issue keeps the tag it had.
//!   - **The amount names no handle.** It is the occurrence's own number, so
//!     adoption has nothing to rewrite — unlike a pool or a stat.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::impacts::PoolRef;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::ui::{
    Anchor, Flow, Layout, Length, RootVisibility, Shown, Strip, Style, SummonGate, TextSource,
    UiError, UiRoot, UiSubject, ValueBinding, ValuePart, Widget, WidgetKind,
};
use stormlight_mod_abi::ui_event::{Coalesce, EventQuantity, UiEvent};

extern crate alloc;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

/// An id space that translates nothing at all.
///
/// The sharpest possible probe for "this descriptor names no handle": anything
/// that reaches for a family under this map fails, so a remap that *succeeds* has
/// demonstrably touched none of them.
struct RefuseEverything;

macro_rules! refuse {
    ($($family:ident => $handle:ty),* $(,)?) => {
        impl IdMap for RefuseEverything {
            type Error = ();
            $(fn $family(&self, _: $handle) -> Result<$handle, ()> { Err(()) })*
        }
    };
}

refuse! {
    stat => StatId,
    resource => ResourceId,
    stack => StackId,
    tag => TagId,
    tag_class => TagClassId,
    param => ParamId,
    event => EventId,
    buff => BuffId,
    curve => CurveId,
    damage_type => DamageTypeId,
    ability => AbilityId,
    talent => TalentId,
    handler => HandlerId,
    unit => UnitId,
    navmesh => NavMeshId,
    anim_state => AnimStateId,
}

/// Which occurrence a scenario listens for.
#[derive(Debug, TypeGenerator, Clone, Copy, PartialEq, Eq)]
enum Which {
    Damaged,
    Healed,
}

impl Which {
    fn event(self) -> UiEvent {
        match self {
            Self::Damaged => UiEvent::Damaged,
            Self::Healed => UiEvent::Healed,
        }
    }
}

/// Which merge rule it declares.
#[derive(Debug, TypeGenerator, Clone, Copy, PartialEq, Eq)]
enum Merge {
    Never,
    PerCause,
    PerUnit,
}

impl Merge {
    fn rule(self) -> Coalesce {
        match self {
            Self::Never => Coalesce::Never,
            Self::PerCause => Coalesce::PerCause,
            Self::PerUnit => Coalesce::PerUnit,
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    which: Which,
    merge: Merge,
    /// The declared lifetime, generated raw — so NaN, infinity, zero and negative
    /// windows all reach `validate` the way a hostile descriptor would.
    seconds: f32,
    /// Empty for some cases: the malformed root the verdict invariant needs, a
    /// break the trigger must neither cause nor cure.
    name_len: u8,
    /// How many children the tree carries, so the walk is not always trivial.
    children: u8,
}

/// A popup: the number it prints, and however many siblings the scenario asked
/// for.
fn a_tree(children: u8) -> Widget {
    let kids: Vec<Widget> = (0..u16::from(children) % 4)
        .map(|_| Widget {
            name: "total".to_string(),
            layout: Layout::default(),
            style: Style::default(),
            kind: WidgetKind::Text {
                text: TextSource::Value {
                    binding: ValueBinding::Event(EventQuantity::Total),
                    part: ValuePart::Current,
                    decimals: 0,
                },
            },
        })
        .collect();
    Widget {
        name: "popup".to_string(),
        layout: Layout {
            anchor: Anchor::BottomCenter,
            offset: [Length::Px(0.0), Length::Px(-48.0)],
            size: [Length::Auto, Length::Auto],
            flow: Flow::Column,
            gap: 0.0,
            padding: 0.0,
            shown: Shown::Always,
            layer: 0,
        },
        style: Style::default(),
        kind: WidgetKind::Panel { children: kids },
    }
}

fn a_root(s: &Scenario, when: RootVisibility) -> UiRoot {
    UiRoot {
        name: "x".repeat(usize::from(s.name_len % 3)),
        when,
        summon: SummonGate::Ignored,
        // The unit the occurrence named — the same resolution a nameplate uses,
        // which is why a transient reuses it rather than declaring a subject of
        // its own.
        subject: UiSubject::EachUnit,
        strip: Strip::default(),
        root: a_tree(s.children),
    }
}

fn on_event(s: &Scenario, seconds: f32) -> RootVisibility {
    RootVisibility::OnEvent { event: s.which.event(), seconds, coalesce: s.merge.rule() }
}

#[test]
fn a_runnable_lifetime_leaves_the_trees_verdict_alone() {
    check!().with_type::<Scenario>().for_each(|s| {
        let baseline = a_root(s, RootVisibility::Always).validate();
        // A second of popup: the lifetime is beyond reproach, so anything the
        // verdict says is about the tree.
        let verdict = a_root(s, on_event(s, 1.0)).validate();
        assert_eq!(
            verdict, baseline,
            "an occurrence root changed the verdict of a tree it does not describe",
        );
    });
}

#[test]
fn a_lifetime_no_clock_can_run_is_refused() {
    check!().with_type::<Scenario>().for_each(|s| {
        // A well-named tree, so the only break available is the lifetime itself.
        let mut root = a_root(s, on_event(s, s.seconds));
        root.name = "popup".to_string();
        let refused = matches!(root.validate(), Err(UiError::BadEventLifetime));
        assert_eq!(
            refused,
            !(s.seconds.is_finite() && s.seconds > 0.0),
            "the lifetime rule and the verdict disagree for {}",
            s.seconds,
        );
    });
}

#[test]
fn a_transient_root_survives_the_wire() {
    check!().with_type::<Scenario>().for_each(|s| {
        let root = a_root(s, on_event(s, s.seconds));
        let bytes = postcard::to_allocvec(&root).expect("serialize");
        let back: UiRoot = postcard::from_bytes(&bytes).expect("deserialize");
        // NaN is not equal to itself, so the lifetime is compared by its bits —
        // what actually has to survive is the pattern, not the number's value.
        match (back.when, root.when) {
            (
                RootVisibility::OnEvent { event: a, seconds: x, coalesce: c },
                RootVisibility::OnEvent { event: b, seconds: y, coalesce: d },
            ) => {
                assert_eq!(a, b, "the occurrence did not survive the wire");
                assert_eq!(c, d, "the merge rule did not survive the wire");
                assert_eq!(x.to_bits(), y.to_bits(), "the lifetime did not survive the wire");
            }
            other => panic!("an occurrence root came back as {other:?}"),
        }
        let again = postcard::to_allocvec(&back).expect("reserialize");
        assert_eq!(bytes, again, "serialization is not stable");
    });
}

#[test]
fn the_new_variants_were_appended_never_inserted() {
    check!().with_type::<Scenario>().for_each(|_| {
        // The three conditions that predate the trigger keep their tags, and the
        // trigger takes the next one. An unversioned ABI has no other way to grow.
        for (tag, when) in [
            RootVisibility::Always,
            RootVisibility::WhileTalentPending,
            RootVisibility::WhileUnitHovered,
        ]
        .iter()
        .enumerate()
        {
            let bytes = postcard::to_allocvec(when).expect("serialize");
            assert_eq!(bytes, vec![tag as u8], "{when:?} moved off wire tag {tag}");
        }
        let occurrence = postcard::to_allocvec(&RootVisibility::OnEvent {
            event: UiEvent::Damaged,
            seconds: 0.0,
            coalesce: Coalesce::Never,
        })
        .expect("serialize");
        assert_eq!(occurrence.first(), Some(&3), "the occurrence trigger is not the fourth tag");
        assert_eq!(RootVisibility::default(), RootVisibility::Always);

        // The five quantities a *unit* carries keep theirs, and the occurrence's
        // own number takes the one after them.
        let unit_bound = [
            ValueBinding::Health,
            ValueBinding::Pool(PoolRef::Shield),
            ValueBinding::Stat(StatId(0)),
            ValueBinding::Level,
            ValueBinding::CastProgress,
        ];
        for (tag, binding) in unit_bound.iter().enumerate() {
            let bytes = postcard::to_allocvec(binding).expect("serialize");
            assert_eq!(bytes.first(), Some(&(tag as u8)), "{binding:?} moved off wire tag {tag}");
        }
        let amount =
            postcard::to_allocvec(&ValueBinding::Event(EventQuantity::Latest)).expect("serialize");
        assert_eq!(amount.first(), Some(&5), "the occurrence amount is not the sixth tag");

        // And the three new enums number themselves by declaration order.
        for (tag, event) in [UiEvent::Damaged, UiEvent::Healed].iter().enumerate() {
            let bytes = postcard::to_allocvec(event).expect("serialize");
            assert_eq!(bytes, vec![tag as u8], "{event:?} moved off wire tag {tag}");
        }
        for (tag, rule) in
            [Coalesce::Never, Coalesce::PerCause, Coalesce::PerUnit].iter().enumerate()
        {
            let bytes = postcard::to_allocvec(rule).expect("serialize");
            assert_eq!(bytes, vec![tag as u8], "{rule:?} moved off wire tag {tag}");
        }
        for (tag, part) in [
            EventQuantity::Latest,
            EventQuantity::Total,
            EventQuantity::Hits,
            EventQuantity::Absorbed,
            EventQuantity::Toll,
        ]
        .iter()
        .enumerate()
        {
            let bytes = postcard::to_allocvec(part).expect("serialize");
            assert_eq!(bytes, vec![tag as u8], "{part:?} moved off wire tag {tag}");
        }
    });
}

#[test]
fn the_occurrences_own_number_names_no_handle() {
    check!().with_type::<Scenario>().for_each(|s| {
        // Adoption rewrites every handle a descriptor carries into the global id
        // space. The amount on a popup is the occurrence's own number and belongs
        // to no family at all, so there is nothing here for a shifting id space to
        // touch — the property that lets a HUD print a damage number without the
        // interface mod knowing one thing about the gameplay mod that dealt it.
        for quantity in [
            EventQuantity::Latest,
            EventQuantity::Total,
            EventQuantity::Hits,
            EventQuantity::Absorbed,
            EventQuantity::Toll,
        ] {
            let mut binding = ValueBinding::Event(quantity);
            binding.remap_ids(&RefuseEverything).expect("an amount cannot fail to remap");
            assert_eq!(binding, ValueBinding::Event(quantity), "the amount was rewritten");
        }
        // And the whole tree it sits in, for the same reason.
        let mut root = a_root(s, on_event(s, 1.0));
        root.remap_ids(&RefuseEverything).expect("a popup names no handle");
    });
}
