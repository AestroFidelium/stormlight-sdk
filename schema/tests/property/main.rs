//! stormlight_mod_abi tests -- property. Bolero only (property / fuzz / structural).
//!
//! One module per feature, one tiny file per "chih". As you add a feature,
//! drop a <feature>.rs beside this file and declare `mod <feature>;` here.
//! Keep invariants directional/structural, never magnitude-only.

mod ability_icon;
mod aiming;
mod anim_notify;
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
mod stats;
mod tags;
mod talent_card;
mod talent_tree;
mod ui;
mod ui_action;
mod ui_anim;
mod ui_anim_validation;
mod ui_summon;
mod ui_sweep;
mod ui_talent_option;
mod ui_transient;
mod ui_validation;
mod visuals;
