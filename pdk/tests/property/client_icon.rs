//! `ClientContext::ability_icon` invariants (server#94) — the icon an ability
//! wears in an interface, declared beside that ability's other cosmetics. Laws:
//!   - **Reachable by name**: every declared icon's handle resolves to the
//!     ability name it was declared under, which is the only thing adoption has
//!     to bridge it to the gameplay mod's global id with.
//!   - **One ability, one handle**: an ability that declares both an icon and a
//!     feedback visual interns once, so the picture and the projectile are facts
//!     about the same ability rather than two entries that could drift apart.
//!   - **Emit/decode**: the icons survive `finish` → postcard → decode intact,
//!     path included — the wasm boundary is where a dropped field would go
//!     unnoticed.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::visuals::{EffectRole, PrimitiveShape, VisualModel};
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;

fn a_model() -> VisualModel {
    VisualModel::Primitive { shape: PrimitiveShape::Sphere, color: [0.1, 0.2, 0.3, 1.0] }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// How many abilities the mod gives an icon to.
    icons: u8,
    /// Whether those abilities also declare a projectile — the case that pins
    /// the icon and the feedback onto one interned handle.
    also_feedback: bool,
}

#[test]
fn every_declared_icon_is_reachable_by_its_ability_name() {
    check!().with_type::<Scenario>().for_each(|s| {
        let n = (s.icons % 6) as usize;
        let mut ctx = ClientContext::new();
        for i in 0..n {
            if s.also_feedback {
                ctx.effect_visual(&format!("a{i}"), EffectRole::Projectile, a_model());
            }
            ctx.ability_icon(&format!("a{i}"), &format!("mod://pack/icon_{i}.png"));
        }
        let reg = ctx.finish();

        assert_eq!(reg.icons.len(), n, "one icon per ability given one");
        assert_eq!(reg.names.abilities.len(), n, "one ability name per distinct ability");
        for icon in &reg.icons {
            let name = reg
                .names
                .abilities
                .get(icon.ability.0 as usize)
                .expect("an icon's ability handle must be in the name table");
            assert_eq!(
                icon.image,
                format!("mod://pack/icon_{}.png", &name[1..]),
                "icon `{}` landed on the handle of ability `{name}`",
                icon.image,
            );
        }
    });
}

#[test]
fn an_icon_and_a_feedback_visual_share_one_ability_handle() {
    check!().with_type::<u8>().for_each(|&seed| {
        let mut ctx = ClientContext::new();
        let icon = ctx.ability_icon("bolt", "mod://pack/bolt.png");
        let shot = ctx.effect_visual("bolt", EffectRole::Projectile, a_model());
        let again = ctx.ability_icon("bolt", &format!("mod://pack/bolt_{seed}.png"));

        assert_eq!(icon, shot, "the same ability name must reuse its handle");
        assert_eq!(icon, again, "re-declaring an icon must reuse the handle, not intern a second");

        let reg = ctx.finish();
        assert_eq!(reg.names.abilities.len(), 1, "one interned ability name");
        assert_eq!(reg.icons.len(), 2, "both declarations are carried; adoption picks the later");
    });
}

#[test]
fn declared_icons_survive_the_guest_encoding() {
    check!().with_type::<Scenario>().for_each(|s| {
        let n = (s.icons % 6) as usize;
        let mut ctx = ClientContext::new();
        for i in 0..n {
            ctx.ability_icon(&format!("a{i}"), &format!("mod://pack/icon_{i}.png"));
        }
        let reg = ctx.finish();

        let decoded = postcard::from_bytes(&to_bytes_client(&reg)).expect("decode");
        assert_eq!(reg, decoded, "icons did not survive the guest encoding");
    });
}
