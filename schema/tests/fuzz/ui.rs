//! UI descriptors arrive as bytes across the wasm boundary, from a mod the host
//! does not trust. Both steps the host takes on them must be *total*: decode must
//! reject garbage rather than unwind, and `validate` — the gate that decides
//! whether a decoded tree is renderable — must classify anything that got
//! through, including empty containers, absurd nesting and non-finite metrics,
//! without panicking or looping.

use bolero::check;
use stormlight_mod_abi::ui::UiRoot;
use stormlight_mod_abi::visuals::ClientRegistration;

extern crate alloc;
use alloc::vec::Vec;

#[test]
fn validating_an_arbitrary_decoded_root_never_panics() {
    check!().with_type::<Vec<u8>>().for_each(|bytes| {
        if let Ok(root) = postcard::from_bytes::<UiRoot>(bytes) {
            let _ = root.validate();
        }
    });
}

#[test]
fn validating_every_root_in_an_arbitrary_bundle_never_panics() {
    check!().with_type::<Vec<u8>>().for_each(|bytes| {
        if let Ok(reg) = postcard::from_bytes::<ClientRegistration>(bytes) {
            for root in &reg.ui {
                let _ = root.validate();
            }
        }
    });
}
