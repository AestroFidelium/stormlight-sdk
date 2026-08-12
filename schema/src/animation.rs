//! The animation ABI (stormlight/server#72) — what a cosmetic mod declares so the
//! engine can animate a character it knows nothing about.
//!
//! The cosmetic sibling of [`crate::visuals::VisualDescriptor`]: that one says how
//! a unit *looks*, this one says how it *moves*. The engine ships no clip, no
//! skeleton and no state vocabulary beyond the generic states it can drive itself;
//! everything concrete — which container, which clip inside it, which bones —
//! is mod content addressed by name.
//!
//! ## Why layers, not one clip at a time
//!
//! A character routinely needs several animations at once: walking with the legs
//! while the upper body casts, with an additive flinch on top. So the model is a
//! set of **layers**, each with its own bone mask, blend mode and weight, and each
//! running its own small state machine over declared [`AnimState`]s. The engine
//! maps generic game state to a semantic state; the mod maps that state to a clip.
//! The engine never learns a clip name.
//!
//! ## Bone masks are declared once, referenced by index
//!
//! Masks are **descriptor-level** [`MaskGroup`]s that layers point at, rather than
//! a free-form bone set per layer, because the runtime addresses a mask as a bit
//! in a fixed-width set shared by the whole character — hence
//! [`MAX_MASK_GROUPS`]. Declaring the groups once also lets one group (an "upper
//! body") be reused by every layer that cares about it, and makes
//! [`BoneMask::Only`] expressible as the complement of the declared groups.
//!
//! A group names bones by the same **name path** the exporter wrote, and the
//! engine resolves it against the real skeleton when the art arrives. A bone the
//! skeleton does not have costs every layer that masked against its group — a
//! layer that cannot be masked as declared would otherwise drive the whole
//! character, which is the failure nobody would trace back to a renamed bone.
//!
//! Pure, serializable data like the rest of the ABI. The [`crate::ids::UnitId`] it
//! dresses and every mod-defined [`crate::ids::AnimStateId`] are authored in the
//! mod's **local** id space and remapped to global at adoption (see
//! [`crate::remap`]).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::{AnimStateId, UnitId};
use crate::remap::{IdMap, RemapIds};

/// How many [`MaskGroup`]s one character may declare.
///
/// The runtime carries a layer's mask as a fixed 64-bit set and **reserves the
/// last bit for itself**: making [`BoneMask::Only`] mean *only* requires a group
/// holding every bone the mod put in no group at all, and that complement has to
/// be addressable like any other. So 63 are declarable and the 64th is the
/// engine's; a descriptor that declares more is rejected at load rather than
/// silently losing its last groups.
pub const MAX_MASK_GROUPS: usize = 63;

/// A bone, addressed by its name path from the animated root — the same way the
/// renderer identifies an animation target, so a mod names bones exactly as its
/// exporter wrote them (`["Root", "Spine", "Arm.L"]`).
pub type BonePath = Vec<String>;

/// One clip inside a mod's animation container.
///
/// Split in two because a `.glb` holds a whole library: `asset` is the container
/// (a `mod://<id>/<path>` URL), `clip` the animation's name inside it. The engine
/// resolves both without knowing what either means.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ClipRef {
    /// The container this clip lives in, as a `mod://<id>/<path>` URL.
    pub asset: String,
    /// The clip's name within that container.
    pub clip: String,
}

/// What a character is *doing*, as the animation system sees it.
///
/// The generic variants are the vocabulary the engine can drive on its own: it
/// knows when a unit is standing, moving, casting, channeling, taking a hit or
/// dying, so it can pick these without any mod code running. [`Self::Custom`] is
/// the open end — a state a mod defines and its own guest code triggers, interned
/// by name like every other handle.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum AnimState {
    /// Standing still.
    Idle,
    /// Moving at ordinary speed.
    Walk,
    /// Moving fast — a separate state rather than a rate on `Walk`, because the
    /// two are usually different clips.
    Run,
    /// A timed cast is winding up.
    Cast,
    /// A channel is running.
    Channel,
    /// Took damage — typically a one-shot on an additive layer.
    Hit,
    /// Died.
    Death,
    /// A mod-defined state, triggered by that mod's own code.
    Custom(AnimStateId),
}

/// A named set of bones a layer can be restricted to.
///
/// `descendants` is what makes this authorable: an "upper body" group is written
/// as the one spine bone it starts at, not as every bone below it, so the group
/// survives the author adding a finger.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct MaskGroup {
    /// Author-facing name, for diagnostics — layers reference the group by index.
    pub name: String,
    /// The bones in the group.
    pub bones: Vec<BonePath>,
    /// Whether each listed bone brings everything beneath it into the group.
    pub descendants: bool,
}

/// Which bones a layer drives, in terms of the declared [`MaskGroup`]s.
///
/// Both directions exist because both are natural to author and they are not
/// equally safe: [`Self::Only`] is the complement of the declared groups, so a
/// bone in *no* group is not driven by such a layer, while [`Self::Except`] drives
/// everything the mod did not explicitly carve out.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BoneMask {
    /// The whole skeleton — no mask at all.
    Whole,
    /// Only the listed groups; every other declared group is masked out.
    Only(Vec<u16>),
    /// Everything except the listed groups.
    Except(Vec<u16>),
}

impl BoneMask {
    /// The group indices this mask names (empty for [`Self::Whole`]).
    #[must_use]
    pub fn groups(&self) -> &[u16] {
        match self {
            Self::Whole => &[],
            Self::Only(g) | Self::Except(g) => g,
        }
    }
}

/// How a layer composites over the layers beneath it.
///
/// The two modes differ in whether the layer *takes the bones away* from what is
/// underneath, which is the whole of the composition rule (see [`AnimLayer`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BlendMode {
    /// Replaces the pose of the bones it drives.
    Override,
    /// Adds its pose on top — how a flinch or a lean rides whatever is already
    /// playing without cancelling it.
    Additive,
}

/// How fast a clip plays.
///
/// The bound variants exist so a clip stays in step with the thing it depicts
/// rather than drifting: a run cycle sliding against the ground, or a wind-up that
/// finishes after the ability already fired, are the two failures this prevents.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum RateBinding {
    /// A constant multiple of the clip's authored rate.
    Fixed(f32),
    /// Scaled by movement: the clip plays at its authored rate when the unit moves
    /// at `reference_speed` world units per second, and proportionally faster or
    /// slower otherwise.
    MoveSpeed {
        /// The speed the cycle was authored for. Must be non-zero to divide by.
        reference_speed: f32,
    },
    /// Time-scaled to the cast or channel currently running, so the clip ends
    /// exactly when the ability resolves however long the declared cast time is.
    CastDuration,
}

impl RateBinding {
    /// Whether every number in the binding is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        match self {
            Self::Fixed(v) => v.is_finite(),
            Self::MoveSpeed { reference_speed } => reference_speed.is_finite(),
            Self::CastDuration => true,
        }
    }
}

/// What one [`AnimState`] plays on one layer, and how it enters and leaves.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct StateClip {
    /// The state this binding answers for. At most one binding per state per
    /// layer.
    pub state: AnimState,
    /// The clip to play in that state.
    pub clip: ClipRef,
    /// Whether the clip repeats (a locomotion cycle) or plays once (a flinch).
    pub looping: bool,
    /// Seconds to fade in when this state takes the layer.
    pub blend_in: f32,
    /// Seconds to fade out when it loses the layer.
    pub blend_out: f32,
    /// How fast it plays.
    pub rate: RateBinding,
    /// Which binding takes the layer when two states claim it at once — higher
    /// wins. Ties are resolved by declaration order, so a mod that does not care
    /// can leave every priority at zero and still get a deterministic result.
    pub priority: i16,
}

/// A per-pair override of the blend time between two of a layer's states — for
/// the specific hand-off that needs to be quicker or softer than the states'
/// own declared defaults.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Transition {
    /// The state being left. Must be declared on the same layer.
    pub from: AnimState,
    /// The state being entered. Must be declared on the same layer.
    pub to: AnimState,
    /// Seconds to blend across, replacing `to`'s own `blend_in` for this pair.
    pub blend_in: f32,
}

/// One layer of a character's animation: a masked, weighted state machine.
///
/// ## Composition order
///
/// Layers are ordered, and the order means exactly two things:
///
/// 1. **A later [`BlendMode::Override`] layer owns the bones it drives.** Every
///    override layer declared before it is masked out of those bones, so an
///    upper-body cast really does replace the upper body while the locomotion
///    layer underneath keeps the legs — rather than the two being averaged.
/// 2. **[`BlendMode::Additive`] layers ride on top of the whole override stack**,
///    in declaration order among themselves. They take no bones away from
///    anything, which is what lets a flinch ride whatever is already playing.
///
/// So a layer that drives a bone no later override layer claims is the one seen on
/// that bone, plus whatever additive layers add to it.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AnimLayer {
    /// Author-facing name, for diagnostics.
    pub name: String,
    /// Which bones this layer drives.
    pub mask: BoneMask,
    /// How it composites over the layers beneath it.
    pub blend: BlendMode,
    /// Its base weight — the ceiling a state's blend-in fades up to.
    pub weight: f32,
    /// The states it can play, one binding each.
    pub states: Vec<StateClip>,
    /// Per-pair blend-time overrides between those states.
    pub transitions: Vec<Transition>,
}

/// Everything a cosmetic mod says about how one unit animates. Keyed by the
/// unit's interned handle (local at authoring, global after adoption), exactly
/// like the unit's visual.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AnimationDescriptor {
    /// The unit this animation is for.
    pub unit: UnitId,
    /// The bone groups its layers may mask against, referenced by index.
    pub mask_groups: Vec<MaskGroup>,
    /// Its layers, in composite order.
    pub layers: Vec<AnimLayer>,
}

/// Why an [`AnimationDescriptor`] cannot be animated as declared.
///
/// Each variant is a break the renderer would otherwise absorb silently — a layer
/// that plays nothing, a transition to a state that does not exist — leaving the
/// author with a still character and no explanation. Indices point at the layer,
/// state binding, or mask group at fault.
#[derive(Clone, PartialEq, Debug)]
pub enum AnimationError {
    /// No layers at all: nothing to play.
    NoLayers,
    /// A layer binds no states.
    EmptyLayer { layer: u16 },
    /// A layer binds the same state twice; which clip wins would be arbitrary.
    DuplicateState { layer: u16, state: AnimState },
    /// A transition names a state its layer never declared.
    UnknownTransitionState { layer: u16, state: AnimState },
    /// A layer's mask names a group index past the declared groups.
    UnknownMaskGroup { layer: u16, group: u16 },
    /// More mask groups than the runtime's bitset can address.
    TooManyMaskGroups { found: usize },
    /// A mask group with no bones, or a bone with an empty name path.
    EmptyMaskGroup { group: u16 },
    /// A clip with an empty container path or an empty clip name.
    EmptyClipRef { layer: u16, state: u16 },
    /// A non-finite weight, blend duration, or playback rate on a layer.
    NonFinite { layer: u16 },
}

impl fmt::Display for AnimationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLayers => f.write_str("animation declares no layers"),
            Self::EmptyLayer { layer } => write!(f, "layer {layer} binds no states"),
            Self::DuplicateState { layer, state } => {
                write!(f, "layer {layer} binds {state:?} more than once")
            }
            Self::UnknownTransitionState { layer, state } => {
                write!(f, "layer {layer} transitions through undeclared state {state:?}")
            }
            Self::UnknownMaskGroup { layer, group } => {
                write!(f, "layer {layer} masks undeclared bone group {group}")
            }
            Self::TooManyMaskGroups { found } => {
                write!(f, "{found} bone groups declared, at most {MAX_MASK_GROUPS} are addressable")
            }
            Self::EmptyMaskGroup { group } => write!(f, "bone group {group} names no bones"),
            Self::EmptyClipRef { layer, state } => {
                write!(f, "layer {layer} state {state} names an empty clip")
            }
            Self::NonFinite { layer } => write!(f, "layer {layer} carries a non-finite number"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AnimationError {}

impl AnimationDescriptor {
    /// Full structural validation, run by the host at load.
    ///
    /// Checks only what is decidable from the descriptor itself. Whether a
    /// [`AnimState::Custom`] handle has a name-table entry, and whether the named
    /// container actually holds the named clip, belong to adoption and to asset
    /// loading respectively — neither is knowable here.
    pub fn validate(&self) -> Result<(), AnimationError> {
        if self.mask_groups.len() > MAX_MASK_GROUPS {
            return Err(AnimationError::TooManyMaskGroups { found: self.mask_groups.len() });
        }
        for (g, group) in self.mask_groups.iter().enumerate() {
            let empty_bone = |b: &BonePath| b.is_empty() || b.iter().any(String::is_empty);
            if group.bones.is_empty() || group.bones.iter().any(empty_bone) {
                return Err(AnimationError::EmptyMaskGroup { group: g as u16 });
            }
        }
        if self.layers.is_empty() {
            return Err(AnimationError::NoLayers);
        }
        for (l, layer) in self.layers.iter().enumerate() {
            let index = l as u16;
            if layer.states.is_empty() {
                return Err(AnimationError::EmptyLayer { layer: index });
            }
            if !layer.weight.is_finite() {
                return Err(AnimationError::NonFinite { layer: index });
            }
            for &group in layer.mask.groups() {
                if usize::from(group) >= self.mask_groups.len() {
                    return Err(AnimationError::UnknownMaskGroup { layer: index, group });
                }
            }
            for (s, binding) in layer.states.iter().enumerate() {
                if binding.clip.asset.is_empty() || binding.clip.clip.is_empty() {
                    return Err(AnimationError::EmptyClipRef { layer: index, state: s as u16 });
                }
                if !binding.blend_in.is_finite()
                    || !binding.blend_out.is_finite()
                    || !binding.rate.is_finite()
                {
                    return Err(AnimationError::NonFinite { layer: index });
                }
                // Quadratic, over a handful of bindings — and allocation-free,
                // which a set would not be on a `no_std` guest.
                if layer.states[..s].iter().any(|other| other.state == binding.state) {
                    return Err(AnimationError::DuplicateState {
                        layer: index,
                        state: binding.state,
                    });
                }
            }
            for transition in &layer.transitions {
                if !transition.blend_in.is_finite() {
                    return Err(AnimationError::NonFinite { layer: index });
                }
                for state in [transition.from, transition.to] {
                    if !layer.states.iter().any(|binding| binding.state == state) {
                        return Err(AnimationError::UnknownTransitionState { layer: index, state });
                    }
                }
            }
        }
        Ok(())
    }
}

impl RemapIds for AnimState {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        if let Self::Custom(id) = self {
            *id = m.anim_state(*id)?;
        }
        Ok(())
    }
}

impl RemapIds for StateClip {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        // `clip`, the timings, the rate and the priority are asset strings and
        // plain numbers — the state is the only handle here.
        self.state.remap_ids(m)
    }
}

impl RemapIds for Transition {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.from.remap_ids(m)?;
        self.to.remap_ids(m)?;
        Ok(())
    }
}

impl RemapIds for AnimLayer {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        // `mask` indexes the descriptor's own group list, not an interned family,
        // so it travels with the descriptor and is never rewritten.
        self.states.remap_ids(m)?;
        self.transitions.remap_ids(m)?;
        Ok(())
    }
}

impl RemapIds for AnimationDescriptor {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.unit = m.unit(self.unit)?;
        // `mask_groups` are bone names — content, not handles.
        self.layers.remap_ids(m)
    }
}
