//! The authoring path for the engine-reserved aim parameters
//! (stormlight/server#59).
//!
//! A mod declares how far an ability reaches by putting a `range` entry in its
//! `Params`. That only works if the name it interns is *exactly* the one the
//! engine reserved — a typo'd `"Range"` mints an ordinary mod-local parameter the
//! cast pipeline never reads, and the ability silently becomes unlimited. The
//! [`ModContext`] accessors exist to make that unrepresentable. Laws:
//!   - **Equivalence**: an accessor returns the same handle as interning the
//!     reserved name by hand — it is a spelling guard, not a second id space.
//!   - **Idempotence**: asking twice yields one handle and one name-table entry.
//!   - **Emission**: whichever reserved params a mod touched appear in the
//!     emitted `Names.params` at exactly their minted indices, which is what lets
//!     the host collapse them onto its reserved ids.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::ids::{Handle, ParamId};
use stormlight_mod_sdk::abi::params;
use stormlight_mod_sdk::context::ModContext;

/// Which reserved accessor to exercise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TypeGenerator)]
enum Reserved {
    Cooldown,
    Range,
    Radius,
    Spread,
}

impl Reserved {
    /// The canonical name behind the accessor.
    fn name(self) -> &'static str {
        match self {
            Self::Cooldown => params::COOLDOWN,
            Self::Range => params::RANGE,
            Self::Radius => params::RADIUS,
            Self::Spread => params::SPREAD,
        }
    }

    /// Call the accessor under test.
    fn intern(self, ctx: &mut ModContext) -> ParamId {
        match self {
            Self::Cooldown => ctx.cooldown_param(),
            Self::Range => ctx.range_param(),
            Self::Radius => ctx.radius_param(),
            Self::Spread => ctx.spread_param(),
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// Reserved params the mod declares, in author order (repeats intended).
    declared: Vec<Reserved>,
    /// Mod-local params interleaved, so the reserved ones are not alone.
    locals: u8,
}

#[test]
fn accessors_are_the_reserved_names() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ModContext::new();
        for &r in &s.declared {
            let by_accessor = r.intern(&mut ctx);
            let by_name = ctx.param(r.name());
            assert_eq!(by_accessor, by_name, "`{}` accessor minted a second id", r.name());
        }
    });
}

#[test]
fn declared_reserved_params_land_at_their_minted_indices() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ModContext::new();
        for i in 0..usize::from(s.locals % 4) {
            ctx.param(&format!("mod_local_{i}"));
        }
        let minted: Vec<(Reserved, ParamId)> =
            s.declared.iter().map(|&r| (r, r.intern(&mut ctx))).collect();

        let reg = ctx.finish();
        for (r, id) in minted {
            assert_eq!(
                reg.names.params.get(id.raw() as usize).map(String::as_str),
                Some(r.name()),
                "emitted name table disagrees with the handle the author holds"
            );
        }
        // Idempotence, seen from the emitted table: one entry per distinct name.
        for r in [Reserved::Cooldown, Reserved::Range, Reserved::Radius, Reserved::Spread] {
            assert!(
                reg.names.params.iter().filter(|n| *n == r.name()).count() <= 1,
                "`{}` emitted twice",
                r.name()
            );
        }
    });
}
