//! Authoring what lies under a unit (stormlight/server#181).
//!
//! The pdk only spells the declaration, so the properties are about that spelling:
//!   - **The short way is a plain mark**: the role and picture named, for every
//!     relation, untinted, painted over, exactly the body's width, keeping the
//!     world's orientation, unturned;
//!   - **each refinement sets what it names and nothing else**, in any order;
//!   - **declarations keep their order** — it is their precedence — and reach the
//!     host intact, through the bytes a guest hands over.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::decal::DecalBlend;
use stormlight_mod_sdk::abi::unit_mark::{MarkRole, Relation};
use stormlight_mod_sdk::abi::visuals::ClientRegistration;
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;
use stormlight_mod_sdk::marks::mark;

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Role {
    Driven,
    Selected,
    Target,
    Hovered,
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Side {
    Own,
    Ally,
    Enemy,
    Neutral,
}

/// One declaration and the refinements it was given.
#[derive(Debug, TypeGenerator)]
struct Declared {
    role: Role,
    only: Option<Side>,
    /// The tint's red, in 255ths.
    tint: Option<u8>,
    additive: bool,
    /// Tenths of the body's width.
    #[generator(1u8..=40)]
    scale: u8,
    scaled: bool,
    turning: bool,
    /// Quarter turns of the picture, if any.
    #[generator(bolero::produce::<Option<u8>>().with().value(1u8..=3))]
    turned: Option<u8>,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    #[generator(bolero::produce::<Vec<Declared>>().with().len(0usize..=6))]
    declared: Vec<Declared>,
}

fn role(r: Role) -> MarkRole {
    match r {
        Role::Driven => MarkRole::Driven,
        Role::Selected => MarkRole::Selected,
        Role::Target => MarkRole::Target,
        Role::Hovered => MarkRole::Hovered,
    }
}

fn side(s: Side) -> Relation {
    match s {
        Side::Own => Relation::Own,
        Side::Ally => Relation::Ally,
        Side::Enemy => Relation::Enemy,
        Side::Neutral => Relation::Neutral,
    }
}

#[test]
fn a_mark_is_declared_as_spelled_and_in_order() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ClientContext::new();
        for (i, d) in s.declared.iter().enumerate() {
            let mut spec = mark(role(d.role), format!("mod://pack/mark{i}.png"));
            if let Some(only) = d.only {
                spec = spec.only(side(only));
            }
            if let Some(red) = d.tint {
                spec = spec.tint([f32::from(red) / 255.0, 0.5, 0.25, 1.0]);
            }
            if d.additive {
                spec = spec.additive();
            }
            if d.scaled {
                spec = spec.scale(f32::from(d.scale) / 10.0);
            }
            if d.turning {
                spec = spec.turning();
            }
            if let Some(quarters) = d.turned {
                spec = spec.turned(f32::from(quarters) * core::f32::consts::FRAC_PI_2);
            }
            ctx.mark_units(spec);
        }
        let reg = ctx.finish();
        let bytes = to_bytes_client(&reg);
        let back: ClientRegistration = postcard::from_bytes(&bytes).expect("the bundle decodes");
        assert_eq!(back.unit_marks, reg.unit_marks, "{s:?}: the marks did not survive the trip");
        assert_eq!(reg.unit_marks.len(), s.declared.len(), "{s:?}");

        for (i, (d, m)) in s.declared.iter().zip(&reg.unit_marks).enumerate() {
            assert_eq!(m.role, role(d.role), "{s:?}");
            assert_eq!(m.relation, d.only.map(side), "{s:?}");
            assert_eq!(m.look.asset, format!("mod://pack/mark{i}.png"), "{s:?}: out of order");
            let tint = d.tint.map_or([1.0; 4], |red| [f32::from(red) / 255.0, 0.5, 0.25, 1.0]);
            assert_eq!(m.look.tint, tint, "{s:?}");
            assert_eq!(m.look.blend, if d.additive { DecalBlend::Add } else { DecalBlend::Blend });
            let scale = if d.scaled { f32::from(d.scale) / 10.0 } else { 1.0 };
            assert_eq!(m.look.scale, scale, "{s:?}");
            assert_eq!(m.look.turns, d.turning, "{s:?}");
            let turn = d.turned.map_or(0.0, |q| f32::from(q) * core::f32::consts::FRAC_PI_2);
            assert_eq!(m.look.yaw_offset, turn, "{s:?}");
            assert!(m.look.is_valid(), "{s:?}: the pdk spelled a mark no renderer can draw");
        }
    });
}
