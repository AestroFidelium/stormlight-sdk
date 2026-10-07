//! Motion over time (stormlight/server#213) — a unit moving itself along a declared
//! path at a declared speed, as opposed to appearing at the far end of it.
//!
//! A path is an ordered list of [`Leg`]s, each heading relative to the one before
//! it. Shapes that look unrelated are one vocabulary:
//!
//! | shape | legs |
//! | --- | --- |
//! | a straight dash | `[Straight { turn: 0, dist }]` |
//! | a dash and back | `[Straight { turn: 0, dist }, Back]` |
//! | a curve | `[Arc { turn: 0, dist, bend: 90 }]` |
//! | a ring | `[Arc { turn: 90, dist: τr, bend: 360 }]` |
//! | a zig-zag | `[Straight{0,d}, Straight{120,d}, Straight{120,d}]` |
//! | an endless orbit | a ring with [`Repeat::UntilEnded`] |
//!
//! Angles are in **degrees**, positive turning left (counter-clockwise seen from
//! above), because that is how an author reads a turn off a sketch of the map.
//! Every number is a [`Value`], so a talent can lengthen a dash or tighten a curve
//! like any other quantity.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::common::Direction;
use crate::impacts::Impact;
use crate::math::Value;

/// A motion: where it heads first, the legs it travels, how fast, and what
/// happens when it is stopped short or runs out.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Motion {
    /// The heading the first leg turns from — resolved once, when the motion
    /// starts, by the same rules every other direction in an effect uses.
    pub dir: Direction,
    /// The path, in order. Empty is a motion that ends where it starts.
    pub legs: Vec<Leg>,
    /// World units per second along the path. A very large speed is how an author
    /// says "practically instant": the path is still travelled, so everything on
    /// it is still met.
    pub speed: Value,
    /// How many times the legs are travelled.
    pub repeat: Repeat,
    /// The longest the motion may last, in seconds — the way an endless orbit ends
    /// on its own. `None` lets the path decide.
    pub max_secs: Option<Value>,
    /// Runs on the moving unit when geometry stops it short.
    pub on_collision: Vec<Impact>,
    /// Runs on the moving unit when the motion comes to an end **on its own**: its
    /// path runs out, a wall stops it (after `on_collision`), or its time is up.
    /// A motion that is cut off — by an interrupt, its unit's death, or a newer
    /// motion — does not run it: it did not finish.
    pub on_end: Vec<Impact>,
}

/// One stretch of a motion's path.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Leg {
    /// A straight line `dist` long, heading `turn` degrees left of where the last
    /// leg ended (or of the motion's starting heading).
    Straight { turn: Value, dist: Value },
    /// A curve `dist` long whose heading turns `bend` degrees in total — a full
    /// ring is `±360` — after first turning `turn` degrees like a straight leg.
    Arc { turn: Value, dist: Value, bend: Value },
    /// Straight back to where the motion started.
    Back,
    /// Straight to the point the effect was aimed at — the target unit's position,
    /// or the aimed point, as it was when the motion started.
    ToTarget,
}

/// How many times a motion travels its legs.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum Repeat {
    /// Once through.
    #[default]
    Once,
    /// This many times through in all; fewer than one is once.
    Times(Value),
    /// Round and round until something ends it: [`Motion::max_secs`], a wall, an
    /// interrupt, death, or a new motion.
    UntilEnded,
}
