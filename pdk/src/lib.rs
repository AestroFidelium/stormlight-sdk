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
pub mod runtime;
pub mod types;
pub mod ui;

/// wasm guest runtime glue: a global allocator + panic handler so mods compile
/// to `wasm32-unknown-unknown` cdylibs without per-mod boilerplate. Present only
/// on the wasm target; host builds (tests) use std's.
#[cfg(target_arch = "wasm32")]
mod wasm_runtime {
    #[global_allocator]
    static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

    #[panic_handler]
    fn panic(_: &core::panic::PanicInfo) -> ! {
        core::arch::wasm32::unreachable()
    }
}
