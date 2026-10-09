//! A picture laid flat on the ground (stormlight/server#170).
//!
//! A decal's opacity rises, holds and falls on a declared envelope. Laws, over any
//! envelope and any two moments:
//!   - **it starts, peaks and ends where it says**: the start opacity at the first
//!     instant, the peak through the hold, the end opacity once the fall is done
//!     and for ever after;
//!   - **it only rises while rising and only falls while falling**;
//!   - **it never leaves the range its three opacities span**, so it cannot flash;
//!   - **only an envelope a clock can run is valid**: finite, non-negative phases
//!     and opacities in `0..=1`.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::decal::DecalFade;

#[derive(Debug, Clone, Copy, TypeGenerator)]
struct Envelope {
    /// Hundredths of a second.
    #[generator(0u16..=300)]
    attack: u16,
    #[generator(0u16..=300)]
    hold: u16,
    #[generator(0u16..=300)]
    decay: u16,
    /// Opacities, in 255ths.
    alpha: [u8; 3],
}

impl Envelope {
    fn fade(self) -> DecalFade {
        DecalFade {
            attack: f32::from(self.attack) / 100.0,
            hold: f32::from(self.hold) / 100.0,
            decay: f32::from(self.decay) / 100.0,
            alpha: self.alpha.map(|a| f32::from(a) / 255.0),
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    envelope: Envelope,
    /// Two moments, in hundredths of a second.
    #[generator(0u16..=1200)]
    a: u16,
    #[generator(0u16..=1200)]
    b: u16,
}

#[test]
fn a_decal_rises_holds_and_falls_as_declared() {
    check!().with_type::<Scenario>().for_each(|s| {
        let fade = s.envelope.fade();
        assert!(fade.is_valid(), "{s:?}");
        let [start, peak, end] = fade.alpha;
        let (lo, hi) = (start.min(peak).min(end), start.max(peak).max(end));
        let (t0, t1) = (f32::from(s.a.min(s.b)) / 100.0, f32::from(s.a.max(s.b)) / 100.0);
        let (x, y) = (fade.alpha_at(t0), fade.alpha_at(t1));
        for v in [x, y] {
            assert!((lo - 1e-5..=hi + 1e-5).contains(&v), "{s:?}: {v} outside {lo}..{hi}");
        }
        let rise_end = fade.attack;
        let hold_end = rise_end + fade.hold;
        let fall_end = hold_end + fade.decay;
        if fade.attack > 0.0 {
            assert!(
                (fade.alpha_at(0.0) - start).abs() < 1e-5,
                "{s:?}: does not start at its start"
            );
        }
        // Away from the boundaries by a hair, so a phase edge that rounds one way in
        // the sum and the other in the subtraction is not mistaken for a fault.
        const EDGE: f32 = 1e-3;
        if t0 >= rise_end && t1 < hold_end - EDGE {
            assert!(
                (x - peak).abs() < 1e-5 && (y - peak).abs() < 1e-5,
                "{s:?}: not held at its peak"
            );
        }
        if t0 >= fall_end + EDGE {
            assert!((x - end).abs() < 1e-5, "{s:?}: not at its end after falling");
        }
        if t1 <= rise_end - EDGE {
            let rising = peak >= start;
            assert!(
                if rising { y >= x - 1e-5 } else { y <= x + 1e-5 },
                "{s:?}: wrong way while rising"
            );
        }
        if t0 >= hold_end + EDGE && t1 <= fall_end - EDGE {
            let falling = end <= peak;
            assert!(
                if falling { y <= x + 1e-5 } else { y >= x - 1e-5 },
                "{s:?}: wrong way while falling"
            );
        }
    });
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Fault {
    NegativeAttack,
    InfiniteHold,
    OpacityAboveOne,
    NegativeOpacity,
}

/// A sound envelope with one thing broken.
#[derive(Debug, TypeGenerator)]
struct Broken {
    envelope: Envelope,
    fault: Fault,
}

#[test]
fn only_an_envelope_a_clock_can_run_is_valid() {
    check!().with_type::<Broken>().for_each(|b| {
        let (mut fade, fault) = (b.envelope.fade(), b.fault);
        match fault {
            Fault::NegativeAttack => fade.attack = -0.1,
            Fault::InfiniteHold => fade.hold = f32::INFINITY,
            Fault::OpacityAboveOne => fade.alpha[1] = 1.5,
            Fault::NegativeOpacity => fade.alpha[2] = -0.2,
        }
        assert!(!fade.is_valid(), "{fault:?} accepted: {fade:?}");
    });
}
