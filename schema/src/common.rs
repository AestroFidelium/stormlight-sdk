//! Shared leaf types used across the ISA — geometry, directions, targeting, and
//! the numeric-op vocabulary. Kept dependency-free (`[f32; 3]` rather than a
//! math-crate `Vec3`) so the ABI stays portable to guest wasm.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::ids::TagId;

/// A position in world space. Plain array to avoid a math-crate dependency in
/// the ABI; the engine converts to its own vector type at adoption.
pub type Point3 = [f32; 3];

/// A generic direction, ported from the prototype's targeting vocabulary.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Direction {
    Forward,
    Backward,
    FromCaster,
    TowardCaster,
    ToTarget,
    Up,
    Custom(Point3),
}

/// Whom a target query keeps, relative to the caster's faction.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Affiliation {
    Enemies,
    Allies,
    All,
}

/// Narrows a resolved target set. Purely declarative; the engine's targeting
/// systems apply it generically.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TargetFilter {
    pub affiliation: Affiliation,
    pub require_tags: Vec<TagId>,
    pub exclude_tags: Vec<TagId>,
    pub include_dead: bool,
}

impl TargetFilter {
    /// A filter that narrows on allegiance alone: no tag requirements, and the
    /// dead excluded. The common shape by far — most abilities care only about
    /// which side a unit is on — so it exists to keep a descriptor from spelling
    /// out three empty fields every time it declares one.
    #[must_use]
    pub fn of(affiliation: Affiliation) -> Self {
        Self {
            affiliation,
            require_tags: Vec::new(),
            exclude_tags: Vec::new(),
            include_dead: false,
        }
    }

    /// A filter that keeps any living unit, whatever its allegiance.
    #[must_use]
    pub fn any() -> Self {
        Self::of(Affiliation::All)
    }
}

/// Whom a leaf acts on, relative to the resolution context. Defaults to the
/// current target after any `Retarget` expansion.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum ImpactTarget {
    Caster,
    PrimaryTarget,
    #[default]
    ResolvedTarget,
    Source,
    AtPoint(Point3),
}

/// How a numeric adjustment combines with the current value — shared by
/// `AdjustPool` (pools) and `ParamPatch` (talent parameter tweaks).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum NumOp {
    Set,
    Add,
    Sub,
    Mul,
}
