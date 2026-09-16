//! A talent that reaches more than one ability (stormlight/server#150, item 1).
//!
//! A talent used to state exactly one [`AbilitySelector`], and every patch and
//! every rider it declared landed on whatever that one selector matched. So a
//! whole ordinary family — "hitting with this one refreshes *that* one", "a rider
//! here plus a widened area there, one card, one choice" — could not be written
//! down at all.
//!
//! The declaration now has two levels, and the invariants below are about how they
//! compose:
//!
//!   - a talent carries a **list** of selectors, which is what "the same treatment,
//!     on several of your abilities" means. An empty list selects nothing, and is
//!     the honest spelling of a talent whose whole content is unit-level;
//!   - a patch or a rider may carry **its own** selector, which replaces the
//!     talent's list *for that part only*. That is what "different treatment per
//!     ability" means, and it is the half a list cannot express.
//!
//! What the schema itself can pin is the derivation a HUD reads
//! ([`TalentDescriptor::changes`], server#129): the answer to "which button does
//! this change" must follow the selectors that actually govern something, whichever
//! level they came from — and must stay `None` the moment there is more than one
//! honest answer.

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

/// A generated talent: a list of talent-level selectors, and patches/riders that
/// either inherit it or override it with one of their own.
#[derive(Debug, TypeGenerator)]
struct Scenario {
    selectors: Vec<Selects>,
    /// Each patch's own selector, or `None` to inherit the talent's list.
    patches: Vec<Option<Selects>>,
    /// Each rider's own selector, or `None` to inherit the talent's list.
    riders: Vec<Option<Selects>>,
    grants: Vec<u8>,
}

fn patch(selector: Option<AbilitySelector>) -> ParamPatch {
    ParamPatch { param: ParamId(1), op: NumOp::Add, value: Value::Const(1.0), selector }
}

fn rider(selector: Option<AbilitySelector>) -> Rider {
    Rider { hook: AbilityHook::OnHit, effects: Vec::new(), selector }
}

fn talent(s: &Scenario) -> TalentDescriptor {
    TalentDescriptor {
        id: TalentId(1),
        selector: s.selectors.iter().take(4).map(Selects::selector).collect(),
        patches: s
            .patches
            .iter()
            .take(4)
            .map(|sel| patch(sel.as_ref().map(Selects::selector)))
            .collect(),
        riders: s
            .riders
            .iter()
            .take(4)
            .map(|sel| rider(sel.as_ref().map(Selects::selector)))
            .collect(),
        add_reactions: Vec::new(),
        grants: s
            .grants
            .iter()
            .take(3)
            .map(|slot| GrantAbility { slot: Slot(*slot), ability: AbilityId(9) })
            .collect(),
        modifiers: Vec::new(),
        tags: Vec::new(),
        quest: None,
    }
}

/// Every selector that governs at least one patch or rider — a part's own when it
/// declares one, the talent's list otherwise. This is the set `changes()` has to
/// answer from, computed here independently of the implementation.
fn governing(t: &TalentDescriptor) -> Vec<AbilitySelector> {
    let parts = t
        .patches
        .iter()
        .map(|p| p.selector)
        .chain(t.riders.iter().map(|r| r.selector))
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    for part in parts {
        match part {
            Some(sel) => {
                if !out.contains(&sel) {
                    out.push(sel);
                }
            }
            None => {
                for sel in &t.selector {
                    if !out.contains(sel) {
                        out.push(*sel);
                    }
                }
            }
        }
    }
    out
}

#[test]
fn a_part_selector_governs_only_its_own_part() {
    check!().with_type::<Scenario>().for_each(|s| {
        let t = talent(s);

        // A talent with no patches and no riders is governed by nothing at all,
        // whatever its selector list says — the list applies to those two halves
        // and to nothing else.
        if t.patches.is_empty() && t.riders.is_empty() {
            assert!(governing(&t).is_empty(), "a selector governed no part and still spoke");
        }

        // An overridden part never drags the talent's list in with it: a talent
        // whose every part overrides is governed exactly by the overrides.
        let all_overridden = t.patches.iter().all(|p| p.selector.is_some())
            && t.riders.iter().all(|r| r.selector.is_some());
        if all_overridden && !(t.patches.is_empty() && t.riders.is_empty()) {
            for sel in governing(&t) {
                let from_part = t
                    .patches
                    .iter()
                    .filter_map(|p| p.selector)
                    .chain(t.riders.iter().filter_map(|r| r.selector))
                    .any(|s| s == sel);
                assert!(from_part, "an inherited selector leaked past a part's own");
            }
        }
    });
}

#[test]
fn focus_follows_the_selectors_that_govern_something() {
    check!().with_type::<Scenario>().for_each(|s| {
        let t = talent(s);
        let governs = governing(&t);
        let focus = t.changes();

        // More than one governing selector means more than one of the player's
        // buttons, and naming one of them would be confidently wrong. The one
        // exception is the rule that predates this (server#129): a lone grant is
        // about the slot its new ability lands in, and still answers when the
        // selectors cannot.
        if governs.len() > 1 && t.grants.len() != 1 {
            assert_eq!(focus, None, "a talent reaching several abilities claimed one button");
        }

        // A single governing selector that names one button *is* the answer,
        // whether it came from the talent's list or from a part's override.
        if let [only] = governs.as_slice() {
            let expected = match *only {
                AbilitySelector::Slot(slot) => Some(AbilityFocus::Slot(slot)),
                AbilitySelector::Ability(id) => Some(AbilityFocus::Ability(id)),
                // A tag selects a set; neither "any" nor "the unit" is a button.
                _ => None,
            };
            if expected.is_some() {
                assert_eq!(focus, expected, "the one governing selector was not the focus");
            } else if t.grants.len() != 1 {
                // A tag, "any" or "the unit itself" is not a button, and with no
                // lone grant to fall back on the honest answer is nothing.
                assert_eq!(focus, None, "a selector that names no button named one");
            }
        }
    });
}

#[test]
fn an_override_moves_the_focus_off_the_talents_list() {
    check!().with_type::<(u8, u8)>().for_each(|&(a, b)| {
        let (listed, overridden) = (Slot(a), Slot(b));
        let t = TalentDescriptor {
            id: TalentId(1),
            selector: vec![AbilitySelector::Slot(listed)],
            // The talent's only governed part names a different slot, so the
            // talent is about that slot and not the one in its list.
            patches: vec![patch(Some(AbilitySelector::Slot(overridden)))],
            riders: Vec::new(),
            add_reactions: Vec::new(),
            grants: Vec::new(),
            modifiers: Vec::new(),
            tags: Vec::new(),
            quest: None,
        };
        assert_eq!(t.changes(), Some(AbilityFocus::Slot(overridden)));
    });
}

#[test]
fn two_selectors_on_one_talent_have_no_single_focus() {
    check!().with_type::<(u8, u8)>().for_each(|&(a, b)| {
        if a == b {
            return;
        }
        let t = TalentDescriptor {
            id: TalentId(1),
            selector: vec![AbilitySelector::Slot(Slot(a)), AbilitySelector::Slot(Slot(b))],
            patches: vec![patch(None)],
            riders: Vec::new(),
            add_reactions: Vec::new(),
            grants: Vec::new(),
            modifiers: Vec::new(),
            tags: Vec::new(),
            quest: None,
        };
        assert_eq!(t.changes(), None, "a two-slot talent named one of them");
    });
}
