//! Which of a player's own buttons a talent lands on (stormlight/server#129).
//!
//! A talent's row says what it is called and what it does, and — until this — said
//! nothing about *which key it changes*. A tier whose talents patch four different
//! slots therefore reads as a list of unrelated sentences, and a player choosing
//! between "the bolt comes back sooner" and "the ward comes back sooner" has to
//! already know which button each of those is.
//!
//! The answer is **derived, not declared**. A talent already says what it applies
//! to — [`AbilitySelector`] — and already says what it hands over —
//! [`GrantAbility`]. A second field for a HUD to print would be a mod's words free
//! to disagree with the mod's mechanism, which is the worst possible label: wrong
//! rather than missing.
//!
//! [`TalentDescriptor::changes`] is that derivation, and what it has to get right is
//! *which* of the two halves is speaking:
//!
//!   - the **selector** governs patches and riders, and nothing else. A talent whose
//!     only content is a unit-level reaction or a grant may carry any selector at
//!     all — that selector applies to nothing, so reading a key off it would point
//!     the player at a button the talent never touches;
//!   - a **grant** is about the slot the new ability appears in, which is exactly
//!     the case derivation was supposed not to cover: a talent whose whole point is
//!     a new button on `E` says so, in the mechanism, already;
//!   - and the honest answer is often **nothing**. A talent selecting by tag, or
//!     granting two abilities, is about more than one of the player's buttons, and a
//!     plate that picked one of them would be a label that is confidently wrong.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::common::NumOp;
use stormlight_mod_abi::ids::{AbilityId, ParamId, Slot, TagId, TalentId};
use stormlight_mod_abi::math::Value;
use stormlight_mod_abi::talents::{
    AbilityFocus, AbilityHook, AbilitySelector, GrantAbility, ParamPatch, Rider, TalentDescriptor,
};

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Debug, TypeGenerator)]
enum Selects {
    Slot(u8),
    Tag(u16),
    Ability(u16),
    Any,
    SelfUnit,
}

impl Selects {
    fn selector(&self) -> AbilitySelector {
        match *self {
            Self::Slot(s) => AbilitySelector::Slot(Slot(s)),
            Self::Tag(t) => AbilitySelector::Tag(TagId(t)),
            Self::Ability(a) => AbilitySelector::Ability(AbilityId(u32::from(a))),
            Self::Any => AbilitySelector::Any,
            Self::SelfUnit => AbilitySelector::SelfUnit,
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    selects: Selects,
    /// Whether it patches one of the selected ability's parameters.
    patches: bool,
    /// Whether it appends effects into one of the selected ability's hooks.
    rides: bool,
    /// The slots it grants a new ability into — none, one, or two.
    grants: Vec<u8>,
}

impl Scenario {
    fn talent(&self) -> TalentDescriptor {
        TalentDescriptor {
            id: TalentId(7),
            selector: self.selects.selector(),
            patches: if self.patches {
                vec![ParamPatch {
                    param: ParamId(1),
                    op: NumOp::Add,
                    value: Value::Const(1.0),
                }]
            } else {
                Vec::new()
            },
            riders: if self.rides {
                vec![Rider { hook: AbilityHook::OnCast, effects: Vec::new() }]
            } else {
                Vec::new()
            },
            add_reactions: Vec::new(),
            grants: self
                .grants
                .iter()
                .take(2)
                .map(|slot| GrantAbility { slot: Slot(*slot), ability: AbilityId(3) })
                .collect(),
            modifiers: Vec::new(),
            tags: Vec::new(),
            quest: None,
        }
    }

    /// Whether anything this talent carries is actually governed by its selector.
    fn touches_a_selection(&self) -> bool {
        self.patches || self.rides
    }
}

/// The selector is only an answer when the talent has something for it to select
/// *for*. This is the property that stops a granting talent's decorative
/// `Any` — or, worse, a stale `Slot(0)` — from labelling a button it never touches.
#[test]
fn a_selector_only_speaks_for_what_it_governs() {
    check!().with_type::<Scenario>().for_each(|s| {
        let changes = s.talent().changes();
        if s.touches_a_selection() {
            let expected = match s.selects.selector() {
                AbilitySelector::Slot(slot) => Some(AbilityFocus::Slot(slot)),
                AbilitySelector::Ability(id) => Some(AbilityFocus::Ability(id)),
                // A tag selects a set, and "any" and "the unit itself" select no
                // ability at all — none of the three is one button.
                _ => None,
            };
            // A talent that patches by tag and also grants exactly one ability is
            // still about that one new button, so the grant is the fallback rather
            // than the alternative.
            let expected = expected.or_else(|| lone_grant(s));
            assert_eq!(changes, expected, "a talent's selector was read wrongly");
        } else {
            assert_eq!(
                changes,
                lone_grant(s),
                "a talent whose selector governs nothing was read off its selector",
            );
        }
    });
}

/// The grant case the issue expected to need a declared field for: a talent whose
/// whole point is a new button already says which slot that button is.
fn lone_grant(s: &Scenario) -> Option<AbilityFocus> {
    match s.grants.iter().take(2).collect::<Vec<_>>().as_slice() {
        [only] => Some(AbilityFocus::Slot(Slot(**only))),
        _ => None,
    }
}

/// Two grants is two buttons, and a plate can only name one. Nothing is the honest
/// answer; a guess would be a label that is confidently wrong, which is worse than
/// the absence this feature set out to fix.
#[test]
fn a_talent_about_more_than_one_button_names_none_of_them() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut many = Scenario {
            selects: Selects::Tag(1),
            patches: s.patches,
            rides: s.rides,
            grants: vec![0, 1],
        };
        assert_eq!(many.talent().changes(), None, "a talent granting two buttons named one");
        // And the same talent with one of them removed does have an answer, so the
        // rule above is about ambiguity rather than about grants being ignored.
        many.grants = vec![2];
        assert_eq!(
            many.talent().changes(),
            Some(AbilityFocus::Slot(Slot(2))),
            "a talent granting exactly one button named none",
        );
    });
}

/// Derivation is a *reading* of a descriptor, so it must not depend on anything the
/// descriptor does not say. Stated as: adding content the selector does not govern —
/// a unit-level reaction, a tag, an inert modifier — never changes the answer.
#[test]
fn unit_level_content_does_not_change_which_button_a_talent_is_about() {
    check!().with_type::<Scenario>().for_each(|s| {
        let plain = s.talent();
        let mut noisy = plain.clone();
        noisy.tags = vec![TagId(9)];
        assert_eq!(
            plain.changes(),
            noisy.changes(),
            "content the selector does not govern moved the button a talent points at",
        );
    });
}
