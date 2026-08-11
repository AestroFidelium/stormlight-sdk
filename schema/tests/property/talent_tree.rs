//! Invariants of the talent-tree ABI (stormlight/server#63) — how a mod declares
//! the structure a player's talent choices are made *within*.
//!
//! The talent fold has always been able to apply anything to anything; what it had
//! no notion of was **which** talents a unit may choose, when they become
//! choosable, and which of them exclude each other. That structure is content —
//! how many tiers a hero has, at what levels, and what each one offers is a balance
//! decision — so it is declared here and the engine only enforces the generic rule
//! that falls out of it: one pick per unlocked tier, from that tier's own options.
//!
//! The contract this pins:
//!
//! - a tree survives the postcard round trip the guest→host boundary uses, tiers,
//!   unlock levels, options and re-pick policy intact. A policy that flipped
//!   crossing the ABI would let a player swap a choice a mod meant to be final;
//! - **unlocking is monotonic in level**, because it is derived from the level
//!   rather than latched: a tier choosable at one level is choosable at every level
//!   above it, and one that is not stays shut until the level reaches it;
//! - the remap walk rewrites **every** option handle a tree carries and leaves the
//!   tier's own level and index alone — an option that kept its local id would name
//!   whatever talent another mod happened to intern there, and a rewritten tier
//!   index would move a whole tier's worth of choices to another level;
//! - the accessors are **total** over whatever a mod declares: an empty tree, an
//!   out-of-range tier index, a tier that offers nothing, a talent that appears in
//!   no tier at all — each has an answer, and none of them panics.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{TalentId, UnitId};
use stormlight_mod_abi::math::Value;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::talent_tree::{MAX_TIERS, RepickPolicy, TalentTier, TalentTree};
use stormlight_mod_abi::units::UnitDescriptor;

mod ids {
    pub use stormlight_mod_abi::ids::*;
}

/// A map that shifts exactly the one family a tree can reference and leaves every
/// other alone — so an option handle the walk forgets shows up as an unshifted
/// number rather than as a silent pass.
struct ShiftTalents;

macro_rules! identity_families {
    ($( $fn_name:ident($ty:ident) ),+ $(,)?) => {$(
        fn $fn_name(&self, id: ids::$ty) -> Result<ids::$ty, ()> {
            Ok(id)
        }
    )+};
}

impl IdMap for ShiftTalents {
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
        handler(HandlerId),
        unit(UnitId),
        navmesh(NavMeshId),
        anim_state(AnimStateId),
    );

    fn talent(&self, id: ids::TalentId) -> Result<ids::TalentId, ()> {
        Ok(ids::TalentId(id.0.wrapping_add(1)))
    }
}

/// One generated tier: the level it opens at and the handles it offers.
#[derive(Debug, TypeGenerator)]
struct Tier {
    level: u8,
    options: Vec<u16>,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    tiers: Vec<Tier>,
    free: bool,
    /// Probed against the tree — deliberately unconstrained, so out-of-range
    /// indices, unoffered talents and empty trees are all generated.
    probe_tier: u8,
    probe_talent: u16,
    probe_level: u8,
    unit: u16,
}

fn tree(s: &Scenario) -> TalentTree {
    TalentTree {
        tiers: s
            .tiers
            .iter()
            .map(|t| TalentTier {
                level: t.level,
                options: t.options.iter().map(|o| TalentId(u32::from(*o))).collect(),
            })
            .collect(),
        repick: if s.free { RepickPolicy::Free } else { RepickPolicy::Locked },
    }
}

/// A unit descriptor carrying `talent_tree`, otherwise as bare as one can be.
fn unit(id: UnitId, talent_tree: Option<TalentTree>) -> UnitDescriptor {
    UnitDescriptor {
        id,
        health: Value::Const(100.0),
        stats: Vec::new(),
        tags: Vec::new(),
        abilities: Vec::new(),
        resources: Vec::new(),
        talents: Vec::new(),
        talent_tree,
        respawn: None,
        progression: None,
    }
}

#[test]
fn a_talent_tree_survives_the_wire_round_trip_intact() {
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = tree(s);
        let bytes = postcard::to_allocvec(&declared).expect("a tree should encode");
        let back: TalentTree = postcard::from_bytes(&bytes).expect("a tree should decode");

        assert_eq!(back, declared, "a talent tree changed crossing the ABI");
        assert_eq!(
            back.repick, declared.repick,
            "the re-pick policy flipped crossing the ABI — a final choice became swappable",
        );
    });
}

#[test]
fn a_tier_once_unlocked_stays_unlocked_as_the_level_climbs() {
    check!().with_type::<Scenario>().for_each(|s| {
        let tree = tree(s);
        let index = s.probe_tier;
        let Some(tier) = tree.tier(index) else { return };

        // Unlocking is derived from the level, not latched from an event, so it is
        // a monotone predicate in the level — the property that also makes a rewind
        // past a threshold re-lock the tier instead of leaving it open.
        let low = s.probe_level;
        for step in 0..=u8::MAX.saturating_sub(low) {
            let high = low.saturating_add(step);
            if tree.unlocked(index, low) {
                assert!(
                    tree.unlocked(index, high),
                    "tier {index} (level {}) closed again at level {high}",
                    tier.level,
                );
            }
            if !tree.unlocked(index, high) {
                assert!(
                    !tree.unlocked(index, low),
                    "tier {index} (level {}) was open below the level that unlocks it",
                    tier.level,
                );
            }
        }
    });
}

#[test]
fn a_tier_is_unlocked_exactly_at_the_level_it_declares() {
    check!().with_type::<Scenario>().for_each(|s| {
        let tree = tree(s);
        let index = s.probe_tier;
        let Some(tier) = tree.tier(index) else {
            // An index past the declared tiers is not a tier, at any level.
            assert!(
                !tree.unlocked(index, u8::MAX),
                "a tier the tree never declared reported itself unlocked",
            );
            return;
        };
        assert_eq!(
            tree.unlocked(index, s.probe_level),
            s.probe_level >= tier.level,
            "the unlock gate disagreed with the level the tier declares",
        );
    });
}

#[test]
fn the_tier_a_talent_belongs_to_is_the_one_that_offers_it() {
    check!().with_type::<Scenario>().for_each(|s| {
        let tree = tree(s);
        let talent = TalentId(u32::from(s.probe_talent));

        match tree.tier_of(talent) {
            Some(index) => {
                let tier = tree.tier(index).expect("tier_of named a tier that is not there");
                assert!(tier.offers(talent), "tier_of named a tier that does not offer the talent");
                // The *first* such tier, so a mod that (wrongly) lists one talent
                // twice still gets one stable answer rather than an arbitrary one.
                for below in 0..index {
                    let earlier = tree.tier(below).expect("a tier below the named one");
                    assert!(
                        !earlier.offers(talent),
                        "tier_of skipped tier {below}, which also offers the talent",
                    );
                }
            }
            None => {
                for (i, tier) in tree.tiers.iter().take(MAX_TIERS).enumerate() {
                    assert!(
                        !tier.offers(talent),
                        "tier {i} offers a talent the tree says belongs to no tier",
                    );
                }
            }
        }
    });
}

#[test]
fn the_remap_walk_rewrites_every_option_a_tree_carries() {
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = tree(s);
        let mut carrier = unit(UnitId(u32::from(s.unit)), Some(declared.clone()));
        carrier.remap_ids(&ShiftTalents).expect("an infallible map cannot fail");

        let after = carrier.talent_tree.expect("a remapped unit lost its talent tree");
        assert_eq!(
            after.repick, declared.repick,
            "the re-pick policy was rewritten as if a handle"
        );
        assert_eq!(
            after.tiers.len(),
            declared.tiers.len(),
            "the remap walk changed the tier count"
        );

        for (i, (before, now)) in declared.tiers.iter().zip(after.tiers.iter()).enumerate() {
            assert_eq!(
                now.level, before.level,
                "tier {i}'s unlock level was rewritten as if it were a handle",
            );
            let expected: Vec<TalentId> =
                before.options.iter().map(|o| TalentId(o.0.wrapping_add(1))).collect();
            assert_eq!(now.options, expected, "tier {i} kept an un-remapped option handle");
        }
    });
}

#[test]
fn a_unit_that_declares_no_tree_is_given_none() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut carrier = unit(UnitId(u32::from(s.unit)), None);
        carrier.remap_ids(&ShiftTalents).expect("an infallible map cannot fail");
        assert!(
            carrier.talent_tree.is_none(),
            "a unit that declared no talent tree was given one by the remap walk",
        );
    });
}

#[test]
fn every_accessor_is_total_over_whatever_a_mod_declares() {
    check!().with_type::<Scenario>().for_each(|s| {
        let tree = tree(s);
        let talent = TalentId(u32::from(s.probe_talent));

        // Addressability: a tier index is a `u8` on the wire, so tiers past the
        // representable window are simply not reachable — and must not be reported
        // as reachable either.
        let reachable = tree.tiers.len().min(MAX_TIERS);
        assert_eq!(tree.len(), reachable, "the tree reported tiers no `u8` index can name",);
        assert_eq!(tree.is_empty(), reachable == 0, "an empty tree disagreed with its own length");

        // Every probe answers rather than panicking, and the answers agree with
        // each other.
        let index = s.probe_tier;
        assert_eq!(
            tree.tier(index).is_some(),
            usize::from(index) < reachable,
            "tier lookup disagreed with the tree's own reachable length",
        );
        if let Some(found) = tree.tier_of(talent) {
            assert!(usize::from(found) < reachable, "tier_of named an unreachable tier");
        }
    });
}
