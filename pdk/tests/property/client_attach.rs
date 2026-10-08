//! Authoring a visual hung on a point of a rig (stormlight/server#160).
//!
//! The pdk only spells the request, so the properties are about that spelling:
//!   - [`point`] and [`AttachExt`] build exactly the request they name, each
//!     setter touching its own field and no other;
//!   - [`ClientContext::effect_visual_at`] carries the request into the
//!     registration beside the visual, where [`ClientContext::effect_visual`]
//!     carries none — and the whole bundle survives the trip to the host.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::visuals::{
    ClientRegistration, EffectRole, PrimitiveShape, VisualModel,
};
use stormlight_mod_sdk::attach::{AttachExt, point};
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;

/// A point name the request might carry.
#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Name {
    Weapon,
    Chest,
    Head,
}

impl Name {
    fn text(self) -> &'static str {
        match self {
            Self::Weapon => "Ref_Weapon",
            Self::Chest => "Ref_Chest",
            Self::Head => "Ref_Head",
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Request {
    point: Name,
    fallback: Option<Name>,
    /// The offset, in hundredths of a world unit.
    offset: [i16; 3],
    upright: bool,
}

fn model() -> VisualModel {
    VisualModel::Primitive { shape: PrimitiveShape::Sphere, color: [1.0; 4] }
}

fn build(r: &Request) -> stormlight_mod_sdk::abi::attach::AttachPoint {
    let mut request = point(r.point.text()).offset(r.offset.map(|v| f32::from(v) / 100.0));
    if let Some(fallback) = r.fallback {
        request = request.or(fallback.text());
    }
    if r.upright {
        request = request.upright();
    }
    request
}

#[test]
fn the_builder_names_exactly_the_request_it_was_given() {
    check!().with_type::<Request>().for_each(|r| {
        let request = build(r);
        assert_eq!(request.point, r.point.text());
        assert_eq!(request.fallback.as_deref(), r.fallback.map(Name::text));
        assert_eq!(request.offset, r.offset.map(|v| f32::from(v) / 100.0));
        assert_eq!(request.upright, r.upright);
        assert!(request.is_usable(), "the builder made a request no rig can be asked");
    });
}

#[test]
fn a_visual_at_a_point_reaches_the_host_with_its_point() {
    check!().with_type::<Request>().for_each(|r| {
        let mut ctx = ClientContext::new();
        ctx.effect_visual("bolt", EffectRole::Projectile, model());
        ctx.effect_visual_at("bolt", EffectRole::CastIndicator, model(), build(r));
        let bundle = ctx.finish();
        let back: ClientRegistration =
            postcard::from_bytes(&to_bytes_client(&bundle)).expect("a bundle decodes");
        assert_eq!(back, bundle, "a visual's point did not survive the trip to the host");
        let at = |role| back.effects.iter().find(|e| e.role == role).expect("declared");
        assert_eq!(at(EffectRole::Projectile).attach, None, "a plain visual grew a point");
        assert_eq!(at(EffectRole::CastIndicator).attach, Some(build(r)));
    });
}
