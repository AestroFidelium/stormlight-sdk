//! The animation authoring path on `ClientContext` (stormlight/server#72) — a
//! cosmetic mod declares how a unit animates beside how it looks, naming both the
//! unit and any mod-defined state with a stable string.
//!
//! Two name spaces meet here, and the laws are about keeping them straight:
//!   - **Unit-keyed like a visual**: `unit_animation` overwrites the descriptor's
//!     own unit handle with the interned one, so an author cannot mis-key an
//!     animation onto a unit they did not name.
//!   - **States intern idempotently**: the same state name always yields the same
//!     handle, and every handle a declared animation carries resolves back to its
//!     name in the emitted `anim_states` table (what the host needs to map it).
//!   - **Emit/decode**: `finish` → postcard → decode reproduces the exact bundle.
//!   - **What the SDK builds, the ABI accepts**: every declared animation passes
//!     `validate`, so the authoring path cannot produce a descriptor the host
//!     would reject at load.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::animation::{
    AnimLayer, AnimState, AnimationDescriptor, BlendMode, BoneMask, ClipRef, MaskGroup,
    RateBinding, StateClip, Transition,
};
use stormlight_mod_sdk::abi::ids::UnitId;
use stormlight_mod_sdk::abi::manifest::ABI_VERSION;
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;

/// A single-layer animation over `states`, with a mask group when `masked`.
fn an_animation(states: &[AnimState], masked: bool) -> AnimationDescriptor {
    let mask_groups = if masked {
        vec![MaskGroup {
            name: "upper".to_string(),
            bones: vec![vec!["Root".to_string(), "Spine".to_string()]],
            descendants: true,
        }]
    } else {
        Vec::new()
    };
    AnimationDescriptor {
        // Overwritten by `unit_animation`; a deliberately wrong value here is what
        // makes the re-keying law observable.
        unit: UnitId(u32::MAX),
        mask_groups,
        layers: vec![AnimLayer {
            name: "base".to_string(),
            mask: if masked { BoneMask::Only(vec![0]) } else { BoneMask::Whole },
            blend: BlendMode::Override,
            weight: 1.0,
            states: states
                .iter()
                .enumerate()
                .map(|(i, &state)| StateClip {
                    state,
                    clip: ClipRef {
                        asset: "mod://c/rig.glb".to_string(),
                        clip: format!("clip{i}"),
                    },
                    looping: i % 2 == 0,
                    blend_in: 0.1,
                    blend_out: 0.2,
                    rate: RateBinding::MoveSpeed { reference_speed: 3.5 },
                    priority: i as i16,
                    notifies: Vec::new(),
                })
                .collect(),
            transitions: vec![Transition {
                from: states[0],
                to: states[states.len() - 1],
                blend_in: 0.05,
            }],
        }],
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// How many units the cosmetic mod animates.
    units: u8,
    /// How many mod-defined states it declares, shared across those units.
    custom: u8,
    masked: bool,
}

/// Build a context from the scenario, returning it alongside the state names it
/// interned in declaration order.
fn build(s: &Scenario) -> (ClientContext, Vec<String>) {
    let nu = (s.units % 5) as usize + 1;
    let nc = (s.custom % 4) as usize;

    let mut ctx = ClientContext::new();
    let names: Vec<String> = (0..nc).map(|c| format!("s{c}")).collect();
    // Generic states always available; the mod-defined ones ride on top.
    let mut states = vec![AnimState::Idle];
    for name in &names {
        states.push(ctx.anim_state(name));
    }
    for u in 0..nu {
        ctx.unit_animation(&format!("u{u}"), an_animation(&states, s.masked));
    }
    (ctx, names)
}

#[test]
fn every_declared_animation_is_keyed_to_its_named_unit() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (ctx, _) = build(s);
        let reg = ctx.finish();

        assert_eq!(reg.abi, ABI_VERSION);
        assert_eq!(
            reg.animations.len(),
            reg.names.units.len(),
            "one animation per distinct named unit",
        );
        for anim in &reg.animations {
            let name = reg
                .names
                .units
                .get(anim.unit.0 as usize)
                .expect("animation handle resolves in the unit table");
            assert!(name.starts_with('u'), "unexpected unit name `{name}`");
        }
    });
}

#[test]
fn every_custom_state_resolves_back_to_its_name() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (ctx, declared) = build(s);
        let reg = ctx.finish();

        assert_eq!(
            reg.names.anim_states.len(),
            declared.len(),
            "one table entry per distinct declared state name",
        );
        for anim in &reg.animations {
            for layer in &anim.layers {
                for binding in &layer.states {
                    if let AnimState::Custom(id) = binding.state {
                        let name = reg
                            .names
                            .anim_states
                            .get(id.0 as usize)
                            .expect("state handle resolves in the anim_states table");
                        assert!(declared.contains(name), "unexpected state name `{name}`");
                    }
                }
            }
        }
    });
}

#[test]
fn interning_a_state_name_twice_reuses_one_handle() {
    check!().with_type::<u8>().for_each(|_| {
        let mut ctx = ClientContext::new();
        let a = ctx.anim_state("dash");
        let b = ctx.anim_state("dash");
        let c = ctx.anim_state("other");
        assert_eq!(a, b, "the same state name must reuse its handle");
        assert_ne!(a, c, "distinct state names must get distinct handles");

        ctx.unit_animation("u", an_animation(&[a, c], false));
        let reg = ctx.finish();
        assert_eq!(reg.names.anim_states.len(), 2, "two interned state names");
    });
}

#[test]
fn a_built_animation_bundle_decodes_equal() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (ctx, _) = build(s);
        let reg = ctx.finish();
        let decoded = postcard::from_bytes(&to_bytes_client(&reg)).expect("decode");
        assert_eq!(reg, decoded);
    });
}

#[test]
fn everything_the_authoring_path_builds_is_valid() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (ctx, _) = build(s);
        for anim in &ctx.finish().animations {
            anim.validate().expect("the SDK must not build a descriptor the host rejects");
        }
    });
}
