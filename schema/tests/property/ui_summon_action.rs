//! Asking for the interface without holding a key (stormlight/server#107).
//!
//! The summon gate (server#98) made a tree wait on an input the player *holds*, and
//! that is the right shape for a peek: it is level-triggered, it cannot get stuck,
//! and letting go always ends it. It is the wrong shape for a panel a player reads
//! for several seconds while clicking things in it with the other hand.
//!
//! [`UiAction::Summon`] is the other half. A declared widget — a portrait, a button
//! on a bar — can latch the same interface open, and a tree is summoned while
//! *either* the key or the latch says so. Which key that is stays the client's and a
//! mod still never learns it; this says only that the interface is wanted, which is
//! the same thing the key says.
//!
//! What is pinned:
//!
//!   - **it is an action**, so `WidgetKind::action` stays the single reading of what
//!     a widget does — the reason `SelectTier` is in that enum too;
//!   - **it names nothing**, so remapping a cosmetic bundle leaves it alone. Every
//!     other id in this ABI belongs to some family; a summon belongs to none;
//!   - **all three requests survive the wire**, including `Close`, which exists
//!     precisely so a button that dismisses cannot reopen by being clicked twice.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ui::{
    Layout, RootVisibility, Strip, Style, SummonRequest, UiAction, UiRoot, UiSubject, Widget,
    WidgetKind,
};

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// One button per entry, each asking for one of the three.
    requests: Vec<u8>,
}

fn request(byte: u8) -> SummonRequest {
    match byte % 3 {
        0 => SummonRequest::Open,
        1 => SummonRequest::Close,
        _ => SummonRequest::Toggle,
    }
}

fn a_root(s: &Scenario) -> UiRoot {
    let children = s
        .requests
        .iter()
        .enumerate()
        .map(|(i, &byte)| Widget {
            name: format!("summon{i}"),
            layout: Layout::default(),
            style: Style::default(),
            kind: WidgetKind::Button {
                action: UiAction::Summon(request(byte)),
                children: Vec::new(),
            },
        })
        .collect();
    UiRoot {
        name: "console".into(),
        when: RootVisibility::Always,
        summon: stormlight_mod_abi::ui::SummonGate::Ignored,
        subject: UiSubject::LocalPlayer,
        strip: Strip::default(),
        root: Widget {
            name: "console".into(),
            layout: Layout::default(),
            style: Style::default(),
            kind: WidgetKind::Panel { children },
        },
    }
}

#[test]
fn a_summon_button_reports_what_it_asks_for() {
    check!().with_type::<Scenario>().for_each(|s| {
        let root = a_root(s);
        let WidgetKind::Panel { children } = &root.root.kind else { panic!("a panel") };
        for (i, child) in children.iter().enumerate() {
            assert_eq!(
                child.kind.action(),
                Some(UiAction::Summon(request(s.requests[i]))),
                "a summon button does not report the request it carries, so the widget \
                 the pointer lights up and the one a click fires can disagree",
            );
        }
    });
}

#[test]
fn a_summon_survives_the_trip_to_the_host() {
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = a_root(s);
        let bytes = postcard::to_allocvec(&declared).expect("a console encodes");
        let decoded: UiRoot = postcard::from_bytes(&bytes).expect("and decodes");
        assert_eq!(decoded, declared, "a summon request did not survive the wire");
        assert_eq!(decoded.validate(), Ok(()), "a summon button does not render as declared");
    });
}

/// The wire tag of every variant that existed before this one, which is its
/// position in the enum. Appending is the only safe way to grow it.
#[test]
fn the_new_variant_was_appended_never_inserted() {
    let tag = |action: UiAction| {
        postcard::to_allocvec(&action).expect("an action encodes").first().copied()
    };
    assert_eq!(tag(UiAction::CastSlot(stormlight_mod_abi::ids::Slot(0))), Some(0));
    assert_eq!(tag(UiAction::PickTalent { tier: 0, option: 0 }), Some(1));
    assert_eq!(tag(UiAction::SelectTier(0)), Some(2));
    assert_eq!(tag(UiAction::Trigger { event: stormlight_mod_abi::ids::EventId(0) }), Some(3));
    assert_eq!(
        tag(UiAction::Summon(SummonRequest::Open)),
        Some(4),
        "the summon was slotted in among the actions that were already on the wire",
    );
    assert_eq!(
        tag(UiAction::PrepickTalent { tier: 0, option: 0 }),
        Some(5),
        "the mark (server#133) was slotted in among the actions that were already on the wire",
    );
}
