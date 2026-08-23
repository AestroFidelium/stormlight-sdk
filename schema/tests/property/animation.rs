//! Invariants of the animation ABI (stormlight/server#72) — what a cosmetic mod
//! declares so the engine can animate a character it knows nothing about.
//!
//! The descriptor is the cosmetic sibling of `VisualDescriptor`: keyed by a unit
//! handle, carrying layers of `semantic state -> clip` bindings. Two interned
//! families reach into it — the `UnitId` it dresses, and an `AnimStateId` for
//! every mod-defined state beyond the generic vocabulary — so the walk has to
//! find both, in state bindings *and* in per-pair transitions.
//!
//! - **Round-trip**: any generated bundle survives postcard unchanged and
//!   re-serializes to identical bytes (this crosses the wasm boundary).
//! - **Identity remap is a no-op**: bone paths, clip names, durations and weights
//!   are untouched by the walk — only handles move.
//! - **The walk reaches every custom state**: under a shifting map every
//!   `AnimState::Custom` moves, in bindings and in transitions alike, and nothing
//!   else does.
//! - **Totality**: a map that fails on a family errors exactly when the bundle
//!   carried that family, never panics.

use core::cell::Cell;

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::animation::{
    AnimLayer, AnimState, AnimationDescriptor, BlendMode, BoneMask, ClipRef, MaskGroup,
    RateBinding, StateClip, Transition,
};
use stormlight_mod_abi::descriptors::Names;
use stormlight_mod_abi::ids::{AnimStateId, EventId, UnitId};
use stormlight_mod_abi::manifest::Version;
use stormlight_mod_abi::notify::{NotifyAction, NotifyAttach, NotifyPoint, NotifyTime};
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::visuals::ClientRegistration;

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

mod ids {
    pub use stormlight_mod_abi::ids::*;
}

macro_rules! identity_families {
    ($( $fn_name:ident($ty:ident) ),+ $(,)?) => {$(
        fn $fn_name(&self, id: ids::$ty) -> Result<ids::$ty, ()> {
            Ok(id)
        }
    )+};
}

/// Every family but the two an animation bundle can name.
macro_rules! unrelated_families {
    () => {
        identity_families!(
            stat(StatId),
            resource(ResourceId),
            stack(StackId),
            tag(TagId),
            tag_class(TagClassId),
            param(ParamId),
            event(EventId),
            buff(BuffId),
            curve(CurveId),
            damage_type(DamageTypeId),
            ability(AbilityId),
            talent(TalentId),
            handler(HandlerId),
            navmesh(NavMeshId),
        );
    };
}

/// An identity map that counts the unit and anim-state handles the walk touched,
/// so a test knows whether a bundle carried any.
#[derive(Default)]
struct Counting {
    units: Cell<u32>,
    states: Cell<u32>,
}

impl IdMap for Counting {
    type Error = ();
    unrelated_families!();

    fn unit(&self, id: UnitId) -> Result<UnitId, ()> {
        self.units.set(self.units.get() + 1);
        Ok(id)
    }

    fn anim_state(&self, id: AnimStateId) -> Result<AnimStateId, ()> {
        self.states.set(self.states.get() + 1);
        Ok(id)
    }
}

/// Shifts every custom-state handle by one and leaves the rest alone — enough to
/// see that the walk reaches a state wherever it hides.
struct ShiftStates;

impl IdMap for ShiftStates {
    type Error = ();
    unrelated_families!();

    fn unit(&self, id: UnitId) -> Result<UnitId, ()> {
        Ok(id)
    }

    fn anim_state(&self, id: AnimStateId) -> Result<AnimStateId, ()> {
        Ok(AnimStateId(id.0 + 1))
    }
}

/// Fails on any custom state — a cosmetic mod naming a state it never interned.
struct FailState;

impl IdMap for FailState {
    type Error = ();
    unrelated_families!();

    fn unit(&self, id: UnitId) -> Result<UnitId, ()> {
        Ok(id)
    }

    fn anim_state(&self, _: AnimStateId) -> Result<AnimStateId, ()> {
        Err(())
    }
}

/// Fails on any unit — a cosmetic mod animating a unit the gameplay side never
/// defined.
struct FailUnit;

impl IdMap for FailUnit {
    type Error = ();
    unrelated_families!();

    fn unit(&self, _: UnitId) -> Result<UnitId, ()> {
        Err(())
    }

    fn anim_state(&self, id: AnimStateId) -> Result<AnimStateId, ()> {
        Ok(id)
    }
}

const IDENT: &[u8] = b"abcdefghijklmnopqrstuvwxyz_0123456789/.";

/// A cursor over a generated `u16` stream — the established generator shape.
struct Gen<'a> {
    ops: &'a [u16],
    pos: usize,
}

impl Gen<'_> {
    fn next(&mut self) -> u16 {
        let v = self.ops.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        v
    }
    fn count(&mut self, ceil: u16) -> u16 {
        self.next() % ceil
    }
    fn bool(&mut self) -> bool {
        self.next().is_multiple_of(2)
    }
    fn f32(&mut self) -> f32 {
        // Always finite: value-equality after a round-trip is meaningful.
        f32::from(self.next()) / f32::from(u16::MAX) * 4.0
    }
    fn string(&mut self) -> String {
        let len = self.count(8) as usize;
        let mut s = String::new();
        for _ in 0..len {
            s.push(IDENT[self.next() as usize % IDENT.len()] as char);
        }
        s
    }
    fn strings(&mut self) -> Vec<String> {
        let n = self.count(4);
        (0..n).map(|_| self.string()).collect()
    }
    fn state(&mut self) -> AnimState {
        match self.next() % 8 {
            0 => AnimState::Idle,
            1 => AnimState::Walk,
            2 => AnimState::Run,
            3 => AnimState::Cast,
            4 => AnimState::Channel,
            5 => AnimState::Hit,
            6 => AnimState::Death,
            _ => AnimState::Custom(AnimStateId(self.next())),
        }
    }
    fn clip(&mut self) -> ClipRef {
        ClipRef { asset: self.string(), clip: self.string() }
    }
    fn rate(&mut self) -> RateBinding {
        match self.next() % 3 {
            0 => RateBinding::Fixed(self.f32()),
            1 => RateBinding::MoveSpeed { reference_speed: self.f32() },
            _ => RateBinding::CastDuration,
        }
    }
    fn notify(&mut self) -> NotifyPoint {
        let at = if self.bool() {
            NotifyTime::Normalized(self.f32() / 4.0)
        } else {
            NotifyTime::Seconds(self.f32())
        };
        let action = if self.bool() {
            NotifyAction::Trigger { event: EventId(self.next()) }
        } else {
            NotifyAction::Effect {
                key: self.string(),
                attach: if self.bool() {
                    NotifyAttach::Root
                } else {
                    NotifyAttach::Socket {
                        bone: self.strings(),
                        offset: [self.f32(), self.f32(), self.f32()],
                    }
                },
                lifetime: self.f32(),
            }
        };
        NotifyPoint { at, action }
    }
    fn state_clip(&mut self) -> StateClip {
        StateClip {
            state: self.state(),
            clip: self.clip(),
            looping: self.bool(),
            blend_in: self.f32(),
            blend_out: self.f32(),
            rate: self.rate(),
            priority: self.next() as i16,
            notifies: (0..self.count(3)).map(|_| self.notify()).collect(),
        }
    }
    fn transition(&mut self) -> Transition {
        Transition { from: self.state(), to: self.state(), blend_in: self.f32() }
    }
    fn mask(&mut self) -> BoneMask {
        match self.next() % 3 {
            0 => BoneMask::Whole,
            1 => BoneMask::Only((0..self.count(3)).map(|_| self.next()).collect()),
            _ => BoneMask::Except((0..self.count(3)).map(|_| self.next()).collect()),
        }
    }
    fn mask_group(&mut self) -> MaskGroup {
        MaskGroup {
            name: self.string(),
            bones: (0..self.count(3)).map(|_| self.strings()).collect(),
            descendants: self.bool(),
        }
    }
    fn layer(&mut self) -> AnimLayer {
        AnimLayer {
            name: self.string(),
            mask: self.mask(),
            blend: if self.bool() { BlendMode::Override } else { BlendMode::Additive },
            weight: self.f32(),
            states: (0..self.count(4)).map(|_| self.state_clip()).collect(),
            transitions: (0..self.count(3)).map(|_| self.transition()).collect(),
        }
    }
    fn animation(&mut self) -> AnimationDescriptor {
        AnimationDescriptor {
            unit: UnitId(u32::from(self.next())),
            mask_groups: (0..self.count(3)).map(|_| self.mask_group()).collect(),
            layers: (0..self.count(3)).map(|_| self.layer()).collect(),
        }
    }
    fn client_registration(&mut self) -> ClientRegistration {
        ClientRegistration {
            abi: Version::new(
                u32::from(self.next()),
                u32::from(self.next()),
                u32::from(self.next()),
            ),
            // A cosmetic bundle names the units it dresses and the extra states
            // its animations declare; the rest stay empty.
            names: Names { units: self.strings(), anim_states: self.strings(), ..Names::default() },
            visuals: Vec::new(),
            effects: Vec::new(),
            icons: Vec::new(),
            cards: Vec::new(),
            unit_icons: Vec::new(),
            named_effects: Vec::new(),
            // Widget trees have their own invariants (see `ui.rs`); this file
            // stays about animations.
            ui: Vec::new(),
            animations: (0..self.count(3)).map(|_| self.animation()).collect(),
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    ops: Vec<u16>,
}

fn build(s: &Scenario) -> ClientRegistration {
    Gen { ops: &s.ops, pos: 0 }.client_registration()
}

/// Every custom-state handle in a bundle, in walk order.
fn custom_states(reg: &ClientRegistration) -> Vec<AnimStateId> {
    let mut out = Vec::new();
    let mut push = |s: &AnimState| {
        if let AnimState::Custom(id) = s {
            out.push(*id);
        }
    };
    for anim in &reg.animations {
        for layer in &anim.layers {
            for sc in &layer.states {
                push(&sc.state);
            }
            for t in &layer.transitions {
                push(&t.from);
                push(&t.to);
            }
        }
    }
    out
}

#[test]
fn any_animation_bundle_survives_a_postcard_round_trip() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = build(s);
        let bytes = postcard::to_allocvec(&reg).expect("serialize");
        let back: ClientRegistration = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(reg, back, "animation bundle did not round-trip");
        let again = postcard::to_allocvec(&back).expect("reserialize");
        assert_eq!(bytes, again, "animation bundle serialization is not stable");
    });
}

#[test]
fn identity_remap_is_a_noop() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = build(s);
        let mut out = reg.clone();
        out.remap_ids(&Counting::default()).expect("identity map never fails");
        let a = postcard::to_allocvec(&reg).expect("serialize");
        let b = postcard::to_allocvec(&out).expect("serialize");
        assert_eq!(a, b, "identity remap changed the tree");
    });
}

#[test]
fn the_walk_reaches_every_custom_state_and_nothing_else() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = build(s);
        let before = custom_states(&reg);

        let mut out = reg.clone();
        out.remap_ids(&ShiftStates).expect("shifting map never fails");
        let after = custom_states(&out);

        assert_eq!(before.len(), after.len(), "the walk added or dropped a state");
        for (b, a) in before.iter().zip(&after) {
            assert_eq!(a.0, b.0.wrapping_add(1), "a custom state was not rewritten");
        }

        // Everything that is not a handle is untouched: clear the states on both
        // sides and the trees must be byte-identical again.
        let mut lhs = reg.clone();
        let mut rhs = out;
        for r in [&mut lhs, &mut rhs] {
            for anim in &mut r.animations {
                for layer in &mut anim.layers {
                    for sc in &mut layer.states {
                        sc.state = AnimState::Idle;
                    }
                    for t in &mut layer.transitions {
                        t.from = AnimState::Idle;
                        t.to = AnimState::Idle;
                    }
                }
            }
        }
        assert_eq!(
            postcard::to_allocvec(&lhs).expect("serialize"),
            postcard::to_allocvec(&rhs).expect("serialize"),
            "the state walk disturbed clips, bones, or timings",
        );
    });
}

#[test]
fn a_failing_state_map_errors_exactly_when_custom_states_are_present() {
    check!().with_type::<Scenario>().for_each(|s| {
        let had_states = !custom_states(&build(s)).is_empty();
        let mut reg = build(s);
        let result = reg.remap_ids(&FailState);
        assert_eq!(
            result.is_err(),
            had_states,
            "state-error propagation disagrees with custom-state presence",
        );
    });
}

#[test]
fn a_failing_unit_map_errors_exactly_when_animations_are_present() {
    check!().with_type::<Scenario>().for_each(|s| {
        let counter = Counting::default();
        let mut probe = build(s);
        probe.remap_ids(&counter).expect("total map succeeds");
        let had_units = counter.units.get() > 0;

        let mut reg = build(s);
        let result = reg.remap_ids(&FailUnit);
        assert_eq!(
            result.is_err(),
            had_units,
            "unit-error propagation disagrees with animation presence",
        );
    });
}
