//! `TalentDescriptor` — a fully generic talent: a **selector** (by slot or tag,
//! never unit identity) plus generic patches/riders/reactions/grants. This is
//! what delivers "any talent on any ability, any ability on any unit".

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

use serde::{Deserialize, Serialize};

use crate::behaviors::Modifier;
use crate::common::NumOp;
use crate::ids::{AbilityId, Handle, ParamId, Slot, TagId, TalentId};
use crate::impacts::Impact;
use crate::math::Value;
use crate::tasks::QuestSpec;
use crate::triggers::Reaction;

/// Which abilities a talent applies to — keyed on slot/tag/id, never identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum AbilitySelector {
    Slot(Slot),
    Tag(TagId),
    Ability(AbilityId),
    Any,
    SelfUnit,
}

/// A generic parameter tweak: the whole "+radius / −cooldown / +charge /
/// +targets" class is one patch.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ParamPatch {
    pub param: ParamId,
    pub op: NumOp,
    pub value: Value,
    /// Which abilities *this patch* lands on, overriding the talent's own list
    /// (stormlight/server#150). `None` — the ordinary case — inherits it.
    ///
    /// This is the half a list of talent-level selectors cannot express: "hitting
    /// with this one widens *that* one" is a rider on one ability and a patch on
    /// another, and it is one choice on one card, not two talents.
    #[serde(default)]
    pub selector: Option<AbilitySelector>,
}

/// Which of a selected ability's own hooks a rider appends into.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum AbilityHook {
    OnCastStart,
    OnCast,
    OnHit,
}

/// Extra effects appended into a selected ability's payload — they travel with
/// its missiles and respect `value_scale` (contrast with unit-level reactions).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Rider {
    pub hook: AbilityHook,
    pub effects: Vec<Impact>,
    /// Which abilities *this rider* hangs off, overriding the talent's own list
    /// (stormlight/server#150). `None` — the ordinary case — inherits it.
    #[serde(default)]
    pub selector: Option<AbilitySelector>,
}

/// Where a granted ability binds (stormlight/server#188).
///
/// Three spellings because handing a unit an ability is three different intents,
/// and they used to share one — "bind if that slot happens to be free", which is
/// the right answer for none of them and silently wrong for two.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum GrantTarget {
    /// **This slot, which is expected to be empty.** The authored position — an
    /// ultimate on the key the mod put it on. An occupied slot is an authoring
    /// mistake and is refused by name rather than absorbed.
    Exact(Slot),
    /// **This slot, whatever is in it.** The kit that swaps one of its own
    /// abilities for another. Always binds: an empty slot is simply bound, because
    /// refusing "replace nothing" would be pedantry rather than a diagnostic.
    ///
    /// What the slot already carries in the way of talent patches and riders stays
    /// with the *slot*, not with the ability that left it — [`AbilitySelector::Slot`]
    /// keys those, and the replacement inherits every one of them. A selector
    /// naming the displaced ability by id stops matching, which is the same rule
    /// read from the other end.
    ///
    /// The slot's **live** state goes the other way, because it belongs to the
    /// ability rather than to the key: the arriving ability is ready if it has never
    /// been cast, the departing one keeps recovering (and keeps counting down) while
    /// it is off the bar, and a cast already in flight completes as the ability that
    /// started it — its program was frozen when it began. Charges are a single
    /// per-unit balance and are not a slot's to hand over at all. That is the rule a
    /// stance change needs, and the only one a *pure* fold can implement: the talent
    /// resolution re-runs on every change to the selection or the bindings, so
    /// anything it did to live state would happen again on each rebuild.
    Replace(Slot),
    /// **A button, anywhere.** The grant with no opinion about position: it takes
    /// the first slot the *unit* offers to grants that nothing is bound in.
    ///
    /// Where it may look is the unit's declaration
    /// ([`UnitDescriptor::grant_slots`]), never the engine's idea of a bar. That is
    /// what lets a unit with an unusual layout receive the same talent unmodified,
    /// and what stops two positional grants on one unit from fighting over a number
    /// the author had to guess.
    ///
    /// [`UnitDescriptor::grant_slots`]: crate::units::UnitDescriptor::grant_slots
    FirstFree,
}

/// Why a grant could not bind (stormlight/server#188).
///
/// Every variant names the thing the author has to change. Silence was the defect:
/// a grant that bound nothing used to leave the talent picked, the tier spent, and
/// the player's bar unchanged, with nothing written down anywhere.
///
/// Not serializable, like [`QuestError`](crate::tasks::QuestError): this is a
/// verdict on a declaration, reached on whichever side is holding it, and never
/// something that crosses the wire.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum GrantError {
    /// An [`Exact`](GrantTarget::Exact) grant named a slot that is already bound.
    /// `held` is what is in the way, so the report is actionable without the reader
    /// having to reconstruct the loadout.
    Occupied { slot: Slot, held: AbilityId },
    /// A [`FirstFree`](GrantTarget::FirstFree) grant on a unit that offers no slots
    /// to grants at all. Knowable before the match starts, and refused there.
    NoGrantSlots,
    /// A [`FirstFree`](GrantTarget::FirstFree) grant where every slot the unit
    /// offers is already taken — the unit ran out of bar.
    PoolFull,
}

impl fmt::Display for GrantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Occupied { slot, held } => {
                write!(f, "slot {} is already bound to ability {}", slot.0, held.raw())
            }
            Self::NoGrantSlots => f.write_str("the unit offers no slots to grants"),
            Self::PoolFull => f.write_str("every slot the unit offers to grants is taken"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for GrantError {}

/// Grant an ability into a slot — see [`GrantTarget`] for which one.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct GrantAbility {
    pub ability: AbilityId,
    pub into: GrantTarget,
}

impl GrantAbility {
    /// Which slot this grant binds into, given what the unit currently has `bound`
    /// and the ordered slots it offers to grants.
    ///
    /// The one place any of the three targets is answered. The talent fold, a
    /// level's grants, a unit spawning at a declared level and the loader's own
    /// check all come through here, so none of them can drift into a fourth
    /// meaning — and a refusal is a value the caller has to do something with
    /// rather than a branch it can forget to write.
    ///
    /// `bound` is the loadout **as it stands**, which is what makes a sequence of
    /// grants fold: each one sees what the ones before it took.
    pub fn resolve(
        &self,
        bound: &BTreeMap<Slot, AbilityId>,
        pool: &[Slot],
    ) -> Result<Slot, GrantError> {
        match self.into {
            GrantTarget::Exact(slot) => match bound.get(&slot) {
                Some(&held) => Err(GrantError::Occupied { slot, held }),
                None => Ok(slot),
            },
            GrantTarget::Replace(slot) => Ok(slot),
            GrantTarget::FirstFree => {
                if pool.is_empty() {
                    return Err(GrantError::NoGrantSlots);
                }
                pool.iter()
                    .copied()
                    .find(|slot| !bound.contains_key(slot))
                    .ok_or(GrantError::PoolFull)
            }
        }
    }
}

/// Which single ability of the caster's a talent is *about*, when there is one
/// (stormlight/server#129).
///
/// Derived from what the talent already declares rather than stated beside it. A
/// talent says what it applies to and what it hands over; a second field for a HUD
/// to print would be a mod's words free to disagree with the mod's mechanism, which
/// is the worst kind of label — wrong rather than missing.
///
/// Two shapes because the declaration has two, and the difference matters to
/// whoever is drawing it: a slot is a position on the caster's bar and is the answer
/// straight away, while an ability has to be looked up in whatever loadout the
/// caster is carrying right now — the same ability may sit under a different key on
/// a different unit, or under none at all.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum AbilityFocus {
    /// The ability bound in this slot, whichever one that is.
    Slot(Slot),
    /// This ability, wherever the caster happens to be carrying it.
    Ability(AbilityId),
}

/// A generic talent.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TalentDescriptor {
    pub id: TalentId,
    /// The abilities this talent's patches and riders apply to, unless a part
    /// names its own (stormlight/server#150).
    ///
    /// A **list** because "the same treatment, on several of your abilities" is an
    /// ordinary talent and used to take one talent per ability — and a tier cannot
    /// offer three cards where the design has one choice. An empty list selects
    /// nothing, which is the honest spelling of a talent whose whole content is
    /// unit-level (a reaction, a modifier, a grant): the old way of saying that was
    /// a selector that quietly matched no slot.
    pub selector: Vec<AbilitySelector>,
    pub patches: Vec<ParamPatch>,
    pub riders: Vec<Rider>,
    pub add_reactions: Vec<Reaction>,
    pub grants: Vec<GrantAbility>,
    pub modifiers: Vec<Modifier>,
    pub tags: Vec<TagId>,
    /// The task this talent sets, if it sets one (stormlight/server#132).
    ///
    /// Declaration only — see [`QuestSpec`]. A talent that declares none is an
    /// ordinary talent and is untouched by any of it, which is almost all of them.
    ///
    /// A talent carrying one is the *ordinary* case of a task rather than the only
    /// one: a unit declares its own the same way ([`UnitDescriptor::tasks`]), and
    /// neither is a different kind of task to the simulation.
    ///
    /// [`UnitDescriptor::tasks`]: crate::units::UnitDescriptor::tasks
    #[serde(default)]
    pub quest: Option<QuestSpec>,
}

impl TalentDescriptor {
    /// Which one of the caster's abilities this talent changes, if that question
    /// has a single answer (stormlight/server#129).
    ///
    /// Read off two halves of the declaration, and which of them speaks is the whole
    /// of the rule:
    ///
    ///   - the **selectors** govern [`patches`](Self::patches) and
    ///     [`riders`](Self::riders), and nothing else. A talent whose only content
    ///     is a unit-level reaction or a grant may carry any selector at all — that
    ///     selector applies to nothing, so a key read off it would point the player
    ///     at a button the talent never touches. Each part answers with **its own**
    ///     selector where it declares one and with the talent's list otherwise
    ///     (server#150), so the set to read from is the union of what actually
    ///     governs something;
    ///   - a **lone grant** is about the button its new ability appears on, which
    ///     covers the case a selector cannot: a talent whose whole point is a new
    ///     button. Which of the two focus shapes answers depends on what the grant
    ///     knows: an authored slot ([`GrantTarget::Exact`] / [`GrantTarget::Replace`])
    ///     *is* the position, while a positional grant ([`GrantTarget::FirstFree`])
    ///     does not know its slot until a unit resolves it — so it answers with the
    ///     ability, which is looked up in whatever loadout the caster is carrying.
    ///
    /// `None` when the honest answer is nothing. A talent selecting by tag, one
    /// selecting nothing at all, one reaching two abilities, or one granting two is
    /// about more than one of the player's buttons, and naming one of them would be
    /// a label that is confidently wrong.
    #[must_use]
    pub fn changes(&self) -> Option<AbilityFocus> {
        let selected = match self.governing_selectors().as_slice() {
            [only] => match only {
                AbilitySelector::Slot(slot) => Some(AbilityFocus::Slot(*slot)),
                AbilitySelector::Ability(id) => Some(AbilityFocus::Ability(*id)),
                // A tag selects a set; "any" and "the unit itself" select no ability
                // at all. None of the three is one button.
                AbilitySelector::Tag(_) | AbilitySelector::Any | AbilitySelector::SelfUnit => None,
            },
            // Nothing governed, or several buttons governed: neither is one answer.
            _ => None,
        };
        selected.or(match self.grants.as_slice() {
            [only] => Some(match only.into {
                GrantTarget::Exact(slot) | GrantTarget::Replace(slot) => AbilityFocus::Slot(slot),
                GrantTarget::FirstFree => AbilityFocus::Ability(only.ability),
            }),
            _ => None,
        })
    }

    /// Every selector that governs at least one patch or rider, de-duplicated and
    /// in declaration order (stormlight/server#150).
    ///
    /// A part's own selector where it declares one, the talent's list where it does
    /// not — so a talent that declares selectors and no parts governs *nothing*,
    /// and one whose every part overrides drags none of its list along.
    #[must_use]
    pub fn governing_selectors(&self) -> Vec<AbilitySelector> {
        let mut out: Vec<AbilitySelector> = Vec::new();
        let parts =
            self.patches.iter().map(|p| p.selector).chain(self.riders.iter().map(|r| r.selector));
        for part in parts {
            match part {
                Some(sel) => {
                    if !out.contains(&sel) {
                        out.push(sel);
                    }
                }
                None => {
                    for sel in &self.selector {
                        if !out.contains(sel) {
                            out.push(*sel);
                        }
                    }
                }
            }
        }
        out
    }

    /// The abilities one patch applies to: its own selector, or the talent's list.
    #[must_use]
    pub fn patch_selectors<'a>(&'a self, patch: &'a ParamPatch) -> &'a [AbilitySelector] {
        Self::part_selectors(patch.selector.as_ref(), &self.selector)
    }

    /// The abilities one rider hangs off: its own selector, or the talent's list.
    #[must_use]
    pub fn rider_selectors<'a>(&'a self, rider: &'a Rider) -> &'a [AbilitySelector] {
        Self::part_selectors(rider.selector.as_ref(), &self.selector)
    }

    /// One part's effective selector list — its override as a one-element slice, or
    /// the talent's own list. The single place the inheritance rule is written.
    fn part_selectors<'a>(
        own: Option<&'a AbilitySelector>,
        talent: &'a [AbilitySelector],
    ) -> &'a [AbilitySelector] {
        match own {
            Some(sel) => core::slice::from_ref(sel),
            None => talent,
        }
    }
}
