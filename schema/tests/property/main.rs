//! stormlight_mod_abi tests -- property. Bolero only (property / fuzz / structural).
//!
//! One module per feature, one tiny file per "chih". As you add a feature,
//! drop a <feature>.rs beside this file and declare `mod <feature>;` here.
//! Keep invariants directional/structural, never magnitude-only.

mod conditions;
mod interner;
mod math;
mod modifiers;
mod tags;
