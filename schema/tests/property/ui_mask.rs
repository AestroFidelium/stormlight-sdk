//! A picture may be cut to a shape the mod supplies (stormlight/server#121).
//!
//! Content art is square. The sockets a HUD draws around it are not — they are
//! six-sided plates, and a square picture drawn at the socket's full size hangs out
//! of every corner. Insetting the picture until the corners fit shrinks it well
//! inside the plate and leaves a gap all round, which reads worse than the spill.
//!
//! So [`Style::mask`] is a second picture whose **alpha multiplies** the drawn
//! one's. It is a shape, exactly as [`Sweep`](stormlight_mod_abi::ui::Sweep) is a
//! shape: the engine knows how to multiply two samples, and which shape, for which
//! widget, is entirely the mod's. Nothing in the ABI knows what a socket looks like.
//!
//! What is pinned here is the declaration, not the drawing:
//!
//!   - **saying nothing is the good case** — a style that declares no mask is a
//!     style that cuts nothing, so every widget written before this field says what
//!     it always said;
//!   - **it survives the wire**, like every other `mod://` path;
//!   - **an empty path is refused at load**, at the widget that carries it, for the
//!     same reason an empty image path is: a URL naming nothing is a typo, and one
//!     that only shows up as a picture quietly not being cut is a typo nobody finds;
//!   - **it is independent of the picture**. A mask is a property of the *widget*
//!     rather than of the file underneath it — a slot's picture is resolved at run
//!     time from whatever ability is bound to it, and the shape of the socket must
//!     not have to be restated per ability.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ui::{
    Layout, RootVisibility, Strip, Style, SummonGate, UiError, UiRoot, Widget, WidgetKind,
};

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec;

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// Whether the widget declares a mask, and whether the path is a real one.
    mask: Option<bool>,
    /// Whether it declares a picture to cut.
    pictured: bool,
}

impl Scenario {
    fn style(&self) -> Style {
        Style {
            image: self.pictured.then(|| "mod://pack/icon.png".to_string()),
            mask: self.mask.map(|named| {
                if named { "mod://pack/socket.png".to_string() } else { String::new() }
            }),
            ..Style::default()
        }
    }

    fn root(&self) -> UiRoot {
        UiRoot {
            name: "hud".to_string(),
            when: RootVisibility::Always,
            summon: SummonGate::Ignored,
            subject: stormlight_mod_abi::ui::UiSubject::LocalPlayer,
            strip: Strip::default(),
            root: Widget {
                name: "socket".to_string(),
                layout: Layout::default(),
                style: self.style(),
                kind: WidgetKind::Panel { children: vec![] },
            },
        }
    }
}

/// The whole reason the field is an `Option` rather than a path with an "uncut"
/// sentinel: almost every widget in a HUD wants no mask, and none of them should
/// have to say so.
#[test]
fn a_style_that_says_nothing_cuts_nothing() {
    assert_eq!(Style::default().mask, None, "the neutral style cuts the pictures drawn under it");
}

#[test]
fn a_declared_mask_survives_the_wire() {
    check!().with_type::<Scenario>().for_each(|s| {
        let style = s.style();
        let bytes = postcard::to_allocvec(&style).expect("a style must serialize");
        let back: Style = postcard::from_bytes(&bytes).expect("a style must deserialize");
        assert_eq!(back.mask, style.mask, "the declared mask did not survive the trip");
    });
}

/// A mask that names nothing is a typo, and the only symptom of one is a picture
/// quietly not being cut — which looks exactly like the fault this field exists to
/// fix. Refused where every other empty `mod://` path is refused.
#[test]
fn a_mask_naming_nothing_is_refused_at_the_widget_that_carries_it() {
    check!().with_type::<Scenario>().for_each(|s| {
        let outcome = s.root().validate();
        match s.mask {
            Some(false) => assert!(
                matches!(outcome, Err(UiError::EmptyImagePath { widget: 0 })),
                "a mask naming nothing was accepted: {outcome:?}",
            ),
            _ => assert!(outcome.is_ok(), "a well-formed mask was refused: {outcome:?}"),
        }
    });
}

/// The mask is the widget's, not the picture's. A slot's picture comes from
/// whichever ability is bound to it and changes as the loadout does; the socket it
/// is cut to belongs to the plate it is drawn in and never changes at all. Stating
/// them on one style is what lets a HUD declare the shape once.
#[test]
fn a_mask_may_be_declared_without_a_picture_to_cut() {
    check!().with_type::<Scenario>().for_each(|s| {
        let bare = Scenario { pictured: false, mask: Some(true) };
        assert!(
            bare.root().validate().is_ok(),
            "a widget declaring the shape of its socket and no picture was refused",
        );
        let _ = s;
    });
}
