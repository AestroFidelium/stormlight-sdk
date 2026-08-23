//! What the host refuses to load about a moving interface (stormlight/server#97).
//!
//! The sibling of [`ui_validation`](super::ui_validation), and for the same reason:
//! a break the client absorbs silently leaves the author with a HUD that sits
//! still and nothing to read. Every rejection here names the widget *and the
//! track*, because a widget may declare four curves and "one of them is broken" is
//! not something an author can act on.
//!
//! Invariants:
//!   - **A declaring widget still loads.** The overwhelming majority of a HUD
//!     declares no track at all, and a tree full of ordinary widgets must not start
//!     failing validation because the ABI grew a time axis.
//!   - **Every fault is caught, at the widget and track that carry it** — no keys, a
//!     NaN, a key before the start, keys that go backwards, more keys or tracks than
//!     the interpreter will walk.
//!   - **A negative catch-up is refused**, like every other negative metric: a
//!     transition that runs backwards is not a slower transition.
//!   - **Validation is decided by the descriptor alone** — it never runs a curve, so
//!     a track that is legal loads whatever it would look like on screen.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::Slot;
use stormlight_mod_abi::ui::{
    Layout, RootVisibility, Strip, Style, SummonGate, UiError, UiRoot, UiSubject, Widget,
    WidgetKind,
};
use stormlight_mod_abi::ui_anim::{
    Ease, MAX_UI_KEYS, MAX_UI_TRACKS, Playback, Shape, TrackFault, UiKey, UiProperty, UiTrack,
    UiTransition, UiTrigger,
};

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// A two-key slide — the shape most of a HUD's tracks have.
fn a_track() -> UiTrack {
    UiTrack {
        property: UiProperty::TranslateY,
        on: UiTrigger::Built,
        playback: Playback::Once,
        keys: vec![
            UiKey { time: 0.0, value: 32.0, arrive: Ease::Linear, leave: Ease::Linear },
            UiKey { time: 0.25, value: 0.0, arrive: Ease::Slow, leave: Ease::Linear },
        ],
    }
}

/// A one-widget tree under a plain root, so an error's index is always 1.
fn a_root(style: Style) -> UiRoot {
    UiRoot {
        name: "hud".to_string(),
        when: RootVisibility::Always,
        summon: SummonGate::Ignored,
        subject: UiSubject::LocalPlayer,
        strip: Strip::default(),
        root: Widget {
            name: "panel".to_string(),
            layout: Layout::default(),
            style: Style::default(),
            kind: WidgetKind::Panel {
                children: vec![Widget {
                    name: "moving".to_string(),
                    layout: Layout::default(),
                    style,
                    kind: WidgetKind::AbilitySlot { slot: Slot(0), key_hint: String::new() },
                }],
            },
        },
    }
}

fn styled(anim: Vec<UiTrack>) -> Style {
    Style { anim, ..Style::default() }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    keys: u8,
    tracks: u8,
}

#[test]
fn an_ordinary_moving_widget_loads() {
    check!().with_type::<Scenario>().for_each(|s| {
        let tracks = usize::from(s.tracks) % (MAX_UI_TRACKS + 1);
        let declared = a_root(styled(vec![a_track(); tracks]));
        assert_eq!(
            declared.validate(),
            Ok(()),
            "{tracks} well-formed tracks were refused, so a HUD that moves cannot load",
        );
    });
}

#[test]
fn a_track_with_nothing_in_it_is_refused_at_the_widget_that_declared_it() {
    let empty = UiTrack { keys: Vec::new(), ..a_track() };
    // Declared second, so the reported index has to be the broken one rather than
    // the first track that happened to be walked.
    let declared = a_root(styled(vec![a_track(), empty]));
    assert_eq!(
        declared.validate(),
        Err(UiError::BadTrack { widget: 1, track: 1, fault: TrackFault::NoKeys }),
        "a track that drives nothing loaded, and its widget will simply never move",
    );
}

#[test]
fn a_non_finite_key_never_reaches_a_layout() {
    let broken = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
    for bad in broken {
        for time in [true, false] {
            let mut track = a_track();
            if time {
                track.keys[1].time = bad;
            } else {
                track.keys[1].value = bad;
            }
            assert_eq!(
                a_root(styled(vec![track])).validate(),
                Err(UiError::BadTrack { widget: 1, track: 0, fault: TrackFault::NonFinite }),
                "a {bad} in a key's {} loaded",
                if time { "time" } else { "value" },
            );
        }
    }
}

#[test]
fn a_key_before_the_start_and_keys_that_go_backwards_are_both_refused() {
    let mut early = a_track();
    early.keys[0].time = -0.5;
    assert_eq!(
        a_root(styled(vec![early])).validate(),
        Err(UiError::BadTrack { widget: 1, track: 0, fault: TrackFault::NegativeTime }),
        "a key before the track started loaded",
    );

    let mut backwards = a_track();
    backwards.keys[1].time = 0.0;
    backwards.keys[0].time = 0.5;
    assert_eq!(
        a_root(styled(vec![backwards])).validate(),
        Err(UiError::BadTrack { widget: 1, track: 0, fault: TrackFault::OutOfOrder }),
        "keys whose times go backwards loaded, so which segment a moment belongs \
         to depends on the walk order",
    );
}

#[test]
fn a_track_or_a_widget_past_the_interpreter_s_budget_is_refused() {
    check!().with_type::<u8>().for_each(|&extra| {
        let over = usize::from(extra % 8) + 1;

        let mut long = a_track();
        long.keys = vec![long.keys[0]; MAX_UI_KEYS + over];
        assert_eq!(
            a_root(styled(vec![long])).validate(),
            Err(UiError::BadTrack { widget: 1, track: 0, fault: TrackFault::TooManyKeys }),
            "a track of {} keys loaded; nothing else bounds how much of a tree moves",
            MAX_UI_KEYS + over,
        );

        assert_eq!(
            a_root(styled(vec![a_track(); MAX_UI_TRACKS + over])).validate(),
            Err(UiError::TooManyTracks { widget: 1 }),
            "a widget declaring {} tracks loaded",
            MAX_UI_TRACKS + over,
        );
    });
}

#[test]
fn the_budgets_themselves_are_reachable() {
    // Past the limit, never at it — a mod that fills its budget exactly must load,
    // or the documented number is a lie by one.
    let mut full = a_track();
    full.keys = vec![full.keys[0]; MAX_UI_KEYS];
    assert_eq!(a_root(styled(vec![full; MAX_UI_TRACKS])).validate(), Ok(()));
}

#[test]
fn a_catch_up_that_runs_backwards_is_refused_and_an_instant_one_is_not() {
    for bad in [-0.5, f32::NAN, f32::INFINITY] {
        let style = Style {
            transition: Some(UiTransition { seconds: bad, shape: Shape::default() }),
            ..Style::default()
        };
        assert_eq!(
            a_root(style).validate(),
            Err(UiError::BadTransition { widget: 1 }),
            "a catch-up of {bad} seconds loaded",
        );
    }
    // Zero is how a HUD says "draw the number exactly", not a malformed duration.
    let snap = Style {
        transition: Some(UiTransition { seconds: 0.0, shape: Shape::default() }),
        ..Style::default()
    };
    assert_eq!(
        a_root(snap).validate(),
        Ok(()),
        "an interface that wants its numbers exact could not say so",
    );
}

#[test]
fn a_declared_animation_survives_the_wasm_boundary_unchanged() {
    check!().with_type::<Scenario>().for_each(|s| {
        let tracks = usize::from(s.tracks) % (MAX_UI_TRACKS + 1);
        let mut style = styled(vec![a_track(); tracks]);
        style.transition = Some(UiTransition {
            seconds: f32::from(s.keys) / 100.0,
            shape: Shape { leave: Ease::Fast, arrive: Ease::Slow },
        });
        let declared = a_root(style);
        let bytes = postcard::to_allocvec(&declared).expect("a ui root serializes");
        let back: UiRoot = postcard::from_bytes(&bytes).expect("and comes back");
        assert_eq!(back, declared, "a declared curve did not survive the trip to the client");
    });
}
