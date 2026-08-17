//! A declared curve is sampled every frame, on the client, from data a mod wrote
//! (stormlight/server#97). Decoding it is already covered by
//! [`ui`](super::ui) — what is fuzzed here is the step after: *running* it.
//!
//! [`UiTrack::sample`] is the one function in the interface ABI that is called
//! sixty times a second per moving widget, with a clock the descriptor does not
//! control, on keys that may be duplicated, coincident, reversed or absurd. It must
//! be total for every one of those and must never hand back a number a layout
//! cannot use — a NaN written into a `Node` costs the whole screen, not one widget.

use bolero::check;
use stormlight_mod_abi::ui_anim::{UiTrack, UiTransition};

extern crate alloc;
use alloc::vec::Vec;

#[test]
fn sampling_an_arbitrary_decoded_track_never_panics_and_never_reads_nan() {
    check!().with_type::<(Vec<u8>, i32)>().for_each(|(bytes, at)| {
        let Ok(track) = postcard::from_bytes::<UiTrack>(bytes) else { return };
        // A clock in milliseconds, over the whole signed range: negative (a frame
        // that started before the track did), enormous (a HUD left up for days),
        // and everything between.
        #[allow(clippy::cast_precision_loss)] // A clock, not an identity.
        let value = track.sample(*at as f32 / 1000.0);
        assert!(
            value.is_finite() || track.keys.iter().any(|key| !key.value.is_finite()),
            "a track invented a non-finite value out of finite keys",
        );
        let _ = track.duration();
        let _ = track.repeats();
        let _ = track.fault();
    });
}

#[test]
fn a_decoded_catch_up_always_reports_a_progress_in_range() {
    check!().with_type::<(Vec<u8>, i32)>().for_each(|(bytes, at)| {
        let Ok(transition) = postcard::from_bytes::<UiTransition>(bytes) else { return };
        #[allow(clippy::cast_precision_loss)] // A clock, not an identity.
        let progress = transition.progress(*at as f32 / 1000.0);
        assert!(
            (0.0..=1.0).contains(&progress),
            "a catch-up reported {progress} of the way there, which would put a bar \
             outside its own trough",
        );
    });
}
