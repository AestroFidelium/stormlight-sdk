//! Invariants of the basic-attack ABI (stormlight/server#89).
//!
//! An attack is content, not an engine feature: a payload of the ordinary effect
//! ISA plus a timeline the engine drives it on. Two things have to hold for that
//! to be true rather than merely intended.
//!
//! - **It crosses the guest→host boundary intact.** The payload, the delivery
//!   shape and all four timeline numbers survive the postcard round trip a mod's
//!   registration goes through. An attack whose wind-up changed in transit would
//!   swing at a moment no mod asked for.
//! - **Every handle inside it is remapped.** The descriptor is authored in the
//!   mod's *local* id space and adopted into the engine's global one, so every
//!   interned handle buried in the payload tree, the target filter, the launched
//!   body and the four `Value`s has to be rewritten — reached through the unit
//!   that carries it. A handle the walk misses points at whatever content happens
//!   to occupy that index globally, which is silent corruption rather than a
//!   failure: the wrong damage type, the wrong tag, another mod's stat.
//!
//! The shifting map below moves **every** family, so a field the walk forgets
//! shows up as an id that stayed put.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::attacks::{AttackDelivery, AttackDescriptor};
use stormlight_mod_abi::common::{Affiliation, ImpactTarget, TargetFilter};
use stormlight_mod_abi::ids::{DamageTypeId, StatId, TagId, UnitId};
use stormlight_mod_abi::impacts::{DamageFlags, Impact};
use stormlight_mod_abi::math::{Value, Var, Who};
use stormlight_mod_abi::missiles::{BodyDescriptor, BodyFlags, BodyKind, CollisionSpec};
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::units::UnitDescriptor;

mod ids {
    pub use stormlight_mod_abi::ids::*;
}

/// How far every handle moves. Non-zero, so "was it rewritten?" is decidable.
/// The generated ids below stay under it, so a shifted id can never collide with
/// an unshifted one and a missed field is always visible.
const SHIFT: u16 = 1000;

/// A map that shifts **every** interned family by [`SHIFT`]. A field the remap
/// walk skips keeps its authored id and fails the assertions below.
struct ShiftAll;

macro_rules! shifted_families {
    ($width:ty; $( $fn_name:ident($ty:ident) ),+ $(,)?) => {$(
        fn $fn_name(&self, id: ids::$ty) -> Result<ids::$ty, ()> {
            Ok(ids::$ty(id.0 + <$width>::from(SHIFT)))
        }
    )+};
}

impl IdMap for ShiftAll {
    type Error = ();

    shifted_families!(u16;
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
        anim_state(AnimStateId),
    );

    shifted_families!(u32;
        ability(AbilityId),
        talent(TalentId),
        handler(HandlerId),
        unit(UnitId),
        navmesh(NavMeshId),
    );
}

/// Which delivery shape a generated attack declares.
#[derive(Debug, TypeGenerator)]
enum Delivery {
    Melee,
    /// A launched body, whose collision filter carries a tag handle of its own.
    Ranged {
        speed: u16,
        body_tag: u8,
    },
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// Every generated handle is a `u8`, so `id + SHIFT` cannot wrap and a
    /// shifted id can never be mistaken for one the walk left alone.
    unit: u8,
    /// The stat the damage scales off — a handle buried two levels down.
    damage_stat: u8,
    /// The damage type the payload deals.
    dtype: u8,
    /// A tag the attack refuses to target.
    excluded: u8,
    /// The stat the wind-up reads, so a timeline `Value` carries a handle too.
    windup_stat: u8,
    delivery: Delivery,
    /// Timeline numbers, in 1/64ths of a second / world units.
    recovery: u16,
    period: u16,
    range: u16,
}

fn sixty_fourths(v: u16) -> f32 {
    f32::from(v) / 64.0
}

fn delivery(s: &Scenario) -> AttackDelivery {
    match s.delivery {
        Delivery::Melee => AttackDelivery::Melee,
        Delivery::Ranged { speed, body_tag } => AttackDelivery::Ranged {
            body: BodyDescriptor {
                kind: BodyKind::Missile {
                    speed: Value::Const(sixty_fourths(speed)),
                    range: Value::Const(20.0),
                    homing: true,
                    pierce: Value::Const(0.0),
                },
                on_spawn: Vec::new(),
                on_hit: Vec::new(),
                on_expire: Vec::new(),
                collision: CollisionSpec {
                    filter: TargetFilter {
                        affiliation: Affiliation::Enemies,
                        require_tags: alloc_vec(TagId(u16::from(body_tag))),
                        exclude_tags: Vec::new(),
                        include_dead: false,
                    },
                    pierce: Value::Const(0.0),
                    through_walls: false,
                },
                flags: BodyFlags::default(),
            },
        },
    }
}

fn alloc_vec(tag: TagId) -> Vec<TagId> {
    vec![tag]
}

fn attack(s: &Scenario) -> AttackDescriptor {
    AttackDescriptor {
        payload: vec![Impact::Damage {
            amount: Value::Read(Var::Stat(StatId(u16::from(s.damage_stat)), Who::Caster)),
            dtype: DamageTypeId(u16::from(s.dtype)),
            target: ImpactTarget::ResolvedTarget,
            flags: DamageFlags { can_crit: true, lifesteal: false },
        }],
        filter: TargetFilter {
            affiliation: Affiliation::Enemies,
            require_tags: Vec::new(),
            exclude_tags: alloc_vec(TagId(u16::from(s.excluded))),
            include_dead: false,
        },
        delivery: delivery(s),
        windup: Value::Read(Var::Stat(StatId(u16::from(s.windup_stat)), Who::Caster)),
        recovery: Value::Const(sixty_fourths(s.recovery)),
        period: Value::Const(sixty_fourths(s.period)),
        range: Value::Const(sixty_fourths(s.range)),
    }
}

/// A unit descriptor carrying `attack`, otherwise as bare as one can be.
fn unit(id: UnitId, attack: Option<AttackDescriptor>) -> UnitDescriptor {
    UnitDescriptor {
        id,
        health: Value::Const(100.0),
        stats: Vec::new(),
        tags: Vec::new(),
        abilities: Vec::new(),
        resources: Vec::new(),
        talents: Vec::new(),
        talent_tree: None,
        respawn: None,
        progression: None,
        turn_rate: None,
        attack,
    }
}

#[test]
fn an_attack_declaration_survives_the_wire_round_trip_intact() {
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = attack(s);
        let bytes = postcard::to_allocvec(&declared).expect("an attack should encode");
        let back: AttackDescriptor = postcard::from_bytes(&bytes).expect("an attack should decode");
        assert_eq!(back, declared, "an attack changed crossing the ABI boundary");
    });
}

#[test]
fn every_handle_in_an_attack_is_rewritten_at_adoption() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut carrier = unit(UnitId(u32::from(s.unit)), Some(attack(s)));
        carrier.remap_ids(&ShiftAll).expect("an infallible map cannot fail");
        let after = carrier.attack.expect("a remapped unit lost its attack");

        // The payload's handles, two levels down inside a `Value`.
        let Impact::Damage { amount, dtype, .. } = &after.payload[0] else {
            panic!("the payload's shape changed under the walk");
        };
        assert_eq!(
            *dtype,
            DamageTypeId(u16::from(s.dtype) + SHIFT),
            "the payload's damage type was left in the mod's local id space",
        );
        assert_eq!(
            *amount,
            Value::Read(Var::Stat(StatId(u16::from(s.damage_stat) + SHIFT), Who::Caster)),
            "a stat read inside the payload was left in the mod's local id space",
        );

        // The filter that decides what may be attacked at all.
        assert_eq!(
            after.filter.exclude_tags,
            alloc_vec(TagId(u16::from(s.excluded) + SHIFT)),
            "the attack's target filter was left in the mod's local id space",
        );

        // A timeline number is a `Value`, and a `Value` may read a stat.
        assert_eq!(
            after.windup,
            Value::Read(Var::Stat(StatId(u16::from(s.windup_stat) + SHIFT), Who::Caster)),
            "a stat read by the attack's timeline was left in the mod's local id space",
        );

        // The launched body, for a ranged attack.
        if let Delivery::Ranged { body_tag, .. } = s.delivery {
            let AttackDelivery::Ranged { body } = &after.delivery else {
                panic!("the delivery shape changed under the walk");
            };
            assert_eq!(
                body.collision.filter.require_tags,
                alloc_vec(TagId(u16::from(body_tag) + SHIFT)),
                "the launched body's collision filter was left in the local id space",
            );
        }
    });
}

#[test]
fn a_unit_that_declares_no_attack_is_given_none_by_adoption() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut carrier = unit(UnitId(u32::from(s.unit)), None);
        carrier.remap_ids(&ShiftAll).expect("an infallible map cannot fail");
        assert!(
            carrier.attack.is_none(),
            "a unit that declared no weapon was handed one by the remap walk",
        );
    });
}
