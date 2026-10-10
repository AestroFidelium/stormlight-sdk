//! A particle emitter a cosmetic mod declares directly (stormlight/server#141).
//!
//! Laws, over any declaration:
//!   - **a declared emitter is playable**: lifetimes and speeds as ordered, finite
//!     bands with something alive; an emission that emits; finite forces with no
//!     negative drag; size and colour keys inside the particle's life and finite;
//!     a shape whose hollow fits inside it; a picture named when one is named; a
//!     capacity of at least one and at most the engine's ceiling;
//!   - **any one fault is refused**, never clamped — a mod author is told what to
//!     fix rather than watching an effect come out different from its card;
//!   - **a declared emitter reaches the host unchanged**, as a drawing.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::particles::{
    ColorKey, EmitterShape, MAX_CAPACITY, ParticleBlend, ParticleEmission, ParticleEmitter,
    ParticleSpray,
};
use stormlight_mod_abi::visuals::VisualModel;

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Shape {
    Point,
    Sphere,
    Disc,
    Box,
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Emits {
    Stream,
    Burst,
    Window,
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Fault {
    Sound,
    DeadOnArrival,
    InvertedLifetime,
    InfiniteSpeed,
    InvertedSpeed,
    SilentStream,
    EmptyBurst,
    NegativeDelay,
    EmptyWindow,
    InfiniteGravity,
    NegativeDrag,
    KeyOutsideLife,
    InfiniteSize,
    NegativeSize,
    InfiniteColour,
    HollowPastRim,
    NegativeRadius,
    NoPicture,
    NoCapacity,
    OverCapacity,
    WideSpray,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    shape: Shape,
    emits: Emits,
    radial: bool,
    world_space: bool,
    textured: bool,
    /// Tenths of the base numbers each field is built from.
    #[generator(1u8..=40)]
    scale: u8,
    fault: Fault,
}

fn emitter(s: &Scenario) -> ParticleEmitter {
    let x = f32::from(s.scale) / 10.0;
    let shape = match s.shape {
        Shape::Point => EmitterShape::Point,
        Shape::Sphere => EmitterShape::Sphere { radius: x, hollow: x / 2.0 },
        Shape::Disc => EmitterShape::Disc { radius: x, hollow: 0.0 },
        Shape::Box => EmitterShape::Box { size: [x, x / 2.0, x] },
    };
    let emission = match s.emits {
        Emits::Stream => ParticleEmission::Stream { rate: 30.0 * x },
        Emits::Burst => ParticleEmission::Burst { count: 12, delay: x / 4.0 },
        Emits::Window => ParticleEmission::Window { rate: 20.0, delay: 0.0, duration: x },
    };
    ParticleEmitter {
        shape,
        emission,
        lifetime: [0.4 * x, x],
        speed: [x, 2.0 * x],
        spray: if s.radial { ParticleSpray::Radial } else { ParticleSpray::Cone { spread: 0.4 } },
        gravity: -2.0,
        drag: 0.5,
        size: vec![[0.0, 0.2 * x], [1.0, 0.05]],
        color: vec![
            ColorKey { at: 0.0, rgba: [0.6, 0.8, 1.0, 1.0] },
            ColorKey { at: 1.0, rgba: [0.2, 0.3, 1.0, 0.0] },
        ],
        texture: s.textured.then(|| "mod://pack/spark.png".into()),
        blend: ParticleBlend::Add,
        world_space: s.world_space,
        capacity: 256,
    }
}

fn break_it(e: &mut ParticleEmitter, fault: Fault) -> bool {
    match fault {
        Fault::Sound => return true,
        Fault::DeadOnArrival => e.lifetime = [0.0, 0.0],
        Fault::InvertedLifetime => e.lifetime = [2.0, 1.0],
        Fault::InfiniteSpeed => e.speed[1] = f32::INFINITY,
        Fault::InvertedSpeed => e.speed = [3.0, 1.0],
        Fault::SilentStream => e.emission = ParticleEmission::Stream { rate: 0.0 },
        Fault::EmptyBurst => e.emission = ParticleEmission::Burst { count: 0, delay: 0.0 },
        Fault::NegativeDelay => e.emission = ParticleEmission::Burst { count: 3, delay: -1.0 },
        Fault::EmptyWindow => {
            e.emission = ParticleEmission::Window { rate: 10.0, delay: 0.0, duration: 0.0 };
        }
        Fault::InfiniteGravity => e.gravity = f32::NAN,
        Fault::NegativeDrag => e.drag = -0.1,
        Fault::KeyOutsideLife => e.size[1][0] = 1.5,
        Fault::InfiniteSize => e.size[0][1] = f32::INFINITY,
        Fault::NegativeSize => e.size[0][1] = -0.1,
        Fault::InfiniteColour => e.color[0].rgba[2] = f32::NAN,
        Fault::HollowPastRim => e.shape = EmitterShape::Disc { radius: 1.0, hollow: 2.0 },
        Fault::NegativeRadius => e.shape = EmitterShape::Sphere { radius: -1.0, hollow: 0.0 },
        Fault::NoPicture => e.texture = Some(String::new()),
        Fault::NoCapacity => e.capacity = 0,
        Fault::OverCapacity => e.capacity = MAX_CAPACITY + 1,
        Fault::WideSpray => e.spray = ParticleSpray::Cone { spread: 1.58 },
    }
    false
}

#[test]
fn a_playable_emitter_is_kept_and_any_fault_refused() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut e = emitter(s);
        let sound = break_it(&mut e, s.fault);
        assert_eq!(e.is_valid(), sound, "{s:?}: {e:?}");
        assert_eq!(VisualModel::Particles(e.clone()).is_drawable(), sound, "{s:?}");

        if !sound {
            return;
        }
        let model = VisualModel::Particles(e);
        let bytes = postcard::to_allocvec(&model).expect("a declaration encodes");
        let back: VisualModel = postcard::from_bytes(&bytes).expect("and decodes");
        assert_eq!(back, model, "{s:?}: not unchanged by the trip");
    });
}
