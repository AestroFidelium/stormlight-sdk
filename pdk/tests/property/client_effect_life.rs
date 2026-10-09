//! Authoring how long an effect lives and what it does when its unit goes
//! (stormlight/server#167).
//!
//! The pdk only spells the declaration, so the properties are about that spelling:
//!   - **Each refinement touches its own field**: saying how long an ability's
//!     visual lasts leaves its point alone and the other way round, in either order;
//!   - **Saying nothing is the default**: a visual declared the short way carries
//!     the default lifetime, and a named effect declared the short way follows its
//!     host;
//!   - **The declaration reaches the host intact**, through the same bytes a guest
//!     hands over.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::lifetime::{EffectLifetime, HostEnd};
use stormlight_mod_sdk::abi::visuals::{
    ClientRegistration, EffectRole, PrimitiveShape, VisualModel,
};
use stormlight_mod_sdk::attach::point;
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;
use stormlight_mod_sdk::effects::{NamedEffectExt, effect, named_effect};

#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Lasting {
    Unsaid,
    Default,
    /// Tenths of a second.
    Seconds(#[generator(1u8..=50)] u8),
    Art,
}

impl Lasting {
    fn declared(self) -> Option<EffectLifetime> {
        match self {
            Self::Unsaid => None,
            Self::Default => Some(EffectLifetime::Default),
            Self::Seconds(tenths) => Some(EffectLifetime::Seconds(f32::from(tenths) / 10.0)),
            Self::Art => Some(EffectLifetime::Art),
        }
    }

    fn expected(self) -> EffectLifetime {
        self.declared().unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Order {
    PointFirst,
    LifetimeFirst,
}

/// How the named effect is declared to end with its host.
#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Ending {
    Unsaid,
    Follow,
    Detach,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    lasting: Lasting,
    hung: bool,
    order: Order,
    ending: Ending,
    named_lasting: Lasting,
}

fn model() -> VisualModel {
    VisualModel::Primitive { shape: PrimitiveShape::Sphere, color: [1.0; 4] }
}

fn build(s: &Scenario) -> ClientRegistration {
    let mut ctx = ClientContext::new();
    ctx.effect_visual("bolt", EffectRole::Projectile, model());
    let mut spec = effect("bolt", EffectRole::Impact, model());
    let point = s.hung.then(|| point("Ref_Chest"));
    match s.order {
        Order::PointFirst => {
            if let Some(point) = point {
                spec = spec.at(point);
            }
            if let Some(lifetime) = s.lasting.declared() {
                spec = spec.lasting(lifetime);
            }
        }
        Order::LifetimeFirst => {
            if let Some(lifetime) = s.lasting.declared() {
                spec = spec.lasting(lifetime);
            }
            if let Some(point) = point {
                spec = spec.at(point);
            }
        }
    }
    ctx.declare_effect(spec);

    let mut puff = named_effect("puff", model());
    if let Some(lifetime) = s.named_lasting.declared() {
        puff = puff.lasting(lifetime);
    }
    puff = match s.ending {
        Ending::Detach => puff.detached(),
        Ending::Follow => puff.on_host_end(HostEnd::Follow),
        Ending::Unsaid => puff,
    };
    ctx.declare_notify_effect(puff);
    ctx.finish()
}

#[test]
fn an_effects_life_reaches_the_host_as_it_was_declared() {
    check!().with_type::<Scenario>().for_each(|s| {
        let bundle = build(s);
        let back: ClientRegistration =
            postcard::from_bytes(&to_bytes_client(&bundle)).expect("a bundle decodes");
        assert_eq!(back, bundle, "the bundle survives the trip");

        let [shot, burst] = back.effects.as_slice() else {
            panic!("two visuals were declared, got {}", back.effects.len());
        };
        assert_eq!(shot.lifetime, EffectLifetime::Default, "the short way says nothing");
        assert_eq!(shot.attach, None);
        assert_eq!(burst.lifetime, s.lasting.expected(), "{s:?}");
        assert_eq!(burst.attach.as_ref().map(|p| p.point.as_str()), s.hung.then_some("Ref_Chest"));

        let [puff] = back.named_effects.as_slice() else {
            panic!("one named effect was declared");
        };
        assert_eq!(puff.lifetime, s.named_lasting.expected(), "{s:?}");
        let host_end =
            if matches!(s.ending, Ending::Detach) { HostEnd::Detach } else { HostEnd::Follow };
        assert_eq!(puff.on_host_end, host_end, "{s:?}");
    });
}
