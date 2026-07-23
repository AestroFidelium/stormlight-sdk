//! `ClientContext` invariants — a cosmetic mod builds visuals by name, and
//! `finish` must produce a `ClientRegistration` whose `units`/`abilities` name
//! tables line up with the handles the declared visuals carry (so the host can map
//! each to the gameplay mod's global id by name). Laws:
//!   - **Reachable**: every declared unit/effect visual's handle resolves to its
//!     name in the matching `Names` table.
//!   - **Role-independent ability interning**: declaring two roles for one ability
//!     name reuses a single handle (one `abilities` entry, one per interned name).
//!   - **Emit/decode**: `finish` → postcard → decode reproduces the exact bundle.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::manifest::ABI_VERSION;
use stormlight_mod_sdk::abi::visuals::{EffectRole, PrimitiveShape, VisualModel};
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;

fn role_of(sel: u8) -> EffectRole {
    match sel % 3 {
        0 => EffectRole::Projectile,
        1 => EffectRole::Impact,
        _ => EffectRole::CastIndicator,
    }
}

fn a_model(seed: u8) -> VisualModel {
    VisualModel::Primitive {
        shape: match seed % 3 {
            0 => PrimitiveShape::Cube,
            1 => PrimitiveShape::Sphere,
            _ => PrimitiveShape::Capsule,
        },
        color: [0.1, 0.2, 0.3, 1.0],
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    units: u8,
    abilities: u8,
    roles: [u8; 8],
}

#[test]
fn finish_makes_every_declared_visual_reachable_by_name() {
    check!().with_type::<Scenario>().for_each(|s| {
        let nu = (s.units % 6) as usize;
        let na = (s.abilities % 6) as usize;

        let mut ctx = ClientContext::new();
        for i in 0..nu {
            ctx.unit_visual(&format!("u{i}"), a_model(i as u8));
        }
        for j in 0..na {
            ctx.effect_visual(&format!("a{j}"), role_of(s.roles[j % 8]), a_model(j as u8));
        }
        let reg = ctx.finish();

        assert_eq!(reg.abi, ABI_VERSION);
        assert_eq!(reg.visuals.len(), nu, "one unit visual per declared unit");
        assert_eq!(reg.effects.len(), na, "one effect visual per declared ability");
        assert_eq!(reg.names.units.len(), nu, "one unit name per distinct unit");
        assert_eq!(reg.names.abilities.len(), na, "one ability name per distinct ability");

        // Every unit visual's handle resolves to its declared name.
        for v in &reg.visuals {
            let name = reg.names.units.get(v.unit.0 as usize).expect("unit handle in table");
            assert!(name.starts_with('u'), "unexpected unit name `{name}`");
        }
        // Every effect's handle resolves to its declared ability name.
        for e in &reg.effects {
            let name = reg.names.abilities.get(e.ability.0 as usize).expect("ability handle in table");
            assert!(name.starts_with('a'), "unexpected ability name `{name}`");
        }
    });
}

#[test]
fn one_ability_can_declare_multiple_roles_under_one_handle() {
    check!().with_type::<u8>().for_each(|&seed| {
        let mut ctx = ClientContext::new();
        // Same ability name, three roles: interning must reuse one handle.
        let p = ctx.effect_visual("bolt", EffectRole::Projectile, a_model(seed));
        let i = ctx.effect_visual("bolt", EffectRole::Impact, a_model(seed));
        let c = ctx.effect_visual("bolt", EffectRole::CastIndicator, a_model(seed));
        assert_eq!(p, i, "same ability name must reuse its handle");
        assert_eq!(i, c, "same ability name must reuse its handle");

        let reg = ctx.finish();
        assert_eq!(reg.names.abilities.len(), 1, "one interned ability name");
        assert_eq!(reg.effects.len(), 3, "three role-distinct effect visuals");
    });
}

#[test]
fn a_built_client_registration_decodes_equal() {
    check!().with_type::<Scenario>().for_each(|s| {
        let nu = (s.units % 6) as usize;
        let na = (s.abilities % 6) as usize;

        let mut ctx = ClientContext::new();
        for i in 0..nu {
            ctx.unit_visual(&format!("u{i}"), a_model(i as u8));
        }
        for j in 0..na {
            ctx.effect_visual(&format!("a{j}"), role_of(s.roles[j % 8]), a_model(j as u8));
        }
        let reg = ctx.finish();

        let decoded = postcard::from_bytes(&to_bytes_client(&reg)).expect("decode");
        assert_eq!(reg, decoded);
    });
}
