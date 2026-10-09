//! Authoring a sound (stormlight/server#176).
//!
//! The pdk only spells the declaration, so the properties are about that spelling:
//!   - **the short way is a plain sound**: the file named, played once as
//!     recorded, fading out across a screen — and playable;
//!   - **each refinement sets what it names and nothing else**, in any order;
//!   - **layers keep what they were given, in order**.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::sound::{SoundFalloff, SoundPlayback};
use stormlight_mod_sdk::abi::visuals::{PrimitiveShape, VisualModel};
use stormlight_mod_sdk::sounds::{FAR, NEAR, layered, sound};

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// Hundredths of the recorded volume, if refined.
    volume: Option<u8>,
    looping: bool,
    /// Tenths of a world unit, if refined.
    #[generator(bolero::produce::<Option<(u8, u8)>>())]
    falloff: Option<(u8, u8)>,
    /// Whether it is layered beside a drawing, and on which side.
    beside: Option<bool>,
}

#[test]
fn a_sound_is_declared_as_spelled() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut spec = sound("mod://pack/hit.ogg");
        if let Some(v) = s.volume {
            spec = spec.volume(f32::from(v) / 100.0);
        }
        if s.looping {
            spec = spec.looping();
        }
        if let Some((near, far)) = s.falloff {
            spec = spec.falloff(f32::from(near) / 10.0, f32::from(far) / 10.0);
        }
        let model: VisualModel = spec.into();
        let falloff = s.falloff.map_or(SoundFalloff { near: NEAR, far: FAR }, |(n, f)| {
            SoundFalloff { near: f32::from(n) / 10.0, far: f32::from(f) / 10.0 }
        });
        let expected = VisualModel::Sound {
            asset: "mod://pack/hit.ogg".into(),
            volume: s.volume.map_or(1.0, |v| f32::from(v) / 100.0),
            playback: if s.looping { SoundPlayback::Loop } else { SoundPlayback::Once },
            falloff,
        };
        assert_eq!(model, expected, "{s:?}");
        if s.falloff.is_none() {
            assert!(model.is_drawable(), "{s:?}: the short way is not playable");
        }

        if let Some(first) = s.beside {
            let drawing = VisualModel::Primitive { shape: PrimitiveShape::Sphere, color: [1.0; 4] };
            let parts =
                if first { vec![drawing, model.clone()] } else { vec![model.clone(), drawing] };
            let layers = layered(parts.clone());
            assert_eq!(layers, VisualModel::Layered(parts), "{s:?}");
        }
    });
}
