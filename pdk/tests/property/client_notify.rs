//! The authoring side of animation notifies (stormlight/server#76): what a
//! cosmetic mod writes so a burst goes off on the frame the swing connects.
//!
//! A notify names two things a `ClientContext` has to carry through `finish`
//! intact — a **cosmetic effect key**, resolved within the declaring mod, and a
//! mod-defined **event** handle, interned like every other name. Laws:
//!   - **Named effects survive by name**: every key declared is in the finished
//!     bundle, and re-declaring one keeps a single entry per name at adoption
//!     (later wins), never a silent drop.
//!   - **Events intern like names**: one name, one handle, and every handle a
//!     trigger notify carries resolves back through `names.events`.
//!   - **Notifies survive the wire**: a clip declared with notify points decodes
//!     equal on the other side of postcard, which is the boundary the guest's
//!     registration actually crosses.
//!   - **What the authoring path builds is loadable**: the whole declaration
//!     passes `validate`, so a mod written this way cannot be refused at adoption.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::animation::{
    AnimLayer, AnimState, AnimationDescriptor, BlendMode, BoneMask, ClipRef, RateBinding, StateClip,
};
use stormlight_mod_sdk::abi::ids::UnitId;
use stormlight_mod_sdk::abi::notify::{NotifyAction, NotifyAttach, NotifyPoint, NotifyTime};
use stormlight_mod_sdk::abi::visuals::{ClientRegistration, PrimitiveShape, VisualModel};
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;

fn a_model(seed: u8) -> VisualModel {
    VisualModel::Primitive {
        shape: match seed % 3 {
            0 => PrimitiveShape::Cube,
            1 => PrimitiveShape::Sphere,
            _ => PrimitiveShape::Capsule,
        },
        color: [0.4, 0.4, 0.4, 1.0],
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// 0..=5 distinct effect keys.
    effects: u8,
    /// 0..=5 distinct event names.
    events: u8,
    /// How many keys are declared a second time.
    redeclared: u8,
    /// Whether the notifies ask for a bone socket.
    socketed: bool,
}

/// A one-layer character whose clip carries one notify per effect key and one
/// per event — the shape a mod actually writes.
fn animation(
    keys: usize,
    events: &[stormlight_mod_sdk::abi::ids::EventId],
    socketed: bool,
) -> AnimationDescriptor {
    let mut notifies: Vec<NotifyPoint> = (0..keys)
        .map(|k| NotifyPoint {
            at: NotifyTime::Normalized(k as f32 / (keys.max(1)) as f32),
            action: NotifyAction::Effect {
                key: format!("fx{k}"),
                attach: if socketed {
                    NotifyAttach::Socket {
                        bone: vec!["Root".to_string(), "Foot.L".to_string()],
                        offset: [0.0, 0.0, 0.0],
                    }
                } else {
                    NotifyAttach::Root
                },
                lifetime: 0.25,
            },
        })
        .collect();
    notifies.extend(events.iter().map(|event| NotifyPoint {
        at: NotifyTime::Seconds(0.1),
        action: NotifyAction::Trigger { event: *event },
    }));

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
                clip: ClipRef { asset: "mod://c/rig.glb".to_string(), clip: "idle".to_string() },
                looping: true,
                blend_in: 0.1,
                blend_out: 0.1,
                rate: RateBinding::Fixed(1.0),
                priority: 0,
                notifies,
            }],
            transitions: Vec::new(),
        }],
    }
}

/// Build the scenario's bundle: `keys` named effects (some declared twice),
/// `events` interned event names, and one animation whose notifies use both.
fn build(s: &Scenario) -> ClientRegistration {
    let keys = usize::from(s.effects % 6);
    let events = usize::from(s.events % 6);
    let again = usize::from(s.redeclared % 4).min(keys);

    let mut ctx = ClientContext::new();
    for k in 0..keys {
        ctx.notify_effect(&format!("fx{k}"), a_model(k as u8));
    }
    // A key declared twice is one key with the later model — an author iterating
    // on a puff must not end up with two.
    for k in 0..again {
        ctx.notify_effect(&format!("fx{k}"), a_model((k + 7) as u8));
    }
    let handles: Vec<_> = (0..events).map(|e| ctx.notify_event(&format!("ev{e}"))).collect();
    ctx.unit_animation("hero", animation(keys, &handles, s.socketed));
    ctx.finish()
}

#[test]
fn every_declared_effect_key_reaches_the_bundle() {
    check!().with_type::<Scenario>().for_each(|s| {
        let keys = usize::from(s.effects % 6);
        let again = usize::from(s.redeclared % 4).min(keys);
        let reg = build(s);

        assert_eq!(
            reg.named_effects.len(),
            keys + again,
            "every declaration is carried; collapsing them is adoption's call",
        );
        for k in 0..keys {
            let name = format!("fx{k}");
            assert!(
                reg.named_effects.iter().any(|e| e.name == name),
                "declared effect key `{name}` is missing from the bundle",
            );
        }
        // Every effect notify names a key the bundle declares — the invariant that
        // makes an unknown key at runtime an authoring mistake, not an ABI hole.
        for point in notifies_of(&reg) {
            if let NotifyAction::Effect { key, .. } = &point.action {
                assert!(
                    reg.named_effects.iter().any(|e| &e.name == key),
                    "notify names undeclared effect key `{key}`",
                );
            }
        }
    });
}

#[test]
fn every_trigger_event_resolves_back_to_its_name() {
    check!().with_type::<Scenario>().for_each(|s| {
        let events = usize::from(s.events % 6);
        let reg = build(s);

        assert_eq!(reg.names.events.len(), events, "one event name per distinct name");
        let triggers: Vec<_> = notifies_of(&reg)
            .filter_map(|p| match p.action {
                NotifyAction::Trigger { event } => Some(event),
                NotifyAction::Effect { .. } => None,
            })
            .collect();
        assert_eq!(triggers.len(), events, "one trigger notify per declared event");
        for event in triggers {
            let name = reg
                .names
                .events
                .get(event.0 as usize)
                .expect("a trigger's event handle is in the name table");
            assert!(name.starts_with("ev"), "unexpected event name `{name}`");
        }
    });
}

#[test]
fn interning_an_event_name_twice_reuses_one_handle() {
    let mut ctx = ClientContext::new();
    let first = ctx.notify_event("footfall");
    let again = ctx.notify_event("footfall");
    let other = ctx.notify_event("swing");
    assert_eq!(first, again, "one name, one handle");
    assert_ne!(first, other, "distinct names take distinct handles");
    assert_eq!(ctx.finish().names.events.len(), 2, "one table entry per distinct name");
}

#[test]
fn a_bundle_with_notifies_decodes_equal_and_validates() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = build(s);
        for animation in &reg.animations {
            animation.validate().expect("the authoring path builds a loadable animation");
        }

        let bytes = to_bytes_client(&reg);
        let back: ClientRegistration = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(reg, back, "a bundle carrying notifies did not survive the wire");
    });
}

/// Every notify point in a bundle, in declaration order.
fn notifies_of(reg: &ClientRegistration) -> impl Iterator<Item = &NotifyPoint> {
    reg.animations
        .iter()
        .flat_map(|a| a.layers.iter())
        .flat_map(|l| l.states.iter())
        .flat_map(|s| s.notifies.iter())
}
