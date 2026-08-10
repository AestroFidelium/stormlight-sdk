//! Stats & modifiers: the "magnitude" half of the effect model (the "binary
//! state" half is tags — see [`crate::units`]).
//!
//! A [`Modifier`] adjusts one stat; the final value is an **ordered fold** over
//! every modifier acting on a unit. The fold order is fixed so the result is
//! deterministic and, crucially, **independent of the order sources are
//! collected** — a replay requirement pinned by the property tests.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::ids::{BuffId, StatId, TagId};
use crate::impacts::Impact;
use crate::math::Value;
use crate::triggers::Reaction;

/// How a modifier combines into a stat.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ModOp {
    /// Flat additive term, summed then added to the base.
    AddFlat,
    /// Additive percent; all `AddPct` sum into a single `× (1 + Σ)` factor.
    AddPct,
    /// Multiplicative factor, applied as a product.
    Mul,
    /// Hard set; the highest-priority override replaces the whole stat.
    Override,
}

/// A single stat adjustment as authored (magnitude is a [`Value`] expression).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Modifier {
    pub stat: StatId,
    pub op: ModOp,
    pub value: Value,
}

/// A modifier whose [`Value`] has been evaluated to a number and whose source
/// priority is known — the input to [`aggregate_stat`]. `priority` breaks ties
/// among `Override`s (higher wins).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ResolvedModifier {
    pub op: ModOp,
    pub value: f32,
    pub priority: i32,
}

/// Fold `mods` onto `base` in the fixed order
/// `base → + ΣAddFlat → × (1 + ΣAddPct) → × ΠMul → Override`.
///
/// The sums/products are taken in a canonical (value-sorted) order, so the
/// result is **bit-identical under any permutation** of `mods` — aggregation is
/// order-stable across sources. An `Override` present at all replaces the stat
/// with the highest-priority override's value (ties broken by larger value).
#[must_use]
pub fn aggregate_stat(base: f32, mods: &[ResolvedModifier]) -> f32 {
    // Highest-priority override, if any, wins outright.
    let mut ovr: Option<ResolvedModifier> = None;
    let mut flat: Vec<f32> = Vec::new();
    let mut pct: Vec<f32> = Vec::new();
    let mut mul: Vec<f32> = Vec::new();
    for m in mods {
        match m.op {
            ModOp::AddFlat => flat.push(m.value),
            ModOp::AddPct => pct.push(m.value),
            ModOp::Mul => mul.push(m.value),
            ModOp::Override => {
                // Take this override if it has strictly higher priority, or equal
                // priority and a larger value (a deterministic tie-break).
                let take = match ovr {
                    None => true,
                    Some(cur) => {
                        (m.priority, m.value.total_cmp(&cur.value))
                            > (cur.priority, core::cmp::Ordering::Equal)
                    }
                };
                if take {
                    ovr = Some(*m);
                }
            }
        }
    }
    if let Some(o) = ovr {
        return o.value;
    }

    // Canonical summation order for float determinism across source orderings.
    flat.sort_unstable_by(f32::total_cmp);
    pct.sort_unstable_by(f32::total_cmp);
    mul.sort_unstable_by(f32::total_cmp);

    let mut v = base + flat.iter().sum::<f32>();
    v *= 1.0 + pct.iter().sum::<f32>();
    for m in mul {
        v *= m;
    }
    v
}

/// What happens when a buff is applied to a holder that already has it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Reapply {
    RefreshDuration,
    AddDuration,
    Independent,
    Ignore,
}

/// Whether stacks are counted per source or globally on the holder.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum StackScope {
    PerSource,
    Global,
}

/// Stacking rule for a buff.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Stacking {
    pub on_reapply: Reapply,
    pub scope: StackScope,
}

/// A buff/status effect definition: duration + stacking + modifiers + tags +
/// scoped reactions + lifecycle effects. A "slow" is a `BuffSpec` carrying a
/// `move_speed` modifier (magnitude) and, optionally, a `slowed` tag (so
/// conditions can detect it) — no dedicated primitive.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct BuffSpec {
    pub id: BuffId,
    /// None = lasts until removed.
    pub duration: Option<Value>,
    pub stacking: Stacking,
    pub max_stacks: u16,
    pub modifiers: Vec<Modifier>,
    pub tags: Vec<TagId>,
    /// Reactions scoped to this buff (removed when the buff ends).
    pub reactions: Vec<Reaction>,
    pub on_apply: Vec<Impact>,
    pub on_expire: Vec<Impact>,
    pub on_remove: Vec<Impact>,
    pub drop_on_death: bool,
}
