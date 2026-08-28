//! Invariants of the respawn ABI (stormlight/server#61) — how a mod says a unit
//! comes back, and what it comes back as.
//!
//! Death without a way back is not a loop: a killed training target stops being a
//! target, and a killed hero ends the session. So a unit declares its own return —
//! how long it stays down, and which of its carried state survives the trip. The
//! engine never invents either number; it reads this. (Buffs are *not* part of it
//! — each declares its own `drop_on_death`, because whether an effect outlives its
//! holder is a property of the effect.)
//!
//! The contract worth pinning here:
//!
//! - a spec survives the postcard round trip the guest→host boundary uses, flags
//!   and delay intact (a policy that flips crossing the boundary would wipe a
//!   hero's cooldowns on a mod's say-so it never gave);
//! - the remap walk leaves it entirely alone — a delay and two booleans hold no
//!   interned handle, and a walk that "fixed" them would corrupt content;
//! - [`RespawnSpec::seconds`] is **total**: whatever a mod declares — negative,
//!   infinite, NaN — reads as a finite, non-negative delay, so no arithmetic
//!   downstream can produce a deadline that never arrives.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::UnitId;
use stormlight_mod_abi::math::Value;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::respawn::RespawnSpec;
use stormlight_mod_abi::units::UnitDescriptor;

mod ids {
    pub use stormlight_mod_abi::ids::*;
}

/// A map that shifts every unit handle by one and leaves other families alone —
/// enough to see whether the walk touches a respawn spec (it must not).
struct ShiftUnit;

macro_rules! identity_families {
    ($( $fn_name:ident($ty:ident) ),+ $(,)?) => {$(
        fn $fn_name(&self, id: ids::$ty) -> Result<ids::$ty, ()> {
            Ok(id)
        }
    )+};
}

impl IdMap for ShiftUnit {
    type Error = ();

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
        anim_state(AnimStateId),
    );

    fn unit(&self, id: UnitId) -> Result<UnitId, ()> {
        Ok(UnitId(id.0 + 1))
    }
}

/// How a generated delay is chosen: ordinary declarations plus the three ways a
/// mod can hand the engine a number no clock can reach.
#[derive(Debug, TypeGenerator)]
enum Delay {
    /// A plain declaration, in 1/64ths of a second.
    Declared(u16),
    /// A delay a mod got the sign wrong on.
    Negative(u16),
    Infinite,
    NotANumber,
}

impl Delay {
    fn value(&self) -> f32 {
        match self {
            Delay::Declared(v) => f32::from(*v) / 64.0,
            Delay::Negative(v) => -f32::from(*v) / 64.0,
            Delay::Infinite => f32::INFINITY,
            Delay::NotANumber => f32::NAN,
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    after: Delay,
    clear_cooldowns: bool,
    refill_resources: bool,
    unit: u16,
}

fn spec(s: &Scenario) -> RespawnSpec {
    RespawnSpec {
        after: s.after.value(),
        clear_cooldowns: s.clear_cooldowns,
        refill_resources: s.refill_resources,
    }
}

/// A unit descriptor carrying `respawn`, otherwise as bare as one can be.
fn unit(id: UnitId, respawn: Option<RespawnSpec>) -> UnitDescriptor {
    UnitDescriptor {
        id,
        health: Value::Const(100.0),
        stats: Vec::new(),
        tags: Vec::new(),
        abilities: Vec::new(),
        resources: Vec::new(),
        talents: Vec::new(),
        talent_tree: None,
        respawn,
        progression: None,
        turn_rate: None,
        attack: None,
    }
}

#[test]
fn a_respawn_spec_survives_the_wire_round_trip_intact() {
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = spec(s);
        let bytes = postcard::to_allocvec(&declared).expect("a spec should encode");
        let back: RespawnSpec = postcard::from_bytes(&bytes).expect("a spec should decode");

        // The delay is compared through `seconds`, because NaN is not equal to
        // itself: the contract is that the *effective* delay crosses unchanged.
        assert_eq!(back.seconds(), declared.seconds(), "the delay changed crossing the ABI");
        assert_eq!(
            (back.clear_cooldowns, back.refill_resources),
            (declared.clear_cooldowns, declared.refill_resources),
            "a respawn policy flag flipped crossing the ABI",
        );
    });
}

#[test]
fn the_effective_delay_is_always_a_finite_non_negative_number() {
    check!().with_type::<Scenario>().for_each(|s| {
        let secs = spec(s).seconds();
        assert!(secs.is_finite(), "a declared delay produced a deadline no clock reaches");
        assert!(secs >= 0.0, "a declared delay produced a unit that returns before it dies");
        // A delay a mod declared honestly is passed through untouched — the
        // normalization is a floor for nonsense, not a policy of its own.
        if let Delay::Declared(v) = s.after {
            assert_eq!(secs, f32::from(v) / 64.0, "an ordinary declared delay was altered");
        }
    });
}

#[test]
fn the_remap_walk_leaves_a_respawn_declaration_alone() {
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = spec(s);
        let mut carrier = unit(UnitId(u32::from(s.unit)), Some(declared.clone()));
        carrier.remap_ids(&ShiftUnit).expect("an infallible map cannot fail");

        assert_eq!(carrier.id, UnitId(u32::from(s.unit) + 1), "the unit handle was not remapped");
        let after = carrier.respawn.expect("a remapped unit lost its respawn declaration");
        assert_eq!(after.seconds(), declared.seconds(), "the remap walk changed the delay");
        assert_eq!(
            (after.clear_cooldowns, after.refill_resources),
            (declared.clear_cooldowns, declared.refill_resources),
            "the remap walk changed the respawn policy",
        );
    });
}

#[test]
fn a_unit_that_declares_no_respawn_stays_down() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut carrier = unit(UnitId(u32::from(s.unit)), None);
        carrier.remap_ids(&ShiftUnit).expect("an infallible map cannot fail");
        assert!(
            carrier.respawn.is_none(),
            "a unit that declared no return was given one by the remap walk",
        );
    });
}
