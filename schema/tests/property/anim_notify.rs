//! Animation notify points — the ABI half (stormlight/server#76).
//!
//! A notify is a *time* on a clip plus what to do when playback reaches it, so
//! the burst goes off on the frame the swing connects rather than on the frame
//! the ability was cast. Everything about it is authored blind: the engine has
//! never seen the clip, so the only things decidable at load are the ones
//! checked here — a time that could never be reached, a key that names nothing,
//! a socket with no bone in it.
//!
//! Three properties, each directional:
//!   - **Validation classifies**: a well-formed declaration with notifies on it
//!     loads, and each single structural break is reported as its own variant.
//!   - **Resolution is ordered**: a normalized time lands inside the clip and
//!     never moves backwards as the fraction grows — the ordering is what makes
//!     "fires once per traversal" expressible at all.
//!   - **Only handles are remapped**: adoption rewrites a notify's mod-defined
//!     event to the global id space and leaves its key, socket and time alone.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::animation::{
    AnimLayer, AnimState, AnimationDescriptor, AnimationError, BlendMode, BoneMask, ClipRef,
    RateBinding, StateClip,
};
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, Handle, HandlerId, NavMeshId,
    ParamId, ResourceId, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::notify::{NotifyAction, NotifyAttach, NotifyPoint, NotifyTime};
use stormlight_mod_abi::remap::{IdMap, RemapIds};

extern crate alloc;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// A total map that offsets every local handle by a fixed amount — the smallest
/// stand-in for adoption that still shows *which* fields the walk touched.
struct ShiftMap(u32);

macro_rules! shift {
    ($( $method:ident($family:ty) ),+ $(,)?) => {$(
        fn $method(&self, id: $family) -> Result<$family, ()> {
            Ok(<$family>::from_raw(id.raw() + self.0))
        }
    )+};
}

impl IdMap for ShiftMap {
    type Error = ();
    shift! {
        stat(StatId), resource(ResourceId), stack(StackId), tag(TagId),
        tag_class(TagClassId), param(ParamId), event(EventId), buff(BuffId),
        curve(CurveId), damage_type(DamageTypeId), ability(AbilityId), talent(TalentId),
        handler(HandlerId), unit(UnitId), navmesh(NavMeshId), anim_state(AnimStateId),
    }
}

/// One way to break a notify. `None` leaves the declaration well-formed.
#[derive(Debug, TypeGenerator, Clone, Copy, PartialEq, Eq)]
enum Break {
    /// Leave it alone — the control case.
    None,
    /// A normalized time past the end of the clip.
    NormalizedPastEnd,
    /// A normalized time before its start.
    NormalizedNegative,
    /// A time that is not a number at all.
    NonFiniteTime,
    /// Seconds counted backwards from the start.
    NegativeSeconds,
    /// An effect key naming nothing.
    EmptyKey,
    /// A socket with no bone path.
    EmptySocket,
    /// A socket whose bone path has an empty segment.
    BlankSocketSegment,
    /// A lifetime that is not a number.
    NonFiniteLifetime,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// 1..=3 notifies on the first state.
    notifies: u8,
    /// Whether the notify carries a socket or sits at the character root.
    socketed: bool,
    /// Whether the first notify dispatches an event instead of an effect.
    triggering: bool,
    brk: Break,
}

fn an_effect(n: usize, socketed: bool) -> NotifyAction {
    NotifyAction::Effect {
        key: format!("fx{n}"),
        attach: if socketed {
            NotifyAttach::Socket {
                bone: vec!["Root".to_string(), format!("Bone{n}")],
                offset: [0.0, 0.1, 0.0],
            }
        } else {
            NotifyAttach::Root
        },
        lifetime: 0.3,
    }
}

/// A well-formed declaration: one layer, one state, `notifies` notify points
/// spread across the clip.
fn well_formed(notifies: usize, socketed: bool, triggering: bool) -> AnimationDescriptor {
    let points = (0..notifies)
        .map(|n| NotifyPoint {
            at: NotifyTime::Normalized(n as f32 / notifies as f32),
            action: if triggering && n == 0 {
                NotifyAction::Trigger { event: EventId(n as u16) }
            } else {
                an_effect(n, socketed)
            },
        })
        .collect();
    AnimationDescriptor {
        unit: UnitId(0),
        mask_groups: Vec::new(),
        layers: vec![AnimLayer {
            name: "base".to_string(),
            mask: BoneMask::Whole,
            blend: BlendMode::Override,
            weight: 1.0,
            states: vec![StateClip {
                state: AnimState::Idle,
                clip: ClipRef { asset: "mod://m/a.glb".to_string(), clip: "idle".to_string() },
                window: None,
                looping: true,
                blend_in: 0.1,
                blend_out: 0.1,
                rate: RateBinding::Fixed(1.0),
                priority: 0,
                notifies: points,
            }],
            transitions: Vec::new(),
        }],
    }
}

/// Apply one break to the last notify — the last, so a scenario that made its
/// first notify a trigger still has an effect action to break.
fn apply(d: &mut AnimationDescriptor, brk: Break) {
    let point = d.layers[0].states[0].notifies.last_mut().expect("at least one notify");
    match brk {
        Break::None => {}
        Break::NormalizedPastEnd => point.at = NotifyTime::Normalized(1.5),
        Break::NormalizedNegative => point.at = NotifyTime::Normalized(-0.25),
        Break::NonFiniteTime => point.at = NotifyTime::Seconds(f32::NAN),
        Break::NegativeSeconds => point.at = NotifyTime::Seconds(-0.5),
        Break::EmptyKey => {
            point.action = NotifyAction::Effect {
                key: String::new(),
                attach: NotifyAttach::Root,
                lifetime: 0.3,
            }
        }
        Break::EmptySocket => {
            point.action = NotifyAction::Effect {
                key: "fx".to_string(),
                attach: NotifyAttach::Socket { bone: Vec::new(), offset: [0.0; 3] },
                lifetime: 0.3,
            }
        }
        Break::BlankSocketSegment => {
            point.action = NotifyAction::Effect {
                key: "fx".to_string(),
                attach: NotifyAttach::Socket {
                    bone: vec!["Root".to_string(), String::new()],
                    offset: [0.0; 3],
                },
                lifetime: 0.3,
            }
        }
        Break::NonFiniteLifetime => {
            point.action = NotifyAction::Effect {
                key: "fx".to_string(),
                attach: NotifyAttach::Root,
                lifetime: f32::INFINITY,
            }
        }
    }
}

fn build(s: &Scenario) -> AnimationDescriptor {
    let notifies = usize::from(s.notifies % 3) + 1;
    let mut d = well_formed(notifies, s.socketed, s.triggering);
    apply(&mut d, s.brk);
    d
}

#[test]
fn a_clip_with_notifies_validates_exactly_when_every_notify_is_reachable() {
    check!().with_type::<Scenario>().for_each(|s| {
        let d = build(s);
        assert_eq!(
            d.validate().is_ok(),
            s.brk == Break::None,
            "validation disagrees with the applied break {:?}",
            s.brk,
        );
    });
}

#[test]
fn each_notify_break_is_reported_as_its_own_kind() {
    check!().with_type::<Scenario>().for_each(|s| {
        let d = build(s);
        let err = d.validate().err();
        let matched = match s.brk {
            Break::None => err.is_none(),
            Break::NormalizedPastEnd
            | Break::NormalizedNegative
            | Break::NonFiniteTime
            | Break::NegativeSeconds => {
                matches!(err, Some(AnimationError::NotifyOutOfRange { .. }))
            }
            Break::EmptyKey => matches!(err, Some(AnimationError::EmptyNotifyKey { .. })),
            Break::EmptySocket | Break::BlankSocketSegment => {
                matches!(err, Some(AnimationError::EmptyNotifySocket { .. }))
            }
            Break::NonFiniteLifetime => matches!(err, Some(AnimationError::NonFinite { .. })),
        };
        assert!(matched, "break {:?} was reported as {:?}", s.brk, err);
    });
}

#[derive(Debug, TypeGenerator)]
struct TimeScenario {
    /// The fraction into the clip, before normalization.
    fraction: u8,
    /// How much further into the clip the second point sits.
    further: u8,
    /// The clip's authored length, in tenths of a second.
    duration: u8,
}

#[test]
fn a_normalized_time_stays_inside_the_clip_and_keeps_its_order() {
    check!().with_type::<TimeScenario>().for_each(|s| {
        let duration = f32::from(s.duration) / 10.0;
        let early = f32::from(s.fraction) / 255.0;
        let late = (early + f32::from(s.further) / 255.0).min(1.0);

        let at_early = NotifyTime::Normalized(early).seconds(duration);
        let at_late = NotifyTime::Normalized(late).seconds(duration);

        assert!(
            (0.0..=duration).contains(&at_early),
            "a fraction of the clip resolves inside it: {at_early} not in 0..={duration}",
        );
        assert!(
            at_early <= at_late + f32::EPSILON,
            "a later fraction never resolves earlier: {at_early} > {at_late}",
        );
        // Seconds are what they say, whatever the clip turns out to be — the point
        // of the variant is that it does *not* stretch with the art.
        let fixed = NotifyTime::Seconds(early);
        assert!(
            (fixed.seconds(duration) - early).abs() < 1e-6,
            "an absolute time ignores the clip's length",
        );
    });
}

#[derive(Debug, TypeGenerator)]
struct RemapScenario {
    /// How far the global id space is offset from the mod's local one.
    shift: u8,
    /// The local event a trigger notify names.
    event: u8,
    socketed: bool,
}

#[test]
fn adoption_rewrites_a_notifys_event_and_nothing_else() {
    check!().with_type::<RemapScenario>().for_each(|s| {
        let key = "fx".to_string();
        let attach = if s.socketed {
            NotifyAttach::Socket { bone: vec!["Root".to_string()], offset: [1.0, 2.0, 3.0] }
        } else {
            NotifyAttach::Root
        };
        let mut points = vec![
            NotifyPoint {
                at: NotifyTime::Seconds(0.25),
                action: NotifyAction::Trigger { event: EventId(u16::from(s.event)) },
            },
            NotifyPoint {
                at: NotifyTime::Normalized(0.5),
                action: NotifyAction::Effect {
                    key: key.clone(),
                    attach: attach.clone(),
                    lifetime: 0.3,
                },
            },
        ];
        let before = points.clone();

        let map = ShiftMap(u32::from(s.shift));
        points.remap_ids(&map).expect("a shift map maps every handle");

        match (&before[0].action, &points[0].action) {
            (NotifyAction::Trigger { event: local }, NotifyAction::Trigger { event: global }) => {
                assert_eq!(
                    u32::from(global.0),
                    u32::from(local.0) + u32::from(s.shift),
                    "a trigger's event is rewritten into the global id space",
                )
            }
            other => panic!("a trigger notify stayed a trigger: {other:?}"),
        }
        assert_eq!(points[1], before[1], "an effect notify carries no handle to rewrite");
        assert_eq!(points[0].at, before[0].at, "remapping never moves a notify in time");
    });
}
