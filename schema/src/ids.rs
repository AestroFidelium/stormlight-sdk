//! Interned handle types for the effect ISA (Layer A, runtime form).
//!
//! Authoring (the ABI on the wire / in mod source) uses stable strings, which
//! are portable and human-readable. The engine interns those strings to these
//! compact newtype handles at adoption (see [`crate::interner`]). Handles are
//! dense, array-indexable indices: stats/tags/resources are read every frame
//! during aggregation, so at 10k units `u16` indexing beats hashing strings.
//!
//! Meanings are mod convention — the engine never hardcodes a specific id; it
//! only knows how to intern, aggregate, and dispatch generically.

use serde::{Deserialize, Serialize};

/// A compact interned handle backed by a raw integer index.
///
/// Implemented by every name-interned id family so a single generic
/// [`crate::interner::Interner`] can mint any of them. The contract:
/// `H::from_raw(raw).raw() == raw` for every `raw <= H::MAX_RAW`.
pub trait Handle: Copy + Eq {
    /// Largest raw index this family can represent (its capacity minus one).
    const MAX_RAW: u32;

    /// Build a handle from a raw interner index. Callers must keep
    /// `raw <= MAX_RAW`; the interner guarantees this by construction.
    fn from_raw(raw: u32) -> Self;

    /// The raw interner index behind this handle.
    fn raw(self) -> u32;
}

/// Declare a batch of interned-id newtypes plus their [`Handle`] impls.
macro_rules! id_handles {
    ($( $(#[$m:meta])* $name:ident($ty:ty) ),+ $(,)?) => {$(
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
        pub struct $name(pub $ty);

        impl Handle for $name {
            const MAX_RAW: u32 = <$ty>::MAX as u32;

            #[inline]
            fn from_raw(raw: u32) -> Self {
                Self(raw as $ty)
            }

            #[inline]
            fn raw(self) -> u32 {
                u32::from(self.0)
            }
        }
    )+};
}

id_handles! {
    /// A generic unit stat (movement speed, an attack stat, …) — mod-defined.
    StatId(u16),
    /// A bounded numeric resource pool (energy, mana-like, …) — mod-defined.
    ResourceId(u16),
    /// A named stack counter (combo points, charges of a mechanic) — mod-defined.
    StackId(u16),
    /// A binary status tag (rooted, silenced, …) — mod-defined.
    TagId(u16),
    /// A capability class a tag registers into (`blocks_move`, …) — mod-defined.
    TagClassId(u16),
    /// A named ability/descriptor parameter (cooldown, radius, …) — mod-defined.
    ParamId(u16),
    /// A lifecycle / custom event kind (for `Emit` and reactions).
    EventId(u16),
    /// A buff/status-effect definition.
    BuffId(u16),
    /// A lookup curve (e.g. level-scaling table) referenced by `Value::Curve`.
    CurveId(u16),
    /// A damage type / school used by mitigation policy — mod-defined.
    DamageTypeId(u16),
    /// A simulation dimension / sub-world an entity belongs to.
    DimId(u16),
    /// An ability definition.
    AbilityId(u32),
    /// A talent definition.
    TalentId(u32),
    /// A `Custom` escape-hatch handler exported by a wasm mod.
    HandlerId(u32),
    /// A spawnable unit definition (a hero, creep, structure — mod-defined).
    UnitId(u32),
}

/// A generic ability slot index. Not name-interned — slots are a small fixed
/// space whose meanings are pure mod convention.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Slot(pub u8);
