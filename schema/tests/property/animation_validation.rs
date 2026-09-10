//! Structural validation of the animation ABI (stormlight/server#72).
//!
//! An animation descriptor is interpreted by a renderer that has never seen the
//! character: a layer with nothing to play, a transition out of a state that was
//! never declared, or a mask pointing past the declared bone groups are all
//! silent no-ops at runtime and a black screen for the author. `validate` is the
//! load-time gate that turns each into a named error instead.
//!
//! Shape of the property: build a well-formed descriptor from the generated
//! counts, apply exactly one structural break chosen by the scenario, and assert
//! `validate` accepts the untouched descriptor and rejects each break with its
//! own variant. Directional — the *classification* is the invariant, never a
//! magnitude.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::animation::{
    AnimLayer, AnimState, AnimationDescriptor, AnimationError, BlendMode, BoneMask, ClipRef,
    MAX_MASK_GROUPS, MaskGroup, RateBinding, StateClip, Transition,
};
use stormlight_mod_abi::ids::UnitId;

extern crate alloc;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// The generic states, in a fixed order, so a layer can take `n` distinct ones.
const VOCABULARY: [AnimState; 7] = [
    AnimState::Idle,
    AnimState::Walk,
    AnimState::Run,
    AnimState::Cast,
    AnimState::Channel,
    AnimState::Hit,
    AnimState::Death,
];

/// One way to break a descriptor. `None` leaves it well-formed.
#[derive(Debug, TypeGenerator, Clone, Copy, PartialEq, Eq)]
enum Break {
    /// Leave it alone — the control case.
    None,
    /// Strip every layer.
    NoLayers,
    /// Strip the first layer's states.
    EmptyLayer,
    /// Bind one state twice on the first layer.
    DuplicateState,
    /// Point a transition at a state the layer never declared.
    UnknownTransitionState,
    /// Mask a group index past the declared groups.
    UnknownMaskGroup,
    /// Declare more mask groups than the renderer's bitset can address.
    TooManyMaskGroups,
    /// Declare a mask group with no bones in it.
    EmptyMaskGroup,
    /// Name a clip with an empty container path.
    EmptyClipRef,
    /// Put a NaN in the first layer's weight.
    NonFinite,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// 1..=3 layers.
    layers: u8,
    /// 1..=3 states on each.
    states: u8,
    /// 0..=2 declared mask groups.
    groups: u8,
    brk: Break,
}

fn a_clip(n: usize) -> ClipRef {
    ClipRef { asset: format!("mod://m/anim{n}.glb"), clip: format!("clip{n}") }
}

fn a_state_clip(state: AnimState, n: usize) -> StateClip {
    StateClip {
        state,
        clip: a_clip(n),
        window: None,
        looping: true,
        blend_in: 0.1,
        blend_out: 0.2,
        rate: RateBinding::Fixed(1.0),
        priority: 0,
        notifies: Vec::new(),
    }
}

/// A well-formed descriptor: `layers` layers, each binding `states` distinct
/// generic states, over `groups` declared bone groups.
fn well_formed(layers: usize, states: usize, groups: usize) -> AnimationDescriptor {
    let mask_groups: Vec<MaskGroup> = (0..groups)
        .map(|g| MaskGroup {
            name: format!("g{g}"),
            bones: vec![vec![format!("root{g}"), format!("bone{g}")]],
            descendants: true,
        })
        .collect();
    let layers = (0..layers)
        .map(|l| AnimLayer {
            name: format!("l{l}"),
            // Only masks the groups that exist; `Whole` when none were declared.
            mask: if groups == 0 {
                BoneMask::Whole
            } else {
                BoneMask::Only((0..groups as u16).collect())
            },
            blend: if l == 0 { BlendMode::Override } else { BlendMode::Additive },
            weight: 1.0,
            states: (0..states).map(|i| a_state_clip(VOCABULARY[i], i)).collect(),
            // A transition between two states this layer declares (or a self-pair
            // when it only has one) — well-formed either way.
            transitions: vec![Transition {
                from: VOCABULARY[0],
                to: VOCABULARY[states - 1],
                blend_in: 0.15,
            }],
        })
        .collect();
    AnimationDescriptor { unit: UnitId(0), mask_groups, layers }
}

fn apply(d: &mut AnimationDescriptor, brk: Break) {
    match brk {
        Break::None => {}
        Break::NoLayers => d.layers.clear(),
        Break::EmptyLayer => {
            d.layers[0].states.clear();
            // A layer with no states also has nothing for its transition to name;
            // drop it so `EmptyLayer` is the only break in play.
            d.layers[0].transitions.clear();
        }
        Break::DuplicateState => {
            let dup = d.layers[0].states[0].clone();
            d.layers[0].states.push(dup);
        }
        Break::UnknownTransitionState => {
            // The last generic state is only ever bound when a layer takes all
            // seven, which `well_formed` never does.
            d.layers[0].transitions[0].to = AnimState::Death;
        }
        Break::UnknownMaskGroup => {
            let past = d.mask_groups.len() as u16;
            d.layers[0].mask = BoneMask::Only(vec![past]);
        }
        Break::TooManyMaskGroups => {
            d.mask_groups = (0..=MAX_MASK_GROUPS)
                .map(|g| MaskGroup {
                    name: format!("g{g}"),
                    bones: vec![vec!["root".to_string()]],
                    descendants: true,
                })
                .collect();
            // Keep the mask itself in range, so the group count is the only break.
            d.layers[0].mask = BoneMask::Whole;
        }
        Break::EmptyMaskGroup => {
            d.mask_groups.push(MaskGroup {
                name: "empty".to_string(),
                bones: Vec::new(),
                descendants: false,
            });
            d.layers[0].mask = BoneMask::Whole;
        }
        Break::EmptyClipRef => d.layers[0].states[0].clip.asset = String::new(),
        Break::NonFinite => d.layers[0].weight = f32::NAN,
    }
}

fn build(s: &Scenario) -> AnimationDescriptor {
    let layers = (s.layers % 3) as usize + 1;
    let states = (s.states % 3) as usize + 1;
    let groups = (s.groups % 3) as usize;
    let mut d = well_formed(layers, states, groups);
    apply(&mut d, s.brk);
    d
}

#[test]
fn validate_accepts_exactly_the_well_formed_descriptors() {
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
fn each_break_is_reported_as_its_own_kind() {
    check!().with_type::<Scenario>().for_each(|s| {
        let d = build(s);
        let err = d.validate().err();
        let matched = match s.brk {
            Break::None => err.is_none(),
            Break::NoLayers => matches!(err, Some(AnimationError::NoLayers)),
            Break::EmptyLayer => matches!(err, Some(AnimationError::EmptyLayer { .. })),
            Break::DuplicateState => matches!(err, Some(AnimationError::DuplicateState { .. })),
            Break::UnknownTransitionState => {
                matches!(err, Some(AnimationError::UnknownTransitionState { .. }))
            }
            Break::UnknownMaskGroup => matches!(err, Some(AnimationError::UnknownMaskGroup { .. })),
            Break::TooManyMaskGroups => {
                matches!(err, Some(AnimationError::TooManyMaskGroups { .. }))
            }
            Break::EmptyMaskGroup => matches!(err, Some(AnimationError::EmptyMaskGroup { .. })),
            Break::EmptyClipRef => matches!(err, Some(AnimationError::EmptyClipRef { .. })),
            Break::NonFinite => matches!(err, Some(AnimationError::NonFinite { .. })),
        };
        assert!(matched, "break {:?} was reported as {:?}", s.brk, err);
    });
}

#[test]
fn one_layer_with_one_state_is_enough() {
    // Additive over one file: the smallest thing a mod can say — a single idle
    // clip, no masks, no transitions — must load.
    let d = AnimationDescriptor {
        unit: UnitId(0),
        mask_groups: Vec::new(),
        layers: vec![AnimLayer {
            name: "base".to_string(),
            mask: BoneMask::Whole,
            blend: BlendMode::Override,
            weight: 1.0,
            states: vec![a_state_clip(AnimState::Idle, 0)],
            transitions: Vec::new(),
        }],
    };
    d.validate().expect("a single idle clip is a valid animation");
}
