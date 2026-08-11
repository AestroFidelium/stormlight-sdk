//! Invariants of the navmesh ABI (stormlight/server#52) — the content-supplied
//! map geometry a mod ships and the engine bakes into its pathing stack.
//!
//! Geometry is the one descriptor family that is pure data: an outline, holes,
//! and an agent radius, with a single interned handle (its own id) so units and
//! abilities can name a mesh. The invariants that matter to the ABI are therefore
//! the wire contract and the remap walk:
//!
//! - a descriptor survives the postcard round trip the guest→host boundary uses,
//!   with its geometry bit-identical (a shifted vertex is a hole in a wall);
//! - the remap walk rewrites the mesh's own handle and nothing else, and is a
//!   no-op under an identity map.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::NavMeshId;
use stormlight_mod_abi::navmesh::NavMeshDescriptor;
use stormlight_mod_abi::remap::{IdMap, RemapIds};

mod ids {
    pub use stormlight_mod_abi::ids::*;
}

/// A map that shifts every navmesh handle by one and leaves other families alone
/// — enough to see that the walk reaches the mesh id and stops there.
struct ShiftNavMesh;

macro_rules! identity_families {
    ($( $fn_name:ident($ty:ident) ),+ $(,)?) => {$(
        fn $fn_name(&self, id: ids::$ty) -> Result<ids::$ty, ()> {
            Ok(id)
        }
    )+};
}

impl IdMap for ShiftNavMesh {
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
        unit(UnitId),
        anim_state(AnimStateId),
    );

    fn navmesh(&self, id: NavMeshId) -> Result<NavMeshId, ()> {
        Ok(NavMeshId(id.0 + 1))
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    id: u16,
    outline: [(i16, i16); 4],
    hole: [(i16, i16); 3],
    radius: u8,
}

fn descriptor(s: &Scenario) -> NavMeshDescriptor {
    NavMeshDescriptor {
        id: NavMeshId(u32::from(s.id)),
        outline: s.outline.iter().map(|(x, z)| [f32::from(*x), f32::from(*z)]).collect(),
        obstacles: vec![s.hole.iter().map(|(x, z)| [f32::from(*x), f32::from(*z)]).collect()],
        agent_radius: f32::from(s.radius) / 64.0,
        placements: Vec::new(),
    }
}

#[test]
fn a_descriptor_survives_the_wire_round_trip_with_its_geometry_intact() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mesh = descriptor(s);
        let bytes = postcard::to_allocvec(&mesh).expect("navmesh should encode");
        let back: NavMeshDescriptor = postcard::from_bytes(&bytes).expect("navmesh should decode");
        assert_eq!(back, mesh, "the mesh changed shape crossing the ABI boundary");
    });
}

#[test]
fn the_remap_walk_rewrites_the_mesh_handle_and_leaves_the_geometry_alone() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mesh = descriptor(s);
        let mut remapped = mesh.clone();
        remapped.remap_ids(&ShiftNavMesh).expect("an infallible map cannot fail");

        assert_eq!(remapped.id, NavMeshId(mesh.id.0 + 1), "the mesh handle was not remapped");
        assert_eq!(remapped.outline, mesh.outline, "remapping moved the outline");
        assert_eq!(remapped.obstacles, mesh.obstacles, "remapping moved an obstacle");
        assert_eq!(remapped.agent_radius, mesh.agent_radius, "remapping changed the agent radius");
    });
}
