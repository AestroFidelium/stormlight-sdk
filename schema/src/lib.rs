//! `stormlight_mod_abi` — the shared modding ABI.
//!
//! Pure, serializable descriptor + manifest data agreed on by BOTH the engine
//! and every mod. No engine, no wasm bindings. `no_std` by default so guest
//! wasm mods can depend on it; the engine enables the `std` feature.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod abilities;
pub mod accumulator;
pub mod behaviors;
pub mod common;
pub mod conditions;
pub mod descriptors;
pub mod ids;
pub mod impacts;
pub mod manifest;
pub mod math;
pub mod missiles;
pub mod talents;
pub mod triggers;
pub mod units;
