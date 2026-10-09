//! The art a status wears on the unit it affects (stormlight/server#171).
//!
//! Invariants:
//!   - **Each viewer sees their own half**: the affected unit's own player sees
//!     the `own` look and everyone else the `others` look — either may be nothing,
//!     and neither falls back to the other;
//!   - **A declaration that shows nothing to anyone says so**, so the host can
//!     refuse it rather than adopt a status visual that can never be seen;
//!   - **It survives the trip to the host** inside a client registration, and its
//!     buff is the one handle a remap rewrites — a failing buff map errors exactly
//!     when the bundle declares a status visual.

use core::cell::Cell;

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::attach::AttachPoint;
use stormlight_mod_abi::descriptors::Names;
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::manifest::ABI_VERSION;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::status_visual::{StatusLook, StatusVisual, Viewer};
use stormlight_mod_abi::visuals::{ClientRegistration, PrimitiveShape, VisualModel};

/// One half of a declaration: nothing, or a look told apart by its colour.
#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Look {
    Nothing,
    Tinted(u8),
}

impl Look {
    fn model(self) -> Option<VisualModel> {
        match self {
            Self::Nothing => None,
            Self::Tinted(t) => Some(VisualModel::Primitive {
                shape: PrimitiveShape::Sphere,
                color: [f32::from(t) / 255.0, 0.5, 0.5, 1.0],
            }),
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Declared {
    buff: u8,
    own: Look,
    others: Look,
    hung: bool,
}

impl Declared {
    fn visual(&self) -> StatusVisual {
        StatusVisual {
            buff: BuffId(u16::from(self.buff)),
            look: StatusLook {
                own: self.own.model(),
                others: self.others.model(),
                attach: self.hung.then(|| AttachPoint::new("Ref_Center")),
            },
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    #[generator(bolero::produce::<Vec<Declared>>().with().len(0usize..=6))]
    declared: Vec<Declared>,
}

#[test]
fn each_viewer_sees_their_own_half_and_nothing_else() {
    check!().with_type::<Declared>().for_each(|d| {
        let visual = d.visual().look;
        assert_eq!(visual.seen_by(Viewer::Owner), d.own.model().as_ref(), "{d:?}");
        assert_eq!(visual.seen_by(Viewer::Other), d.others.model().as_ref(), "{d:?}");
        assert_eq!(
            visual.shows_anything(),
            d.own.model().is_some() || d.others.model().is_some(),
            "{d:?}",
        );
    });
}

/// An identity map that counts the buff handles it was asked about, and can be
/// told to refuse them.
#[derive(Default)]
struct Buffs {
    seen: Cell<u32>,
    refuse: bool,
}

impl IdMap for Buffs {
    type Error = ();
    fn stat(&self, id: StatId) -> Result<StatId, ()> {
        Ok(id)
    }
    fn resource(&self, id: ResourceId) -> Result<ResourceId, ()> {
        Ok(id)
    }
    fn stack(&self, id: StackId) -> Result<StackId, ()> {
        Ok(id)
    }
    fn tag(&self, id: TagId) -> Result<TagId, ()> {
        Ok(id)
    }
    fn tag_class(&self, id: TagClassId) -> Result<TagClassId, ()> {
        Ok(id)
    }
    fn param(&self, id: ParamId) -> Result<ParamId, ()> {
        Ok(id)
    }
    fn event(&self, id: EventId) -> Result<EventId, ()> {
        Ok(id)
    }
    fn buff(&self, id: BuffId) -> Result<BuffId, ()> {
        self.seen.set(self.seen.get() + 1);
        if self.refuse { Err(()) } else { Ok(BuffId(id.0 + 1)) }
    }
    fn curve(&self, id: CurveId) -> Result<CurveId, ()> {
        Ok(id)
    }
    fn damage_type(&self, id: DamageTypeId) -> Result<DamageTypeId, ()> {
        Ok(id)
    }
    fn ability(&self, id: AbilityId) -> Result<AbilityId, ()> {
        Ok(id)
    }
    fn talent(&self, id: TalentId) -> Result<TalentId, ()> {
        Ok(id)
    }
    fn handler(&self, id: HandlerId) -> Result<HandlerId, ()> {
        Ok(id)
    }
    fn unit(&self, id: UnitId) -> Result<UnitId, ()> {
        Ok(id)
    }
    fn navmesh(&self, id: NavMeshId) -> Result<NavMeshId, ()> {
        Ok(id)
    }
    fn anim_state(&self, id: AnimStateId) -> Result<AnimStateId, ()> {
        Ok(id)
    }
}

fn registration(s: &Scenario) -> ClientRegistration {
    ClientRegistration {
        abi: ABI_VERSION,
        names: Names {
            buffs: (0..=u8::MAX).map(|b| format!("buff{b}")).collect(),
            ..Names::default()
        },
        status_visuals: s.declared.iter().map(Declared::visual).collect(),
        ..ClientRegistration::default()
    }
}

#[test]
fn a_status_visual_reaches_the_host_and_remaps_its_buff() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = registration(s);
        let bytes = postcard::to_allocvec(&reg).expect("a bundle encodes");
        let back: ClientRegistration = postcard::from_bytes(&bytes).expect("and decodes");
        assert_eq!(back, reg, "the bundle survives the trip");

        let map = Buffs::default();
        let mut shifted = reg.clone();
        shifted.remap_ids(&map).expect("a total map never fails");
        assert_eq!(map.seen.get() as usize, s.declared.len(), "every buff was asked about once");
        for (before, after) in reg.status_visuals.iter().zip(&shifted.status_visuals) {
            assert_eq!(after.buff.0, before.buff.0 + 1, "the buff went through the map");
            assert_eq!(after.look, before.look, "a remap touched something that is not a handle");
        }

        let mut refused = reg.clone();
        let result = refused.remap_ids(&Buffs { refuse: true, ..Buffs::default() });
        assert_eq!(result.is_err(), !s.declared.is_empty(), "{s:?}");
    });
}
