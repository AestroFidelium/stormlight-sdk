//! Small FFI value types shared by the guest bindings and the host.
//!
//! The `(ptr, len)` return convention is part of the wire ABI, so its single
//! definition lives in the schema; re-exported here for ergonomic use from mod
//! code (`stormlight_mod_sdk::types::pack_ptr_len`).

pub use stormlight_mod_abi::bridge::{pack_ptr_len, unpack_ptr_len};
