//! The **engine-reserved stat names** — the handful of aggregated unit stats the
//! engine reads generically, by name.
//!
//! Everything else a mod interns as a stat is its own vocabulary, and the engine
//! only ever adds it up. These three are different: movement reads a speed, and
//! damage mitigation reads a flat and a percent input. Naming them here — in the
//! ABI both sides compile against — is what makes a mod's `"armor"` land on the
//! id the engine mitigates with, and the sibling of [`crate::params`] for the same
//! reason.
//!
//! It matters on the **client**, too, and that is why the list lives in the ABI
//! rather than in the server: a cosmetic mod's HUD binds a bar to a stat by name
//! ([`ValueBinding::Stat`](crate::ui::ValueBinding::Stat)), and the client resolves
//! that name against an id space it rebuilds locally from the gameplay mods. If the
//! two sides seeded a different set of reserved names, every mod-defined stat would
//! be off by the difference — a bar quietly reading the wrong number, which is worse
//! than reading none.
//!
//! **The order of [`RESERVED`] is the id order.** The engine seeds its global stat
//! interner with these names before any mod interns above them, so `RESERVED[i]` is
//! `StatId(i)`. Inserting or reordering a name silently re-points every
//! already-authored mod's stats — append only.

/// How fast a unit moves, in world units per second — read by the movement phase.
pub const MOVE_SPEED: &str = "move_speed";

/// Flat mitigation input with diminishing returns (armor-like).
pub const ARMOR: &str = "armor";

/// Percent mitigation input (resist-like), a fraction in `[0, 1]`.
pub const RESIST: &str = "resist";

/// Every reserved stat name **in id order**: `RESERVED[i]` is `StatId(i)`.
pub const RESERVED: [&str; 3] = [MOVE_SPEED, ARMOR, RESIST];

/// The reserved id `name` occupies, or `None` when it is a mod's own stat. The
/// engine interns [`RESERVED`] in order, so this index *is* the `StatId`.
#[must_use]
pub fn reserved_index(name: &str) -> Option<usize> {
    RESERVED.iter().position(|n| *n == name)
}
