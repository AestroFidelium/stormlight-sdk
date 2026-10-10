//! Units spelled short.

use alloc::vec::Vec;

use stormlight_mod_abi::attacks::AttackDescriptor;
use stormlight_mod_abi::ids::{AbilityId, ResourceId, Slot, StatId, TagId, TalentId, UnitId};
use stormlight_mod_abi::progression::ProgressionSpec;
use stormlight_mod_abi::respawn::RespawnSpec;
use stormlight_mod_abi::talent_tree::TalentTree;
use stormlight_mod_abi::tasks::QuestSpec;
use stormlight_mod_abi::units::{ResourcePool, UnitDescriptor};

use super::value::IntoValue;

/// A unit being spelled — see [`unit`].
#[derive(Clone, Debug, PartialEq)]
#[must_use = "a unit does nothing until the context declares it"]
pub struct UnitSpec(UnitDescriptor);

/// A unit with `health`, and nothing else yet: no stats, tags, abilities or pools,
/// no attack — a building or a ward should not acquire one by omission — and none
/// of the progression a hero carries.
pub fn unit(health: impl IntoValue) -> UnitSpec {
    UnitSpec(UnitDescriptor {
        id: UnitId(0),
        health: health.into_value(),
        stats: Vec::new(),
        tags: Vec::new(),
        abilities: Vec::new(),
        resources: Vec::new(),
        talents: Vec::new(),
        talent_tree: None,
        respawn: None,
        progression: None,
        turn_rate: None,
        tasks: Vec::new(),
        grant_slots: Vec::new(),
        attack: None,
    })
}

impl UnitSpec {
    /// With `stat` at `value`.
    pub fn stat(mut self, stat: StatId, value: impl IntoValue) -> Self {
        self.0.stats.push((stat, value.into_value()));
        self
    }

    /// Tagged `tag`.
    pub fn tag(mut self, tag: TagId) -> Self {
        self.0.tags.push(tag);
        self
    }

    /// With `ability` in `slot`.
    pub fn ability(mut self, slot: Slot, ability: AbilityId) -> Self {
        self.0.abilities.push((slot, ability));
        self
    }

    /// With a pool of `resource`, `max` deep and refilling `regen` a second.
    pub fn pool(
        mut self,
        resource: ResourceId,
        max: impl IntoValue,
        regen: impl IntoValue,
    ) -> Self {
        self.0.resources.push(ResourcePool {
            id: resource,
            max: max.into_value(),
            regen: regen.into_value(),
        });
        self
    }

    /// Attacking as `attack` says.
    pub fn attack(mut self, attack: AttackDescriptor) -> Self {
        self.0.attack = Some(attack);
        self
    }

    /// Turning at `radians_per_second`.
    pub fn turn_rate(mut self, radians_per_second: impl IntoValue) -> Self {
        self.0.turn_rate = Some(radians_per_second.into_value());
        self
    }

    /// Holding `talent` from the start.
    pub fn talent(mut self, talent: TalentId) -> Self {
        self.0.talents.push(talent);
        self
    }

    /// Choosing its talents from `tree`.
    pub fn talent_tree(mut self, tree: TalentTree) -> Self {
        self.0.talent_tree = Some(tree);
        self
    }

    /// Coming back as `respawn` says when it dies.
    pub fn respawn(mut self, respawn: RespawnSpec) -> Self {
        self.0.respawn = Some(respawn);
        self
    }

    /// Gaining levels as `progression` says.
    pub fn progression(mut self, progression: ProgressionSpec) -> Self {
        self.0.progression = Some(progression);
        self
    }

    /// Carrying `quest`.
    pub fn task(mut self, quest: QuestSpec) -> Self {
        self.0.tasks.push(quest);
        self
    }

    /// With `slot` free for a granted ability.
    pub fn grant_slot(mut self, slot: Slot) -> Self {
        self.0.grant_slots.push(slot);
        self
    }
}

impl From<UnitSpec> for UnitDescriptor {
    fn from(spec: UnitSpec) -> Self {
        spec.0
    }
}
