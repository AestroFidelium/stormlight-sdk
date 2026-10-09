//! `stormlight_mod_abi` — the shared modding ABI.
//!
//! Pure, serializable descriptor + manifest data agreed on by BOTH the engine
//! and every mod. No engine, no wasm bindings. `no_std` by default so guest
//! wasm mods can depend on it; the engine enables the `std` feature.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod abilities;
pub mod accumulator;
pub mod animation;
pub mod attach;
pub mod attacks;
pub mod behaviors;
pub mod bridge;
pub mod common;
pub mod conditions;
pub mod decal;
pub mod descriptors;
pub mod environment;
pub mod ids;
pub mod impacts;
pub mod interner;
pub mod lifetime;
pub mod manifest;
pub mod math;
pub mod missiles;
pub mod motion;
pub mod navmesh;
pub mod notify;
pub mod params;
pub mod placement;
pub mod progression;
pub mod remap;
pub mod respawn;
pub mod roles;
pub mod runtime;
pub mod scenery;
pub mod shadow;
pub mod slot_ref;
pub mod stats;
pub mod status_visual;
pub mod talent_tree;
pub mod talents;
pub mod tasks;
pub mod triggers;
pub mod ui;
pub mod ui_anim;
pub mod ui_event;
pub mod units;
pub mod visuals;
pub mod volley;
