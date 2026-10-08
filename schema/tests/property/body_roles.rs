//! The pieces of feedback a body that stays in the world is dressed by
//! (stormlight/server#220).
//!
//! A missile is drawn by the [`EffectRole::Projectile`] its ability declares; the
//! three bodies that do not travel — a ground zone, an item lying on the ground and
//! a summoned unit — are keyed the same way, by the ability that spawned them, so a
//! cosmetic mod dresses them without the engine learning what they are.
//!
//! The one invariant worth a file of its own is the wire: the roles are a tag on
//! every effect visual a cosmetic bundle has ever shipped, so the new ones are
//! **appended** and every role that existed before keeps its number.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::visuals::EffectRole;

/// Every role, in declaration order, with the wire tag it has to keep.
const ROLES: [(EffectRole, u8); 7] = [
    (EffectRole::Projectile, 0),
    (EffectRole::Impact, 1),
    (EffectRole::CastIndicator, 2),
    (EffectRole::Miss, 3),
    (EffectRole::Zone, 4),
    (EffectRole::Pickup, 5),
    (EffectRole::Summon, 6),
];

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// Which role this run encodes.
    role: u8,
}

#[test]
fn every_role_keeps_its_wire_tag() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (role, tag) = ROLES[usize::from(s.role) % ROLES.len()];
        let bytes = postcard::to_allocvec(&role).expect("a role encodes");
        assert_eq!(bytes, [tag], "{role:?} moved on the wire");
        let back: EffectRole = postcard::from_bytes(&bytes).expect("and decodes");
        assert_eq!(back, role, "{role:?} did not survive the wire");
    });
}
