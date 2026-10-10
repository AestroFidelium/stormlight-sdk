//! Authoring a particle emitter (stormlight/server#141).
//!
//! The pdk only spells the declaration: **the output equals the emitter spelled
//! out in full** for any combination of refinements, the short way is playable,
//! and keys keep the order they were given in.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::particles::{
    ColorKey, EmitterShape, ParticleBlend, ParticleEmission, ParticleEmitter, ParticleSpray,
};
use stormlight_mod_sdk::abi::visuals::VisualModel;
use stormlight_mod_sdk::particles::{CAPACITY, SPREAD, emitter};

fn tenth(v: u8) -> f32 {
    f32::from(v) / 10.0
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Spray {
    Untold,
    Cone(u8),
    Radial,
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Blend {
    Untold,
    Additive,
    Multiply,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    #[generator(1u32..=40)]
    burst: u32,
    sphere: Option<u8>,
    lifetime: Option<(u8, u8)>,
    speed: Option<(u8, u8)>,
    spray: Spray,
    gravity: Option<i8>,
    drag: Option<u8>,
    #[generator(bolero::produce::<Vec<(u8, u8)>>().with().len(0usize..=3))]
    sizes: Vec<(u8, u8)>,
    #[generator(bolero::produce::<Vec<(u8, u8)>>().with().len(0usize..=3))]
    colors: Vec<(u8, u8)>,
    textured: bool,
    blend: Blend,
    world_space: bool,
    capacity: Option<u16>,
}

#[test]
fn an_emitter_is_the_declaration_spelled_out() {
    check!().with_type::<Scenario>().for_each(|s| {
        let emission = ParticleEmission::Burst { count: s.burst, delay: 0.0 };
        let mut e = emitter(emission);
        if let Some(r) = s.sphere {
            e = e.shape(EmitterShape::Sphere { radius: tenth(r), hollow: 0.0 });
        }
        if let Some((a, b)) = s.lifetime {
            e = e.lifetime(tenth(a), tenth(b));
        }
        if let Some((a, b)) = s.speed {
            e = e.speed(tenth(a), tenth(b));
        }
        e = match s.spray {
            Spray::Untold => e,
            Spray::Cone(spread) => e.cone(tenth(spread)),
            Spray::Radial => e.radial(),
        };
        if let Some(g) = s.gravity {
            e = e.gravity(f32::from(g));
        }
        if let Some(d) = s.drag {
            e = e.drag(tenth(d));
        }
        for &(at, w) in &s.sizes {
            e = e.size(tenth(at), tenth(w));
        }
        for &(at, red) in &s.colors {
            e = e.color(tenth(at), [tenth(red), 0.5, 1.0, 1.0]);
        }
        if s.textured {
            e = e.texture("mod://pack/spark.png");
        }
        e = match s.blend {
            Blend::Untold => e,
            Blend::Additive => e.additive(),
            Blend::Multiply => e.multiply(),
        };
        if s.world_space {
            e = e.world_space();
        }
        if let Some(c) = s.capacity {
            e = e.capacity(u32::from(c));
        }
        let built: VisualModel = e.into();

        let spelled = VisualModel::Particles(ParticleEmitter {
            shape: s.sphere.map_or(EmitterShape::Point, |r| EmitterShape::Sphere {
                radius: tenth(r),
                hollow: 0.0,
            }),
            emission,
            lifetime: s.lifetime.map_or([1.0, 1.0], |(a, b)| [tenth(a), tenth(b)]),
            speed: s.speed.map_or([1.0, 1.0], |(a, b)| [tenth(a), tenth(b)]),
            spray: match s.spray {
                Spray::Untold => ParticleSpray::Cone { spread: SPREAD },
                Spray::Cone(spread) => ParticleSpray::Cone { spread: tenth(spread) },
                Spray::Radial => ParticleSpray::Radial,
            },
            gravity: s.gravity.map_or(0.0, f32::from),
            drag: s.drag.map_or(0.0, tenth),
            size: s.sizes.iter().map(|&(at, w)| [tenth(at), tenth(w)]).collect(),
            color: s
                .colors
                .iter()
                .map(|&(at, red)| ColorKey { at: tenth(at), rgba: [tenth(red), 0.5, 1.0, 1.0] })
                .collect(),
            texture: s.textured.then(|| "mod://pack/spark.png".into()),
            blend: match s.blend {
                Blend::Untold => ParticleBlend::Blend,
                Blend::Additive => ParticleBlend::Add,
                Blend::Multiply => ParticleBlend::Multiply,
            },
            world_space: s.world_space,
            capacity: s.capacity.map_or(CAPACITY, u32::from),
        });
        assert_eq!(built, spelled, "{s:?}");
    });
}

#[test]
fn the_short_way_is_playable() {
    let model: VisualModel = emitter(ParticleEmission::Stream { rate: 20.0 }).into();
    assert!(model.is_drawable(), "{model:?}");
}
