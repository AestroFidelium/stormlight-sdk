//! `ClientContext` — the guest-side authoring context for a `*_client` (cosmetic)
//! mod, the render-side counterpart to [`ModContext`](crate::context::ModContext).
//!
//! A cosmetic mod declares how content *looks*, how it *moves*, and what the
//! player *reads*: a [`VisualModel`] per unit, a per-ability feedback visual
//! ([`EffectVisualDescriptor`], keyed by an [`EffectRole`]), an
//! [`AnimationDescriptor`] per unit, and the widget trees of its interface
//! ([`UiRoot`], server#66). Like the gameplay context it names everything with
//! stable strings that intern to dense handles; [`ClientContext::finish`] emits
//! the [`Names`] tables (a cosmetic bundle fills `units`, `abilities`,
//! `anim_states`, `events`, and — for what its HUD binds to — `stats`,
//! `resources` and `stacks`) alongside the accumulated declarations, so the host
//! can map each handle to the gameplay mod's global id by **name** at adoption.
//!
//! Content-free host: the engine ships nothing here — it renders whatever asset a
//! declared [`VisualModel`] names via a `mod://<id>/<path>` URL, falling back to a
//! procedural primitive for anything a mod left undressed.

use alloc::vec::Vec;

use stormlight_mod_abi::animation::{AnimState, AnimationDescriptor};
use stormlight_mod_abi::descriptors::Names;
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, EventId, ResourceId, StackId, StatId, UnitId,
};
use stormlight_mod_abi::interner::Interner;
use stormlight_mod_abi::manifest::{ABI_VERSION, Version};
use stormlight_mod_abi::ui::{RootVisibility, UiRoot, UiSubject, Widget};
use stormlight_mod_abi::visuals::{
    ClientRegistration, EffectRole, EffectVisualDescriptor, NamedEffect, VisualDescriptor,
    VisualModel,
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
    event_names: Interner<EventId>,
    stat_names: Interner<StatId>,
    resource_names: Interner<ResourceId>,
    stack_names: Interner<StackId>,

    visuals: Vec<VisualDescriptor>,
    effects: Vec<EffectVisualDescriptor>,
    animations: Vec<AnimationDescriptor>,
    named_effects: Vec<NamedEffect>,
    ui: Vec<UiRoot>,
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

    /// Declare a cosmetic effect under a name of this mod's own choosing, for an
    /// animation notify to spawn (server#76) — a footstep puff, a weapon trail.
    ///
    /// Named rather than keyed by an ability, because these belong to no ability:
    /// the name is resolved within this mod alone, the way a clip name is resolved
    /// within its container. Re-declaring a name overrides the earlier model at
    /// adoption (later wins).
    pub fn notify_effect(&mut self, name: &str, model: VisualModel) {
        self.named_effects.push(NamedEffect { name: name.into(), model });
    }

    /// Intern a mod-defined event by name, returning the handle a
    /// [`NotifyAction::Trigger`](stormlight_mod_abi::notify::NotifyAction::Trigger)
    /// names. Idempotent: one name, one handle.
    ///
    /// Nothing dispatches these yet — a cosmetic mod is registered but never
    /// invoked at runtime — and the client reports each declared trigger as inert
    /// rather than leaving an author waiting for a reaction that cannot come.
    pub fn notify_event(&mut self, name: &str) -> EventId {
        self.event_names.intern(name)
    }

    /// Declare a widget tree (server#66): what it is called, when the client
    /// shows it, whose state its bindings read, and the tree itself.
    ///
    /// The interface counterpart of [`unit_visual`](Self::unit_visual) — roots
    /// accumulate in declaration order, and nothing here is keyed by a handle:
    /// a HUD belongs to the *player*, not to a unit. Build the tree with the
    /// constructors in [`crate::ui`].
    pub fn ui(&mut self, name: &str, when: RootVisibility, subject: UiSubject, root: Widget) {
        self.ui.push(UiRoot { name: name.into(), when, subject, root });
    }

    /// Intern a unit stat by name, returning the handle a
    /// [`ValueBinding::Stat`](stormlight_mod_abi::ui::ValueBinding::Stat) names.
    /// The same name the gameplay mod interned, so adoption maps the two onto one
    /// global handle. Idempotent: one name, one handle.
    pub fn stat(&mut self, name: &str) -> StatId {
        self.stat_names.intern(name)
    }

    /// Intern a resource pool by name, returning the handle a
    /// [`PoolRef::Resource`](stormlight_mod_abi::impacts::PoolRef::Resource)
    /// binding names. Idempotent: one name, one handle.
    pub fn resource(&mut self, name: &str) -> ResourceId {
        self.resource_names.intern(name)
    }

    /// Intern a stack counter by name, returning the handle a
    /// [`PoolRef::Stacks`](stormlight_mod_abi::impacts::PoolRef::Stacks) binding
    /// names. Idempotent: one name, one handle.
    pub fn stack(&mut self, name: &str) -> StackId {
        self.stack_names.intern(name)
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
                events: table(&self.event_names),
                stats: table(&self.stat_names),
                resources: table(&self.resource_names),
                stacks: table(&self.stack_names),
                ..Names::default()
            },
            visuals: self.visuals,
            effects: self.effects,
            animations: self.animations,
            named_effects: self.named_effects,
            ui: self.ui,
        }
    }
}
