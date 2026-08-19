//! `TalentTree` — the structure a unit's talent choices are made *within*
//! (stormlight/server#63).
//!
//! A [`TalentDescriptor`](crate::talents::TalentDescriptor) says what a talent
//! *does*, to any ability on any unit. It deliberately says nothing about who may
//! take it or when, because a talent is not owned by anyone. What is owned is the
//! **choice**: a unit declares tiers, each tier opens at a level and offers a
//! handful of mutually-exclusive options, and a player picks one per tier as they
//! level. That is the tree, and it lives on the unit for the same reason the
//! loadout does — it is that unit's, and two units may offer the same talent from
//! different tiers.
//!
//! Everything here is content. How many tiers a hero has, at what levels, what each
//! one offers, and whether a choice can be taken back are balance decisions; an
//! engine that knew any of them would be an engine you could not re-balance. What
//! the engine enforces is only the generic rule the structure implies: **one pick
//! per unlocked tier, from that tier's own options**.
//!
//! # Unlocking is derived from the level, never latched
//!
//! A tier declares the level that opens it, and "is it open" is asked of the unit's
//! current level every time it matters. Nothing records that a tier was *once*
//! unlocked. This is the same shape as death being derived from vitals, and it buys
//! the same thing: a rewind past the threshold re-locks the tier, with no separate
//! history to reconcile, and a mod that re-balances a tier's level moves every unit
//! at once because no unit is storing the answer.
//!
//! # A tier index is a `u8`
//!
//! Tiers are addressed by their position in [`TalentTree::tiers`], and that index is
//! what a player's pick names on the wire. It is a single byte, so a tree longer
//! than [`MAX_TIERS`] has a tail no pick can reach; the accessors here report only
//! the reachable prefix rather than pretending otherwise.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::ids::TalentId;

/// How many tiers a tree can address. A pick names its tier as a `u8`, so this is
/// the whole reachable space — far past the handful of tiers a hero realistically
/// declares.
pub const MAX_TIERS: usize = u8::MAX as usize + 1;

/// Whether a choice already made may be changed.
///
/// Declared rather than assumed, because both answers are real designs: a pick that
/// is final for the match is what makes a talent choice a commitment, and a pick
/// that can be swapped freely is what makes a practice context useful. A mod says
/// which it wants; the engine has no opinion.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum RepickPolicy {
    /// A tier's choice is made once and stands for the rest of the unit's life.
    /// The default, because it is the commitment the genre is built on — and
    /// because a mod that forgot to declare a policy should not silently hand out
    /// free respecs.
    #[default]
    Locked,
    /// A tier's choice may be replaced by another option from the same tier, any
    /// number of times.
    Free,
}

/// One tier: the level that opens it, and the options it offers.
///
/// The options are **mutually exclusive** — taking one is what closes the tier —
/// which is the only relationship the engine reads from this list. A tier that
/// offers nothing is legal and simply has nothing to choose; a tier whose level is
/// `0` is open from the moment a unit exists.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct TalentTier {
    /// The unit level at which this tier becomes choosable.
    pub level: u8,
    /// The talents this tier offers, of which a unit may hold at most one.
    pub options: Vec<TalentId>,
    /// Which of them this tree suggests, by its index in `options`
    /// (stormlight/server#127).
    ///
    /// A **suggestion and nothing else**: the engine never applies it, never
    /// defaults to it, and never refuses anything because of it. It exists so a
    /// player meeting a tree for the first time has somewhere to start, and so a
    /// mod can say so without shipping a second interface to say it in.
    ///
    /// On the **tier**, not on the talent, for the same reason the unlock level is:
    /// a talent may be offered by several trees and be right in one of them and
    /// wrong in another. `None` is a tier with no opinion, which is the honest
    /// default and what every tree authored before this says.
    ///
    /// An index past the end of `options` suggests nothing — see
    /// [`recommends`](Self::recommends). Nothing validates a mod's declaration, and
    /// one stable answer is worth more than a diagnostic nobody reads.
    #[serde(default)]
    pub recommended: Option<u8>,
}

impl TalentTier {
    /// Whether a unit standing at `level` may choose from this tier.
    #[must_use]
    pub fn unlocked(&self, level: u8) -> bool {
        level >= self.level
    }

    /// Whether `talent` is one of this tier's options.
    #[must_use]
    pub fn offers(&self, talent: TalentId) -> bool {
        self.options.contains(&talent)
    }

    /// Whether this tier suggests the option at `index`.
    ///
    /// False for a tier with no opinion, and false for an index this tier does not
    /// reach — including the case where the *suggestion itself* is out of range,
    /// which suggests nothing rather than suggesting whatever happens to sit at that
    /// index in some other tier.
    #[must_use]
    pub fn recommends(&self, index: u8) -> bool {
        self.recommended == Some(index) && usize::from(index) < self.options.len()
    }
}

/// A unit's whole talent tree.
///
/// The default is an empty tree under [`RepickPolicy::Locked`] — a unit with
/// nothing to choose, which is the honest shape for a creep, a projectile, or
/// anything else that is not a character.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct TalentTree {
    /// The tiers, in the order a pick addresses them (see the module docs).
    pub tiers: Vec<TalentTier>,
    /// Whether a choice already made may be replaced.
    pub repick: RepickPolicy,
}

impl TalentTree {
    /// How many tiers a pick can actually name — the declared count, capped at the
    /// [`MAX_TIERS`] a `u8` index can reach.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tiers.len().min(MAX_TIERS)
    }

    /// Whether there is nothing to choose.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The tier at `index`, or `None` when no reachable tier sits there.
    #[must_use]
    pub fn tier(&self, index: u8) -> Option<&TalentTier> {
        self.tiers.get(usize::from(index))
    }

    /// Every reachable tier, paired with the index a pick names it by.
    pub fn reachable(&self) -> impl Iterator<Item = (u8, &TalentTier)> {
        self.tiers.iter().take(MAX_TIERS).enumerate().map(|(i, tier)| (i as u8, tier))
    }

    /// Whether the tier at `index` is choosable by a unit standing at `level`.
    /// An index naming no tier is never unlocked — there is nothing there to open.
    #[must_use]
    pub fn unlocked(&self, index: u8, level: u8) -> bool {
        self.tier(index).is_some_and(|tier| tier.unlocked(level))
    }

    /// The index of the tier `talent` belongs to, or `None` if this tree never
    /// offers it.
    ///
    /// The **first** tier that offers it. A talent should appear in exactly one
    /// tier of one tree, but nothing validates a mod's declaration, and one stable
    /// answer is worth more than a diagnostic nobody reads: a duplicate resolves
    /// the same way on every machine rather than by iteration luck.
    #[must_use]
    pub fn tier_of(&self, talent: TalentId) -> Option<u8> {
        self.reachable().find(|(_, tier)| tier.offers(talent)).map(|(index, _)| index)
    }
}
