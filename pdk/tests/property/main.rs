//! stormlight_mod_sdk tests -- property. Bolero only (property / fuzz / structural).
//!
//! One module per feature, one tiny file per "chih". As you add a feature,
//! drop a <feature>.rs beside this file and declare `mod <feature>;` here.
//! Keep invariants directional/structural, never magnitude-only.

mod aim_params;
mod balance;
mod bridge;
mod builders;
mod client_ability_card;
mod client_animation;
mod client_attach;
mod client_context;
mod client_effect_life;
mod client_environment;
mod client_icon;
mod client_notify;
mod client_option_gate;
mod client_scenery;
mod client_status_visual;
mod client_summon;
mod client_tier_view;
mod client_ui;
mod client_unit_marks;
mod client_value_gate;
mod context;
mod effect_key;
mod sounds;
mod ui_anim;
mod ui_mask;
mod ui_sweep;
