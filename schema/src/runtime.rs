//! The wasm **runtime** ABI — the contract for invoking a guest *after* load.
//!
//! Registration ([`crate::descriptors`]) is one-shot and pull-only: the guest
//! emits its content once and the host reads it. Runtime invocation is different
//! — the host must **push** a context blob into the guest and get effects back,
//! repeatedly, for the whole session. This module fixes that contract so both
//! sides agree on it forever.
//!
//! ## Stateless guests, effects as the only output
//!
//! A guest invocation is a **pure function of its context**. It holds no state
//! across calls: hidden state in wasm linear memory could not be snapshotted or
//! rewound by the engine's generic rewind mechanism, and would break determinism
//! and replays. The engine owns *all* simulation state; a guest's only output is
//! a serialized list of [`Impact`]s ([`GuestEffects`]) — the very ISA the
//! interpreter already dispatches. Those effects carry *symbolic*
//! [`ImpactTarget`](crate::common::ImpactTarget)s, which the engine resolves
//! against the invoking context, so no entity identity ever crosses the sandbox.
//!
//! ## The fixed export set
//!
//! A guest exposes a small, fixed set of exports — never one symbol per handler,
//! so the export table does not grow with content and there is no name mangling:
//!
//! | Export | Signature | Role |
//! | --- | --- | --- |
//! | [`ALLOC_EXPORT`] | `mod_alloc(len: u32) -> u32` | host→guest push: reserve `len` writable bytes, return their pointer |
//! | [`HANDLE_EXPORT`] | `mod_handle(handler: u32, ptr: u32, len: u32) -> u64` | a `Custom` impact handler; the guest matches on its local [`HandlerId`] |
//! | [`TICK_EXPORT`] | `mod_tick(ptr: u32, len: u32) -> u64` | per-tick entry over a [`TickContext`] |
//! | [`TRIGGER_EXPORT`] | `mod_trigger(event: u32, ptr: u32, len: u32) -> u64` | reaction entry over a [`TriggerContext`] |
//!
//! Each returning `u64` packs a `(ptr, len)`
//! ([`pack_ptr_len`](crate::bridge::pack_ptr_len)) addressing a postcard-encoded
//! [`GuestEffects`] in the guest's memory; a `(0, 0)` return means "no effects".
//! An entry point a mod does not need is simply not exported — the host skips it.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::impacts::Impact;
use crate::math::Value;

/// Export name: the guest allocator the host calls to push a context blob in.
pub const ALLOC_EXPORT: &str = "mod_alloc";
/// Export name: the `Custom`-impact handler dispatch entry.
pub const HANDLE_EXPORT: &str = "mod_handle";
/// Export name: the per-tick entry.
pub const TICK_EXPORT: &str = "mod_tick";
/// Export name: the trigger/reaction entry.
pub const TRIGGER_EXPORT: &str = "mod_trigger";

/// A guest entry point's entire output: the effects it proposes for the engine to
/// interpret. Empty (the [`Default`]) is the common case — a handler that only
/// reads state proposes nothing.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct GuestEffects {
    pub effects: Vec<Impact>,
}

impl GuestEffects {
    /// A result proposing no effects.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// A result proposing exactly `effects`.
    #[must_use]
    pub fn new(effects: Vec<Impact>) -> Self {
        Self { effects }
    }
}

/// The context handed to [`TICK_EXPORT`]: the current simulation tick, so a
/// guest's per-tick logic is a pure function of time (any world reads it needs
/// arrive through host calls, out of scope for this contract).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct TickContext {
    pub tick: u64,
}

/// The context handed to [`TRIGGER_EXPORT`]: which mod-defined event fired and the
/// payload the `Emit` carried. Mirrors the `Impact::Emit { event, payload }`
/// shape, so a guest reacts to the same events the ISA raises.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TriggerContext {
    pub event: crate::ids::EventId,
    pub payload: Value,
}
