//! `Registration` — the top-level bundle a mod emits and the host decodes.
//!
//! This is the postcard payload that crosses the wasm boundary once, at
//! `mod_register`. Per the locked id model (O-3: a single concrete handle-based
//! ISA, no mirror string enum), a mod ships **handle-based** descriptors indexed
//! in its *own* local id space, together with the **name tables** that gave
//! those handles meaning. At adoption the host re-interns every name into the
//! global [`crate::interner::Interner`], building a local→global remap, and
//! rewrites the descriptors' handles — so runtime never sees a string.
//!
//! "String-keyed" therefore lives in [`Names`]: each field is the dense
//! `local raw index -> stable name` table for one id family (exactly what an
//! `Interner` produces). The descriptor collections are indexed by that same
//! local raw index.
//!
//! `Registration` also embeds the [`crate::manifest::ABI_VERSION`] it was built
//! against, so the host can reject a major mismatch at decode as well as at
//! manifest load.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::abilities::AbilityDescriptor;
use crate::behaviors::BuffSpec;
use crate::ids::{TagClassId, TagId};
use crate::manifest::Version;
use crate::talents::TalentDescriptor;

/// The `raw index -> stable name` table for every interned id family a mod may
/// reference. Each `Vec` is dense (`0..len`) and mirrors one `Interner`'s output
/// on the guest side; index `i` is the name the mod interned as raw handle `i`.
///
/// Families map one-to-one to [`crate::ids`]. `Slot` is intentionally absent: it
/// is a small fixed space whose meanings are pure mod convention, never interned.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Names {
    pub stats: Vec<String>,
    pub resources: Vec<String>,
    pub stacks: Vec<String>,
    pub tags: Vec<String>,
    pub tag_classes: Vec<String>,
    pub params: Vec<String>,
    pub events: Vec<String>,
    pub buffs: Vec<String>,
    pub curves: Vec<String>,
    pub damage_types: Vec<String>,
    pub dims: Vec<String>,
    pub abilities: Vec<String>,
    pub talents: Vec<String>,
    pub handlers: Vec<String>,
}

/// A piecewise lookup curve referenced by `Value::Curve` (e.g. a level-scaling
/// table). Control points are `[input, output]` pairs; the interpreter reads or
/// interpolates them. Kept as plain arrays to stay dependency-free like the rest
/// of the ABI.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Curve {
    pub points: Vec<[f32; 2]>,
}

/// Everything a mod registers, in one serializable bundle.
///
/// Descriptor collections are indexed by the mod's *local* handle raw index; the
/// [`Names`] tables give each of those handles a stable string the host interns
/// at adoption. Content families without a descriptor type yet (e.g. units) are
/// deferred to the slice that designs their adoption.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Registration {
    /// The ABI the mod was built against; the host rejects a major mismatch.
    pub abi: Version,
    /// Stable names for every interned handle the descriptors reference.
    pub names: Names,
    /// Ability definitions, indexed by local `AbilityId`.
    pub abilities: Vec<AbilityDescriptor>,
    /// Talent definitions, indexed by local `TalentId`.
    pub talents: Vec<TalentDescriptor>,
    /// Buff/status definitions, indexed by local `BuffId`.
    pub buffs: Vec<BuffSpec>,
    /// Tag → capability-class registrations (a tag may join several classes).
    pub tag_classes: Vec<(TagId, TagClassId)>,
    /// Lookup curves, indexed by local `CurveId`.
    pub curves: Vec<Curve>,
}
