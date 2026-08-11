//! stormlight_mod_abi tests -- property. Bolero only (property / fuzz / structural).
//!
//! One module per feature, one tiny file per "chih". As you add a feature,
//! drop a <feature>.rs beside this file and declare `mod <feature>;` here.
//! Keep invariants directional/structural, never magnitude-only.

mod aiming;
mod animation;
mod animation_validation;
mod conditions;
mod interner;
mod manifest;
mod math;
mod modifiers;
mod navmesh;
mod placement;
mod progression;
mod registration;
mod remap;
mod respawn;
mod runtime;
mod tags;
mod talent_tree;
mod visuals;
