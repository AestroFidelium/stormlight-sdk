//! Invariants of tags + capability classes — the "binary state" half of the
//! effect model. Generic engine systems ask "does this unit have capability
//! class X?" and never name a specific status, so a mod invents new crowd
//! control by registering a tag into existing classes with no engine change.
//! Laws:
//!   - **Soundness**: `has_class(c)` iff some active tag is registered into `c`.
//!   - **Monotonicity**: adding a tag never removes a capability.
//!   - An unregistered tag grants no capability ("slow is a modifier, not a
//!     class").

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{TagClassId, TagId};
use stormlight_mod_abi::units::{TagRegistry, TagSet};

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// (tag, class) registrations; small ids so overlaps are common.
    registrations: Vec<(u8, u8)>,
    /// Active tags on the unit.
    active: Vec<u8>,
    /// A tag to add for the monotonicity probe.
    extra: u8,
    /// A class to probe.
    probe: u8,
}

const TAGS: u16 = 8;
const CLASSES: u16 = 4;

fn tag(seed: u8) -> TagId {
    TagId(u16::from(seed) % TAGS)
}
fn class(seed: u8) -> TagClassId {
    TagClassId(u16::from(seed) % CLASSES)
}

fn build(s: &Scenario) -> (TagRegistry, TagSet, TagClassId) {
    let mut reg = TagRegistry::default();
    for &(t, c) in &s.registrations {
        reg.register(tag(t), class(c));
    }
    let mut set = TagSet::default();
    for &t in &s.active {
        set.insert(tag(t));
    }
    (reg, set, class(s.probe))
}

/// Independent recomputation: is any active tag registered into `c`?
fn expected_has_class(reg: &TagRegistry, active: &[TagId], c: TagClassId) -> bool {
    active.iter().any(|t| reg.classes_of(*t).contains(&c))
}

#[test]
fn has_class_matches_independent_recompute() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (reg, set, c) = build(s);
        let active: Vec<TagId> = s.active.iter().map(|&t| tag(t)).collect();
        assert_eq!(set.has_class(c, &reg), expected_has_class(&reg, &active, c));
    });
}

#[test]
fn adding_a_tag_never_removes_a_capability() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (reg, mut set, c) = build(s);
        let before = set.has_class(c, &reg);
        set.insert(tag(s.extra));
        let after = set.has_class(c, &reg);
        assert!(!before || after, "adding a tag dropped capability {c:?}");
    });
}

#[test]
fn removing_the_added_tag_restores_membership() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (reg, set, _c) = build(s);
        // Round-trip: insert then remove a tag not otherwise present.
        let t = tag(s.extra);
        if set.contains(t) {
            return;
        }
        let mut probe = set.clone();
        probe.insert(t);
        probe.remove(t);
        for cls in 0..CLASSES {
            let c = TagClassId(cls);
            assert_eq!(
                probe.has_class(c, &reg),
                set.has_class(c, &reg),
                "insert+remove changed capability {c:?}"
            );
        }
    });
}

#[test]
fn unregistered_tag_grants_no_capability() {
    check!().with_type::<u8>().for_each(|&t| {
        let reg = TagRegistry::default(); // nothing registered
        let mut set = TagSet::default();
        set.insert(tag(t));
        for cls in 0..CLASSES {
            assert!(!set.has_class(TagClassId(cls), &reg), "unregistered tag granted a class");
        }
    });
}

#[test]
fn empty_set_has_no_capability() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (reg, _set, _c) = build(s);
        let empty = TagSet::default();
        for cls in 0..CLASSES {
            assert!(!empty.has_class(TagClassId(cls), &reg));
        }
    });
}
