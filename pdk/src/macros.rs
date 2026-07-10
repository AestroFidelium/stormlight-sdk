//! `register_mod!` — the entry point a mod uses to publish its content.
//!
//! It takes a builder `fn(&mut ModContext)`, runs it to accumulate the mod's
//! registrations, and generates the `mod_register` wasm export that serializes
//! the resulting [`Registration`] and returns its packed `(ptr, len)` to the
//! host (the pull/return bridge in [`crate::bindings`]).
//!
//! ```ignore
//! use stormlight_mod_sdk::context::ModContext;
//! register_mod!(|ctx: &mut ModContext| {
//!     let _speed = ctx.stat("move_speed");
//!     // ctx.ability("strike", ...); ctx.buff("shield", ...); ...
//! });
//! ```
//!
//! The export is `#[cfg(target_arch = "wasm32")]`: only real guest builds emit
//! the `#[unsafe(no_mangle)]` symbol, so host test builds can call the generated
//! [`registration`](crate) builder directly without a symbol clash, and the one
//! unsafe site (the export attribute) is isolated behind an explicit
//! `#[allow(unsafe_code)]`.

/// Generate a mod's registration builder and its `mod_register` export.
#[macro_export]
macro_rules! register_mod {
    ($build:expr) => {
        /// Build this mod's registration by running the author's builder over a
        /// fresh [`ModContext`](stormlight_mod_sdk::context::ModContext).
        pub fn __stormlight_registration() -> $crate::abi::descriptors::Registration {
            let mut ctx = $crate::context::ModContext::new();
            let build: fn(&mut $crate::context::ModContext) = $build;
            build(&mut ctx);
            ctx.finish()
        }

        /// The wasm entry point the host calls once at load. Returns the packed
        /// `(ptr, len)` of the postcard-encoded registration in linear memory.
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn mod_register() -> u64 {
            $crate::bindings::emit(&__stormlight_registration())
        }
    };
}
