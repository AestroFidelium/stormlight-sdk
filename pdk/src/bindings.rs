//! The guest→host memory bridge for registration.
//!
//! We use a **pull / return** model: the guest's `mod_register` export (built by
//! [`crate::register_mod!`]) serializes its [`Registration`] to postcard, hands
//! the bytes to [`emit`], and returns the packed `(ptr, len)`. The host reads
//! those bytes out of the guest's linear memory and decodes them.
//!
//! The buffer is intentionally leaked: registration is one-shot, the host reads
//! it immediately, and M3 has no runtime free path (there is no `mod_tick`). No
//! host imports and no `unsafe` are needed here — only the export attribute
//! itself is unsafe, and that lives on the macro-generated function.

use alloc::vec::Vec;

use stormlight_mod_abi::descriptors::Registration;

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

