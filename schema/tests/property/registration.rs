//! The `Registration` envelope — the postcard payload a mod emits across the
//! wasm boundary and the host decodes into its `ModRegistry`.
//!
//! Per the locked id model (O-3: a single concrete handle-based ISA; authoring
//! strings intern to handles at adoption), a mod ships **handle-based**
//! descriptors indexed in their local space, plus the **name tables** that gave
//! those handles their meaning. The host re-interns the names into the global
//! space and remaps at adoption. This file pins the one wire law S2 owns:
//!
//!   - **Round-trip**: any generated `Registration` survives a postcard
//!     serialize/deserialize unchanged, and re-serializes to identical bytes
//!     (stability — a hard requirement for a cross-boundary payload).
//!
//! Floats are drawn from bounded integer seeds so they are always finite and
//! value-equality is meaningful (no NaN); byte-stability is asserted regardless.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::abilities::{AbilityDescriptor, CastSpec, Params, Targeting};
use stormlight_mod_abi::behaviors::{BuffSpec, ModOp, Modifier, Reapply, StackScope, Stacking};
use stormlight_mod_abi::common::NumOp;
use stormlight_mod_abi::conditions::Condition;
use stormlight_mod_abi::descriptors::{Curve, Names, Registration};
use stormlight_mod_abi::ids::{
    AbilityId, BuffId, NavMeshId, ParamId, ResourceId, Slot, StackId, StatId, TagClassId, TagId,
    TalentId, UnitId,
};
use stormlight_mod_abi::manifest::Version;
use stormlight_mod_abi::math::Value;
use stormlight_mod_abi::navmesh::NavMeshDescriptor;
use stormlight_mod_abi::placement::UnitPlacement;
use stormlight_mod_abi::talents::{AbilitySelector, ParamPatch, QuestSpec, TalentDescriptor};
use stormlight_mod_abi::units::{ResourcePool, UnitDescriptor};

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

const IDENT: &[u8] = b"abcdefghijklmnopqrstuvwxyz_";

/// A cursor over a generated `u16` stream — the established generator shape in
/// this crate (see `integration/isa_roundtrip.rs`).
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
        let len = self.count(6) as usize;
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
    fn value(&mut self) -> Value {
        Value::Const(self.f32())
    }
    fn numop(&mut self) -> NumOp {
        match self.next() % 4 {
            0 => NumOp::Set,
            1 => NumOp::Add,
            2 => NumOp::Sub,
            _ => NumOp::Mul,
        }
    }
    fn modop(&mut self) -> ModOp {
        match self.next() % 4 {
            0 => ModOp::AddFlat,
            1 => ModOp::AddPct,
            2 => ModOp::Mul,
            _ => ModOp::Override,
        }
    }
    fn modifier(&mut self) -> Modifier {
        Modifier { stat: StatId(self.next()), op: self.modop(), value: self.value() }
    }
    fn modifiers(&mut self) -> Vec<Modifier> {
        let n = self.count(3);
        (0..n).map(|_| self.modifier()).collect()
    }
    fn tags(&mut self) -> Vec<TagId> {
        let n = self.count(3);
        (0..n).map(|_| TagId(self.next())).collect()
    }
    fn ability(&mut self) -> AbilityDescriptor {
        let params = (0..self.count(3)).map(|_| (ParamId(self.next()), self.value())).collect();
        AbilityDescriptor {
            id: AbilityId(u32::from(self.next())),
            params: Params(params),
            targeting: Targeting::NoTarget,
            cast: CastSpec::Instant,
            cost: Vec::new(),
            cast_gate: Condition::Always,
            on_cast_start: Vec::new(),
            on_cast: Vec::new(),
            tags: self.tags(),
        }
    }
    fn talent(&mut self) -> TalentDescriptor {
        let patches = (0..self.count(3))
            .map(|_| ParamPatch {
                param: ParamId(self.next()),
                op: self.numop(),
                value: self.value(),
            })
            .collect();
        TalentDescriptor {
            id: TalentId(u32::from(self.next())),
            selector: AbilitySelector::Any,
            patches,
            riders: Vec::new(),
            add_reactions: Vec::new(),
            grants: Vec::new(),
            modifiers: self.modifiers(),
            tags: self.tags(),
            // Half the talents set a task, so the round trip covers both
            // (server#132).
            quest: (self.next().is_multiple_of(2))
                .then(|| QuestSpec { counter: StackId(self.next()), goal: f32::from(self.next()) }),
        }
    }
    fn buff(&mut self) -> BuffSpec {
        let stacking = Stacking {
            on_reapply: match self.next() % 4 {
                0 => Reapply::RefreshDuration,
                1 => Reapply::AddDuration,
                2 => Reapply::Independent,
                _ => Reapply::Ignore,
            },
            scope: if self.next().is_multiple_of(2) {
                StackScope::PerSource
            } else {
                StackScope::Global
            },
        };
        BuffSpec {
            id: BuffId(self.next()),
            duration: self.next().is_multiple_of(2).then(|| self.value()),
            stacking,
            max_stacks: self.next(),
            modifiers: self.modifiers(),
            tags: self.tags(),
            reactions: Vec::new(),
            on_apply: Vec::new(),
            on_expire: Vec::new(),
            on_remove: Vec::new(),
            drop_on_death: self.next().is_multiple_of(2),
        }
    }
    fn curve(&mut self) -> Curve {
        let points = (0..self.count(5)).map(|_| [self.f32(), self.f32()]).collect();
        Curve { points }
    }
    /// A small walkable region with one hole and whatever it stands up. Geometry
    /// is leaf data, so the generator only has to exercise the shape; the
    /// placements carry the one handle a map descriptor holds beyond its own id.
    fn navmesh(&mut self) -> NavMeshDescriptor {
        NavMeshDescriptor {
            id: NavMeshId(u32::from(self.next())),
            outline: (0..self.count(5) + 3).map(|_| [self.f32(), self.f32()]).collect(),
            obstacles: (0..self.count(2))
                .map(|_| (0..self.count(3) + 3).map(|_| [self.f32(), self.f32()]).collect())
                .collect(),
            agent_radius: self.f32().abs(),
            placements: (0..self.count(3))
                .map(|_| UnitPlacement {
                    unit: UnitId(u32::from(self.next())),
                    team: u32::from(self.next() % 4),
                    at: [self.f32(), self.f32()],
                    facing: self.f32(),
                    respawn: self.next().is_multiple_of(2).then(|| self.f32().abs()),
                })
                .collect(),
        }
    }
    fn unit(&mut self) -> UnitDescriptor {
        let stats = (0..self.count(3)).map(|_| (StatId(self.next()), self.value())).collect();
        let abilities = (0..self.count(3))
            .map(|_| (Slot(self.next() as u8), AbilityId(u32::from(self.next()))))
            .collect();
        let resources = (0..self.count(3))
            .map(|_| ResourcePool {
                id: ResourceId(self.next()),
                max: self.value(),
                regen: self.value(),
            })
            .collect();
        let talents = (0..self.count(3)).map(|_| TalentId(u32::from(self.next()))).collect();
        UnitDescriptor {
            id: UnitId(u32::from(self.next())),
            health: self.value(),
            stats,
            tags: self.tags(),
            abilities,
            resources,
            talents,
            talent_tree: None,
            respawn: None,
            progression: None,
            turn_rate: None,
            attack: None,
        }
    }
    fn names(&mut self) -> Names {
        Names {
            stats: self.strings(),
            resources: self.strings(),
            stacks: self.strings(),
            tags: self.strings(),
            tag_classes: self.strings(),
            params: self.strings(),
            events: self.strings(),
            buffs: self.strings(),
            curves: self.strings(),
            damage_types: self.strings(),
            dims: self.strings(),
            abilities: self.strings(),
            talents: self.strings(),
            handlers: self.strings(),
            units: self.strings(),
            navmeshes: self.strings(),
            anim_states: self.strings(),
        }
    }
    fn registration(&mut self) -> Registration {
        Registration {
            abi: Version::new(
                u32::from(self.next()),
                u32::from(self.next()),
                u32::from(self.next()),
            ),
            names: self.names(),
            abilities: (0..self.count(3)).map(|_| self.ability()).collect(),
            talents: (0..self.count(3)).map(|_| self.talent()).collect(),
            buffs: (0..self.count(3)).map(|_| self.buff()).collect(),
            tag_classes: (0..self.count(4))
                .map(|_| (TagId(self.next()), TagClassId(self.next())))
                .collect(),
            curves: (0..self.count(3)).map(|_| self.curve()).collect(),
            units: (0..self.count(3)).map(|_| self.unit()).collect(),
            navmeshes: (0..self.count(2)).map(|_| self.navmesh()).collect(),
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    ops: Vec<u16>,
}

#[test]
fn any_registration_survives_a_postcard_round_trip() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut g = Gen { ops: &s.ops, pos: 0 };
        let reg = g.registration();

        let bytes = postcard::to_allocvec(&reg).expect("serialize");
        let back: Registration = postcard::from_bytes(&bytes).expect("deserialize");
        // Finite floats throughout, so value-equality is the strong statement.
        assert_eq!(reg, back, "registration did not round-trip");
        // And the byte form is stable (no ambiguity in the wire encoding).
        let again = postcard::to_allocvec(&back).expect("reserialize");
        assert_eq!(bytes, again, "registration serialization is not stable");
    });
}
