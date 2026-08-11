//! `ClientContext` — the guest-side authoring context for a `*_client` (cosmetic)
//! mod, the render-side counterpart to [`ModContext`](crate::context::ModContext).
//!
//! A cosmetic mod declares how content *looks* and how it *moves*: a
//! [`VisualModel`] per unit, a per-ability feedback visual
//! ([`EffectVisualDescriptor`], keyed by an [`EffectRole`]), and an
//! [`AnimationDescriptor`] per unit. Like the gameplay context it names everything
//! with stable strings that intern to dense handles;
//! [`ClientContext::finish`] emits the [`Names`] tables (only `units`,
//! `abilities` and `anim_states` are meaningful to a cosmetic bundle) alongside
//! the accumulated declarations, so the host can map each handle to the gameplay
//! mod's global id by **name** at adoption.
//!
//! Content-free host: the engine ships nothing here — it renders whatever asset a
//! declared [`VisualModel`] names via a `mod://<id>/<path>` URL, falling back to a
//! procedural primitive for anything a mod left undressed.

use alloc::vec::Vec;

use stormlight_mod_abi::animation::{AnimState, AnimationDescriptor};
use stormlight_mod_abi::descriptors::Names;
use stormlight_mod_abi::ids::{AbilityId, AnimStateId, UnitId};
use stormlight_mod_abi::interner::Interner;
use stormlight_mod_abi::manifest::{ABI_VERSION, Version};
use stormlight_mod_abi::visuals::{
    ClientRegistration, EffectRole, EffectVisualDescriptor, VisualDescriptor, VisualModel,
};

use crate::context::table;

/// Accumulates a cosmetic mod's declared visuals and finalizes them into a
/// [`ClientRegistration`]. Interns unit and ability names the same way
/// [`ModContext`](crate::context::ModContext) does, so a separately-authored
/// cosmetic mod dresses a gameplay mod's content purely by shared names.
#[derive(Default)]
pub struct ClientContext {
    unit_names: Interner<UnitId>,
    ability_names: Interner<AbilityId>,
    anim_state_names: Interner<AnimStateId>,

    visuals: Vec<VisualDescriptor>,
    effects: Vec<EffectVisualDescriptor>,
    animations: Vec<AnimationDescriptor>,
}

impl ClientContext {
    /// A fresh, empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare how the unit named `unit` is drawn, returning its interned handle.
    /// Interning is idempotent — re-declaring a unit's visual overrides the earlier
    /// one at adoption (later wins).
    pub fn unit_visual(&mut self, unit: &str, model: VisualModel) -> UnitId {
        let id = self.unit_names.intern(unit);
        self.visuals.push(VisualDescriptor { unit: id, model });
        id
    }

    /// Declare a piece of the ability named `ability`'s feedback in `role` (the
    /// projectile body, the impact burst, or the cast indicator), returning the
    /// ability's interned handle. One ability can declare a visual per role.
    pub fn effect_visual(
        &mut self,
        ability: &str,
        role: EffectRole,
        model: VisualModel,
    ) -> AbilityId {
        let id = self.ability_names.intern(ability);
        self.effects.push(EffectVisualDescriptor { ability: id, role, model });
        id
    }

    /// Intern a mod-defined animation state by name, returning the [`AnimState`]
    /// to bind clips to. Only for states beyond the generic vocabulary
    /// ([`AnimState::Idle`], [`AnimState::Walk`], …) — those the engine drives
    /// itself and needs no name for. Idempotent: one name, one handle.
    pub fn anim_state(&mut self, name: &str) -> AnimState {
        AnimState::Custom(self.anim_state_names.intern(name))
    }

    /// Declare how the unit named `unit` animates, returning its interned handle.
    /// The descriptor's own `unit` field is overwritten with that handle, so an
    /// animation can only ever be keyed to the unit named here.
    ///
    /// Independent of [`unit_visual`](Self::unit_visual) — a mod may animate a
    /// unit it does not dress, or dress one it does not animate. Re-declaring a
    /// unit's animation overrides the earlier one at adoption (later wins).
    pub fn unit_animation(&mut self, unit: &str, mut animation: AnimationDescriptor) -> UnitId {
        let id = self.unit_names.intern(unit);
        animation.unit = id;
        self.animations.push(animation);
        id
    }

    /// The ABI version this context targets (always the SDK's [`ABI_VERSION`]).
    #[must_use]
    pub fn abi(&self) -> Version {
        ABI_VERSION
    }

    /// Finalize into the postcard-ready [`ClientRegistration`]: emit the `units`
    /// and `abilities` name tables from the interners and hand over the declared
    /// visuals.
    #[must_use]
    pub fn finish(self) -> ClientRegistration {
        ClientRegistration {
            abi: ABI_VERSION,
            names: Names {
                units: table(&self.unit_names),
                abilities: table(&self.ability_names),
                anim_states: table(&self.anim_state_names),
                ..Names::default()
            },
            visuals: self.visuals,
            effects: self.effects,
            animations: self.animations,
        }
    }
}
