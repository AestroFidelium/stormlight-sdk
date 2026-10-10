//! Declaring a particle emitter (stormlight/server#141) — the authoring half.
//!
//! An emitter is a [`VisualModel`] like any drawing, so it goes wherever one does.
//!
//! ```ignore
//! use stormlight_mod_sdk::particles::emitter;
//! use stormlight_mod_sdk::abi::particles::ParticleEmission;
//!
//! // A burst of sparks, flying out and falling.
//! let sparks = emitter(ParticleEmission::Burst { count: 24, delay: 0.0 })
//!     .radial()
//!     .speed(2.0, 5.0)
//!     .lifetime(0.3, 0.6)
//!     .gravity(-9.0)
//!     .color(0.0, [1.0, 0.8, 0.4, 1.0])
//!     .color(1.0, [1.0, 0.3, 0.1, 0.0])
//!     .additive();
//! ctx.declare_effect(effect("bolt", EffectRole::Impact, sparks.into()));
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use stormlight_mod_abi::particles::{
    ColorKey, EmitterShape, ParticleBlend, ParticleEmission, ParticleEmitter, ParticleSpray,
};
use stormlight_mod_abi::visuals::VisualModel;

/// An emitter being declared — see [`emitter`]. Becomes a [`VisualModel`] with
/// `.into()`.
#[derive(Clone, Debug, PartialEq)]
#[must_use = "an emitter is declared only once it is handed to a declaration"]
pub struct EmitterSpec(ParticleEmitter);

/// How many alive at once an emitter holds unless told otherwise.
pub const CAPACITY: u32 = 64;
/// How far from straight up a fountain sprays unless told otherwise, radians.
pub const SPREAD: f32 = 0.3;

/// Particles born as `emission` says, from the point it is placed at: living a
/// second, flying up at a unit a second within a narrow cone, unaffected by
/// gravity or drag, a small white dot each, painted over by their alpha, carried
/// with the emitter, at most [`CAPACITY`] alive.
pub fn emitter(emission: ParticleEmission) -> EmitterSpec {
    EmitterSpec(ParticleEmitter {
        shape: EmitterShape::Point,
        emission,
        lifetime: [1.0, 1.0],
        speed: [1.0, 1.0],
        spray: ParticleSpray::Cone { spread: SPREAD },
        gravity: 0.0,
        drag: 0.0,
        size: Vec::new(),
        color: Vec::new(),
        texture: None,
        blend: ParticleBlend::Blend,
        world_space: false,
        capacity: CAPACITY,
    })
}

impl EmitterSpec {
    /// Born anywhere in `shape` instead.
    pub fn shape(mut self, shape: EmitterShape) -> Self {
        self.0.shape = shape;
        self
    }

    /// Living between `shortest` and `longest` seconds.
    pub fn lifetime(mut self, shortest: f32, longest: f32) -> Self {
        self.0.lifetime = [shortest, longest];
        self
    }

    /// Born flying between `slowest` and `fastest` units a second.
    pub fn speed(mut self, slowest: f32, fastest: f32) -> Self {
        self.0.speed = [slowest, fastest];
        self
    }

    /// Flying up within `spread` radians of straight up.
    pub fn cone(mut self, spread: f32) -> Self {
        self.0.spray = ParticleSpray::Cone { spread };
        self
    }

    /// Flying outward from the centre, every way.
    pub fn radial(mut self) -> Self {
        self.0.spray = ParticleSpray::Radial;
        self
    }

    /// Accelerated vertically: negative falls, positive rises.
    pub fn gravity(mut self, gravity: f32) -> Self {
        self.0.gravity = gravity;
        self
    }

    /// Slowed by `drag` a second.
    pub fn drag(mut self, drag: f32) -> Self {
        self.0.drag = drag;
        self
    }

    /// `width` world units wide `at` this far through its life.
    pub fn size(mut self, at: f32, width: f32) -> Self {
        self.0.size.push([at, width]);
        self
    }

    /// Coloured `rgba` (linear) `at` this far through its life.
    pub fn color(mut self, at: f32, rgba: [f32; 4]) -> Self {
        self.0.color.push(ColorKey { at, rgba });
        self
    }

    /// Each wearing the picture in `asset`.
    pub fn texture(mut self, asset: impl Into<String>) -> Self {
        self.0.texture = Some(asset.into());
        self
    }

    /// Added as light.
    pub fn additive(mut self) -> Self {
        self.0.blend = ParticleBlend::Add;
        self
    }

    /// Darkening what is behind.
    pub fn multiply(mut self) -> Self {
        self.0.blend = ParticleBlend::Multiply;
        self
    }

    /// Left where they were born when the emitter moves.
    pub fn world_space(mut self) -> Self {
        self.0.world_space = true;
        self
    }

    /// At most `capacity` alive at once.
    pub fn capacity(mut self, capacity: u32) -> Self {
        self.0.capacity = capacity;
        self
    }
}

impl From<EmitterSpec> for VisualModel {
    fn from(spec: EmitterSpec) -> Self {
        VisualModel::Particles(spec.0)
    }
}
