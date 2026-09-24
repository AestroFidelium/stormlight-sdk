//! Balance sugar: author a number as a **share of a declared baseline** instead
//! of a literal (stormlight/server#195).
//!
//! A literal says nothing about how it relates to any other number in the game,
//! so balancing a roster turns into comparing constants across files. Here a mod
//! declares, once, what an *average* unit has, as curves over level. Everything
//! else is written relative to that:
//!
//! ```ignore
//! let base = Baseline::declare(ctx, BaselineSpec {
//!     health: Curve { points: vec![[1.0, 5000.0], [20.0, 9000.0]] },
//!     move_speed: flat(4.8),
//!     cooldown: flat(8.0),
//! });
//!
//! health: base.health(pct(80)),   // a fragile unit: 80% of average, at every level
//! amount: base.damage(pct(2)),    // ~50 hits to down an average unit
//! ```
//!
//! A share expands to `fraction × curve(level)` in the ordinary [`Value`] tree,
//! so it follows progression: 80% of the baseline is still 80% of it at level 20.
//! The output is plain descriptor data, so the host needs nothing new.
//!
//! The SDK holds the mechanism and none of the numbers. Every baseline value
//! comes from the mod, which keeps the engine content-free. Mods that should agree
//! share one [`BaselineSpec`] (e.g. from a common crate), so moving it retunes
//! every unit that reads it.

use alloc::boxed::Box;
use alloc::vec;

use stormlight_mod_abi::descriptors::Curve;
use stormlight_mod_abi::ids::CurveId;
use stormlight_mod_abi::math::{BinOp, Value, Var};

use crate::context::ModContext;

/// A fraction of some whole: `pct(80)` is 0.8 of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Share(f32);

impl Share {
    /// The share as a plain fraction: 1.0 is the whole.
    #[must_use]
    pub fn fraction(self) -> f32 {
        self.0
    }
}

/// A percentage, written as a whole number (`pct(80)`) or a decimal (`pct(2.5)`).
#[must_use]
pub fn pct(percent: impl Percent) -> Share {
    Share((percent.into_f64() / 100.0) as f32)
}

/// A number [`pct`] accepts. Implemented for the types a literal defaults to,
/// so `pct(80)` and `pct(2.5)` both read naturally.
pub trait Percent: sealed::Sealed {
    #[doc(hidden)]
    fn into_f64(self) -> f64;
}

mod sealed {
    pub trait Sealed {}
}

macro_rules! percent_from {
    ($($t:ty),*) => {$(
        impl sealed::Sealed for $t {}
        impl Percent for $t {
            fn into_f64(self) -> f64 {
                f64::from(self)
            }
        }
    )*};
}
percent_from!(i32, u32, f32, f64);

/// `share` of any value: for a number with no baseline of its own, such as a
/// cost measured against the pool that pays it (`share_of(pct(25), pool_max)`).
#[must_use]
pub fn share_of(share: Share, whole: Value) -> Value {
    // 100% is the whole itself, not an expression that evaluates to it.
    if share.0 == 1.0 {
        return whole;
    }
    Value::Bin(BinOp::Mul, Box::new(Value::Const(share.0)), Box::new(whole))
}

/// A curve with the same value at every level.
#[must_use]
pub fn flat(value: f32) -> Curve {
    Curve { points: vec![[0.0, value]] }
}

/// What an average unit has, as level curves. All numbers here are the mod's.
#[derive(Clone, Debug, PartialEq)]
pub struct BaselineSpec {
    /// Maximum health. Damage, healing and shields are measured against it too.
    pub health: Curve,
    /// Movement speed, in world units per second.
    pub move_speed: Curve,
    /// Ability cooldown, in seconds.
    pub cooldown: Curve,
}

/// A declared baseline: the handles of its registered curves.
///
/// Each method returns a [`Share`] of one axis *at the level of whoever the
/// value is evaluated for*: a unit's own level for its stats, the caster's
/// level for an ability's numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Baseline {
    health: CurveId,
    move_speed: CurveId,
    cooldown: CurveId,
}

impl Baseline {
    /// Register `spec`'s curves in this mod as `baseline.health`,
    /// `baseline.move_speed` and `baseline.cooldown`. Declare it once per mod.
    pub fn declare(ctx: &mut ModContext, spec: BaselineSpec) -> Self {
        Self {
            health: ctx.curve("baseline.health", spec.health),
            move_speed: ctx.curve("baseline.move_speed", spec.move_speed),
            cooldown: ctx.curve("baseline.cooldown", spec.cooldown),
        }
    }

    /// A unit's maximum health, as a share of the average unit's.
    #[must_use]
    pub fn health(&self, share: Share) -> Value {
        share_of(share, at_level(self.health))
    }

    /// Damage, as a share of the **average** unit's health. `pct(2)` is about
    /// fifty hits to down one. This is not a share of the *target's* health: it
    /// deals the same amount to a fragile unit and to a sturdy one.
    #[must_use]
    pub fn damage(&self, share: Share) -> Value {
        self.health(share)
    }

    /// Healing or shielding, on the same scale as [`Self::damage`].
    #[must_use]
    pub fn heal(&self, share: Share) -> Value {
        self.health(share)
    }

    /// Movement speed, as a share of the average unit's.
    #[must_use]
    pub fn move_speed(&self, share: Share) -> Value {
        share_of(share, at_level(self.move_speed))
    }

    /// An ability cooldown, as a share of the average one. Below 100% is faster.
    #[must_use]
    pub fn cooldown(&self, share: Share) -> Value {
        share_of(share, at_level(self.cooldown))
    }
}

/// `curve` read at the level of whoever the value is evaluated for.
fn at_level(curve: CurveId) -> Value {
    Value::Curve(curve, Box::new(Value::Read(Var::Level)))
}
