//! What an interface may say about the hits held on a stopped unit
//! (stormlight/server#230) — the ABI half.
//!
//! Two additions, and each is the general mechanism rather than the one widget the
//! issue had in mind:
//!
//!   - **[`ValueBinding::Held`]**, a number like any other: how many deliveries are
//!     waiting on the subject, or how much damage they carry. A number rather than a
//!     dedicated widget, so it is printable, drawable as a bar and gateable by every
//!     mechanism a number already has;
//!   - **[`Shown::WhilePositive`]**, a gate on *any* binding reading above zero for
//!     the tree's subject. A nameplate's count of held blows is a figure that must
//!     not print `0` over every unit on screen, and that is the same question as a
//!     shield bar that should only exist while there is a shield. One gate answers
//!     both, and every binding added later.
//!
//! A gate that carries a binding carries whatever handle the binding names, so it is
//! held to the two rules every other binding position is: its handles are remapped
//! at adoption, and a relative slot reference in it is refused at load.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{ResourceId, Slot, StatId};
use stormlight_mod_abi::impacts::PoolRef;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::slot_ref::SlotRef;
use stormlight_mod_abi::ui::{
    HeldQuantity, Layout, RootVisibility, Shown, Strip, Style, SummonGate, TextSource, UiError,
    UiRoot, UiSubject, ValueBinding, ValuePart, Widget, WidgetKind,
};

extern crate alloc;
use alloc::string::ToString;
use alloc::vec;

/// Which of the two held numbers a widget reads.
#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Held {
    Count,
    Damage,
}

impl Held {
    fn quantity(self) -> HeldQuantity {
        match self {
            Self::Count => HeldQuantity::Count,
            Self::Damage => HeldQuantity::Damage,
        }
    }
}

/// What a gate is about: any binding a gate could name, including the two that
/// carry an interned handle.
#[derive(Clone, Copy, Debug, TypeGenerator)]
enum GateOn {
    Held(Held),
    Stat(u16),
    Resource(u16),
    Shield,
    Charges(u8),
}

impl GateOn {
    fn binding(self) -> ValueBinding {
        match self {
            Self::Held(held) => ValueBinding::Held(held.quantity()),
            Self::Stat(id) => ValueBinding::Stat(StatId(id)),
            Self::Resource(id) => ValueBinding::Pool(PoolRef::Resource(ResourceId(id))),
            Self::Shield => ValueBinding::Pool(PoolRef::Shield),
            Self::Charges(slot) => ValueBinding::Pool(PoolRef::Charges(SlotRef::At(Slot(slot)))),
        }
    }
}

/// A nameplate a mod might declare: a bar of the damage waiting, and a figure that
/// is only laid out while something is.
#[derive(Debug, TypeGenerator)]
struct Nameplate {
    /// What the bar reads.
    bar: Held,
    /// What the figure prints.
    figure: Held,
    /// What the figure waits on.
    gate: GateOn,
    part: u8,
}

fn part(byte: u8) -> ValuePart {
    match byte % 3 {
        0 => ValuePart::Current,
        1 => ValuePart::Max,
        _ => ValuePart::Fraction,
    }
}

fn node(name: &str, kind: WidgetKind) -> Widget {
    Widget { name: name.to_string(), layout: Layout::default(), style: Style::default(), kind }
}

fn root(s: &Nameplate) -> UiRoot {
    let bar = node("held_bar", WidgetKind::Bar { value: ValueBinding::Held(s.bar.quantity()) });
    let mut figure = node(
        "held_count",
        WidgetKind::Text {
            text: TextSource::Value {
                binding: ValueBinding::Held(s.figure.quantity()),
                part: part(s.part),
                decimals: 0,
            },
        },
    );
    figure.layout.shown = Shown::WhilePositive(s.gate.binding());
    UiRoot {
        name: "nameplate".to_string(),
        when: RootVisibility::Always,
        summon: SummonGate::Ignored,
        subject: UiSubject::EachUnit,
        strip: Strip::default(),
        root: node("plate", WidgetKind::Panel { children: vec![bar, figure] }),
    }
}

/// A nameplate adopted through a map that shifts every interned handle.
#[derive(Debug, TypeGenerator)]
struct Adoption {
    plate: Nameplate,
    shift: u16,
}

/// A map that renames every stat and resource it is given and refuses nothing.
struct Shift(u16);

macro_rules! unchanged {
    ($($method:ident, $ty:path);* $(;)?) => {
        $(fn $method(&self, id: $ty) -> Result<$ty, ()> { Ok(id) })*
    };
}

impl IdMap for Shift {
    type Error = ();
    fn stat(&self, id: StatId) -> Result<StatId, ()> {
        Ok(StatId(id.0.wrapping_add(self.0)))
    }
    fn resource(&self, id: ResourceId) -> Result<ResourceId, ()> {
        Ok(ResourceId(id.0.wrapping_add(self.0)))
    }
    unchanged!(
        stack, stormlight_mod_abi::ids::StackId;
        tag, stormlight_mod_abi::ids::TagId;
        tag_class, stormlight_mod_abi::ids::TagClassId;
        param, stormlight_mod_abi::ids::ParamId;
        event, stormlight_mod_abi::ids::EventId;
        buff, stormlight_mod_abi::ids::BuffId;
        curve, stormlight_mod_abi::ids::CurveId;
        damage_type, stormlight_mod_abi::ids::DamageTypeId;
        ability, stormlight_mod_abi::ids::AbilityId;
        talent, stormlight_mod_abi::ids::TalentId;
        handler, stormlight_mod_abi::ids::HandlerId;
        unit, stormlight_mod_abi::ids::UnitId;
        navmesh, stormlight_mod_abi::ids::NavMeshId;
        anim_state, stormlight_mod_abi::ids::AnimStateId;
    );
}

/// The figure's gate, wherever the tree keeps it.
fn gate_of(root: &UiRoot) -> Shown {
    let WidgetKind::Panel { children } = &root.root.kind else { unreachable!("built as a panel") };
    children[1].layout.shown
}

#[test]
fn a_held_nameplate_survives_the_trip_to_the_host() {
    check!().with_type::<Nameplate>().for_each(|s| {
        let declared = root(s);
        let bytes = postcard::to_allocvec(&declared).expect("a nameplate encodes");
        let decoded: UiRoot = postcard::from_bytes(&bytes).expect("and decodes");
        assert_eq!(decoded, declared, "a held binding or a value gate did not survive the wire");
    });
}

#[test]
fn a_gate_on_a_named_slot_or_any_quantity_is_legal() {
    check!().with_type::<Nameplate>().for_each(|s| {
        assert_eq!(root(s).validate(), Ok(()), "a well-formed value gate was refused");
    });
}

#[test]
fn a_gate_on_a_relative_slot_is_refused_like_every_other_binding() {
    check!().with_type::<Nameplate>().for_each(|s| {
        let mut declared = root(s);
        let WidgetKind::Panel { children } = &mut declared.root.kind else { unreachable!() };
        // `This` names whichever slot an effect runs from, and a widget is not an
        // effect: there is nothing for it to resolve against (server#187). Refused in
        // a gate for the reason it is refused in a bar — the typo would otherwise
        // read exactly like a unit whose state has not arrived.
        children[1].layout.shown =
            Shown::WhilePositive(ValueBinding::Pool(PoolRef::Cooldown(SlotRef::This)));
        assert_eq!(
            declared.validate(),
            Err(UiError::UnboundSlot { widget: 2 }),
            "a gate that can never resolve was let through",
        );
    });
}

#[test]
fn adoption_rewrites_the_handle_a_gate_names() {
    check!().with_type::<Adoption>().for_each(|Adoption { plate: s, shift }| {
        let mut adopted = root(s);
        adopted.remap_ids(&Shift(*shift)).expect("a total map refuses nothing");
        let expected = match s.gate {
            GateOn::Stat(id) => ValueBinding::Stat(StatId(id.wrapping_add(*shift))),
            GateOn::Resource(id) => {
                ValueBinding::Pool(PoolRef::Resource(ResourceId(id.wrapping_add(*shift))))
            }
            // Nothing else a gate here names is interned, so nothing else moves.
            other => other.binding(),
        };
        assert_eq!(
            gate_of(&adopted),
            Shown::WhilePositive(expected),
            "a gate's handle was left in the declaring mod's id space",
        );
    });
}

#[test]
fn a_value_gate_is_about_no_tier() {
    check!().with_type::<GateOn>().for_each(|on| {
        // The tier gates are answered from the player's own tree; this one is
        // answered per subject, from whatever the tree's subject is. Saying it names
        // no tier is what keeps the two passes from both claiming it.
        assert_eq!(Shown::WhilePositive(on.binding()).tier(), None);
    });
}
