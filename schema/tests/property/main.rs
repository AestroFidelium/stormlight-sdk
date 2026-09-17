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
mod attacks;
mod conditions;
mod effect_origin;
mod interner;
mod manifest;
mod math;
mod modifiers;
mod navmesh;
mod placement;
mod progression;
mod quest_done;
mod registration;
mod remap;
mod respawn;
mod runtime;
mod shot_sockets;
mod slot_bind;
mod slot_ref;
mod stats;
mod tags;
mod talent_card;
mod talent_focus;
mod talent_multi_selector;
mod talent_quest;
mod talent_quest_reward;
mod talent_tree;
mod task_interface;
mod task_per_count;
mod task_shortcut;
mod task_stages;
mod ui;
mod ui_action;
mod ui_anim;
mod ui_anim_validation;
mod ui_layer;
mod ui_mask;
mod ui_option_gate;
mod ui_overlap;
mod ui_prepick;
mod ui_quest_progress;
mod ui_roster;
mod ui_summon;
mod ui_summon_action;
mod ui_sweep;
mod ui_talent_option;
mod ui_tier_level;
mod ui_tier_view;
mod ui_tooltip;
mod ui_transient;
mod ui_validation;
mod visuals;
