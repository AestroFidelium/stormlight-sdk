//! A particle emitter a cosmetic mod declares directly (stormlight/server#141).
//!
//! Imported art carries its emitters inside the model it ships; a mod writing an
//! effect of its own needs to say the same thing without a model. This is that
//! vocabulary, deliberately small — the knobs that decide whether an effect reads
//! right, not every curve a particle editor has: where particles start, how many
//! and when, how long they live, how they fly, what they look like over their
//! life. It is another payload of [`VisualModel`](crate::visuals::VisualModel), so
//! an emitter is placed, kept and dropped exactly as any drawing in its place —
//! on an impact, a notify, a status, a unit.
//!
//! Directions are the world's, with up as up: a cone sprays upward about the
//! vertical through the point it is placed at, a disc lies flat on the ground.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

/// The most particles one emitter may hold alive at once.
pub const MAX_CAPACITY: u32 = 65_536;

/// Where particles start, around the point the emitter is placed at.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum EmitterShape {
    /// The point itself.
    Point,
    /// Anywhere in a ball of `radius`, outside `hollow` (zero fills it).
    Sphere { radius: f32, hollow: f32 },
    /// Anywhere on a flat disc of `radius` lying on the ground, outside `hollow` —
    /// a ring when hollow.
    Disc { radius: f32, hollow: f32 },
    /// Anywhere in a box this wide, tall and deep, centred on the point.
    Box { size: [f32; 3] },
}

/// How many particles are born, and when.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum ParticleEmission {
    /// `rate` a second for as long as the emitter lasts.
    Stream { rate: f32 },
    /// `count` at once, `delay` seconds after it appears.
    Burst { count: u32, delay: f32 },
    /// `rate` a second, from `delay` seconds after it appears, for `duration`.
    Window { rate: f32, delay: f32, duration: f32 },
}

/// Which way particles fly when born.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum ParticleSpray {
    /// Upward, anywhere within `spread` radians of straight up — a fountain at
    /// small spreads, a hemisphere at a quarter turn, which is the widest.
    Cone { spread: f32 },
    /// Outward from the emitter's centre, every way.
    Radial,
}

/// How particles composite over what is behind them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ParticleBlend {
    /// Painted over by their own alpha.
    #[default]
    Blend,
    /// Added as light.
    Add,
    /// Darkening: what is behind scaled by them.
    Multiply,
}

/// A colour at a moment of a particle's life.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct ColorKey {
    /// How far through its life, `0..=1`.
    pub at: f32,
    /// Linear RGBA.
    pub rgba: [f32; 4],
}

/// A particle emitter.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ParticleEmitter {
    pub shape: EmitterShape,
    pub emission: ParticleEmission,
    /// Seconds a particle lives, drawn from this band.
    pub lifetime: [f32; 2],
    /// World units a second it is born flying at, drawn from this band.
    pub speed: [f32; 2],
    pub spray: ParticleSpray,
    /// Vertical acceleration, world units a second squared: negative falls,
    /// positive rises.
    pub gravity: f32,
    /// How quickly flight slows, per second.
    pub drag: f32,
    /// Its width over its life, `[at, world units]` keys. Empty: a small dot.
    pub size: Vec<[f32; 2]>,
    /// Its colour over its life. Empty: white.
    pub color: Vec<ColorKey>,
    /// The `mod://` picture each particle wears. `None`: a flat square of colour.
    pub texture: Option<String>,
    pub blend: ParticleBlend,
    /// Whether particles stay where they were born when the emitter moves, rather
    /// than travelling with it.
    pub world_space: bool,
    /// The most alive at once.
    pub capacity: u32,
}

/// Whether `band` is an ordered pair of finite numbers.
fn ordered(band: [f32; 2]) -> bool {
    band.iter().all(|v| v.is_finite()) && band[0] <= band[1]
}

/// Whether `at` is a moment of a particle's life.
fn in_life(at: f32) -> bool {
    (0.0..=1.0).contains(&at)
}

impl EmitterShape {
    fn is_valid(&self) -> bool {
        let fits = |radius: f32, hollow: f32| {
            radius.is_finite()
                && hollow.is_finite()
                && radius >= 0.0
                && (0.0..=radius).contains(&hollow)
        };
        match *self {
            Self::Point => true,
            Self::Sphere { radius, hollow } | Self::Disc { radius, hollow } => fits(radius, hollow),
            Self::Box { size } => size.iter().all(|v| v.is_finite() && *v >= 0.0),
        }
    }
}

impl ParticleEmission {
    fn is_valid(&self) -> bool {
        match *self {
            Self::Stream { rate } => rate.is_finite() && rate > 0.0,
            Self::Burst { count, delay } => count > 0 && delay.is_finite() && delay >= 0.0,
            Self::Window { rate, delay, duration } => {
                rate.is_finite()
                    && rate > 0.0
                    && delay.is_finite()
                    && delay >= 0.0
                    && duration.is_finite()
                    && duration > 0.0
            }
        }
    }
}

impl ParticleEmitter {
    /// Whether a renderer can run it as declared. Every refusal is a declaration a
    /// mod author can fix: nothing is clamped into something it did not say.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let spray = match self.spray {
            // A hemisphere at most: wider would spray downward, which is what a
            // radial spray is for.
            ParticleSpray::Cone { spread } => {
                spread.is_finite() && (0.0..=core::f32::consts::FRAC_PI_2).contains(&spread)
            }
            ParticleSpray::Radial => true,
        };
        self.shape.is_valid()
            && self.emission.is_valid()
            && ordered(self.lifetime)
            && self.lifetime[1] > 0.0
            && self.lifetime[0] >= 0.0
            && ordered(self.speed)
            && spray
            && self.gravity.is_finite()
            && self.drag.is_finite()
            && self.drag >= 0.0
            && self.size.iter().all(|[at, w]| in_life(*at) && w.is_finite() && *w >= 0.0)
            && self.color.iter().all(|k| in_life(k.at) && k.rgba.iter().all(|c| c.is_finite()))
            && self.texture.as_ref().is_none_or(|t| !t.is_empty())
            && (1..=MAX_CAPACITY).contains(&self.capacity)
    }
}
