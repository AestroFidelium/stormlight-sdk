//! What a declared widget *does*, and what it looks like while it is being done
//! (stormlight/server#69).
//!
//! G1 gave the ABI a [`UiAction`] nothing read. This pins the two properties the
//! interaction slice rests on, both of which are about the descriptor alone —
//! neither needs a client, a server or a wire.
//!
//! ## Every interactive widget names its action in one place
//!
//! Two kinds are clickable and they arrive at their action differently: a
//! [`WidgetKind::Button`] carries one explicitly, while a
//! [`WidgetKind::AbilitySlot`] *is* a cast of the slot it shows. The client must
//! not be the thing that knows that — a second reading of "what does this widget
//! do" is exactly how a bar clicked with the mouse drifts from the same bar
//! pressed with a key. [`WidgetKind::action`] is that single reading, and the
//! property is that it is total: a leaf never acts, an ability slot always casts
//! its own slot, and a button always reports the action it was declared with.
//!
//! ## A state override is an override, not a replacement
//!
//! [`StateStyle`] fields are optional one at a time, so a mod that recolours a
//! button on hover keeps its declared picture, and one that swaps the picture
//! keeps its declared colours. The property is directional: resolving a state
//! changes exactly the properties that state declared and leaves every other one
//! byte-identical to the base [`Style`].

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{EventId, Slot};
use stormlight_mod_abi::ui::{
    InteractionStyle, Layout, StateStyle, Style, TextSource, UiAction, Widget, WidgetKind,
    WidgetState,
};

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// Which clickable kind a scenario builds.
#[derive(Debug, TypeGenerator, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A button carrying a cast action.
    ButtonCast(u8),
    /// A button carrying a talent pick.
    ButtonPick { tier: u8, option: u8 },
    /// A button carrying a mod-defined trigger.
    ButtonTrigger(u16),
    /// The blessed ability-slot composite.
    Slot(u8),
    /// A container — declared as clickable by nobody.
    Panel,
    /// A leaf.
    Text,
}

fn a_widget(kind: WidgetKind) -> Widget {
    Widget { name: "w".to_string(), layout: Layout::default(), style: Style::default(), kind }
}

fn build(kind: Kind) -> Widget {
    a_widget(match kind {
        Kind::ButtonCast(slot) => {
            WidgetKind::Button { action: UiAction::CastSlot(Slot(slot)), children: Vec::new() }
        }
        Kind::ButtonPick { tier, option } => {
            WidgetKind::Button { action: UiAction::PickTalent { tier, option }, children: vec![] }
        }
        Kind::ButtonTrigger(event) => WidgetKind::Button {
            action: UiAction::Trigger { event: EventId(event) },
            children: Vec::new(),
        },
        Kind::Slot(slot) => WidgetKind::AbilitySlot { slot: Slot(slot), key_hint: "Q".to_string() },
        Kind::Panel => WidgetKind::Panel { children: Vec::new() },
        Kind::Text => WidgetKind::Text { text: TextSource::Literal("x".to_string()) },
    })
}

#[test]
fn only_the_clickable_kinds_report_an_action_and_a_slot_casts_its_own_slot() {
    check!().with_type::<Kind>().for_each(|&kind| {
        let widget = build(kind);
        match kind {
            Kind::ButtonCast(slot) => {
                assert_eq!(widget.kind.action(), Some(UiAction::CastSlot(Slot(slot))));
            }
            Kind::ButtonPick { tier, option } => {
                assert_eq!(widget.kind.action(), Some(UiAction::PickTalent { tier, option }));
            }
            Kind::ButtonTrigger(event) => {
                assert_eq!(widget.kind.action(), Some(UiAction::Trigger { event: EventId(event) }));
            }
            // The composite acts as a cast of the very slot it draws — the one
            // place that equivalence is stated.
            Kind::Slot(slot) => {
                assert_eq!(widget.kind.action(), Some(UiAction::CastSlot(Slot(slot))));
            }
            Kind::Panel | Kind::Text => assert_eq!(widget.kind.action(), None),
        }
    });
}

/// Which properties a generated state override declares.
#[derive(Debug, TypeGenerator, Clone, Copy, PartialEq, Eq)]
struct Overrides {
    color: bool,
    background: bool,
    image: bool,
}

#[derive(Debug, TypeGenerator)]
struct StateScenario {
    over: Overrides,
    state: GenState,
}

/// A generatable mirror of [`WidgetState`] (the ABI type is not a generator).
#[derive(Debug, TypeGenerator, Clone, Copy, PartialEq, Eq)]
enum GenState {
    Idle,
    Hovered,
    Pressed,
    Disabled,
}

impl GenState {
    fn state(self) -> WidgetState {
        match self {
            Self::Idle => WidgetState::Idle,
            Self::Hovered => WidgetState::Hovered,
            Self::Pressed => WidgetState::Pressed,
            Self::Disabled => WidgetState::Disabled,
        }
    }
}

/// A base a scenario measures its overrides against — every property distinct, so
/// a leak from one into another is visible.
fn a_base() -> Style {
    Style {
        color: [0.1, 0.2, 0.3, 1.0],
        background: [0.4, 0.5, 0.6, 1.0],
        image: Some("mod://m/base.png".to_string()),
        ..Style::default()
    }
}

fn an_override(over: Overrides) -> StateStyle {
    StateStyle {
        color: over.color.then_some([0.9, 0.8, 0.7, 1.0]),
        background: over.background.then_some([0.7, 0.6, 0.5, 1.0]),
        image: over.image.then(|| "mod://m/state.png".to_string()),
    }
}

#[test]
fn a_state_overrides_only_what_it_declares() {
    check!().with_type::<StateScenario>().for_each(|s| {
        let base = a_base();
        let over = an_override(s.over);
        let mut style = base.clone();
        let state = s.state.state();
        match state {
            WidgetState::Idle => {}
            WidgetState::Hovered => style.states.hover = Some(over.clone()),
            WidgetState::Pressed => style.states.press = Some(over.clone()),
            WidgetState::Disabled => style.states.disabled = Some(over.clone()),
        }

        let resolved = style.resolve(state);

        // Idle never consults the table at all: the declared style *is* the
        // resting appearance, and a state that overrode it would be a widget the
        // author cannot see the base of.
        if state == WidgetState::Idle {
            assert_eq!(resolved.color, base.color);
            assert_eq!(resolved.background, base.background);
            assert_eq!(resolved.image, base.image);
            return;
        }

        // Directional: a declared property moves, an undeclared one does not.
        assert_eq!(
            resolved.color,
            if s.over.color { over.color.unwrap() } else { base.color },
            "colour override {:?}",
            s.over
        );
        assert_eq!(
            resolved.background,
            if s.over.background { over.background.unwrap() } else { base.background },
        );
        assert_eq!(
            resolved.image,
            if s.over.image { over.image.clone() } else { base.image.clone() },
        );
        // Properties no state can reach are untouched whatever was declared.
        assert_eq!(resolved.border, base.border);
        assert_eq!(resolved.font_size, base.font_size);
    });
}

#[test]
fn a_widget_that_declares_no_state_looks_the_same_in_every_one() {
    check!().with_type::<GenState>().for_each(|&state| {
        let base = a_base();
        assert_eq!(base.states, InteractionStyle::default());
        let resolved = base.resolve(state.state());
        assert_eq!(resolved.color, base.color);
        assert_eq!(resolved.background, base.background);
        assert_eq!(resolved.image, base.image);
    });
}

#[test]
fn an_undeclared_state_falls_back_to_the_base_even_when_others_are_declared() {
    check!().with_type::<GenState>().for_each(|&state| {
        // Only `hover` is declared: press and disabled must not borrow it. A
        // button that darkened on press *because* it had a hover style would be
        // the engine inventing an appearance the mod never asked for.
        let mut style = a_base();
        style.states.hover =
            Some(StateStyle { color: Some([1.0, 0.0, 0.0, 1.0]), background: None, image: None });
        let resolved = style.resolve(state.state());
        let expected = if state == GenState::Hovered { [1.0, 0.0, 0.0, 1.0] } else { style.color };
        assert_eq!(resolved.color, expected);
    });
}

/// A state image that is present but empty is the same break as a base one — a
/// `mod://` URL to nothing — and must be reported, not drawn as a blank.
#[test]
fn an_empty_state_image_is_not_silently_accepted() {
    check!().with_type::<GenState>().for_each(|&state| {
        let mut style = a_base();
        let empty = StateStyle { color: None, background: None, image: Some(String::new()) };
        match state.state() {
            WidgetState::Idle => return,
            WidgetState::Hovered => style.states.hover = Some(empty),
            WidgetState::Pressed => style.states.press = Some(empty),
            WidgetState::Disabled => style.states.disabled = Some(empty),
        }
        assert!(style.states.has_empty_image(), "an empty state image must be detectable");
    });
}
