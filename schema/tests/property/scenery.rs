//! Invariants of map scenery (`SceneryPiece`) — the static art a map is dressed
//! in: its ground, its merged props, and the few models that animate in place.
//! Directional / structural only:
//!   - **Round-trip**: a registration carrying scenery survives a postcard
//!     serialize / deserialize unchanged and re-serializes to identical bytes —
//!     scenery crosses the wasm boundary inside `ClientRegistration` like every
//!     other cosmetic.
//!   - **Placements are kept whole and in order**: every placement of every
//!     piece comes back with its translation, rotation and scale intact, because
//!     a map's thousand props are only right if each one lands where it was put.
//!   - **Identity is a transform**: `SceneryPlacement::IDENTITY` is the neutral
//!     placement — zero translation, the unit quaternion, unit scale — which is
//!     what a piece of art already authored in world space is placed with.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::scenery::{SceneryPiece, SceneryPlacement};
use stormlight_mod_abi::visuals::{ClientRegistration, ModelClips};

/// One placement's raw translation / rotation / scale.
type RawPlacement = ([i16; 3], [i8; 4], [u8; 3]);

#[derive(Debug, TypeGenerator)]
struct Scenario {
    pieces: Vec<(u8, bool, Vec<RawPlacement>)>,
}

fn build(s: &Scenario) -> Vec<SceneryPiece> {
    s.pieces
        .iter()
        .take(8)
        .map(|(n, animated, at)| SceneryPiece {
            asset: format!("mod://map/scenery/piece_{n}.glb"),
            clips: if *animated {
                ModelClips { birth: String::new(), live: format!("idle_{n}") }
            } else {
                ModelClips::default()
            },
            placements: at
                .iter()
                .take(16)
                .map(|(t, r, sc)| SceneryPlacement {
                    translation: t.map(|v| f32::from(v) / 8.0),
                    rotation: r.map(|v| f32::from(v) / 127.0),
                    scale: sc.map(|v| f32::from(v) / 64.0),
                })
                .collect(),
        })
        .collect()
}

#[test]
fn scenery_round_trips_through_the_registration() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = ClientRegistration { scenery: build(s), ..ClientRegistration::default() };
        let bytes = postcard::to_allocvec(&reg).expect("encode");
        let back: ClientRegistration = postcard::from_bytes(&bytes).expect("decode");
        assert_eq!(back, reg, "scenery changed across the encoding");
        assert_eq!(postcard::to_allocvec(&back).expect("re-encode"), bytes, "re-encoding differs");
    });
}

#[test]
fn every_placement_survives_in_order() {
    check!().with_type::<Scenario>().for_each(|s| {
        let pieces = build(s);
        let reg = ClientRegistration { scenery: pieces.clone(), ..ClientRegistration::default() };
        let back: ClientRegistration =
            postcard::from_bytes(&postcard::to_allocvec(&reg).expect("encode")).expect("decode");
        assert_eq!(back.scenery.len(), pieces.len());
        for (a, b) in back.scenery.iter().zip(&pieces) {
            assert_eq!(a.asset, b.asset);
            assert_eq!(a.clips, b.clips);
            assert_eq!(a.placements.len(), b.placements.len(), "a placement was dropped");
            for (p, q) in a.placements.iter().zip(&b.placements) {
                assert_eq!(p.translation.map(f32::to_bits), q.translation.map(f32::to_bits));
                assert_eq!(p.rotation.map(f32::to_bits), q.rotation.map(f32::to_bits));
                assert_eq!(p.scale.map(f32::to_bits), q.scale.map(f32::to_bits));
            }
        }
    });
}

#[test]
fn identity_placement_is_neutral() {
    check!().with_type::<[i16; 3]>().for_each(|p| {
        let id = SceneryPlacement::IDENTITY;
        assert_eq!(id.translation, [0.0; 3]);
        assert_eq!(id.scale, [1.0; 3]);
        let len: f32 = id.rotation.iter().map(|c| c * c).sum();
        assert!(
            (len - 1.0).abs() < 1e-6 && (id.rotation[3] - 1.0).abs() < 1e-6,
            "not the unit quaternion"
        );
        // Applying it to a point leaves the point where it was: no translation,
        // no turn, no scale.
        let v = p.map(f32::from);
        let moved: [f32; 3] = std::array::from_fn(|i| v[i] * id.scale[i] + id.translation[i]);
        assert_eq!(moved, v);
    });
}
