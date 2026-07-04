//! The effect ISA — ~4 control combinators over ~14 leaf operations. This is the
//! whole moddable vocabulary. It does **not** grow with content: new content is a
//! new *composition* of these leaves plus new `Value`/`Stat`/`Tag`/`Param` data.
//! A new leaf is allowed only for a genuinely new structural verb the engine
//! cannot compose (governance rule, §3).
//!
//! Ids here are interned handles (the runtime form). Authoring uses stable
//! strings converted to handles at adoption (§6.3); the tree shape is identical.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::common::{Direction, ImpactTarget, NumOp, Point3, TargetFilter};
use crate::conditions::Condition;
use crate::ids::{
    BuffId, DamageTypeId, EventId, HandlerId, ResourceId, Slot, StackId, TagClassId, TagId,
};
use crate::math::Value;
use crate::missiles::BodyDescriptor;

/// The geometric target set a `Retarget` resolves; params live inside the shape.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum TargetShape {
    SelfOnly,
    Circle { radius: Value },
    Cone { radius: Value, angle: Value },
    Chain { jumps: Value, range: Value },
    Line { length: Value, width: Value },
    AllAllies,
    AllEnemies,
}

/// How a `Loop` iterates. Iterations after the first schedule pending resolutions.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum LoopKind {
    /// Repeat `count` times, `gap` seconds apart (echo/repeat).
    Times { count: Value, gap: Value },
    /// Fire every `period` seconds for `ticks` iterations (periodic).
    Interval { period: Value, ticks: Value },
}

/// Which bounded numeric pool an `AdjustPool` targets. Data, not behavior — the
/// pool selects a component; the op is a generic clamp-adjust.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum PoolRef {
    Shield,
    Resource(ResourceId),
    Stacks(StackId),
    Cooldown(Slot),
    Charges(Slot),
}

/// Selects buffs to remove — by exact id or by capability tag-class (a cleanse).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum BuffSelector {
    Id(BuffId),
    Class(TagClassId),
}

/// Where a `Teleport` sends its target.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum TeleportDest {
    ToTarget,
    ToPoint(Point3),
    Offset(Direction, Value),
    Home,
}

/// Where a `Spawn` anchors the new body/bodies.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum SpawnAnchor {
    Caster,
    Target,
    Source,
    Point(Point3),
}

/// The arrangement of multiple spawned bodies.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum SpawnPattern {
    Single,
    Radial { count: Value },
    Arc { count: Value, spread: Value },
}

/// Whom a `CastAbility` sub-cast aims at.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum AbilityTarget {
    /// Reuse the enclosing resolution's target.
    Inherit,
    PrimaryTarget,
    Caster,
    Point(Point3),
}

/// Whether a sub-cast pays its normal cost.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum CostMode {
    Normal,
    Free,
}

/// Selects which pending (delayed) resolutions a `ResolvePending` force-fires.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum PendingFilter {
    FromCaster,
    OriginTag(TagId),
}

/// Generic damage switches (no content: crit/lifesteal are engine capabilities).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct DamageFlags {
    pub can_crit: bool,
    pub lifesteal: bool,
}

/// Generic heal switches.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct HealFlags {
    pub can_overheal: bool,
}

/// A single ISA instruction: `Impact = Structure<Leaf>`. The first four variants
/// are the control combinators expanded by the resolution walker; the rest are
/// leaves, each with exactly one observer in `server/src/systems/impacts/`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Impact {
    // --- combinators (expanded by the walker, never by an observer) ---
    Retarget {
        shape: TargetShape,
        filter: TargetFilter,
        max_targets: Value,
        exclude_primary: bool,
        inner: Vec<Impact>,
    },
    If {
        cond: Condition,
        then: Vec<Impact>,
        els: Vec<Impact>,
    },
    Loop {
        kind: LoopKind,
        inner: Vec<Impact>,
    },
    Delay {
        secs: Value,
        inner: Vec<Impact>,
    },

    // --- leaves (one observer each) ---
    Damage {
        amount: Value,
        dtype: DamageTypeId,
        target: ImpactTarget,
        flags: DamageFlags,
    },
    Heal {
        amount: Value,
        target: ImpactTarget,
        flags: HealFlags,
    },
    AdjustPool {
        pool: PoolRef,
        op: NumOp,
        amount: Value,
        target: ImpactTarget,
    },
    ApplyModifiers {
        buff: BuffId,
        stacks: Value,
        duration_override: Option<Value>,
        target: ImpactTarget,
    },
    RemoveModifiers {
        sel: BuffSelector,
        target: ImpactTarget,
    },
    Dash {
        dir: Direction,
        dist: Value,
        speed: Option<Value>,
        on_collision: Vec<Impact>,
        target: ImpactTarget,
    },
    Knockback {
        dir: Direction,
        force: Value,
        target: ImpactTarget,
    },
    Teleport {
        dest: TeleportDest,
        target: ImpactTarget,
        record: bool,
    },
    Spawn {
        body: BodyDescriptor,
        at: SpawnAnchor,
        count: Value,
        pattern: SpawnPattern,
    },
    CastAbility {
        slot: Slot,
        target: AbilityTarget,
        value_scale: Value,
        cost: CostMode,
    },
    Interrupt {
        target: ImpactTarget,
    },
    ResolvePending {
        filter: PendingFilter,
    },
    Emit {
        event: EventId,
        target: ImpactTarget,
        payload: Value,
    },
    /// Narrow escape hatch into the owning mod's wasm export. No-op without the
    /// engine's `modloading` feature.
    Custom {
        handler: HandlerId,
        params: Vec<u8>,
        target: ImpactTarget,
    },
}

impl Impact {
    /// Whether this instruction is a control combinator (expanded by the walker)
    /// rather than a leaf (dispatched to an observer). The walker relies on this
    /// partition being total — every variant is exactly one or the other.
    #[must_use]
    pub fn is_combinator(&self) -> bool {
        matches!(
            self,
            Impact::Retarget { .. } | Impact::If { .. } | Impact::Loop { .. } | Impact::Delay { .. }
        )
    }
}
