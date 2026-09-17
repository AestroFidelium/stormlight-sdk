//! Where a granted ability lands, and what happens when it cannot land
//! (stormlight/server#188).
//!
//! Handing a unit a new ability used to have exactly one spelling — a slot number
//! and an ability — and exactly one meaning: *bind, if that slot happens to be
//! free*. An occupied slot did nothing and said nothing, so the talent was picked,
//! the tier was spent, and the player's bar was unchanged. That one meaning is the
//! wrong default for two of the three things content actually asks for:
//!
//!   - **this exact slot** — the ultimate case. The number is authored, the slot is
//!     expected to be empty, and an occupied one is a mistake to say out loud;
//!   - **whatever is there** — a kit swapping one of its own abilities for another.
//!     Not expressible at all before this: there was no override, so the only way to
//!     replace an ability was never to have bound it;
//!   - **a button, anywhere** — the grant with no opinion about position. The author
//!     used to hardcode a number and hope, which collides with the first case and
//!     with every other grant on the same unit.
//!
//! [`GrantTarget`] is those three, and [`GrantAbility::resolve`] is the one place
//! any of them is answered — the talent fold, a level's grants, a unit spawning at a
//! declared level, and the loader's own check all ask it, so none of them can drift
//! into a fourth meaning.
//!
//! Where `FirstFree` is allowed to look is the **unit's** business: it offers an
//! ordered list of slots open to grants, and the engine never learns a slot
//! convention of its own. That is what lets a unit with an unusual bar receive the
//! same talent unmodified.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{AbilityId, Slot, TalentId};
use stormlight_mod_abi::talents::{
    AbilityFocus, AbilitySelector, GrantAbility, GrantError, GrantTarget, TalentDescriptor,
};

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Debug, TypeGenerator)]
enum Target {
    Exact(u8),
    Replace(u8),
    FirstFree,
}

impl Target {
    fn target(&self) -> GrantTarget {
        match *self {
            Self::Exact(s) => GrantTarget::Exact(Slot(s)),
            Self::Replace(s) => GrantTarget::Replace(Slot(s)),
            Self::FirstFree => GrantTarget::FirstFree,
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// What the unit already has bound, as `(slot, ability)` pairs.
    bound: Vec<(u8, u16)>,
    /// The slots the unit declares open to grants, in the order it declares them.
    pool: Vec<u8>,
    into: Target,
    ability: u16,
}

impl Scenario {
    fn bound(&self) -> BTreeMap<Slot, AbilityId> {
        self.bound.iter().map(|(s, a)| (Slot(*s), AbilityId(u32::from(*a)))).collect()
    }

    fn pool(&self) -> Vec<Slot> {
        self.pool.iter().map(|s| Slot(*s)).collect()
    }

    fn grant(&self) -> GrantAbility {
        GrantAbility { ability: AbilityId(u32::from(self.ability)), into: self.into.target() }
    }
}

/// The headline: a grant either **binds** or **says why not**. There is no third
/// outcome, and in particular there is no outcome where the ability is missing and
/// nothing was reported — which is the entire defect.
#[test]
fn a_grant_either_binds_or_names_its_refusal() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut bound = s.bound();
        let grant = s.grant();
        match grant.resolve(&bound, &s.pool()) {
            Ok(slot) => {
                bound.insert(slot, grant.ability);
                assert_eq!(
                    bound.get(&slot).copied(),
                    Some(grant.ability),
                    "a grant that resolved to a slot did not put its ability there",
                );
            }
            Err(error) => {
                // A refusal has to name something the author can act on: which slot
                // was in the way, or that the unit offers nowhere to put it.
                match error {
                    GrantError::Occupied { slot, held } => {
                        assert_eq!(
                            s.bound().get(&slot).copied(),
                            Some(held),
                            "the refusal named a slot/ability pair the unit does not hold",
                        );
                    }
                    GrantError::NoGrantSlots => {
                        assert!(s.pool.is_empty(), "refused for an empty pool that is not empty");
                    }
                    GrantError::PoolFull => {
                        assert!(
                            !s.pool.is_empty(),
                            "refused for a full pool while declaring no pool at all",
                        );
                    }
                }
            }
        }
    });
}

/// `Exact` is the authored answer: it binds the slot the author wrote or it binds
/// nothing. It never quietly lands somewhere else, which is what separates it from
/// `FirstFree` and is why an ultimate stays on the key the mod put it on.
#[test]
fn exact_binds_its_own_slot_or_refuses() {
    check!().with_type::<Scenario>().for_each(|s| {
        let bound = s.bound();
        let Target::Exact(slot) = s.into else { return };
        let slot = Slot(slot);
        let grant = GrantAbility {
            ability: AbilityId(u32::from(s.ability)),
            into: GrantTarget::Exact(slot),
        };
        match grant.resolve(&bound, &s.pool()) {
            Ok(landed) => {
                assert_eq!(landed, slot, "an exact grant landed on a slot nobody named");
                assert!(!bound.contains_key(&slot), "an exact grant bound over a live binding");
            }
            Err(error) => {
                assert!(bound.contains_key(&slot), "an exact grant refused a free slot");
                assert!(
                    matches!(error, GrantError::Occupied { slot: named, .. } if named == slot),
                    "an exact grant's refusal named the wrong slot",
                );
            }
        }
    });
}

/// `Replace` is total. It is the shape a kit swapping one of its own abilities for
/// another needs, and there is nothing for it to fail on: an empty slot is simply
/// bound, because refusing "replace nothing" would be pedantry rather than a
/// diagnostic — the author asked for the ability to end up there, and it does.
#[test]
fn replace_always_takes_the_slot_it_names() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut bound = s.bound();
        let Target::Replace(slot) = s.into else { return };
        let slot = Slot(slot);
        let grant = GrantAbility {
            ability: AbilityId(u32::from(s.ability)),
            into: GrantTarget::Replace(slot),
        };
        let landed = grant.resolve(&bound, &s.pool()).expect("a replacement cannot be refused");
        assert_eq!(landed, slot, "a replacement landed on a slot nobody named");
        bound.insert(landed, grant.ability);
        assert_eq!(
            bound.get(&slot).copied(),
            Some(grant.ability),
            "the replacement is not what the slot holds afterwards",
        );
    });
}

/// `FirstFree` only ever looks where the **unit** said it may, and takes the first
/// such slot in the unit's own declaration order. Both halves matter: the first
/// keeps slot conventions out of the engine, and the second is what makes two
/// talents granting into the same bar deterministic rather than a race between
/// whichever the fold happened to reach first.
#[test]
fn first_free_takes_the_units_first_offered_free_slot() {
    check!().with_type::<Scenario>().for_each(|s| {
        let bound = s.bound();
        let pool = s.pool();
        if !matches!(s.into, Target::FirstFree) {
            return;
        }
        let grant =
            GrantAbility { ability: AbilityId(u32::from(s.ability)), into: GrantTarget::FirstFree };
        let first_free = pool.iter().copied().find(|slot| !bound.contains_key(slot));
        match grant.resolve(&bound, &pool) {
            Ok(landed) => {
                assert_eq!(
                    Some(landed),
                    first_free,
                    "a positional grant skipped the unit's first free offered slot",
                );
                assert!(
                    pool.contains(&landed),
                    "a positional grant landed outside the unit's pool"
                );
                assert!(
                    !bound.contains_key(&landed),
                    "a positional grant landed on a live binding"
                );
            }
            Err(error) => {
                assert!(
                    first_free.is_none(),
                    "a positional grant refused with a free slot to take"
                );
                let expected =
                    if pool.is_empty() { GrantError::NoGrantSlots } else { GrantError::PoolFull };
                assert_eq!(error, expected, "a positional refusal gave the wrong reason");
            }
        }
    });
}

/// Two grants never silently share a landing. Resolving a grant, binding it, and
/// asking again is the shape the fold actually runs — one talent after another onto
/// the same growing loadout — and the second answer must never be the first one's
/// slot, because that is the overwrite nobody asked for.
///
/// `Replace` is the exception that proves it: it is *defined* as taking the slot
/// whatever is there, so a second replacement of the same slot is the author saying
/// so twice, not a collision.
#[test]
fn a_second_grant_never_lands_on_the_first() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut bound = s.bound();
        let pool = s.pool();
        let grant = s.grant();
        let Ok(first) = grant.resolve(&bound, &pool) else { return };
        bound.insert(first, grant.ability);

        match grant.resolve(&bound, &pool) {
            Ok(second) if matches!(s.into, Target::Replace(_)) => {
                assert_eq!(second, first, "a replacement moved on being asked twice");
            }
            Ok(second) => {
                assert_ne!(second, first, "a second grant landed on the slot the first took");
            }
            // Refused the second time round: the slot it wanted is now taken, which
            // is exactly the report the silent `or_insert` never made.
            Err(_) => {}
        }
    });
}

/// A talent whose whole content is one grant is *about* the button it hands over,
/// and which of the two focus shapes says so depends on what the grant knows
/// (stormlight/server#129 meeting server#188).
///
/// An authored slot is the position outright — the ultimate is on the key the mod
/// put it on, and a HUD can draw the mark against that key with no loadout in hand.
/// A positional grant has no slot until a unit resolves it, so naming one would be
/// a label that is confidently wrong; it answers with the **ability** instead, which
/// is looked up in whatever the caster is carrying — the shape [`AbilityFocus`]
/// already had for exactly this reason.
#[test]
fn a_lone_grant_is_about_the_button_it_can_name() {
    check!().with_type::<Scenario>().for_each(|s| {
        let grant = s.grant();
        let talent = TalentDescriptor {
            id: TalentId(1),
            // Selectors govern patches and riders only, and this talent has
            // neither — so nothing here can speak for the focus.
            selector: vec![AbilitySelector::Any],
            patches: Vec::new(),
            riders: Vec::new(),
            add_reactions: Vec::new(),
            grants: vec![grant],
            modifiers: Vec::new(),
            tags: Vec::new(),
            quest: None,
        };
        let expected = match s.into {
            Target::Exact(slot) | Target::Replace(slot) => AbilityFocus::Slot(Slot(slot)),
            Target::FirstFree => AbilityFocus::Ability(grant.ability),
        };
        assert_eq!(
            talent.changes(),
            Some(expected),
            "a lone grant pointed the player at the wrong button",
        );
    });
}
