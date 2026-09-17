//! Progression declarations (stormlight/server#62) — how a unit earns levels, and
//! what killing it is worth.
//!
//! Talents are gated by level, so a real talent experience needs a real economy
//! behind it. That economy is **entirely content**. How much a kill yields, how the
//! yield is divided, how much is needed for the next level, what the level grants —
//! every one of those is a balance decision, and an engine that knew any of them
//! would be an engine you could not re-balance without rebuilding it. So this module
//! holds no number at all: it is the shape a mod fills in.
//!
//! # Two directions, one declaration
//!
//! A unit's place in the XP economy has two halves and they are not the same half:
//! what it **earns** ([`XpSource`]) and what it is **worth** ([`XpBounty`]). A
//! training dummy earns nothing and is worth something; a hero is usually both.
//! They live in one [`ProgressionSpec`] because they are one decision — a mod
//! tuning the economy moves both together — and because a unit that declares
//! neither declares no spec at all and simply never participates.
//!
//! # Stat growth is not here, deliberately
//!
//! There is no per-level stat table, and that absence is the design. A unit's
//! [`stats`](crate::units::UnitDescriptor::stats) and
//! [`health`](crate::units::UnitDescriptor::health) are already [`Value`]
//! expressions, and [`Var::Level`](crate::math::Var::Level) is already a thing a
//! `Value` can read — so "gains 8 attack per level" is
//! `Curve(growth, Read(Level))` in the field that already exists, re-evaluated when
//! the level changes. A second place for stat numbers to come from would be a
//! second place for them to disagree.
//!
//! What is left is exactly what a `Value` cannot express: [`LevelGrants`], the
//! structural things a level hands over — a tag, an ability appearing in a slot.
//!
//! # Talent tiers are not granted here either
//!
//! A tier becoming choosable is *also* a function of the level, so it is declared
//! where the tier is — [`TalentTree`](crate::talent_tree::TalentTree), on the unit —
//! and asked of the current level whenever it matters, rather than handed over once
//! on the way up. A grant here could only be a latch, and a latch survives the
//! rewind that took the level away again.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::ids::{CurveId, TagId};
use crate::math::Value;
use crate::talents::GrantAbility;

/// How much total XP standing at a given level requires.
///
/// Cumulative, not per-level: a threshold is the XP a unit must have accumulated
/// in total, which is what makes a single grant able to cross several levels at
/// once without the engine having to subtract anything.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum XpCurve {
    /// Explicit totals, one per level above the first: `totals[i]` is the XP
    /// required to stand at level `i + 2`. The direct form, for a mod that wants
    /// to read its own numbers off a table.
    Table(Vec<f32>),
    /// A lookup curve sampled at the level in question — the form for a
    /// smooth (and short) declaration. The interpreter samples it; see
    /// `server/src/curves.rs`.
    Curve(CurveId),
}

/// What reaching a level hands over, beyond the numbers a `Value` already scales
/// (see the module docs).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct LevelGrants {
    /// Tags the unit carries from this level on.
    pub tags: Vec<TagId>,
    /// Abilities that appear on the unit's bar at this level.
    ///
    /// The same [`GrantAbility`] a talent hands over (stormlight/server#188), and
    /// deliberately the same type rather than a parallel `(slot, ability)` pair: a
    /// level handing a unit its ultimate, a level replacing a starter ability, and a
    /// level adding a button wherever there is room are the same three intents a
    /// talent has, answered in the same one place. A pair could only ever spell the
    /// first of them, and spelled it silently.
    pub abilities: Vec<GrantAbility>,
}

/// How a kill's XP is divided among the living.
///
/// A generic policy over a generic relationship — "who damaged it" and "whose team
/// the killer is on". Nothing here knows what died or who killed it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum XpShare {
    /// The unit that landed the killing blow takes the whole amount. The narrow
    /// policy: last-hit contests are a design a mod may want, and this is how it
    /// asks for one.
    Killer,
    /// Split across everyone who damaged the victim inside the credit window, in
    /// proportion to the damage each dealt. Participation pays, and the sum of the
    /// shares is the declared amount — no more.
    ByDamage,
    /// Every unit on the killer's team earns the **full** amount, wherever it is
    /// and whatever state it is in — across the map, out of the fight, or dead.
    ///
    /// The default, because a team-wide XP pool is the shape of the genre: a
    /// player who is dead when the objective falls has not opted out of their
    /// team's progress, and one who is fighting elsewhere is the reason it fell.
    /// Making that positional would quietly turn every teamfight into a race to
    /// stand near a corpse.
    #[default]
    Team,
}

/// What killing a unit is worth, and to whom.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct XpBounty {
    /// The XP a kill yields, as an expression — so a bounty may scale with the
    /// victim's own level, its remaining health, or anything else a `Value` reads.
    pub amount: Value,
    /// How the amount is divided. See [`XpShare`].
    pub share: XpShare,
    /// Seconds a damage contribution stays creditable, counted back from the
    /// death. `None` credits the unit's whole life. Read through
    /// [`window`](Self::window).
    pub credit_window: Option<f32>,
}

impl XpBounty {
    /// A bounty of `amount`, shared team-wide over the victim's whole life — the
    /// genre default, and the starting point a mod adjusts from.
    #[must_use]
    pub fn worth(amount: Value) -> Self {
        Self { amount, share: XpShare::Team, credit_window: None }
    }

    /// The effective credit window: finite and non-negative, or `None` for "the
    /// whole life".
    ///
    /// Total by design, because nothing else validates this number. A negative
    /// window is a sign error and credits nothing but the killing blow itself; an
    /// infinite or `NaN` one reads as absent, since an unbounded window *is* the
    /// whole life and that is the field's own default rather than an invented one.
    #[must_use]
    pub fn window(&self) -> Option<f32> {
        match self.credit_window {
            Some(w) if w.is_finite() => Some(w.max(0.0)),
            _ => None,
        }
    }
}

/// What makes a unit earn XP on its own — as opposed to what it earns from a
/// kill, which is the victim's [`XpBounty`] to declare.
///
/// Deliberately short. Anything hookable already flows through **event → reaction**,
/// and a reaction whose effect is
/// `AdjustPool { pool: Xp, … }` earns XP from any event the sim announces without
/// a new trigger here. These two are the ones with no event to hang off: a cadence
/// is not an announcement, and damage dealt is a quantity rather than an occurrence.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum XpTrigger {
    /// Every `period` seconds the unit is alive — the passive trickle a match
    /// gives everyone so a player who has not fought is not left behind.
    Timed { period: f32 },
    /// Per point of damage the unit deals. The amount is multiplied by the damage,
    /// so the declaration is a rate rather than a lump.
    DamageDealt,
}

/// One way a unit earns XP, and how much.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct XpSource {
    pub trigger: XpTrigger,
    /// The XP yielded, as an expression evaluated against the earner.
    pub amount: Value,
}

impl XpSource {
    /// The firing cadence in seconds, or `None` for a source that is not on one.
    ///
    /// Total, and strict about it: a zero, negative, infinite or `NaN` period is a
    /// source that **never fires**, never one that fires every tick forever. A
    /// cadence the engine had to invent would be a balance number with no author.
    #[must_use]
    pub fn period(&self) -> Option<f32> {
        match self.trigger {
            XpTrigger::Timed { period } if period.is_finite() && period > 0.0 => Some(period),
            _ => None,
        }
    }
}

/// A unit's whole place in the XP economy: how it levels, how it earns, and what
/// it is worth.
///
/// A unit that declares none of this never levels and yields nothing — the honest
/// default for a projectile, a wall, or anything else that is not a participant,
/// and the reason the field on [`UnitDescriptor`](crate::units::UnitDescriptor) is
/// an `Option`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ProgressionSpec {
    /// Total XP required at each level above the first.
    pub thresholds: XpCurve,
    /// The highest level this unit can reach. Read through
    /// [`ceiling`](Self::ceiling).
    pub max_level: u8,
    /// The level this unit **begins** at, read through [`start`](Self::start)
    /// (stormlight/server#131).
    ///
    /// `0` and `1` both mean the ordinary thing — a unit that starts at the bottom
    /// and climbs — so a spec written before this field says what it always said.
    ///
    /// It is a real content decision rather than only a testing convenience: a
    /// practice dummy worth practising on, a boss that is not level one, a mode that
    /// starts everybody at the ceiling. What it does **not** do is hand over XP: the
    /// unit's account starts empty and its level is simply floored here, so the
    /// number it earns is still the number it earned.
    #[serde(default)]
    pub starting_level: u8,
    /// What each level hands over, keyed by the level reached. Rows for the same
    /// level apply in declaration order; a level with no row grants nothing
    /// structural (its stat growth is in the unit's own `Value`s — see the module
    /// docs).
    pub grants: Vec<(u8, LevelGrants)>,
    /// How this unit earns XP under its own power.
    pub sources: Vec<XpSource>,
    /// What killing this unit yields, if anything.
    pub bounty: Option<XpBounty>,
}

impl ProgressionSpec {
    /// A spec that never levels and only says what the unit is worth — the shape a
    /// creep, an objective or a training target declares.
    #[must_use]
    pub fn bounty_only(bounty: XpBounty) -> Self {
        Self {
            thresholds: XpCurve::Table(Vec::new()),
            max_level: 1,
            grants: Vec::new(),
            sources: Vec::new(),
            bounty: Some(bounty),
            // It never levels, so where it starts is where it stays.
            starting_level: 1,
        }
    }

    /// The effective level ceiling: never below the level every unit spawns at.
    ///
    /// A declared `0` is not a unit that cannot exist; it is a unit that never
    /// levels, which is level 1.
    #[must_use]
    pub fn ceiling(&self) -> u8 {
        self.max_level.max(1)
    }

    /// The level this unit begins at: what it declared, floored at the first level
    /// and capped at its own [`ceiling`](Self::ceiling).
    ///
    /// The cap is why this is a method. A unit declaring a start above its own
    /// maximum would otherwise stand at a level no threshold describes and no climb
    /// could produce — so it starts at the top instead, which is what it meant.
    #[must_use]
    pub fn start(&self) -> u8 {
        self.starting_level.max(1).min(self.ceiling())
    }

    /// Every grant declared for `level`, in declaration order.
    pub fn grants_at(&self, level: u8) -> impl Iterator<Item = &LevelGrants> {
        self.grants.iter().filter(move |(l, _)| *l == level).map(|(_, g)| g)
    }
}
