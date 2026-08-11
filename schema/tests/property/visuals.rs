//! Invariants of the client cosmetic ABI (`VisualDescriptor` / `ClientRegistration`)
//! — the payload a *client* (cosmetic) mod emits: how to draw a unit, keyed by
//! the unit's interned handle. Directional/structural only:
//!   - **Round-trip**: any generated bundle survives a postcard serialize /
//!     deserialize unchanged and re-serializes to identical bytes (wire
//!     stability — this crosses the wasm boundary like `Registration`).
//!   - **Remap identity is a no-op**: remapping through a map that returns every
//!     unit id unchanged leaves the tree byte-identical (the walk rewrites the
//!     unit handle and touches nothing else — asset strings, colors, sizes).
//!   - **Totality**: a map that fails on the unit id makes the walk return `Err`
//!     exactly when the bundle carried a visual, never a panic.

use core::cell::Cell;

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::descriptors::Names;
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::manifest::Version;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::visuals::{
    ClientRegistration, EffectRole, EffectVisualDescriptor, PrimitiveShape, VisualDescriptor,
    VisualModel,
};

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// An identity map that counts how many unit / ability handles the walk touched,
/// so a test knows whether a bundle carried any. Every family returns its id
/// unchanged. Units are keyed by [`VisualDescriptor`]; abilities by
/// [`EffectVisualDescriptor`].
#[derive(Default)]
struct Counting {
    units: Cell<u32>,
    abilities: Cell<u32>,
}

impl IdMap for Counting {
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
        Ok(id)
    }
    fn curve(&self, id: CurveId) -> Result<CurveId, ()> {
        Ok(id)
    }
    fn damage_type(&self, id: DamageTypeId) -> Result<DamageTypeId, ()> {
        Ok(id)
    }
    fn ability(&self, id: AbilityId) -> Result<AbilityId, ()> {
        self.abilities.set(self.abilities.get() + 1);
        Ok(id)
    }
    fn talent(&self, id: TalentId) -> Result<TalentId, ()> {
        Ok(id)
    }
    fn handler(&self, id: HandlerId) -> Result<HandlerId, ()> {
        Ok(id)
    }
    fn unit(&self, id: UnitId) -> Result<UnitId, ()> {
        self.units.set(self.units.get() + 1);
        Ok(id)
    }
    fn navmesh(&self, id: NavMeshId) -> Result<NavMeshId, ()> {
        Ok(id)
    }
    fn anim_state(&self, id: AnimStateId) -> Result<AnimStateId, ()> {
        Ok(id)
    }
}

/// A map that fails on any unit handle — models a cosmetic mod naming a unit the
/// gameplay side never defined. The walk must surface the error, never panic.
struct FailUnit;

impl IdMap for FailUnit {
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
        Ok(id)
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
    fn unit(&self, _: UnitId) -> Result<UnitId, ()> {
        Err(())
    }
    fn navmesh(&self, id: NavMeshId) -> Result<NavMeshId, ()> {
        Ok(id)
    }
    fn anim_state(&self, id: AnimStateId) -> Result<AnimStateId, ()> {
        Ok(id)
    }
}

/// A map that fails on any ability handle — models a cosmetic mod attaching an
/// effect visual to an ability the gameplay side never defined. The walk must
/// surface the error, never panic.
struct FailAbility;

impl IdMap for FailAbility {
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
        Ok(id)
    }
    fn curve(&self, id: CurveId) -> Result<CurveId, ()> {
        Ok(id)
    }
    fn damage_type(&self, id: DamageTypeId) -> Result<DamageTypeId, ()> {
        Ok(id)
    }
    fn ability(&self, _: AbilityId) -> Result<AbilityId, ()> {
        Err(())
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

const IDENT: &[u8] = b"abcdefghijklmnopqrstuvwxyz_0123456789/";

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
    fn f32(&mut self) -> f32 {
        // Always finite: value-equality after a round-trip is meaningful.
        f32::from(self.next()) / f32::from(u16::MAX) * 200.0 - 100.0
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
    fn model(&mut self) -> VisualModel {
        match self.next() % 3 {
            0 => VisualModel::Primitive {
                shape: match self.next() % 3 {
                    0 => PrimitiveShape::Cube,
                    1 => PrimitiveShape::Sphere,
                    _ => PrimitiveShape::Capsule,
                },
                color: [self.f32(), self.f32(), self.f32(), self.f32()],
            },
            1 => VisualModel::Model {
                asset: self.string(),
                scale: self.f32(),
                yaw_offset: self.f32(),
            },
            _ => VisualModel::Sprite { asset: self.string(), size: [self.f32(), self.f32()] },
        }
    }
    fn visual(&mut self) -> VisualDescriptor {
        VisualDescriptor { unit: UnitId(u32::from(self.next())), model: self.model() }
    }
    fn role(&mut self) -> EffectRole {
        match self.next() % 3 {
            0 => EffectRole::Projectile,
            1 => EffectRole::Impact,
            _ => EffectRole::CastIndicator,
        }
    }
    fn effect(&mut self) -> EffectVisualDescriptor {
        EffectVisualDescriptor {
            ability: AbilityId(u32::from(self.next())),
            role: self.role(),
            model: self.model(),
        }
    }
    fn names(&mut self) -> Names {
        // A cosmetic bundle names the units it dresses and the abilities its
        // effect visuals key on; the rest stay empty.
        Names { units: self.strings(), abilities: self.strings(), ..Names::default() }
    }
    fn client_registration(&mut self) -> ClientRegistration {
        ClientRegistration {
            abi: Version::new(
                u32::from(self.next()),
                u32::from(self.next()),
                u32::from(self.next()),
            ),
            names: self.names(),
            visuals: (0..self.count(4)).map(|_| self.visual()).collect(),
            effects: (0..self.count(4)).map(|_| self.effect()).collect(),
            // Animations have their own bundle-level invariants (see
            // `animation.rs`); this file stays about visuals.
            animations: Vec::new(),
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

#[test]
fn any_client_registration_survives_a_postcard_round_trip() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = build(s);
        let bytes = postcard::to_allocvec(&reg).expect("serialize");
        let back: ClientRegistration = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(reg, back, "client registration did not round-trip");
        let again = postcard::to_allocvec(&back).expect("reserialize");
        assert_eq!(bytes, again, "client registration serialization is not stable");
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
fn a_failing_unit_map_errors_without_panicking() {
    check!().with_type::<Scenario>().for_each(|s| {
        let counter = Counting::default();
        let mut probe = build(s);
        probe.remap_ids(&counter).expect("total map succeeds");
        let had_units = counter.units.get() > 0;

        let mut reg = build(s);
        let result = reg.remap_ids(&FailUnit);
        assert_eq!(result.is_err(), had_units, "error propagation disagrees with visual presence",);
    });
}

#[test]
fn a_failing_ability_map_errors_exactly_when_effect_visuals_are_present() {
    check!().with_type::<Scenario>().for_each(|s| {
        let counter = Counting::default();
        let mut probe = build(s);
        probe.remap_ids(&counter).expect("total map succeeds");
        // Effect visuals are the only place a cosmetic bundle names an ability.
        let had_effects = counter.abilities.get() > 0;

        let mut reg = build(s);
        let result = reg.remap_ids(&FailAbility);
        assert_eq!(
            result.is_err(),
            had_effects,
            "ability-error propagation disagrees with effect-visual presence",
        );
    });
}
