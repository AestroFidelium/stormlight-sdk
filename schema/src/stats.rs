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

/// How fast a unit swings, as a **multiplier** on its declared attack period —
/// `1.0` is the period exactly as authored, `2.0` is twice as many swings a
/// second. Read by the Combat phase, which divides by it (server#89).
///
/// A multiplier rather than a period so the ordinary additive/multiplicative
/// modifier stack already says "+20% attack speed" without the engine knowing
/// what an attack speed is; a unit aggregating none swings at its declared rate.
pub const ATTACK_SPEED: &str = "attack_speed";

/// How far a unit can reach with its basic attack, in world units. Overrides the
/// range its attack descriptor declares, the same way `move_speed` overrides
/// `MoveSpeed` — so a range buff is a modifier, not an attack-specific feature.
pub const ATTACK_RANGE: &str = "attack_range";

/// The damage a unit's basic attack carries. The engine never reads this one: it
/// is reserved so a mod's payload (`Damage { amount: Stat(attack_damage) }`) and
/// a cosmetic mod's HUD agree on the spelling of the number they both show.
pub const ATTACK_DAMAGE: &str = "attack_damage";

/// How much a unit wants to be attacked when something is choosing between
/// several — highest first, before distance decides (stormlight/server#89).
///
/// The engine must never learn what a hero, a creep or a siege engine *is*, and a
/// distance alone cannot say "shoot the hero, not the creep standing in front of
/// it". A declared number can, and being an ordinary stat it folds through the
/// modifier stack — which is also how a taunt or a decoy works without the engine
/// having any notion of one. A unit that declares none reads as zero.
pub const TARGET_PRIORITY: &str = "target_priority";

/// Every reserved stat name **in id order**: `RESERVED[i]` is `StatId(i)`.
pub const RESERVED: [&str; 7] =
    [MOVE_SPEED, ARMOR, RESIST, ATTACK_SPEED, ATTACK_RANGE, ATTACK_DAMAGE, TARGET_PRIORITY];

/// The reserved id `name` occupies, or `None` when it is a mod's own stat. The
/// engine interns [`RESERVED`] in order, so this index *is* the `StatId`.
#[must_use]
pub fn reserved_index(name: &str) -> Option<usize> {
    RESERVED.iter().position(|n| *n == name)
}
