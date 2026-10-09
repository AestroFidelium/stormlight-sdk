//! How a drawn unit sits on the ground (stormlight/server#179).
//!
//! Invariants:
//!   - **Only a radius a renderer can draw is valid**: finite and positive; fitting
//!     the drawing and having no shadow at all always are;
//!   - **Saying nothing is a shadow that fits the drawing**, so every unit already
//!     declared stands on the ground rather than floating over it;
//!   - **It survives the trip to the host** on the model it belongs to.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::shadow::ModelShadow;
use stormlight_mod_abi::visuals::{ModelClips, VisualModel};

#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Declared {
    Fit,
    /// Hundredths of a world unit, signed so zero and below are covered.
    Radius(#[generator(-100i16..=400)] i16),
    NotANumber,
    None,
}

impl Declared {
    fn shadow(self) -> ModelShadow {
        match self {
            Self::Fit => ModelShadow::Fit,
            Self::Radius(h) => ModelShadow::Radius(f32::from(h) / 100.0),
            Self::NotANumber => ModelShadow::Radius(f32::NAN),
            Self::None => ModelShadow::None,
        }
    }
}

#[test]
fn only_a_drawable_radius_is_valid_and_saying_nothing_fits() {
    check!().with_type::<Declared>().for_each(|d| {
        let shadow = d.shadow();
        let drawable = match shadow {
            ModelShadow::Radius(r) => r.is_finite() && r > 0.0,
            ModelShadow::Fit | ModelShadow::None => true,
        };
        assert_eq!(shadow.is_valid(), drawable, "{d:?}");

        let model = VisualModel::Model {
            asset: "mod://pack/hero.glb".into(),
            scale: 1.0,
            yaw_offset: 0.0,
            launch: None,
            impact: None,
            clips: ModelClips::default(),
            offset: [0.0; 3],
            shadow,
        };
        if drawable {
            let bytes = postcard::to_allocvec(&model).expect("a model encodes");
            let back: VisualModel = postcard::from_bytes(&bytes).expect("and decodes");
            assert_eq!(back, model, "{d:?}");
        }
    });
    assert_eq!(ModelShadow::default(), ModelShadow::Fit, "saying nothing no longer fits");
}
