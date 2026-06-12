//! `stormlight_mod_sdk` — the guest PDK every mod is authored against.
//!
//! Re-exports the shared ABI and adds host-function bindings + ergonomic
//! registration helpers. Mods depend on THIS, not on the schema directly.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

// Re-export the whole ABI so mods write `use stormlight_mod_sdk::abi::…`.
pub use stormlight_mod_abi as abi;

pub mod balance;
pub mod bindings;
pub mod client;
pub mod context;
pub mod macros;
pub mod types;
