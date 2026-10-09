//! Authoring the art a status wears (stormlight/server#171).
//!
//! The pdk only spells the declaration, so the properties are about that spelling:
//!   - **The buff is named, not numbered**: every status visual's buff handle
//!     resolves through the bundle's `buffs` name table to the name it was
//!     declared under, so the host can bridge it to the gameplay mod's buff;
//!   - **The short way shows everyone the same**; refining one half leaves the
//!     other as it was;
//!   - **The declaration reaches the host intact**, through the bytes a guest
//!     hands over.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::status_visual::Viewer;
use stormlight_mod_sdk::abi::visuals::{ClientRegistration, PrimitiveShape, VisualModel};
use stormlight_mod_sdk::attach::point;
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;
use stormlight_mod_sdk::status::status;

const BUFFS: [&str; 3] = ["warded", "harried", "hastened"];

/// How one half of a declaration is refined, if at all.
#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Half {
    /// Left as the short way set it: the shared look.
    Shared,
    /// Hidden from this viewer.
    Hidden,
    /// A look of its own.
    Distinct,
}

#[derive(Debug, TypeGenerator)]
struct Declared {
    buff: u8,
    own: Half,
    others: Half,
    hung: bool,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    #[generator(bolero::produce::<Vec<Declared>>().with().len(0usize..=6))]
    declared: Vec<Declared>,
}

fn look(tint: f32) -> VisualModel {
    VisualModel::Primitive { shape: PrimitiveShape::Sphere, color: [tint, 0.5, 0.5, 1.0] }
}

const SHARED: f32 = 0.1;
const OWN: f32 = 0.2;
const OTHERS: f32 = 0.3;

fn expected(half: Half, distinct: f32) -> Option<VisualModel> {
    match half {
        Half::Shared => Some(look(SHARED)),
        Half::Hidden => None,
        Half::Distinct => Some(look(distinct)),
    }
}

#[test]
fn a_status_visual_reaches_the_host_under_its_buffs_name() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ClientContext::new();
        for d in &s.declared {
            let name = BUFFS[usize::from(d.buff) % BUFFS.len()];
            let mut spec = status(name, look(SHARED));
            match d.own {
                Half::Shared => {}
                Half::Hidden => spec = spec.own(None),
                Half::Distinct => spec = spec.own(Some(look(OWN))),
            }
            match d.others {
                Half::Shared => {}
                Half::Hidden => spec = spec.others(None),
                Half::Distinct => spec = spec.others(Some(look(OTHERS))),
            }
            if d.hung {
                spec = spec.at(point("Ref_Center"));
            }
            ctx.declare_status(spec);
        }
        let bundle = ctx.finish();
        let back: ClientRegistration =
            postcard::from_bytes(&to_bytes_client(&bundle)).expect("a bundle decodes");
        assert_eq!(back, bundle, "the bundle survives the trip");

        assert_eq!(back.status_visuals.len(), s.declared.len());
        for (d, visual) in s.declared.iter().zip(&back.status_visuals) {
            let name = back.names.buffs.get(usize::from(visual.buff.0)).map(String::as_str);
            assert_eq!(name, Some(BUFFS[usize::from(d.buff) % BUFFS.len()]), "{d:?}");
            let look = &visual.look;
            assert_eq!(look.seen_by(Viewer::Owner), expected(d.own, OWN).as_ref(), "{d:?}");
            assert_eq!(look.seen_by(Viewer::Other), expected(d.others, OTHERS).as_ref(), "{d:?}");
            assert_eq!(look.attach.is_some(), d.hung, "{d:?}");
        }
    });
}
