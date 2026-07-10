//! The FFI return bridge: a guest export packs a `(ptr, len)` into one `u64`
//! that the host unpacks to read the registration bytes. The only law that
//! matters is that the packing is a lossless, invertible encoding — otherwise
//! the host reads the wrong slice of linear memory.

use bolero::check;
use stormlight_mod_sdk::types::{pack_ptr_len, unpack_ptr_len};

#[test]
fn ptr_len_round_trips_through_the_packed_u64() {
    check!().with_type::<(u32, u32)>().for_each(|&(ptr, len)| {
        // Left-inverse ⇒ the encoding is injective: no two (ptr,len) collide.
        assert_eq!(unpack_ptr_len(pack_ptr_len(ptr, len)), (ptr, len));
    });
}
