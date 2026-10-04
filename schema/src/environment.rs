//! A map's environment — the light its art is seen in, and what lies past its edge.
//!
//! Like [`crate::scenery`], purely cosmetic and declared by a map's client half.
//! The engine knows no map's sun: without an environment it lights the field with a
//! neutral default, and with one it draws exactly what the map asked for.
//!
//! Intensities are **relative**, not photometric: a colour of `1.0` on a light means
//! a white surface facing it is drawn at its full albedo, and an ambient of `1.0`
//! does the same from every side. That is the scale a map's authoring tools work
//! in, and it leaves the renderer free to pick its own physical units.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

/// A distant light — the sun, the sky's fill, a rim from behind. Unshadowed: the
/// one shadow a map casts is declared apart, as [`Shadow`].
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct SunLight {
    /// Linear RGB with the intensity folded in (see the module's units).
    pub color: [f32; 3],
    /// The way the light **travels**, in world space (`Y` up): a sun overhead is
    /// `[0, -1, 0]`. Any non-zero length; [`toward`](Self::toward) normalises.
    pub direction: [f32; 3],
}

impl SunLight {
    /// The travel direction, unit length. `[0, -1, 0]` for a zero vector.
    #[must_use]
    pub fn toward(&self) -> [f32; 3] {
        unit(self.direction)
    }

    fn is_valid(&self) -> bool {
        colour(&self.color) && direction(&self.direction)
    }
}

/// The shadow a map casts. Separate from its lights' directions because the two
/// are art directed apart: a map's shadows are placed to read well under its
/// camera, not to agree with whichever light makes its colours.
///
/// The shadow is cast by the map's **first** light: `strength` of that light is
/// turned onto the shadow's direction and stopped by whatever stands in the way,
/// and the rest keeps the light's own direction and reaches everything. So the
/// total light is unchanged, and a spot in full shadow loses exactly `strength` of
/// the first light.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Shadow {
    /// The way the shadowing light travels, world space, any non-zero length.
    pub direction: [f32; 3],
    /// The share of the first light that shadows can block, `0` to `1`.
    pub strength: f32,
}

impl Shadow {
    /// The travel direction, unit length.
    #[must_use]
    pub fn toward(&self) -> [f32; 3] {
        unit(self.direction)
    }

    fn is_valid(&self) -> bool {
        direction(&self.direction) && (0.0..=1.0).contains(&self.strength)
    }
}

/// Everything a map says about the light it is seen in.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Environment {
    /// Light from every side, linear RGB (see the module's units).
    pub ambient: [f32; 3],
    /// The distant lights.
    pub lights: Vec<SunLight>,
    /// The map's one shadow, if it casts any.
    pub shadow: Option<Shadow>,
    /// A final brightness multiplier over everything lit.
    pub exposure: f32,
    /// What is drawn where there is nothing — past the map's edge — linear RGB.
    pub backdrop: [f32; 3],
}

impl Environment {
    /// Whether every colour is finite and non-negative, every direction finite and
    /// non-zero, a shadow's strength within `0..=1`, and the exposure positive.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        colour(&self.ambient)
            && colour(&self.backdrop)
            && self.exposure.is_finite()
            && self.exposure > 0.0
            && self.lights.iter().all(SunLight::is_valid)
            && self.shadow.as_ref().is_none_or(Shadow::is_valid)
    }
}

fn colour(c: &[f32; 3]) -> bool {
    c.iter().all(|v| v.is_finite() && *v >= 0.0)
}

fn direction(d: &[f32; 3]) -> bool {
    d.iter().all(|v| v.is_finite()) && d.iter().any(|v| *v != 0.0)
}

fn unit(d: [f32; 3]) -> [f32; 3] {
    let len = sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
    if len > 0.0 && len.is_finite() { d.map(|v| v / len) } else { [0.0, -1.0, 0.0] }
}

/// A square root without `std`: Newton's method from a power-of-two first guess,
/// which converges to `f32` precision in a handful of steps for any normal input.
fn sqrt(x: f32) -> f32 {
    if !(x.is_finite() && x > 0.0) {
        return if x == 0.0 { 0.0 } else { f32::NAN };
    }
    // Halving the exponent gives a guess within a factor of two.
    let mut r = f32::from_bits((x.to_bits() >> 1) + (127 << 22));
    for _ in 0..6 {
        r = 0.5 * (r + x / r);
    }
    r
}
