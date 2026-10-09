//! Asking a rig for a point to hang a visual on (stormlight/server#160) — the ABI
//! half.
//!
//! [`AttachPoint`] names the point a visual wants, a second one to try where the
//! rig lacks the first, an offset from it, and whether to keep only its heading.
//! It is asked in two places: by an ability's feedback ([`EffectVisualDescriptor`])
//! and by an animation notify ([`NotifyAttach::Point`]). Invariants:
//!
//!   - **It survives the trip to the host**, in both places, every field intact;
//!   - **A request that cannot be asked is refused at load**: a blank point, a blank
//!     fallback or a non-finite offset on a notify is a named error, not an effect
//!     quietly hung on the character's origin;
//!   - **Appended, not inserted**: a notify's existing ways of being placed keep
//!     their wire tags.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::animation::{
    AnimLayer, AnimState, AnimationDescriptor, AnimationError, BlendMode, BoneMask, ClipRef,
    RateBinding, StateClip,
};
use stormlight_mod_abi::attach::AttachPoint;
use stormlight_mod_abi::ids::{AbilityId, UnitId};
use stormlight_mod_abi::lifetime::EffectLifetime;
use stormlight_mod_abi::notify::{NotifyAction, NotifyAttach, NotifyPoint, NotifyTime};
use stormlight_mod_abi::visuals::{
    EffectRole, EffectVisualDescriptor, PrimitiveShape, VisualModel,
};

/// A name a request might carry, blank ones included.
#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Name {
    Weapon,
    Chest,
    Head,
    Blank,
}

impl Name {
    fn text(self) -> String {
        match self {
            Self::Weapon => "Ref_Weapon".into(),
            Self::Chest => "Ref_Chest".into(),
            Self::Head => "Ref_Head".into(),
            Self::Blank => String::new(),
        }
    }
}

/// An offset component, hostile ones included.
#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Coord {
    Hundredths(i16),
    Undefined,
}

impl Coord {
    fn value(self) -> f32 {
        match self {
            Self::Hundredths(v) => f32::from(v) / 100.0,
            Self::Undefined => f32::NAN,
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Request {
    point: Name,
    fallback: Option<Name>,
    offset: [Coord; 3],
    upright: bool,
}

impl Request {
    fn attach(&self) -> AttachPoint {
        AttachPoint {
            point: self.point.text(),
            fallback: self.fallback.map(Name::text),
            offset: self.offset.map(Coord::value),
            upright: self.upright,
        }
    }

    /// Whether the generated request is one a rig can be asked.
    fn askable(&self) -> bool {
        !matches!(self.point, Name::Blank)
            && !matches!(self.fallback, Some(Name::Blank))
            && self.offset.iter().all(|c| matches!(c, Coord::Hundredths(_)))
    }
}

fn notify_on(attach: AttachPoint) -> AnimationDescriptor {
    AnimationDescriptor {
        unit: UnitId(0),
        mask_groups: Vec::new(),
        layers: vec![AnimLayer {
            name: "body".into(),
            mask: BoneMask::Whole,
            blend: BlendMode::Override,
            weight: 1.0,
            states: vec![StateClip {
                state: AnimState::Attack,
                clip: ClipRef { asset: "mod://p/a.glb".into(), clip: "swing".into() },
                window: None,
                looping: false,
                blend_in: 0.0,
                blend_out: 0.0,
                rate: RateBinding::Fixed(1.0),
                priority: 0,
                notifies: vec![NotifyPoint {
                    at: NotifyTime::Normalized(0.0),
                    action: NotifyAction::Effect {
                        key: "flare".into(),
                        attach: NotifyAttach::Point(attach),
                        lifetime: 0.5,
                    },
                }],
            }],
            transitions: Vec::new(),
        }],
    }
}

#[test]
fn a_request_survives_the_trip_to_the_host_in_both_places() {
    check!().with_type::<Request>().for_each(|r| {
        let effect = EffectVisualDescriptor {
            ability: AbilityId(3),
            role: EffectRole::CastIndicator,
            model: VisualModel::Primitive { shape: PrimitiveShape::Sphere, color: [1.0; 4] },
            attach: Some(r.attach()),
            lifetime: EffectLifetime::Default,
        };
        let bytes = postcard::to_allocvec(&effect).expect("an effect visual encodes");
        let back: EffectVisualDescriptor = postcard::from_bytes(&bytes).expect("and decodes");
        // NaN is not equal to itself, so compare what a request *means*: the names
        // exactly, the offset bit for bit.
        let sent = r.attach();
        let got = back.attach.expect("the request survived");
        assert_eq!(
            (got.point, got.fallback, got.upright),
            (sent.point.clone(), sent.fallback.clone(), sent.upright)
        );
        assert_eq!(got.offset.map(f32::to_bits), sent.offset.map(f32::to_bits));

        let anim = notify_on(r.attach());
        let bytes = postcard::to_allocvec(&anim).expect("an animation encodes");
        let back: AnimationDescriptor = postcard::from_bytes(&bytes).expect("and decodes");
        let NotifyAction::Effect { attach: NotifyAttach::Point(got), .. } =
            &back.layers[0].states[0].notifies[0].action
        else {
            panic!("the notify lost its point");
        };
        assert_eq!(got.point, sent.point);
    });
}

#[test]
fn a_request_that_cannot_be_asked_is_refused_at_load() {
    check!().with_type::<Request>().for_each(|r| {
        assert_eq!(r.attach().is_usable(), r.askable(), "{r:?} was judged wrongly");
        let verdict = notify_on(r.attach()).validate();
        if r.askable() {
            assert_eq!(verdict, Ok(()), "a sound request on a notify was refused");
        } else {
            assert!(
                matches!(
                    verdict,
                    Err(AnimationError::EmptyNotifySocket { .. } | AnimationError::NonFinite { .. })
                ),
                "{r:?} reached the runtime: {verdict:?}",
            );
        }
    });
}

#[test]
fn a_notifys_old_placements_keep_their_wire_tags() {
    check!().with_type::<Request>().for_each(|r| {
        let root = postcard::to_allocvec(&NotifyAttach::Root).expect("encodes");
        assert_eq!(root[0], 0, "Root moved on the wire");
        let socket = postcard::to_allocvec(&NotifyAttach::Socket {
            bone: vec!["root".into()],
            offset: [0.0; 3],
        })
        .expect("encodes");
        assert_eq!(socket[0], 1, "Socket moved on the wire");
        let point = postcard::to_allocvec(&NotifyAttach::Point(r.attach())).expect("encodes");
        assert_eq!(point[0], 2, "Point was not appended");
    });
}
