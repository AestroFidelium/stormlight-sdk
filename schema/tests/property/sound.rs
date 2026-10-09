//! A sound as one more thing a cosmetic declaration lays down, and layers of
//! drawings at one place (stormlight/server#176). Laws, over any declaration:
//!   - **a sound is at full volume within its near distance, silent past its far
//!     one, and never louder further away** — the gain is in `0..=1` and does not
//!     rise with distance;
//!   - **a falloff a clock cannot run is refused**: negative, non-finite, a far
//!     edge nearer than the near one, or one that silences everything;
//!   - **a sound is playable** with a picture-less asset path, a finite volume of
//!     zero or more and a sound falloff;
//!   - **layers are playable when there are some and every one is**, and they
//!     read back flat, in declared order, with the first model among them found.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::sound::{SoundFalloff, SoundPlayback};
use stormlight_mod_abi::visuals::{ModelClips, PrimitiveShape, VisualModel};

#[derive(Debug, Clone, Copy, TypeGenerator)]
struct Falloff {
    /// Tenths of a world unit.
    #[generator(0u16..=400)]
    near: u16,
    #[generator(0u16..=400)]
    far: u16,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    falloff: Falloff,
    /// Tenths of a world unit, two distances to compare.
    #[generator(0u16..=600)]
    a: u16,
    #[generator(0u16..=600)]
    b: u16,
}

fn tenth(v: u16) -> f32 {
    f32::from(v) / 10.0
}

#[test]
fn a_sound_fades_with_distance_and_never_grows_louder() {
    check!().with_type::<Scenario>().for_each(|s| {
        let falloff = SoundFalloff { near: tenth(s.falloff.near), far: tenth(s.falloff.far) };
        assert_eq!(
            falloff.is_valid(),
            s.falloff.near <= s.falloff.far && s.falloff.far > 0,
            "{s:?}",
        );
        if !falloff.is_valid() {
            return;
        }
        let (near, far) = (tenth(s.a.min(s.b)), tenth(s.a.max(s.b)));
        let (loud, quiet) = (falloff.gain_at(near), falloff.gain_at(far));
        assert!((0.0..=1.0).contains(&loud) && (0.0..=1.0).contains(&quiet), "{s:?}");
        assert!(quiet <= loud, "{s:?}: louder at {far} than at {near}");
        if near <= falloff.near {
            assert_eq!(loud, 1.0, "{s:?}: quieter than full within reach");
        }
        if far >= falloff.far && falloff.far > falloff.near {
            assert_eq!(quiet, 0.0, "{s:?}: still heard past its far edge");
        }
    });
}

#[test]
fn a_falloff_no_clock_can_run_is_refused() {
    for (near, far) in [(-1.0, 4.0), (2.0, 1.0), (0.0, 0.0), (f32::NAN, 3.0), (1.0, f32::INFINITY)]
    {
        assert!(!SoundFalloff { near, far }.is_valid(), "({near}, {far})");
    }
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Fault {
    Sound,
    NoAsset,
    NegativeVolume,
    InfiniteVolume,
    BrokenFalloff,
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Layer {
    Primitive,
    Model,
    Sound(Fault),
}

#[derive(Debug, TypeGenerator)]
struct Layers {
    #[generator(bolero::produce::<Vec<Layer>>().with().len(0usize..=4))]
    outer: Vec<Layer>,
    #[generator(bolero::produce::<Vec<Layer>>().with().len(0usize..=3))]
    nested: Vec<Layer>,
}

fn sound(fault: Fault, i: usize) -> VisualModel {
    VisualModel::Sound {
        asset: if matches!(fault, Fault::NoAsset) {
            String::new()
        } else {
            format!("mod://p/s{i}.ogg")
        },
        volume: match fault {
            Fault::NegativeVolume => -0.5,
            Fault::InfiniteVolume => f32::INFINITY,
            _ => 0.8,
        },
        playback: if i.is_multiple_of(2) { SoundPlayback::Once } else { SoundPlayback::Loop },
        falloff: if matches!(fault, Fault::BrokenFalloff) {
            SoundFalloff { near: 5.0, far: 1.0 }
        } else {
            SoundFalloff { near: 2.0, far: 20.0 }
        },
    }
}

fn layer(l: Layer, i: usize) -> VisualModel {
    match l {
        Layer::Primitive => VisualModel::Primitive { shape: PrimitiveShape::Cube, color: [1.0; 4] },
        Layer::Model => VisualModel::Model {
            asset: format!("mod://p/m{i}.glb"),
            scale: 1.0,
            yaw_offset: 0.0,
            launch: None,
            impact: None,
            clips: ModelClips::default(),
            offset: [0.0; 3],
            shadow: Default::default(),
        },
        Layer::Sound(fault) => sound(fault, i),
    }
}

#[test]
fn layers_play_when_every_layer_does_and_read_back_flat_in_order() {
    check!().with_type::<Layers>().for_each(|s| {
        let outer: Vec<VisualModel> =
            s.outer.iter().enumerate().map(|(i, l)| layer(*l, i)).collect();
        let nested: Vec<VisualModel> =
            s.nested.iter().enumerate().map(|(i, l)| layer(*l, 10 + i)).collect();
        let mut parts = outer.clone();
        parts.push(VisualModel::Layered(nested.clone()));
        let model = VisualModel::Layered(parts);

        let leaves: Vec<&VisualModel> = outer.iter().chain(&nested).collect();
        assert_eq!(model.layers(), leaves, "{s:?}: not flat and in order");
        let first_model = leaves.iter().copied().find(|m| matches!(m, VisualModel::Model { .. }));
        assert_eq!(model.model_part(), first_model, "{s:?}");

        let sound_ok = |l: &Layer| !matches!(l, Layer::Sound(f) if !matches!(f, Fault::Sound));
        let playable = s.outer.iter().chain(&s.nested).all(sound_ok) && !s.nested.is_empty();
        assert_eq!(model.is_drawable(), playable, "{s:?}");
        assert!(!VisualModel::Layered(Vec::new()).is_drawable(), "empty layers draw nothing");
    });
}
