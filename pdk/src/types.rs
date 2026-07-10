//! Small FFI value types shared by the guest bindings and the host.
//!
//! A guest export returns a single `u64` that packs a `(ptr, len)` pair into the
//! guest's linear memory (wasm32 pointers are 32-bit). The host unpacks it and
//! reads `len` bytes at `ptr`. Keeping this in one place means both sides agree
//! on the layout without an `extern` type.

/// The high 32 bits hold the pointer, the low 32 bits hold the length.
#[must_use]
pub const fn pack_ptr_len(ptr: u32, len: u32) -> u64 {
    ((ptr as u64) << 32) | (len as u64)
}

/// Inverse of [`pack_ptr_len`]: `(ptr, len)`.
#[must_use]
pub const fn unpack_ptr_len(packed: u64) -> (u32, u32) {
    ((packed >> 32) as u32, (packed & 0xFFFF_FFFF) as u32)
}
