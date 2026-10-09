//! A picture laid flat on the ground (stormlight/server#170).
//!
//! Nothing a mod declared used to touch the ground: an area's indicator, a scorch
//! mark, a ring under a unit all had to be drawn as a mesh standing on the floor.
//! A decal is the primitive for all of them — a picture with a size on the ground,
//! a tint, a way of blending, and an opacity that may rise, hold and fall on its
//! own clock.

use serde::{Deserialize, Serialize};

/// How a decal is laid over what it lies on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum DecalBlend {
    /// Painted over the ground by its own opacity — a marking, a shadow, a scorch.
    #[default]
    Blend,
    /// Added to the ground as light — a glowing rune, a ring of frost.
    Add,
}

/// How a decal's opacity moves over its life: it rises over `attack`, holds for
/// `hold`, and falls over `decay`, through the three opacities in `alpha` —
/// at the start, at the peak, and at the end, where it then stays.
///
/// Seconds from the moment the decal appears. A phase of zero is an instant step.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct DecalFade {
    /// Seconds to rise from the start opacity to the peak.
    pub attack: f32,
    /// Seconds held at the peak.
    pub hold: f32,
    /// Seconds to fall from the peak to the end opacity.
    pub decay: f32,
    /// The opacity at the start, at the peak and at the end, each `0..=1`.
    pub alpha: [f32; 3],
}

impl DecalFade {
    /// Whether a clock can run it: finite, non-negative phases, and opacities in
    /// `0..=1`.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        [self.attack, self.hold, self.decay].iter().all(|p| p.is_finite() && *p >= 0.0)
            && self.alpha.iter().all(|a| a.is_finite() && (0.0..=1.0).contains(a))
    }

    /// The opacity `age` seconds after the decal appeared.
    #[must_use]
    pub fn alpha_at(&self, age: f32) -> f32 {
        let [start, peak, end] = self.alpha;
        let lerp = |from: f32, to: f32, t: f32| from + (to - from) * t.clamp(0.0, 1.0);
        let age = age.max(0.0);
        if age < self.attack {
            return lerp(start, peak, age / self.attack);
        }
        // The hold is half-open: at its very end the fall has begun, so a decay of
        // zero is a step to the end opacity there rather than one more instant at
        // the peak.
        let since_peak = age - self.attack;
        if since_peak < self.hold {
            return peak;
        }
        let falling = since_peak - self.hold;
        if self.decay > 0.0 && falling < self.decay {
            return lerp(peak, end, falling / self.decay);
        }
        end
    }
}
