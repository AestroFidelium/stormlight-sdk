//! Animation descriptors arrive as bytes across the wasm boundary, from a mod the
//! host does not trust. Both steps the host takes on them must be *total*: decode
//! must reject garbage rather than unwind, and `validate` — the gate that decides
//! whether a decoded descriptor is animatable — must classify anything that got
//! through, including deeply nested layers, empty vectors and non-finite numbers,
//! without panicking or looping.

use bolero::check;
use stormlight_mod_abi::animation::AnimationDescriptor;
use stormlight_mod_abi::visuals::ClientRegistration;

extern crate alloc;
use alloc::vec::Vec;

#[test]
fn validating_an_arbitrary_decoded_descriptor_never_panics() {
    check!().with_type::<Vec<u8>>().for_each(|bytes| {
        if let Ok(descriptor) = postcard::from_bytes::<AnimationDescriptor>(bytes) {
            let _ = descriptor.validate();
        }
    });
}

#[test]
fn validating_every_animation_in_an_arbitrary_bundle_never_panics() {
    check!().with_type::<Vec<u8>>().for_each(|bytes| {
        if let Ok(reg) = postcard::from_bytes::<ClientRegistration>(bytes) {
            for animation in &reg.animations {
                let _ = animation.validate();
            }
        }
    });
}
