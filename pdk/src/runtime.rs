//! Guest-side runtime entry points — the `Custom`-handler / tick / trigger code
//! a mod runs *after* load, and the input each one receives.
//!
//! The engine invokes these through the fixed wasm exports the [`register_mod!`]
//! macro generates (`mod_handle` / `mod_tick` / `mod_trigger`). An author never
//! writes those exports; they register closures on the
//! [`ModContext`](crate::context::ModContext) (`on_handler` / `on_tick` /
//! `on_trigger`) and the macro wires the rest. Every closure must be a **pure
//! function of its input** — a guest holds no state across invocations (see
//! [`stormlight_mod_abi::runtime`] for why).

pub use stormlight_mod_abi::runtime::{GuestEffects, TickContext, TriggerContext};

/// What a `Custom`-impact handler receives: the opaque parameter blob the
/// invoking `Impact::Custom { params, .. }` carried. The engine treats it as
/// bytes; interpreting them is the mod's business (typically a postcard-decoded
/// author struct).
pub struct HandlerCall<'a> {
    pub params: &'a [u8],
}
