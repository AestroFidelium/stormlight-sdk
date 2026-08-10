//! Manifest schema invariants — the load-time contract every mod declares.
//!
//! The manifest is pure, serializable data (like the rest of the ABI): a mod
//! states its `id`, `name`, semantic `version`, `kind`, wasm `entry`, the `abi`
//! it was built against, and optional `deps`/`assets`. Laws pinned here:
//!   - **Round-trip**: any manifest survives a postcard serialize round-trip
//!     unchanged (wire-format stability, same law as the ISA).
//!   - **`Version` <-> string**: `parse(v.to_string()) == v` for every version,
//!     and parsing arbitrary text never panics (it returns `Err`).
//!   - **Validation is total & directional**: a well-formed manifest validates;
//!     a bad id / empty-or-non-`.wasm` entry / **major** ABI mismatch is
//!     rejected, and a major match with any minor/patch is accepted.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::manifest::{
    ABI_VERSION, Dependency, Manifest, ManifestError, ModKind, Version,
};

/// Lowercase-ident charset the manifest id/entry-stem rule accepts.
const IDENT: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";

/// Map arbitrary seed bytes to a *valid* lowercase identifier (non-empty, first
/// char a letter) so the id/entry are well-formed by construction.
fn ident(seed: &[u8], fallback: char) -> alloc::string::String {
    use alloc::string::String;
    let mut s = String::new();
    for &b in seed {
        s.push(IDENT[b as usize % IDENT.len()] as char);
    }
    if s.chars().next().is_none_or(|c| !c.is_ascii_lowercase()) {
        s.insert(0, fallback);
    }
    s
}

extern crate alloc;

#[derive(Debug, TypeGenerator)]
struct Scenario {
    id_seed: alloc::vec::Vec<u8>,
    name: alloc::string::String,
    ver: (u16, u16, u16),
    abi: (u16, u16, u16),
    server: bool,
    entry_seed: alloc::vec::Vec<u8>,
    deps: alloc::vec::Vec<(alloc::vec::Vec<u8>, (u16, u16, u16))>,
    assets: alloc::vec::Vec<alloc::string::String>,
}

fn ver((a, b, c): (u16, u16, u16)) -> Version {
    Version::new(u32::from(a), u32::from(b), u32::from(c))
}

/// A well-formed manifest built from a scenario (id/entry are valid idents).
fn build(s: &Scenario) -> Manifest {
    use alloc::format;
    // Keep the name non-empty so it never pre-empts the field under test.
    let name = if s.name.is_empty() { alloc::string::String::from("n") } else { s.name.clone() };
    Manifest {
        id: ident(&s.id_seed, 'm'),
        name,
        version: ver(s.ver),
        kind: if s.server { ModKind::Server } else { ModKind::Client },
        entry: format!("{}.wasm", ident(&s.entry_seed, 'e')),
        abi: ver(s.abi),
        deps: s
            .deps
            .iter()
            .map(|(seed, v)| Dependency { id: ident(seed, 'd'), version: ver(*v) })
            .collect(),
        assets: s.assets.clone(),
    }
}

#[test]
fn any_manifest_survives_a_postcard_round_trip() {
    check!().with_type::<Scenario>().for_each(|s| {
        let m = build(s);
        let bytes = postcard::to_allocvec(&m).expect("serialize");
        let back: Manifest = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(m, back, "manifest did not round-trip through postcard");
        let again = postcard::to_allocvec(&back).expect("reserialize");
        assert_eq!(bytes, again, "manifest serialization is not stable");
    });
}

#[test]
fn version_round_trips_through_its_string_form() {
    check!().with_type::<(u16, u16, u16)>().for_each(|&t| {
        let v = ver(t);
        let printed = alloc::format!("{v}");
        assert_eq!(Version::parse(&printed), Ok(v), "version <-> string not a round-trip");
    });
}

#[test]
fn version_parse_never_panics_on_arbitrary_text() {
    check!().with_type::<alloc::string::String>().for_each(|s| {
        // Only invariant: total function. Malformed input is an `Err`, not a panic.
        let _ = Version::parse(s);
    });
}

#[test]
fn a_well_formed_manifest_with_matching_abi_validates() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut m = build(s);
        // Force the ABI major to match; minor/patch stay arbitrary.
        m.abi.major = ABI_VERSION.major;
        assert_eq!(m.validate(), Ok(()), "well-formed manifest rejected: {m:?}");
    });
}

#[test]
fn abi_is_accepted_exactly_when_the_major_matches() {
    check!().with_type::<Scenario>().for_each(|s| {
        let m = build(s);
        let ok = m.check_abi().is_ok();
        assert_eq!(ok, m.abi.major == ABI_VERSION.major, "abi gate is not major-only");
    });
}

#[test]
fn an_empty_or_bad_charset_id_is_rejected() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut m = build(s);
        m.abi.major = ABI_VERSION.major; // isolate the id from the abi gate

        let mut empty = m.clone();
        empty.id.clear();
        assert!(matches!(empty.validate(), Err(ManifestError::EmptyId)), "empty id accepted");

        // Injecting a character outside `[a-z0-9_]` must fail validation.
        let mut bad = m.clone();
        bad.id.push('A');
        assert!(matches!(bad.validate(), Err(ManifestError::InvalidId)), "uppercase id accepted");
    });
}

#[test]
fn an_empty_or_non_wasm_entry_is_rejected() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut m = build(s);
        m.abi.major = ABI_VERSION.major;

        let mut empty = m.clone();
        empty.entry.clear();
        assert!(matches!(empty.validate(), Err(ManifestError::EmptyEntry)), "empty entry accepted");

        let mut wrong = m.clone();
        wrong.entry = alloc::string::String::from("mod.txt");
        assert!(
            matches!(wrong.validate(), Err(ManifestError::InvalidEntry)),
            "non-.wasm entry accepted"
        );
    });
}
