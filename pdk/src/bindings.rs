//! The guest→host memory bridge for registration.
//!
//! We use a **pull / return** model: the guest's `mod_register` export (built by
//! [`crate::register_mod!`]) serializes its [`Registration`] to postcard, hands
//! the bytes to [`emit`], and returns the packed `(ptr, len)`. The host reads
//! those bytes out of the guest's linear memory and decodes them.
//!
//! The buffer is intentionally leaked: the host reads it immediately, and every
//! call runs in a fresh store whose whole linear memory is dropped afterwards, so
//! there is nothing a free path would reclaim. No host imports are needed; the
//! only `unsafe` is the export attribute on the macro-generated functions and the
//! slice view in `input`, each behind an explicit `#[allow(unsafe_code)]`.

use alloc::vec::Vec;

use stormlight_mod_abi::descriptors::Registration;
use stormlight_mod_abi::runtime::GuestEffects;
use stormlight_mod_abi::visuals::ClientRegistration;

use crate::types::pack_ptr_len;

/// Serialize a [`Registration`] to the bytes the host will decode.
///
/// Split out from [`emit`] so the encoding is testable on the host without going
/// through a raw pointer.
#[must_use]
pub fn to_bytes(reg: &Registration) -> Vec<u8> {
    postcard::to_allocvec(reg).expect("a Registration always serializes")
}

/// Serialize `reg`, leak the buffer into linear memory, and return the packed
/// `(ptr, len)` a guest export hands back to the host.
#[must_use]
pub fn emit(reg: &Registration) -> u64 {
    emit_bytes(to_bytes(reg))
}

/// Leak `bytes` and return their packed `(ptr, len)`. Meaningful only inside a
/// wasm guest, where the pointer is a valid offset into linear memory; on a
/// 64-bit host the pointer is truncated (this is never called host-side).
#[must_use]
pub fn emit_bytes(bytes: Vec<u8>) -> u64 {
    let boxed = bytes.into_boxed_slice();
    let len = boxed.len() as u32;
    let ptr = boxed.as_ptr() as usize as u32;
    // Hand ownership to the host; it reads `len` bytes at `ptr` then drops us.
    core::mem::forget(boxed);
    pack_ptr_len(ptr, len)
}

/// Serialize a [`ClientRegistration`] to the bytes the client host will decode —
/// the cosmetic counterpart of [`to_bytes`]. Split out so the encoding is testable
/// host-side without a raw pointer.
#[must_use]
pub fn to_bytes_client(reg: &ClientRegistration) -> Vec<u8> {
    postcard::to_allocvec(reg).expect("a ClientRegistration always serializes")
}

/// Serialize `reg`, leak the buffer into linear memory, and return the packed
/// `(ptr, len)` a cosmetic guest's `mod_register` hands back to the client host.
#[must_use]
pub fn emit_client(reg: &ClientRegistration) -> u64 {
    emit_bytes(to_bytes_client(reg))
}

/// Serialize a runtime entry point's [`GuestEffects`] and emit them as the packed
/// `(ptr, len)` the export returns. The counterpart of [`emit`] for the *runtime*
/// ABI (handlers / tick / trigger) rather than registration.
#[must_use]
pub fn emit_effects(effects: &GuestEffects) -> u64 {
    emit_bytes(postcard::to_allocvec(effects).expect("GuestEffects always serializes"))
}

/// Reserve `len` writable bytes in linear memory and return their pointer. The
/// host calls this through the `mod_alloc` export to **push** a context blob in
/// before calling an entry point. Wasm-only: the returned value is a valid
/// linear-memory offset only inside a guest.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn alloc(len: u32) -> u32 {
    let buf = alloc::vec![0u8; len as usize].into_boxed_slice();
    let ptr = buf.as_ptr() as usize as u32;
    // Leak it: the host writes here, then the entry point reads it via `input`.
    // A fresh store per invocation reclaims the whole allocation afterwards.
    core::mem::forget(buf);
    ptr
}

/// View the `len` bytes the host wrote at `ptr` (a region we handed it from
/// [`alloc`]). Wasm-only.
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
#[must_use]
pub fn input(ptr: u32, len: u32) -> &'static [u8] {
    // SAFETY: the host wrote exactly `len` bytes at `ptr`, which `alloc` reserved
    // and leaked; the region outlives this (single) invocation.
    unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) }
}

/// Decode a postcard context the host pushed at `(ptr, len)`. Malformed bytes
/// yield `None` so an entry point can degrade to "no effects" rather than trap —
/// total over bad input. Wasm-only.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn decode_input<T: serde::de::DeserializeOwned>(ptr: u32, len: u32) -> Option<T> {
    postcard::from_bytes(input(ptr, len)).ok()
}
