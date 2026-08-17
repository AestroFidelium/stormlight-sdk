//! The cooldown sweep a slot declares (stormlight/server#99).
//!
//! [`Sweep`] is the first thing this ABI declares that is a *shape* rather than a
//! box: the interpreter draws the unelapsed part of a cooldown as a wedge, so the
//! only questions left for an author are what colour it is and which way round it
//! goes. Everything pinned here is about that pair surviving the trip and meaning
//! the same thing at both ends:
//!
//!   - **A slot that says nothing still shows its cooldown.** The default is drawn
//!     and is see-through, because a scrim that hides the icon leaves the player
//!     waiting on an ability they can no longer identify. This is the invariant the
//!     whole "declarable at all" decision rests on: declaring nothing must be the
//!     good case, or every mod is obliged to restate it.
//!   - **Invisible is off.** One field decides whether there is an overlay, so a
//!     HUD that draws its cooldowns some other way cannot end up with a scrim it
//!     did not ask for *and* a number that disagrees with it.
//!   - **It survives the wire.** Style crosses the wasm boundary with everything
//!     else; a sweep that round-tripped to a different colour would be a HUD that
//!     looks different in the client than in the mod's own tests.
//!   - **A non-finite colour is refused at load**, at the widget that carries it —
//!     the same as every other colour, because it reaches a shader uniform where a
//!     NaN is not a wrong colour but an undefined pixel.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ui::{
    Layout, RootVisibility, Style, SummonGate, Sweep, SweepDirection, UiError, UiRoot, Widget,
    WidgetKind,
};

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec;

/// A generated colour: bytes, so every run is finite and in range, and the alpha
/// is reachable at both ends of its range.
#[derive(Debug, TypeGenerator)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Rgba {
    fn linear(&self) -> [f32; 4] {
        let of = |v: u8| f32::from(v) / 255.0;
        [of(self.r), of(self.g), of(self.b), of(self.a)]
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    color: Rgba,
    counter: bool,
}

impl Scenario {
    fn sweep(&self) -> Sweep {
        Sweep {
            color: self.color.linear(),
            direction: if self.counter {
                SweepDirection::CounterClockwise
            } else {
                SweepDirection::Clockwise
            },
        }
    }
}

/// A one-slot tree carrying `style`, which is what the host validates.
fn a_root(style: Style) -> UiRoot {
    UiRoot {
        name: "hud".to_string(),
        when: RootVisibility::Always,
        summon: SummonGate::Ignored,
        subject: stormlight_mod_abi::ui::UiSubject::LocalPlayer,
        root: Widget {
            name: "bar".to_string(),
            layout: Layout::default(),
            style: Style::default(),
            kind: WidgetKind::Panel {
                children: vec![Widget {
                    name: "slot".to_string(),
                    layout: Layout::default(),
                    style,
                    kind: WidgetKind::AbilitySlot {
                        slot: stormlight_mod_abi::ids::Slot(0),
                        key_hint: String::new(),
                    },
                }],
            },
        },
    }
}

#[test]
fn a_slot_that_declares_nothing_still_shows_a_cooldown_through_its_icon() {
    let default = Sweep::default();
    assert!(
        default.is_drawn(),
        "the default sweep is invisible, so every slot of every mod that did not \
         think about cooldowns silently stopped showing them",
    );
    assert!(
        default.color[3] < 1.0,
        "the default scrim is opaque, so a slot on cooldown hides the ability the \
         player is waiting for",
    );
}

#[test]
fn a_sweep_is_drawn_exactly_when_it_can_be_seen() {
    check!().with_type::<Scenario>().for_each(|s| {
        let sweep = s.sweep();
        assert_eq!(
            sweep.is_drawn(),
            sweep.color[3] > 0.0,
            "'is there an overlay' and 'can it be seen' must be the same question",
        );
    });
}

#[test]
fn a_declared_sweep_survives_the_wasm_boundary_unchanged() {
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = a_root(Style { sweep: s.sweep(), ..Style::default() });
        let bytes = postcard::to_allocvec(&declared).expect("a ui root serializes");
        let back: UiRoot = postcard::from_bytes(&bytes).expect("and comes back");
        assert_eq!(back, declared, "the sweep did not survive the trip the descriptor took");
        assert_eq!(
            postcard::to_allocvec(&back).expect("re-serializes"),
            bytes,
            "re-serializing a decoded sweep produced different bytes",
        );
    });
}

#[test]
fn a_non_finite_sweep_colour_is_refused_at_the_widget_that_carries_it() {
    let broken = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
    for (channel, bad) in (0..4).flat_map(|c| broken.iter().map(move |b| (c, *b))) {
        let mut color = [0.0, 0.0, 0.0, 0.6];
        color[channel] = bad;
        let declared =
            a_root(Style { sweep: Sweep { color, ..Sweep::default() }, ..Style::default() });
        assert_eq!(
            declared.validate(),
            // Index 1: the root panel is 0, the slot under it is 1.
            Err(UiError::NonFinite { widget: 1 }),
            "a {bad} in channel {channel} of a sweep reached the renderer",
        );
    }
}
