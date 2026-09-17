//! Tags & capability classes — the "binary state" half of the effect model
//! (the "magnitude" half is modifiers, see [`crate::behaviors`]).
//!
//! A [`TagId`] is grouped into one or more capability [`TagClassId`]s at
//! registration. Generic engine systems consult the **class** (`blocks_move`,
//! `untargetable`, …) and never a specific status, so a mod can invent new crowd
//! control by registering a tag into existing classes — with no engine change.
//! A "slow" is *not* a class: it is a `move_speed` modifier that may also carry a
//! `slowed` tag so conditions can detect it.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::attacks::AttackDescriptor;
use crate::ids::{AbilityId, ResourceId, Slot, StatId, TagClassId, TagId, TalentId, UnitId};
use crate::math::Value;
use crate::progression::ProgressionSpec;
use crate::respawn::RespawnSpec;
use crate::talent_tree::TalentTree;
use crate::tasks::QuestSpec;

/// One resource pool a unit carries — a bounded numeric reserve (energy,
/// mana-like, a fury meter, …) that ability costs draw from and that refills over
/// time. Generic: the engine only knows "a pool with a ceiling that regenerates";
/// what the pool *means* is mod convention.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ResourcePool {
    pub id: ResourceId,
    /// Ceiling of the pool, as a `Value` (the wallet is seeded full at spawn).
    pub max: Value,
    /// Passive regeneration per second, as a `Value` (may be zero).
    pub regen: Value,
}

/// A spawnable unit — the generic template the engine instantiates into an
/// entity. Purely declarative: base health, base stat values (e.g. movement
/// speed), the tags it starts with, and its **loadout** (what it can *do*):
/// ability slots, resource pools, and default talent picks. Which units exist
/// and their numbers are mod content; the engine only knows how to spawn this
/// generic shape.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct UnitDescriptor {
    pub id: UnitId,
    /// Base maximum health, as a `Value` expression (usually a constant).
    pub health: Value,
    /// Base stat values applied at spawn (movement speed, mitigation inputs, …).
    pub stats: Vec<(StatId, Value)>,
    /// Tags the unit spawns carrying.
    pub tags: Vec<TagId>,
    /// Ability slots: which ability each slot binds to. Slots are pure mod
    /// convention (see [`Slot`]); the engine casts whatever id a slot resolves to.
    pub abilities: Vec<(Slot, AbilityId)>,
    /// Resource pools the unit spawns with, seeding the cast wallet.
    pub resources: Vec<ResourcePool>,
    /// Talents the unit spawns with already selected (selection stays generic —
    /// a talent targets abilities by slot/tag, never by identity).
    pub talents: Vec<TalentId>,
    /// The tiers a player may *choose* talents from as this unit levels, and
    /// whether a choice can be taken back. `None` is a unit whose talents are
    /// whatever it spawned with — nothing to pick. See [`TalentTree`].
    pub talent_tree: Option<TalentTree>,
    /// How the unit comes back from a death, if it does. `None` means it stays
    /// down once killed — the right answer for a one-shot objective, and the
    /// wrong one for anything a player drives. See [`RespawnSpec`].
    pub respawn: Option<RespawnSpec>,
    /// The unit's place in the XP economy: how it levels, how it earns, and what
    /// killing it is worth. `None` is a unit that never levels and yields nothing
    /// — the honest default for anything that is not a participant. See
    /// [`ProgressionSpec`].
    pub progression: Option<ProgressionSpec>,
    /// How fast the unit pivots toward the direction it is travelling, in radians
    /// per second. `None` takes the engine's default.
    ///
    /// A unit is turned toward where it actually went rather than snapped onto it,
    /// so this is a feel knob with a real cost: too low and a hero lags behind the
    /// player's clicks and visibly swings around after them. A heavy siege engine
    /// that pivots slowly and a scout that spins on the spot are the same
    /// declaration with different numbers.
    ///
    /// A `Value` like every other tunable, so a mod can scale it off a stat rather
    /// than fixing it at authoring time.
    pub turn_rate: Option<Value>,
    /// The tasks this unit carries of its own, with nothing chosen
    /// (stormlight/server#136).
    ///
    /// A task is a **rider, not a kind of thing**: a hero's own baseline objective,
    /// an event's, a map's are the same declaration a talent carries, minus the
    /// choice. Building it as a talent feature would have meant building it twice —
    /// and would have left a mod with no way at all to set an objective that is not
    /// a choice.
    ///
    /// Indexed: rung 0 of the unit's first task is a different thing from rung 0 of
    /// its second, so the *position* in this list is the task's identity for the
    /// latch that remembers what has been paid. Which makes this list append-only in
    /// the same sense a wire tag is — reordering it hands a player a prize they
    /// already have and withholds one they earned.
    ///
    /// Empty for almost every unit, which is the honest default: a creep sets
    /// nobody an objective.
    #[serde(default)]
    pub tasks: Vec<QuestSpec>,
    /// The slots this unit opens to **positional** grants, in the order it wants
    /// them filled (stormlight/server#188).
    ///
    /// A [`GrantTarget::FirstFree`] grant — "give me a button, anywhere" — takes the
    /// first of these nothing is bound in. The list lives on the unit rather than in
    /// the engine so that a slot convention is never engine knowledge: a unit with
    /// an unusual bar declares its own order and receives the same talents
    /// unmodified, and the engine never learns that "1 through 4" means anything.
    ///
    /// Empty is the honest default and means the unit accepts no positional grants
    /// at all — a creep, a ward, a building. A talent that would need one on such a
    /// unit is refused by name at load rather than binding nothing.
    ///
    /// Order is the whole of the determinism: two talents both asking for a button
    /// fill this list front to back, in the fold's own sorted order, so the same
    /// picks always produce the same bar.
    ///
    /// [`GrantTarget::FirstFree`]: crate::talents::GrantTarget::FirstFree
    #[serde(default)]
    pub grant_slots: Vec<Slot>,
    /// The unit's basic attack, if it has one (stormlight/server#89). `None` is a
    /// unit that cannot attack at all — the honest default, because the engine
    /// ships no weapon of its own and a building or a ward should not acquire one
    /// by omission. See [`AttackDescriptor`].
    pub attack: Option<AttackDescriptor>,
}

/// Well-known capability classes the engine's generic systems consult. These ids
/// are engine-reserved; mod-defined classes (if any) are interned above them.
/// (Provisional pending open decision O-2 on engine-well-known vs mod-registered.)
pub mod capability {
    use crate::ids::TagClassId;

    pub const BLOCKS_MOVE: TagClassId = TagClassId(0);
    pub const BLOCKS_CAST: TagClassId = TagClassId(1);
    pub const BLOCKS_ATTACK: TagClassId = TagClassId(2);
    pub const UNTARGETABLE: TagClassId = TagClassId(3);
    pub const INVULNERABLE: TagClassId = TagClassId(4);
    pub const UNKILLABLE: TagClassId = TagClassId(5);
    pub const HIDDEN: TagClassId = TagClassId(6);
    /// A unit a **player drives**. Unlike the classes above — which gate what may
    /// happen *to* a unit mid-match — this one answers a setup question: which of
    /// the mod's units does the match hand a connecting player? Declared the same
    /// way (a mod registers one of its tags into the class and puts that tag on the
    /// unit), so the engine selects a hero without ever naming one.
    pub const PLAYABLE: TagClassId = TagClassId(7);

    /// All well-known classes, in id order.
    pub const WELL_KNOWN: [TagClassId; 8] = [
        BLOCKS_MOVE,
        BLOCKS_CAST,
        BLOCKS_ATTACK,
        UNTARGETABLE,
        INVULNERABLE,
        UNKILLABLE,
        HIDDEN,
        PLAYABLE,
    ];
}

/// Maps each tag to the capability classes it belongs to. Built at adoption from
/// mod registrations; queried read-only during simulation.
#[derive(Clone, Debug, Default)]
pub struct TagRegistry {
    classes: BTreeMap<TagId, Vec<TagClassId>>,
}

impl TagRegistry {
    /// Register `tag` into capability `class`. Idempotent: a (tag, class) pair is
    /// stored at most once, and the class list stays sorted for determinism.
    pub fn register(&mut self, tag: TagId, class: TagClassId) {
        let list = self.classes.entry(tag).or_default();
        if let Err(pos) = list.binary_search(&class) {
            list.insert(pos, class);
        }
    }

    /// The capability classes `tag` belongs to (sorted, deduplicated). Empty for
    /// an unregistered tag.
    #[must_use]
    pub fn classes_of(&self, tag: TagId) -> &[TagClassId] {
        self.classes.get(&tag).map_or(&[], Vec::as_slice)
    }
}

/// The status tags currently active on a unit.
#[derive(Clone, Debug, Default)]
pub struct TagSet {
    tags: BTreeSet<TagId>,
}

impl TagSet {
    /// Add a tag (idempotent).
    pub fn insert(&mut self, tag: TagId) {
        self.tags.insert(tag);
    }

    /// Remove a tag (idempotent).
    pub fn remove(&mut self, tag: TagId) {
        self.tags.remove(&tag);
    }

    /// Whether `tag` is active.
    #[must_use]
    pub fn contains(&self, tag: TagId) -> bool {
        self.tags.contains(&tag)
    }

    /// Whether any active tag is registered into capability `class`. This is the
    /// only question the engine's generic systems ask about status.
    #[must_use]
    pub fn has_class(&self, class: TagClassId, reg: &TagRegistry) -> bool {
        self.tags.iter().any(|t| reg.classes_of(*t).contains(&class))
    }

    /// Every active tag, in ascending id order.
    ///
    /// Ascending because the set is ordered underneath and the engine reads this
    /// where the answer has to be the same on every replay — the tag list carried
    /// on a fired event, which a reaction's filter is matched against
    /// (stormlight/server#186). An unordered iteration would make two runs of the
    /// same match disagree about nothing.
    pub fn iter(&self) -> impl Iterator<Item = TagId> + '_ {
        self.tags.iter().copied()
    }

    /// Number of active tags.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tags.len()
    }

    /// Whether no tags are active.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }
}
