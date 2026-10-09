//! How long a transient effect lives, and what it does when the unit carrying it
//! goes (stormlight/server#167).
//!
//! The lifetime is a declaration the client resolves against two facts it alone
//! has: its own default for the kind of drawing, and — once the art has loaded —
//! how long the art actually plays. Invariants:
//!   - **An explicit duration is final**: neither the default nor the art's length
//!     changes it;
//!   - **The default ignores the art**, so declaring nothing behaves exactly as it
//!     did before lifetimes could be declared;
//!   - **"As long as the art" is the art's length when there is one**, and the
//!     default when the art could not be measured;
//!   - **Runnable means a clock can run it**: every resolved lifetime of a runnable
//!     declaration is a positive, finite number of seconds, and an explicit duration
//!     that is not is refused.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::lifetime::EffectLifetime;

/// A declaration, with its duration drawn from a band that includes the unrunnable.
#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Declared {
    Default,
    /// Hundredths of a second, signed so zero and negatives are covered.
    Seconds(#[generator(-200i16..=2000)] i16),
    Art,
}

impl Declared {
    fn lifetime(self) -> EffectLifetime {
        match self {
            Self::Default => EffectLifetime::Default,
            Self::Seconds(h) => EffectLifetime::Seconds(f32::from(h) / 100.0),
            Self::Art => EffectLifetime::Art,
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    declared: Declared,
    /// The client's default for this drawing, in hundredths.
    #[generator(1u16..=500)]
    default: u16,
    /// How long the art plays, in hundredths, when it could be measured.
    measured: Option<u16>,
}

#[test]
fn a_lifetime_resolves_to_what_it_declared() {
    check!().with_type::<Scenario>().for_each(|s| {
        let default = f32::from(s.default) / 100.0;
        let art = s.measured.map(|h| f32::from(h.max(1)) / 100.0);
        let lifetime = s.declared.lifetime();
        let resolved = lifetime.seconds(default, art);
        match lifetime {
            EffectLifetime::Seconds(seconds) => assert_eq!(resolved, seconds, "{s:?}"),
            EffectLifetime::Default => assert_eq!(resolved, default, "{s:?}"),
            EffectLifetime::Art => assert_eq!(resolved, art.unwrap_or(default), "{s:?}"),
        }
        if lifetime.is_runnable() {
            assert!(resolved.is_finite() && resolved > 0.0, "{s:?} resolved to {resolved}");
        }
    });
}

#[test]
fn only_a_duration_no_clock_can_run_is_refused() {
    check!().with_type::<Declared>().for_each(|declared| {
        let lifetime = declared.lifetime();
        let runnable = match lifetime {
            EffectLifetime::Seconds(seconds) => seconds.is_finite() && seconds > 0.0,
            EffectLifetime::Default | EffectLifetime::Art => true,
        };
        assert_eq!(lifetime.is_runnable(), runnable, "{declared:?}");
    });
    for unrunnable in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(!EffectLifetime::Seconds(unrunnable).is_runnable());
    }
}
