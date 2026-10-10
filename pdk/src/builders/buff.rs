//! Buffs spelled short.

use alloc::vec::Vec;

use stormlight_mod_abi::behaviors::{BuffSpec, ModOp, Modifier, Reapply, StackScope, Stacking};
use stormlight_mod_abi::ids::{BuffId, StatId, TagId};
use stormlight_mod_abi::impacts::Impact;
use stormlight_mod_abi::triggers::Reaction;

use super::value::IntoValue;

/// A buff being spelled — see [`buff`].
#[derive(Clone, Debug, PartialEq)]
#[must_use = "a buff does nothing until the context declares it"]
pub struct BuffBuilder(BuffSpec);

/// A buff that lasts until it is removed, holds one stack — applying it again
/// starts its time over — and ends with its unit's death.
pub fn buff() -> BuffBuilder {
    BuffBuilder(BuffSpec {
        id: BuffId(0),
        duration: None,
        stacking: Stacking { on_reapply: Reapply::RefreshDuration, scope: StackScope::Global },
        max_stacks: 1,
        modifiers: Vec::new(),
        tags: Vec::new(),
        reactions: Vec::new(),
        on_apply: Vec::new(),
        on_expire: Vec::new(),
        on_remove: Vec::new(),
        drop_on_death: true,
    })
}

impl BuffBuilder {
    /// Lasting `seconds`.
    pub fn lasting(mut self, seconds: impl IntoValue) -> Self {
        self.0.duration = Some(seconds.into_value());
        self
    }

    /// Changing `stat` by `op` with `value` while it lasts.
    pub fn modifier(mut self, stat: StatId, op: ModOp, value: impl IntoValue) -> Self {
        self.0.modifiers.push(Modifier { stat, op, value: value.into_value() });
        self
    }

    /// Stacking up to `max` times, as `on_reapply` and `scope` say.
    pub fn stacking(mut self, max: u16, on_reapply: Reapply, scope: StackScope) -> Self {
        self.0.max_stacks = max;
        self.0.stacking = Stacking { on_reapply, scope };
        self
    }

    /// Kept through its unit's death.
    pub fn survives_death(mut self) -> Self {
        self.0.drop_on_death = false;
        self
    }

    /// Tagged `tag`.
    pub fn tag(mut self, tag: TagId) -> Self {
        self.0.tags.push(tag);
        self
    }

    /// Reacting to events as `reaction` says while it lasts.
    pub fn reaction(mut self, reaction: Reaction) -> Self {
        self.0.reactions.push(reaction);
        self
    }

    /// Doing `impact` when it lands.
    pub fn on_apply(mut self, impact: impl Into<Impact>) -> Self {
        self.0.on_apply.push(impact.into());
        self
    }

    /// Doing `impact` when its time runs out.
    pub fn on_expire(mut self, impact: impl Into<Impact>) -> Self {
        self.0.on_expire.push(impact.into());
        self
    }

    /// Doing `impact` when it ends for any reason.
    pub fn on_remove(mut self, impact: impl Into<Impact>) -> Self {
        self.0.on_remove.push(impact.into());
        self
    }
}

impl From<BuffBuilder> for BuffSpec {
    fn from(builder: BuffBuilder) -> Self {
        builder.0
    }
}
