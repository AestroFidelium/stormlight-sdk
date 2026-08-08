//! The **engine-reserved ability parameter names** — the handful of `Params`
//! entries the engine reads generically, by name.
//!
//! Everything else in `Params` is a mod's own vocabulary the engine never looks
//! at. These four are different: the cast pipeline needs a cooldown to start, a
//! range to validate an aim against, and (for an aimed area or cone) the size of
//! the shape the client draws and the server resolves. Naming them here — in the
//! ABI both sides compile against — is what makes a mod's `"range"` land on the
//! id the engine reads, and what stops the client and the server from disagreeing
//! about the spelling.
//!
//! Why parameters rather than fields on [`Targeting`](crate::abilities::Targeting):
//! range and radius are exactly the numbers talents tune ("+2 range", "−20%
//! radius"), and `Params` is already the talent-patchable surface
//! (`ParamPatch`). A field would need its own patching path; a param gets it for
//! free, and costs the ABI nothing.
//!
//! **The order of [`RESERVED`] is the id order.** The engine seeds its global
//! param interner with these names before any mod interns above them, so
//! `RESERVED[i]` is `ParamId(i)`. Inserting or reordering a name silently
//! re-points every already-authored mod's parameters — append only.

/// Seconds before the slot the ability was cast from can be cast again.
pub const COOLDOWN: &str = "cooldown";

/// Maximum distance from the caster an aim may be, in world units. An ability
/// that declares no `range` is unrestricted.
pub const RANGE: &str = "range";

/// Radius of an aimed area, in world units — the ground circle a `Point` aim
/// centres. Meaningless (and ignored) for a mode with no area.
pub const RADIUS: &str = "radius";

/// Half-angle of an aimed cone, in radians — how wide a `Vector` aim opens.
/// Meaningless (and ignored) for a mode that is not a cone.
pub const SPREAD: &str = "spread";

/// Every reserved param name **in id order**: `RESERVED[i]` is `ParamId(i)`.
pub const RESERVED: [&str; 4] = [COOLDOWN, RANGE, RADIUS, SPREAD];

/// The reserved id `name` occupies, or `None` when it is a mod's own parameter.
/// The engine interns [`RESERVED`] in order, so this index *is* the `ParamId`.
#[must_use]
pub fn reserved_index(name: &str) -> Option<usize> {
    RESERVED.iter().position(|n| *n == name)
}
