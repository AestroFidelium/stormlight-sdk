//! A player's own plan for a tier they have not reached (stormlight/server#133).
//!
//! [`OptionState::Recommended`] is the **tree's** opinion and the same for everybody
//! driving that unit. A prepick is one **person's** plan for one match, and every
//! way it differs from the four states around it follows from that:
//!
//!   - it changes no simulation state, so nothing validates it and nothing carries
//!     it — the same footing as paging a panel (server#104) rather than as picking;
//!   - it can be set for a tier the player has **not reached**, which is the whole
//!     reason to have one: a plan stops being a plan the moment it is actionable;
//!   - and therefore **marking and picking must not share a refusal**. A row of a
//!     locked tier is refused a pick and must still accept a mark. The ABI's answer
//!     is that they are two actions on two widgets, so a HUD never reads one click
//!     two ways — which is what this file pins.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ui::{OptionState, Shown, UiAction, WidgetKind};

extern crate alloc;
use alloc::string::ToString;
use alloc::vec;

#[derive(Debug, TypeGenerator)]
struct Scenario {
    tier: u8,
    option: u8,
}

/// The two actions are distinct at every coordinate. A HUD that could not tell them
/// apart would be one where marking a locked tier submits a pick the server refuses.
#[test]
fn marking_and_picking_are_different_requests() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mark = UiAction::PrepickTalent { tier: s.tier, option: s.option };
        let pick = UiAction::PickTalent { tier: s.tier, option: s.option };
        assert_ne!(mark, pick, "a mark and a pick at ({}, {}) are one action", s.tier, s.option);
    });
}

/// A mark is carried by a button, like every other action — so it is declared as a
/// widget of its own inside a row rather than as a second reading of the row.
#[test]
fn a_marking_widget_reports_the_action_it_carries() {
    check!().with_type::<Scenario>().for_each(|s| {
        let kind = WidgetKind::Button {
            action: UiAction::PrepickTalent { tier: s.tier, option: s.option },
            children: vec![],
        };
        assert_eq!(
            kind.action(),
            Some(UiAction::PrepickTalent { tier: s.tier, option: s.option }),
            "a marking button did not report its own action",
        );
        let _ = "".to_string();
    });
}

/// The mark reads back as a coordinate gate, in the same family as the four states
/// beside it — so a HUD draws it with the machinery it already has rather than with
/// a fifth kind of condition.
#[test]
fn a_mark_is_a_coordinate_gate_like_its_neighbours() {
    check!().with_type::<Scenario>().for_each(|s| {
        let gate =
            Shown::WhileOption { tier: s.tier, option: s.option, is: OptionState::Prepicked };
        assert_eq!(gate.tier(), Some(s.tier), "a mark's gate does not answer for its own tier");
        // And it survives the trip, like every other declaration.
        let bytes = postcard::to_allocvec(&gate).expect("a gate must serialize");
        let back: Shown = postcard::from_bytes(&bytes).expect("a gate must deserialize");
        assert_eq!(back, gate, "a mark's gate did not survive the wire");
    });
}

/// The wire tag of every option state that existed before this one, which is its
/// position in the enum. Appending is the only safe way to grow it — a state slotted
/// in among the others re-reads every gate a mod already shipped as some other
/// condition, and the symptom is a panel that marks the wrong rows.
#[test]
fn the_new_states_were_appended_never_inserted() {
    let tag = |state: OptionState| {
        postcard::to_allocvec(&state).expect("a state encodes").first().copied()
    };
    assert_eq!(tag(OptionState::Offered), Some(0));
    assert_eq!(tag(OptionState::Taken), Some(1));
    assert_eq!(tag(OptionState::PassedOver), Some(2));
    assert_eq!(tag(OptionState::Recommended), Some(3));
    assert_eq!(tag(OptionState::Keyed), Some(4), "the hotkey state (server#129) was inserted");
    assert_eq!(tag(OptionState::Prepicked), Some(5), "the mark (server#133) was inserted");
}
