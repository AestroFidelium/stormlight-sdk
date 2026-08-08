//! Invariants of the unit-placement ABI (stormlight/server#60) — how a map mod
//! says "one of these stands here", so the engine can populate a battlefield it
//! knows nothing about.
//!
//! A placement is the second half of map content: the geometry says where a unit
//! *may* stand, a placement says which unit *does*, on which side, facing which
//! way. It carries exactly one interned handle — the unit it names — so the
//! invariants that matter to the ABI are the wire contract and the remap walk:
//!
//! - a placement survives the postcard round trip the guest→host boundary uses,
//!   with its position and facing bit-identical (a shifted dummy is a dummy in a
//!   wall);
//! - the remap walk rewrites the unit handle and *only* that: team, position,
//!   facing and respawn delay are numbers, not ids;
//! - placements ride along inside the map descriptor, so remapping a mesh reaches
//!   every placement it declares — a map's units cannot be left in the mod's local
//!   id space while its geometry goes global.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{NavMeshId, UnitId};
use stormlight_mod_abi::navmesh::NavMeshDescriptor;
use stormlight_mod_abi::placement::UnitPlacement;
use stormlight_mod_abi::remap::{IdMap, RemapIds};

mod ids {
    pub use stormlight_mod_abi::ids::*;
}

/// A map that shifts every unit handle by one and leaves other families alone —
/// enough to see that the walk reaches the placed unit and stops there.
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
    );

    fn unit(&self, id: UnitId) -> Result<UnitId, ()> {
        Ok(UnitId(id.0 + 1))
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    unit: u16,
    team: u8,
    x: i16,
    z: i16,
    facing: i16,
    respawn: Option<u16>,
}

fn placement(s: &Scenario) -> UnitPlacement {
    UnitPlacement {
        unit: UnitId(u32::from(s.unit)),
        team: u32::from(s.team),
        at: [f32::from(s.x) / 8.0, f32::from(s.z) / 8.0],
        facing: f32::from(s.facing) / 1024.0,
        respawn: s.respawn.map(|r| f32::from(r) / 64.0),
    }
}

#[test]
fn a_placement_survives_the_wire_round_trip_intact() {
    check!().with_type::<Scenario>().for_each(|s| {
        let placed = placement(s);
        let bytes = postcard::to_allocvec(&placed).expect("placement should encode");
        let back: UnitPlacement = postcard::from_bytes(&bytes).expect("placement should decode");
        assert_eq!(back, placed, "the placement changed crossing the ABI boundary");
    });
}

#[test]
fn the_remap_walk_rewrites_the_unit_and_leaves_the_transform_alone() {
    check!().with_type::<Scenario>().for_each(|s| {
        let placed = placement(s);
        let mut remapped = placed.clone();
        remapped.remap_ids(&ShiftUnit).expect("an infallible map cannot fail");

        assert_eq!(remapped.unit, UnitId(placed.unit.0 + 1), "the unit handle was not remapped");
        assert_eq!(remapped.team, placed.team, "remapping changed the side");
        assert_eq!(remapped.at, placed.at, "remapping moved the placement");
        assert_eq!(remapped.facing, placed.facing, "remapping turned the placement");
        assert_eq!(remapped.respawn, placed.respawn, "remapping changed the respawn delay");
    });
}

#[test]
fn remapping_a_map_reaches_every_unit_it_places() {
    check!().with_type::<[Scenario; 3]>().for_each(|scenarios| {
        let placements: Vec<UnitPlacement> = scenarios.iter().map(placement).collect();
        let mesh = NavMeshDescriptor {
            id: NavMeshId(0),
            outline: vec![[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]],
            obstacles: Vec::new(),
            agent_radius: 0.5,
            placements: placements.clone(),
        };

        let mut remapped = mesh.clone();
        remapped.remap_ids(&ShiftUnit).expect("an infallible map cannot fail");

        for (after, before) in remapped.placements.iter().zip(&placements) {
            assert_eq!(
                after.unit,
                UnitId(before.unit.0 + 1),
                "a placement kept its local unit handle while the map went global",
            );
        }
        assert_eq!(remapped.outline, mesh.outline, "remapping moved the map outline");
    });
}
