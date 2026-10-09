//! Declaring a sound (stormlight/server#176) — the authoring half.
//!
//! A sound is a [`VisualModel`] like any drawing, so it goes wherever one does: an
//! ability's impact, an animation notify, a status. Spell it with [`sound`], and
//! put it beside a drawing on the same moment with [`layered`]:
//!
//! ```ignore
//! use stormlight_mod_sdk::sounds::{layered, sound};
//!
//! let crack = sound(asset("sounds/frost_hit.ogg")).volume(0.8).falloff(4.0, 30.0);
//! ctx.declare_effect(effect("frost_bolt", EffectRole::Impact, layered([burst, crack.into()])));
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use stormlight_mod_abi::sound::{SoundFalloff, SoundPlayback};
use stormlight_mod_abi::visuals::VisualModel;

/// A sound being declared — see [`sound`]. Becomes a [`VisualModel`] with
/// `.into()`.
#[derive(Clone, Debug, PartialEq)]
#[must_use = "a sound is declared only once it is handed to a declaration"]
pub struct SoundSpec {
    asset: String,
    volume: f32,
    playback: SoundPlayback,
    falloff: SoundFalloff,
}

/// Heard at full volume within this many world units by default.
pub const NEAR: f32 = 8.0;
/// Not heard past this many by default: about a screen's width away.
pub const FAR: f32 = 40.0;

/// The sound in `asset`, played once as recorded, fading out across a screen.
pub fn sound(asset: impl Into<String>) -> SoundSpec {
    SoundSpec {
        asset: asset.into(),
        volume: 1.0,
        playback: SoundPlayback::Once,
        falloff: SoundFalloff { near: NEAR, far: FAR },
    }
}

impl SoundSpec {
    /// A linear gain over the file as recorded.
    pub fn volume(mut self, volume: f32) -> Self {
        self.volume = volume;
        self
    }

    /// Played over and over for as long as what holds it lasts.
    pub fn looping(mut self) -> Self {
        self.playback = SoundPlayback::Loop;
        self
    }

    /// Full volume within `near` world units of the listener, silent past `far`.
    pub fn falloff(mut self, near: f32, far: f32) -> Self {
        self.falloff = SoundFalloff { near, far };
        self
    }
}

impl From<SoundSpec> for VisualModel {
    fn from(spec: SoundSpec) -> Self {
        VisualModel::Sound {
            asset: spec.asset,
            volume: spec.volume,
            playback: spec.playback,
            falloff: spec.falloff,
        }
    }
}

/// Several drawings — and sounds — at one place, sharing its life.
pub fn layered(layers: impl IntoIterator<Item = VisualModel>) -> VisualModel {
    VisualModel::Layered(layers.into_iter().collect::<Vec<_>>())
}
