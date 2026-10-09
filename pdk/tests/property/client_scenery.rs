//! `ClientContext::scenery` invariants — how a map's cosmetic half declares the
//! static art it is dressed in. Laws:
//!   - **Declaration order is kept**: pieces reach the registration in the order
//!     they were declared, so a map that lays its ground first draws it first.
//!   - **Nothing is interned**: scenery names no unit, ability or talent, so
//!     declaring it adds nothing to any name table — a map can ship a thousand
//!     props without growing the id space a gameplay mod lives in.
//!   - **Emit/decode**: the pieces survive `finish` → postcard → decode intact.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::scenery::{SceneryPiece, SceneryPlacement};
use stormlight_mod_sdk::abi::visuals::ModelClips;
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;

#[derive(Debug, TypeGenerator)]
struct Scenario {
    pieces: u8,
    placements: u8,
    animated: bool,
}

fn piece(i: usize, placements: usize, animated: bool) -> SceneryPiece {
    SceneryPiece {
        asset: format!("mod://map/scenery/p{i}.glb"),
        clips: if animated {
            ModelClips { live: "idle".into(), ..ModelClips::default() }
        } else {
            ModelClips::default()
        },
        placements: (0..placements)
            .map(|k| SceneryPlacement {
                translation: [k as f32, 0.0, i as f32],
                ..SceneryPlacement::IDENTITY
            })
            .collect(),
    }
}

#[test]
fn pieces_arrive_in_declaration_order() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (n, k) = (usize::from(s.pieces % 10), usize::from(s.placements % 12));
        let mut ctx = ClientContext::new();
        for i in 0..n {
            ctx.scenery(piece(i, k, s.animated));
        }
        let reg = ctx.finish();
        assert_eq!(reg.scenery.len(), n);
        for (i, p) in reg.scenery.iter().enumerate() {
            assert_eq!(p, &piece(i, k, s.animated), "piece {i} out of place or altered");
        }
    });
}

#[test]
fn scenery_interns_nothing() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ClientContext::new();
        for i in 0..usize::from(s.pieces % 10) {
            ctx.scenery(piece(i, usize::from(s.placements % 12), s.animated));
        }
        let reg = ctx.finish();
        assert!(
            reg.names.units.is_empty()
                && reg.names.abilities.is_empty()
                && reg.names.talents.is_empty()
        );
        assert!(reg.names.anim_states.is_empty() && reg.names.events.is_empty());
    });
}

#[test]
fn declared_scenery_survives_the_guest_encoding() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ClientContext::new();
        for i in 0..usize::from(s.pieces % 10) {
            ctx.scenery(piece(i, usize::from(s.placements % 12), s.animated));
        }
        let reg = ctx.finish();
        let decoded = postcard::from_bytes(&to_bytes_client(&reg)).expect("decode");
        assert_eq!(reg, decoded, "scenery did not survive the guest encoding");
    });
}

#[test]
fn the_last_declared_ground_is_the_one_registered() {
    check!().with_type::<(u8, u8)>().for_each(|(a, b)| {
        let field = |v: u8| stormlight_mod_sdk::abi::scenery::HeightField {
            origin: [0.0; 2],
            cell: 1.0,
            size: [1, 1],
            heights: vec![f32::from(v)],
        };
        let mut ctx = ClientContext::new();
        assert!(ClientContext::new().finish().ground.is_none(), "no declaration, no field");
        ctx.ground(field(*a));
        ctx.ground(field(*b));
        let reg = ctx.finish();
        assert_eq!(reg.ground, Some(field(*b)));
        let decoded = postcard::from_bytes(&to_bytes_client(&reg)).expect("decode");
        assert_eq!(reg, decoded);
    });
}
