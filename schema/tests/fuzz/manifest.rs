//! Manifest parsing must be *total*: no byte string — malformed TOML, wrong
//! types, missing fields, garbage — may panic the host. A mod is untrusted
//! input at load time, so `parse_manifest` returns `Err`, never unwinds.
#![cfg(feature = "manifest-parse")]

use bolero::check;
use stormlight_mod_abi::manifest::parse_manifest;

#[test]
fn parsing_arbitrary_text_never_panics() {
    check!().with_type::<alloc::string::String>().for_each(|s| {
        let _ = parse_manifest(s);
    });
}

extern crate alloc;
