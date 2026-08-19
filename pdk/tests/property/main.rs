//! stormlight_mod_sdk tests -- property. Bolero only (property / fuzz / structural).
//!
//! One module per feature, one tiny file per "chih". As you add a feature,
//! drop a <feature>.rs beside this file and declare `mod <feature>;` here.
//! Keep invariants directional/structural, never magnitude-only.

mod aim_params;
mod bridge;
mod client_animation;
mod client_context;
mod client_icon;
mod client_notify;
mod client_option_gate;
mod client_summon;
mod client_tier_view;
mod client_ui;
mod context;
mod ui_anim;
mod ui_sweep;
