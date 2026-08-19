//! Which tier a talent panel is looking at (stormlight/server#104).
//!
//! Every talent declaration the ABI had addressed a cell by a **literal**
//! `(tier, option)` coordinate, so a panel could only ever lay out every tier at
//! once. A talent panel is read one tier at a time, which needs two things the
//! descriptor could not say: *page to that tier*, and *this part of me belongs to
//! that page*.
//!
//! | declaration | says |
//! | --- | --- |
//! | [`UiAction::SelectTier`] | page this client's panel to tier `t` |
//! | [`Shown::WhileTierSelected`] | lay this widget out only while it is on tier `t` |
//!
//! ## The action that asks the server for nothing
//!
//! The other three [`UiAction`]s are *requests* the server validates. This one is
//! not a request at all — it changes which page of a panel this one client is
//! reading and touches nothing else, on either side of the wire. That is a
//! strengthening of the interface ABI's safety property rather than a hole in it:
//! a click could already do nothing a keypress could not, and this one does less.
//! So the property here is that it is **inert to adoption** — nothing in it is a
//! handle, exactly as a pick's coordinate is nothing but an index.
//!
//! ## A gate is layout, not identity
//!
//! [`Shown`] lives on [`Layout`], beside the anchor and the size, because "does
//! this node take part in the layout" is the same kind of question as "where does
//! it sit". The properties that follow from that placement are the ones worth
//! pinning: a gate never changes what a widget *is* or what it *does*, and a
//! declaration written before the gate existed means [`Shown::Always`] — the
//! default has to be the whole HUD, or every widget of every mod disappears.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::ui::{
    Layout, RootVisibility, Shown, Style, SummonGate, TextSource, UiAction, UiRoot, UiSubject,
    Widget, WidgetKind,
};

extern crate alloc;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

/// Every family fails, so a declaration that survives it names no interned handle.
///
/// `pub(crate)` because the sibling coordinate-gate file
/// ([`ui_option_gate`](super::ui_option_gate)) asks the same question of the same
/// enum, and one implementation that fails everything is worth more than two that
/// might drift apart about what "everything" is.
pub(crate) struct FailEverything;

impl IdMap for FailEverything {
    type Error = ();
    fn stat(&self, _: StatId) -> Result<StatId, ()> {
        Err(())
    }
    fn resource(&self, _: ResourceId) -> Result<ResourceId, ()> {
        Err(())
    }
    fn stack(&self, _: StackId) -> Result<StackId, ()> {
        Err(())
    }
    fn tag(&self, _: TagId) -> Result<TagId, ()> {
        Err(())
    }
    fn tag_class(&self, _: TagClassId) -> Result<TagClassId, ()> {
        Err(())
    }
    fn param(&self, _: ParamId) -> Result<ParamId, ()> {
        Err(())
    }
    fn event(&self, _: EventId) -> Result<EventId, ()> {
        Err(())
    }
    fn buff(&self, _: BuffId) -> Result<BuffId, ()> {
        Err(())
    }
    fn curve(&self, _: CurveId) -> Result<CurveId, ()> {
        Err(())
    }
    fn damage_type(&self, _: DamageTypeId) -> Result<DamageTypeId, ()> {
        Err(())
    }
    fn ability(&self, _: AbilityId) -> Result<AbilityId, ()> {
        Err(())
    }
    fn talent(&self, _: TalentId) -> Result<TalentId, ()> {
        Err(())
    }
    fn handler(&self, _: HandlerId) -> Result<HandlerId, ()> {
        Err(())
    }
    fn unit(&self, _: UnitId) -> Result<UnitId, ()> {
        Err(())
    }
    fn navmesh(&self, _: NavMeshId) -> Result<NavMeshId, ()> {
        Err(())
    }
    fn anim_state(&self, _: AnimStateId) -> Result<AnimStateId, ()> {
        Err(())
    }
}

/// One part of a declared panel: which tier it pages to, and which tier it belongs
/// to. Both coordinates are generated independently so a scenario can gate a
/// widget on a page other than the one it selects — which is the ordinary case,
/// since a tier button is on screen whatever page the panel is showing.
#[derive(Debug, TypeGenerator, Clone, Copy)]
struct Part {
    /// The tier this part's button pages to.
    selects: u8,
    /// The tier its row is laid out on, or `None` for a part shown on every page.
    gate: Option<u8>,
}

fn a_widget(kind: WidgetKind) -> Widget {
    Widget { name: "w".to_string(), layout: Layout::default(), style: Style::default(), kind }
}

/// A tier button paging to `selects`, with a row under it gated as `gate` says.
fn build(part: Part) -> Widget {
    let mut row = a_widget(WidgetKind::Text { text: TextSource::Literal("row".to_string()) });
    row.layout.shown = part.gate.map_or(Shown::Always, Shown::WhileTierSelected);
    a_widget(WidgetKind::Button { action: UiAction::SelectTier(part.selects), children: vec![row] })
}

fn a_panel(parts: &[Part]) -> UiRoot {
    UiRoot {
        name: "talents".to_string(),
        when: RootVisibility::WhileTalentPending,
        summon: SummonGate::Held,
        subject: UiSubject::LocalPlayer,
        root: a_widget(WidgetKind::Panel {
            children: parts.iter().copied().map(build).collect::<Vec<_>>(),
        }),
    }
}

#[test]
fn paging_to_a_tier_names_no_interned_handle() {
    check!().with_type::<Vec<Part>>().for_each(|parts| {
        let declared = a_panel(parts);
        let mut out = declared.clone();
        out.remap_ids(&FailEverything).expect("a tier index is not a handle");
        assert_eq!(declared, out, "adoption rewrote a tier index or a layout gate");
    });
}

#[test]
fn a_paged_panel_survives_a_postcard_round_trip() {
    check!().with_type::<Vec<Part>>().for_each(|parts| {
        let declared = a_panel(parts);
        let bytes = postcard::to_allocvec(&declared).expect("serialize");
        let back: UiRoot = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(declared, back, "a paged talent panel did not round-trip");
        let again = postcard::to_allocvec(&back).expect("reserialize");
        assert_eq!(bytes, again, "paged panel serialization is not stable");
    });
}

/// The single reading of "what does this widget do" has to answer for the new
/// action too, or the pass that fires widgets never sees it.
#[test]
fn a_button_reports_the_tier_it_pages_to() {
    check!().with_type::<Part>().for_each(|&part| {
        let widget = build(part);
        assert_eq!(widget.kind.action(), Some(UiAction::SelectTier(part.selects)));
    });
}

/// A gate is on the layout, so it may not reach what the widget is or does. If it
/// ever does, some appearance has become identity and a hidden row would stop
/// being the same row when it comes back.
#[test]
fn a_gate_changes_neither_what_a_widget_is_nor_what_it_does() {
    check!().with_type::<Part>().for_each(|&part| {
        // The gate goes on the button's *own* layout here — the same widget either
        // way, so anything that differs is the gate having leaked out of the
        // layout it was declared in.
        let ungated = a_widget(WidgetKind::Button {
            action: UiAction::SelectTier(part.selects),
            children: Vec::new(),
        });
        let mut gated = ungated.clone();
        gated.layout.shown = part.gate.map_or(Shown::Always, Shown::WhileTierSelected);
        assert_eq!(ungated.kind, gated.kind, "a layout gate reached the widget's kind");
        assert_eq!(ungated.kind.action(), gated.kind.action(), "a layout gate reached the action");
        assert_eq!(ungated.style, gated.style, "a layout gate reached the widget's paint");
        assert_eq!(ungated.name, gated.name, "a layout gate reached the widget's name");
    });
}

/// The default is the whole HUD. Every widget of every mod declared before this
/// existed goes through `Layout::default()`, so anything but `Always` empties the
/// screen.
#[test]
fn an_undeclared_gate_is_always_shown() {
    assert_eq!(Layout::default().shown, Shown::Always);
    check!().with_type::<u8>().for_each(|&tier| {
        assert_eq!(Shown::Always.tier(), None, "the ungated case named a tier");
        assert_eq!(
            Shown::WhileTierSelected(tier).tier(),
            Some(tier),
            "a gate forgot which tier it belongs to",
        );
    });
}

/// A gate is not a break: a panel full of them loads.
#[test]
fn a_gated_panel_validates() {
    check!().with_type::<Vec<Part>>().for_each(|parts| {
        assert_eq!(a_panel(parts).validate(), Ok(()), "a paged talent panel was refused at load");
    });
}
