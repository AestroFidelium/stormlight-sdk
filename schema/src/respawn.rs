//! Respawn declarations (stormlight/server#61) — how a unit says it comes back.
//!
//! Death is only half a loop. A training target that never returns stops being a
//! target after one volley, and a hero that never returns ends the match for the
//! player driving it. The other half is this: a unit declaring **how long it stays
//! down**, and **what it is** when it stands back up.
//!
//! It is descriptor data, never an engine constant. The engine knows how to run a
//! death/respawn loop; how long a mod's fighter is out of the fight, and whether
//! its cooldowns and buffs survive the trip, is a balance decision only the mod can
//! make. A unit that declares no [`RespawnSpec`] stays down — the honest default
//! for a one-shot objective, and the reason the field is an `Option`.
//!
//! Two places can speak, and they do not overlap: a [`UnitPlacement`] may override
//! the **delay** for the one unit it stands up (a map's forward camp may return
//! faster than the same unit elsewhere), while the **policy** below always comes
//! from the unit itself. Where a unit comes back is the map's business; what it
//! comes back as is the unit's.
//!
//! [`UnitPlacement`]: crate::placement::UnitPlacement

use serde::{Deserialize, Serialize};

/// What a unit's return looks like: the wait, and which carried state is wiped on
/// the way back.
///
/// The flags are stated positively as *clears*, so the all-`false` spec is the
/// conservative one — a unit comes back exactly as it went down, and every reset
/// is something a mod asked for. Health is the exception and is not a flag:
/// respawning at the health you died with is not a respawn, so vitals are always
/// restored to the descriptor's ceiling.
///
/// Buffs are not here either, and deliberately: whether an effect survives its
/// holder's death is a property of *that effect*, declared per buff as
/// [`BuffSpec::drop_on_death`](crate::behaviors::BuffSpec::drop_on_death). A unit
/// may hold a blessing that outlives it and a shield that does not, and one flag
/// on the unit could not say both.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RespawnSpec {
    /// Seconds the unit stays down before it returns. Read through
    /// [`seconds`](Self::seconds) — the raw field is whatever the mod declared.
    pub after: f32,
    /// Clear every ability cooldown on the way back, so the unit returns ready to
    /// act rather than mid-recovery from the fight it lost.
    pub clear_cooldowns: bool,
    /// Refill every declared resource pool to its ceiling. Independent of health,
    /// because a resource is a spend meter and a mod may want it earned back.
    pub refill_resources: bool,
}

impl RespawnSpec {
    /// The effective delay: finite and non-negative, whatever was declared.
    ///
    /// Total by design. The delay becomes a deadline on the simulation clock, and
    /// a NaN or infinite one produces a unit that is dead for the rest of the
    /// match with no way to tell why — a content mistake reading as an engine
    /// failure. A negative delay is a sign error, and returns at once. Numbers a
    /// mod declared honestly pass through untouched.
    #[must_use]
    pub fn seconds(&self) -> f32 {
        if self.after.is_finite() { self.after.max(0.0) } else { 0.0 }
    }

    /// A respawn after `secs` that changes nothing else — the unit returns whole,
    /// carrying what it carried. The starting point a mod adjusts from.
    #[must_use]
    pub fn after(secs: f32) -> Self {
        Self { after: secs, clear_cooldowns: false, refill_resources: false }
    }
}
