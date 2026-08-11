//! The client cosmetic ABI — how a `*_client` mod tells the content-free client
//! to *draw* a unit.
//!
//! The gameplay ABI (`Registration`, `descriptors.rs`) is content the **server**
//! interprets; this is the parallel bundle the **client** interprets. A cosmetic
//! mod emits a [`ClientRegistration`]: a set of [`VisualDescriptor`]s, each keyed
//! by a unit's interned handle, saying how an entity spawned from that unit looks.
//! The engine still ships nothing — it renders whatever asset the descriptor
//! names via a `mod://<id>/<path>` URL, and a [`VisualModel::Primitive`] is the
//! asset-free fallback an unknown or missing model degrades to.
//!
//! Pure, serializable data like the rest of the ABI. Handles are authored in the
//! mod's **local** id space and remapped to global at adoption (see
//! [`crate::remap`]); the [`crate::descriptors::Names`] `units` table gives each
//! local unit handle the stable name the host maps to the gameplay unit.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::animation::AnimationDescriptor;
use crate::descriptors::Names;
use crate::ids::{AbilityId, UnitId};
use crate::manifest::Version;

/// A procedural primitive the client can draw with no asset. The graceful
/// fallback a cosmetic mod always has, and what a missing/unknown model degrades
/// to so the field is still *seen* rather than blank.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum PrimitiveShape {
    Cube,
    Sphere,
    Capsule,
}

/// How a unit is drawn. Generic: the engine knows "a tinted primitive", "a 3D
/// model", or "a flat sprite"; what the asset *is* is mod content, addressed by a
/// `mod://<id>/<path>` URL the host resolves within the package root.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum VisualModel {
    /// A procedural primitive tinted `color` (linear RGBA, `0.0..=1.0`). No asset.
    Primitive { shape: PrimitiveShape, color: [f32; 4] },
    /// A 3D model/scene loaded from a `mod://` asset, uniformly scaled, and turned
    /// about `+Y` by `yaw_offset` radians within the unit it dresses.
    ///
    /// The engine faces a unit along its travel direction with **`-Z` forward**
    /// (Bevy's and glTF's own convention), so art authored that way needs no offset
    /// at all — `0.0` is the ordinary case. `yaw_offset` is the escape hatch for art
    /// that faces some other axis: rather than the engine guessing, or the author
    /// re-exporting the asset, the mod states which way its own model looks. A
    /// quarter turn is `FRAC_PI_2`.
    Model { asset: String, scale: f32, yaw_offset: f32 },
    /// A flat, billboarded sprite from a `mod://` asset, sized in world units.
    Sprite { asset: String, size: [f32; 2] },
}

/// The cosmetic descriptor a client mod attaches to a unit: how the client draws
/// an entity spawned from that unit. Keyed by the unit's interned handle (local
/// at authoring, global after adoption).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct VisualDescriptor {
    /// The unit this visual is for.
    pub unit: UnitId,
    /// How to draw it.
    pub model: VisualModel,
}

/// Which piece of ability feedback an [`EffectVisualDescriptor`] dresses. Generic:
/// the engine knows "a flying missile", "a one-shot burst where a shot lands", or
/// "an in-progress cast/channel indicator" — never what ability or hero it is for.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum EffectRole {
    /// The body drawn for a locally-simulated projectile the ability launches.
    Projectile,
    /// The transient burst played where an ability's shot authoritatively lands.
    Impact,
    /// The indicator shown on the caster while a timed cast / channel is running.
    CastIndicator,
}

/// The cosmetic descriptor a client mod attaches to an *ability's feedback*: how
/// the client draws that ability's projectile, impact, or cast indicator. The
/// parallel to [`VisualDescriptor`] (which dresses a *unit*), keyed by the
/// ability's interned handle (local at authoring, global after adoption) plus the
/// [`EffectRole`] it fills, so one ability can declare a visual per role.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct EffectVisualDescriptor {
    /// The ability whose feedback this visual is for.
    pub ability: AbilityId,
    /// Which piece of that ability's feedback it dresses.
    pub role: EffectRole,
    /// How to draw it.
    pub model: VisualModel,
}

/// Everything a *client* (cosmetic) mod registers — the client-side parallel to
/// [`crate::descriptors::Registration`]. Emitted once at `mod_register` on the
/// client's wasm runtime and decoded by the client host.
///
/// `visuals` and `animations` are indexed by the mod's *local* unit handle;
/// `effects` key on a local ability handle plus a role. The [`Names`]
/// `units`/`abilities` tables give each referenced handle a stable name the host
/// maps to the gameplay mod's global id, and `anim_states` names the mod-defined
/// animation states its animations declare. Embeds the
/// [`crate::manifest::ABI_VERSION`] it was built against so the host can reject a
/// major mismatch at decode.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct ClientRegistration {
    /// The ABI the mod was built against; the host rejects a major mismatch.
    pub abi: Version,
    /// Stable names for every interned handle the visuals reference.
    pub names: Names,
    /// The visuals this cosmetic mod declares, one per unit it dresses.
    pub visuals: Vec<VisualDescriptor>,
    /// The ability-feedback visuals this cosmetic mod declares (projectile /
    /// impact / cast indicator), keyed by ability + role.
    pub effects: Vec<EffectVisualDescriptor>,
    /// The animations this cosmetic mod declares, one per unit it animates
    /// (stormlight/server#72). Independent of `visuals`: a mod may dress a unit
    /// without animating it, or animate a unit whose model another mod supplied.
    pub animations: Vec<AnimationDescriptor>,
}
