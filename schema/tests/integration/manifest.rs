//! TOML parsing invariants for the manifest (host side, `manifest-parse`).
//!
//! The engine loads a mod by reading its `manifest.toml`. These laws pin the
//! parse path end to end:
//!   - **Authored-TOML round-trip**: a canonical TOML rendering of any
//!     well-formed manifest parses back to that exact manifest.
//!   - **A hand-written fixture** (the shape real mods ship) parses to the
//!     expected structured values, including the `abi` gate.
//!
//! Malformed-input robustness lives in the fuzz binary; here inputs are
//! well-formed by construction.
#![cfg(feature = "manifest-parse")]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::manifest::{
    ABI_VERSION, Dependency, Manifest, ModKind, Version, parse_manifest,
};

const IDENT: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";

/// Seed bytes -> a valid, TOML-safe lowercase identifier.
fn ident(seed: &[u8], fallback: char) -> String {
    let mut s = String::new();
    for &b in seed {
        s.push(IDENT[b as usize % IDENT.len()] as char);
    }
    if s.chars().next().is_none_or(|c| !c.is_ascii_lowercase()) {
        s.insert(0, fallback);
    }
    s
}

fn ver((a, b, c): (u16, u16, u16)) -> Version {
    Version::new(u32::from(a), u32::from(b), u32::from(c))
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    id_seed: Vec<u8>,
    name_seed: Vec<u8>,
    ver: (u16, u16, u16),
    abi: (u16, u16, u16),
    server: bool,
    entry_seed: Vec<u8>,
    deps: Vec<(Vec<u8>, (u16, u16, u16))>,
    assets: Vec<Vec<u8>>,
}

/// Build a well-formed manifest whose every string field is TOML-safe, so it
/// can be rendered to canonical TOML without escaping concerns.
fn build(s: &Scenario) -> Manifest {
    // Pin the ABI major to the engine's so `parse_manifest`'s validation gate
    // accepts the manifest; the round-trip is about the parse, not the gate.
    let mut abi = ver(s.abi);
    abi.major = ABI_VERSION.major;
    Manifest {
        id: ident(&s.id_seed, 'm'),
        name: ident(&s.name_seed, 'n'),
        version: ver(s.ver),
        kind: if s.server { ModKind::Server } else { ModKind::Client },
        entry: format!("{}.wasm", ident(&s.entry_seed, 'e')),
        abi,
        deps: s
            .deps
            .iter()
            .map(|(seed, v)| Dependency { id: ident(seed, 'd'), version: ver(*v) })
            .collect(),
        assets: s.assets.iter().map(|seed| format!("{}.bin", ident(seed, 'a'))).collect(),
    }
}

/// Render a manifest to a canonical `manifest.toml` string (test-only; the
/// engine only ever *parses*).
fn to_toml(m: &Manifest) -> String {
    let kind = match m.kind {
        ModKind::Server => "server",
        ModKind::Client => "client",
    };
    let mut out = String::new();
    out.push_str(&format!("id = \"{}\"\n", m.id));
    out.push_str(&format!("name = \"{}\"\n", m.name));
    out.push_str(&format!("version = \"{}\"\n", m.version));
    out.push_str(&format!("kind = \"{kind}\"\n"));
    out.push_str(&format!("entry = \"{}\"\n", m.entry));
    out.push_str(&format!("abi = \"{}\"\n", m.abi));
    let assets: Vec<String> = m.assets.iter().map(|a| format!("\"{a}\"")).collect();
    out.push_str(&format!("assets = [{}]\n", assets.join(", ")));
    for d in &m.deps {
        out.push_str(&format!("\n[[deps]]\nid = \"{}\"\nversion = \"{}\"\n", d.id, d.version));
    }
    out
}

#[test]
fn canonical_toml_round_trips_to_the_same_manifest() {
    check!().with_type::<Scenario>().for_each(|s| {
        let m = build(s);
        let src = to_toml(&m);
        let parsed = parse_manifest(&src).unwrap_or_else(|e| panic!("parse failed: {e:?}\n{src}"));
        assert_eq!(parsed, m, "TOML did not round-trip\n{src}");
    });
}

#[test]
fn a_hand_written_fixture_parses_to_expected_values() {
    let src = format!(
        "id = \"base\"\n\
         name = \"Stormlight Base\"\n\
         version = \"0.2.1\"\n\
         kind = \"server\"\n\
         entry = \"stormlight_mod_base.wasm\"\n\
         abi = \"{}.0.0\"\n",
        ABI_VERSION.major
    );
    let m = parse_manifest(&src).expect("fixture must parse");
    assert_eq!(m.id, "base");
    assert_eq!(m.name, "Stormlight Base");
    assert_eq!(m.version, Version::new(0, 2, 1));
    assert_eq!(m.kind, ModKind::Server);
    assert_eq!(m.entry, "stormlight_mod_base.wasm");
    assert_eq!(m.abi.major, ABI_VERSION.major);
    assert!(m.deps.is_empty());
    assert!(m.assets.is_empty());
}
