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
//! The exports are `#[cfg(target_arch = "wasm32")]`: only real guest builds emit
//! the `#[unsafe(no_mangle)]` symbols, so host test builds can call the generated
//! [`__stormlight_build`]/`__stormlight_registration` helpers directly without a
//! symbol clash, and the one unsafe site (the export attribute) is isolated
//! behind an explicit `#[allow(unsafe_code)]`.
//!
//! ## Runtime entry points
//!
//! Beyond the one-shot `mod_register`, the macro emits the *runtime* ABI exports
//! ([`stormlight_mod_abi::runtime`]): `mod_alloc` (the host's push channel) and
//! `mod_handle` / `mod_tick` / `mod_trigger`. Each of the latter three **re-runs
//! the author's builder** to reconstruct the dispatch tables, then routes to the
//! registered closure — so a guest carries no state between host calls. Fixed
//! export names, no per-handler symbols.

/// Generate a mod's registration builder and every wasm entry point (`mod_register`,
/// `mod_alloc`, `mod_handle`, `mod_tick`, `mod_trigger`).
#[macro_export]
macro_rules! register_mod {
    ($build:expr) => {
        /// Run the author's builder over a fresh
        /// [`ModContext`](stormlight_mod_sdk::context::ModContext), returning it
        /// intact — content *and* the runtime dispatch tables. Re-run per
        /// invocation so a guest stays stateless across host calls.
        pub fn __stormlight_build() -> $crate::context::ModContext {
            let mut ctx = $crate::context::ModContext::new();
            let build: fn(&mut $crate::context::ModContext) = $build;
            build(&mut ctx);
            ctx
        }

        /// Build this mod's registration (content only) for the host to decode.
        pub fn __stormlight_registration() -> $crate::abi::descriptors::Registration {
            __stormlight_build().finish()
        }

        /// The wasm entry point the host calls once at load. Returns the packed
        /// `(ptr, len)` of the postcard-encoded registration in linear memory.
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn mod_register() -> u64 {
            $crate::bindings::emit(&__stormlight_registration())
        }

        /// Reserve `len` writable bytes for the host to push a context blob into.
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn mod_alloc(len: u32) -> u32 {
            $crate::bindings::alloc(len)
        }

        /// Invoke the `Custom` handler with (local) id `handler`, passing the
        /// param blob at `(ptr, len)`. Returns the packed effects.
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn mod_handle(handler: u32, ptr: u32, len: u32) -> u64 {
            let call = $crate::runtime::HandlerCall { params: $crate::bindings::input(ptr, len) };
            let effects =
                __stormlight_build().run_handler($crate::abi::ids::HandlerId(handler), &call);
            $crate::bindings::emit_effects(&effects)
        }

        /// Run the per-tick entry over the [`TickContext`] at `(ptr, len)`.
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn mod_tick(ptr: u32, len: u32) -> u64 {
            let effects = match $crate::bindings::decode_input(ptr, len) {
                Some(ctx) => __stormlight_build().run_tick(&ctx),
                None => $crate::runtime::GuestEffects::none(),
            };
            $crate::bindings::emit_effects(&effects)
        }

        /// Run the trigger entry for `event` over the [`TriggerContext`] at
        /// `(ptr, len)`.
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn mod_trigger(event: u32, ptr: u32, len: u32) -> u64 {
            // `event` is redundant with the decoded context's `event`, but keeping
            // it in the signature lets the host route without decoding first.
            let _ = event;
            let effects = match $crate::bindings::decode_input(ptr, len) {
                Some(ctx) => __stormlight_build().run_trigger(&ctx),
                None => $crate::runtime::GuestEffects::none(),
            };
            $crate::bindings::emit_effects(&effects)
        }
    };
}

/// Generate a **cosmetic** (`*_client`) mod's registration builder and its one
/// wasm entry point (`mod_register`).
///
/// The counterpart of [`register_mod!`] for a `kind = "client"` package: it takes
/// a builder `fn(&mut ClientContext)`, runs it to accumulate the mod's declared
/// visuals, and emits `mod_register` returning the packed `(ptr, len)` of the
/// postcard-encoded [`ClientRegistration`](stormlight_mod_abi::visuals::ClientRegistration)
/// the client host decodes. A cosmetic guest has no runtime entry points — it is
/// pure, one-shot registration — so no `mod_alloc`/`mod_handle`/… are emitted.
///
/// ```ignore
/// use stormlight_mod_sdk::client::ClientContext;
/// use stormlight_mod_sdk::abi::visuals::{EffectRole, PrimitiveShape, VisualModel};
/// register_client_mod!(|ctx: &mut ClientContext| {
///     ctx.unit_visual("skirmisher", VisualModel::Model { asset: "mod://…".into(), scale: 1.0 });
///     ctx.effect_visual("bolt", EffectRole::Projectile,
///         VisualModel::Primitive { shape: PrimitiveShape::Sphere, color: [1.0, 0.9, 0.3, 1.0] });
/// });
/// ```
#[macro_export]
macro_rules! register_client_mod {
    ($build:expr) => {
        /// Run the author's builder over a fresh
        /// [`ClientContext`](stormlight_mod_sdk::client::ClientContext), returning
        /// it intact for finalization.
        pub fn __stormlight_client_build() -> $crate::client::ClientContext {
            let mut ctx = $crate::client::ClientContext::new();
            let build: fn(&mut $crate::client::ClientContext) = $build;
            build(&mut ctx);
            ctx
        }

        /// Build this cosmetic mod's registration for the client host to decode.
        pub fn __stormlight_client_registration()
        -> $crate::abi::visuals::ClientRegistration {
            __stormlight_client_build().finish()
        }

        /// The wasm entry point the client host calls once at load. Returns the
        /// packed `(ptr, len)` of the postcard-encoded [`ClientRegistration`].
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn mod_register() -> u64 {
            $crate::bindings::emit_client(&__stormlight_client_registration())
        }
    };
}
