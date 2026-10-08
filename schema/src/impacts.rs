//! The effect ISA — ~4 control combinators over ~14 leaf operations. This is the
//! whole moddable vocabulary. It does **not** grow with content: new content is a
//! new *composition* of these leaves plus new `Value`/`Stat`/`Tag`/`Param` data.
//! A new leaf is allowed only for a genuinely new structural verb the engine
//! cannot compose (governance rule, §3).
//!
//! Ids here are interned handles (the runtime form). Authoring uses stable
//! strings converted to handles at adoption (§6.3); the tree shape is identical.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::common::{Direction, ImpactTarget, NumOp, Point3, TargetFilter};
use crate::conditions::Condition;
use crate::ids::{
    BuffId, DamageTypeId, EventId, HandlerId, ResourceId, StackId, TagClassId, TagId,
};
use crate::math::Value;
use crate::missiles::BodyDescriptor;
use crate::motion::Motion;
use crate::params::ParamOverride;
use crate::slot_ref::SlotRef;

/// The geometric target set a `Retarget` resolves; params live inside the shape.
///
/// A geometric shape needs an **origin** as well as a size, and it is never a
/// world axis: like every other position in the ISA it is named relationally, by
/// the actor it hangs off (or an explicit point). `Circle { at: Caster }` is the
/// classic "blast around me"; `at: ResolvedTarget` is "blast around what this
/// landed on" — the form a spawned body's `on_spawn`/`on_expire` wants, since
/// those resolve against the body's own point rather than its owner.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum TargetShape {
    SelfOnly,
    Circle {
        at: ImpactTarget,
        radius: Value,
    },
    /// Everything within `radius` of `at` and at most `angle` radians off the
    /// resolution's aim — `angle` is the **half**-angle, the same measure as the
    /// reserved `spread` param (stormlight/server#233).
    ///
    /// The aim is the direction the resolution was aimed along: the press's aim, an
    /// attack's line to its target, a missile's line of flight. With none, the cone
    /// opens from `at` toward the resolved target; with neither, it selects only
    /// what stands on `at`.
    Cone {
        at: ImpactTarget,
        radius: Value,
        angle: Value,
    },
    /// A chain: its first link is the unit `from` names when that unit passes the
    /// filter (else the nearest one within `range` of it), and each next link is the
    /// nearest unit not yet linked within `range` of the last, for up to `jumps`
    /// jumps (stormlight/server#233).
    Chain {
        from: ImpactTarget,
        jumps: Value,
        range: Value,
    },
    /// A strip `length` long and `width` wide (half to each side), running from
    /// `at` along the resolution's aim — oriented like [`TargetShape::Cone`]
    /// (stormlight/server#233).
    Line {
        at: ImpactTarget,
        length: Value,
        width: Value,
    },
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
    /// A slot's remaining cooldown. The slot is a [`SlotRef`], so a talent can pay
    /// back "the one I just used" as easily as a written-down one
    /// (stormlight/server#187).
    Cooldown(SlotRef),
    /// A slot's charge balance — the same reference rule as [`Self::Cooldown`].
    Charges(SlotRef),
    /// Accumulated experience (stormlight/server#62). Granting XP is an adjust of
    /// a bounded numeric reserve, not a new structural verb, so it is a pool ref
    /// rather than a leaf of its own (§3 governance). That is also what lets a mod
    /// pay XP out from *any* effect — a reaction to an arbitrary event, an
    /// ability, a buff expiring — without the engine enumerating the occasions.
    Xp,
    /// The resource the ability in a slot pays its cost in — its first resource
    /// cost (stormlight/server#210). "Refund what this costs" written once works on
    /// every unit: one that pays in energy gets energy back, one that pays in
    /// another resource gets that. A slot whose ability costs no resource names no
    /// pool, and an adjust of it does nothing.
    ///
    /// **Appended, never inserted**: the variant order is the wire tag.
    AbilityCost(SlotRef),
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
    Radial {
        count: Value,
    },
    Arc {
        count: Value,
        spread: Value,
    },
    /// `inner`, turned off the aim by `angle` radians about the vertical — the
    /// whole arrangement, so a turned arc is still an arc (stormlight/server#210).
    /// Turns nest by adding. What a fan talent's copies fly
    /// ([`VolleyMod::Fan`](crate::volley::VolleyMod::Fan)).
    ///
    /// **Appended, never inserted**: the variant order is the wire tag.
    Turned {
        angle: Value,
        inner: Box<SpawnPattern>,
    },
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
// The ISA is deliberately granular (one variant per structural verb, never
// collapsed into a mode-field), so some variants carry a `BodyDescriptor` while
// others are unit-like. That size spread is intended vocabulary shape, not an
// oversight — the tree is heap-allocated (`Vec<Impact>`) at every nesting point.
#[allow(clippy::large_enum_variant)]
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
    /// The target moves *itself* along a declared path, over time
    /// (stormlight/server#213). A root stops it starting.
    Dash {
        motion: Motion,
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
        /// Which slot re-fires. [`SlotRef::This`] is the enclosing resolution's own
        /// slot, which is the only way an ability granted into a slot chosen at
        /// runtime can re-cast *itself* (stormlight/server#187).
        slot: SlotRef,
        target: AbilityTarget,
        value_scale: Value,
        cost: CostMode,
        /// Parameters this sub-cast restates — "the same ability, over a wider
        /// area" (stormlight/server#150). Empty is the ordinary case: the nested
        /// cast runs exactly as the caster's own would.
        ///
        /// Folded on top of the slot's talent patches, so the caller has the last
        /// word over what a talent shaped ([`ParamOverride`]).
        #[serde(default)]
        params: Vec<ParamOverride>,
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
            Impact::Retarget { .. }
                | Impact::If { .. }
                | Impact::Loop { .. }
                | Impact::Delay { .. }
        )
    }
}
