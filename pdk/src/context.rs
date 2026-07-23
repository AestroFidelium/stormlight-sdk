//! `ModContext` — the guest-side registration context a mod builds up, then
//! finalizes into a [`Registration`].
//!
//! Per the locked id model (O-3: one concrete handle-based ISA; authoring
//! strings intern to handles), a mod names its content with stable strings. The
//! context interns each name into a per-family [`Interner`] — reusing the schema
//! interner so the guest and host share one interning rule — minting the same
//! dense `Handle`s the descriptors carry. [`ModContext::finish`] then emits the
//! [`Names`] tables (the interners' contents) alongside the accumulated
//! descriptors, keeping `descriptor[i].id == Handle(i)` by construction.

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use stormlight_mod_abi::abilities::AbilityDescriptor;
use stormlight_mod_abi::behaviors::BuffSpec;
use stormlight_mod_abi::descriptors::{Curve, Names, Registration};
use stormlight_mod_abi::ids::{
    AbilityId, BuffId, CurveId, DamageTypeId, DimId, EventId, Handle, HandlerId, ParamId,
    ResourceId, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::interner::Interner;
use stormlight_mod_abi::manifest::{ABI_VERSION, Version};
use stormlight_mod_abi::runtime::{GuestEffects, TickContext, TriggerContext};
use stormlight_mod_abi::talents::TalentDescriptor;
use stormlight_mod_abi::units::UnitDescriptor;

use crate::runtime::HandlerCall;

/// A `Custom`-handler closure: a pure function of its invoking [`HandlerCall`].
type HandlerFn = Box<dyn Fn(&HandlerCall) -> GuestEffects>;
/// The per-tick closure: a pure function of the [`TickContext`].
type TickFn = Box<dyn Fn(&TickContext) -> GuestEffects>;
/// A trigger closure: a pure function of the [`TriggerContext`].
type TriggerFn = Box<dyn Fn(&TriggerContext) -> GuestEffects>;

/// Accumulates a mod's registrations and finalizes them into a [`Registration`].
#[derive(Default)]
pub struct ModContext {
    stats: Interner<StatId>,
    resources: Interner<ResourceId>,
    stacks: Interner<StackId>,
    tags: Interner<TagId>,
    tag_class_names: Interner<TagClassId>,
    params: Interner<ParamId>,
    events: Interner<EventId>,
    buff_names: Interner<BuffId>,
    curve_names: Interner<CurveId>,
    damage_types: Interner<DamageTypeId>,
    dims: Interner<DimId>,
    ability_names: Interner<AbilityId>,
    talent_names: Interner<TalentId>,
    handlers: Interner<HandlerId>,
    unit_names: Interner<UnitId>,

    abilities: Vec<AbilityDescriptor>,
    talents: Vec<TalentDescriptor>,
    buffs: Vec<BuffSpec>,
    tag_classes: Vec<(TagId, TagClassId)>,
    curves: Vec<Curve>,
    units: Vec<UnitDescriptor>,

    // --- Runtime dispatch tables (guest code, not serialized) ---
    // These hold the author's entry-point closures. They are *not* part of the
    // emitted [`Registration`]; the guest reconstructs them by re-running the
    // builder on every invocation, keeping a guest stateless across host calls.
    handler_fns: Vec<Option<HandlerFn>>,
    tick_fn: Option<TickFn>,
    trigger_fns: Vec<(EventId, TriggerFn)>,
}

/// Collect an interner's contents as a dense `raw index -> name` table. Shared
/// with the client (cosmetic) context ([`crate::client`]).
pub(crate) fn table<H: Handle>(interner: &Interner<H>) -> Vec<String> {
    (0..interner.len() as u32)
        .map(|raw| {
            interner
                .resolve(H::from_raw(raw))
                .expect("dense interner has no gaps")
                .to_string()
        })
        .collect()
}

impl ModContext {
    /// A fresh, empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // --- Name-only id families: intern a stable string, get its handle. ---

    /// Intern a stat name.
    pub fn stat(&mut self, name: &str) -> StatId {
        self.stats.intern(name)
    }
    /// Intern a resource-pool name.
    pub fn resource(&mut self, name: &str) -> ResourceId {
        self.resources.intern(name)
    }
    /// Intern a stack-counter name.
    pub fn stack(&mut self, name: &str) -> StackId {
        self.stacks.intern(name)
    }
    /// Intern a status-tag name.
    pub fn tag(&mut self, name: &str) -> TagId {
        self.tags.intern(name)
    }
    /// Intern a capability-class name.
    pub fn tag_class(&mut self, name: &str) -> TagClassId {
        self.tag_class_names.intern(name)
    }
    /// Intern an ability/descriptor parameter name.
    pub fn param(&mut self, name: &str) -> ParamId {
        self.params.intern(name)
    }
    /// Intern a custom-event name.
    pub fn event(&mut self, name: &str) -> EventId {
        self.events.intern(name)
    }
    /// Intern a damage-type name.
    pub fn damage_type(&mut self, name: &str) -> DamageTypeId {
        self.damage_types.intern(name)
    }
    /// Intern a dimension name.
    pub fn dim(&mut self, name: &str) -> DimId {
        self.dims.intern(name)
    }
    /// Intern a `Custom` handler name.
    pub fn handler(&mut self, name: &str) -> HandlerId {
        self.handlers.intern(name)
    }

    // --- Runtime entry points: register the *code* a guest runs after load. ---

    /// Register a `Custom`-impact handler under `name`, returning the [`HandlerId`]
    /// to embed in `Impact::Custom { handler, .. }`. Interning the name and storing
    /// the closure in one call keeps the id and the code in lock-step. `f` must be
    /// a pure function of its [`HandlerCall`] (the stateless-guest contract).
    pub fn on_handler<F>(&mut self, name: &str, f: F) -> HandlerId
    where
        F: Fn(&HandlerCall) -> GuestEffects + 'static,
    {
        let id = self.handlers.intern(name);
        let idx = id.raw() as usize;
        if idx >= self.handler_fns.len() {
            self.handler_fns.resize_with(idx + 1, || None);
        }
        self.handler_fns[idx] = Some(Box::new(f));
        id
    }

    /// Register the per-tick entry point (the `mod_tick` export). At most one; a
    /// second call replaces the first.
    pub fn on_tick<F>(&mut self, f: F)
    where
        F: Fn(&TickContext) -> GuestEffects + 'static,
    {
        self.tick_fn = Some(Box::new(f));
    }

    /// Register a trigger handler for a mod-defined `event` (the `mod_trigger`
    /// export). The event is the same [`EventId`] an `Impact::Emit` raises.
    pub fn on_trigger<F>(&mut self, event: EventId, f: F)
    where
        F: Fn(&TriggerContext) -> GuestEffects + 'static,
    {
        self.trigger_fns.push((event, Box::new(f)));
    }

    /// Dispatch a `Custom` handler by its (local) [`HandlerId`]. An id with no
    /// registered closure yields [`GuestEffects::none`] — total, never a panic, so
    /// a stale descriptor cannot crash the guest.
    #[must_use]
    pub fn run_handler(&self, id: HandlerId, call: &HandlerCall) -> GuestEffects {
        self.handler_fns
            .get(id.raw() as usize)
            .and_then(Option::as_ref)
            .map_or_else(GuestEffects::none, |f| f(call))
    }

    /// Dispatch the per-tick entry. No `on_tick` registered ⇒ no effects.
    #[must_use]
    pub fn run_tick(&self, ctx: &TickContext) -> GuestEffects {
        self.tick_fn.as_ref().map_or_else(GuestEffects::none, |f| f(ctx))
    }

    /// Dispatch the trigger entry for `ctx.event`. No matching trigger ⇒ no
    /// effects. First match wins (registration order).
    #[must_use]
    pub fn run_trigger(&self, ctx: &TriggerContext) -> GuestEffects {
        self.trigger_fns
            .iter()
            .find(|(event, _)| *event == ctx.event)
            .map_or_else(GuestEffects::none, |(_, f)| f(ctx))
    }

    /// Register `tag` into capability `class`, interning both names. The pair is
    /// what the host's `TagRegistry` adopts.
    pub fn register_tag_class(&mut self, tag: &str, class: &str) -> (TagId, TagClassId) {
        let t = self.tag(tag);
        let c = self.tag_class(class);
        self.tag_classes.push((t, c));
        (t, c)
    }

    // --- Descriptor families: define a named item, get its handle back. The
    // returned handle indexes the descriptor, so callers can reference it. ---

    /// Define an ability under `name`; its `id` is set to the minted handle.
    pub fn ability(&mut self, name: &str, mut desc: AbilityDescriptor) -> AbilityId {
        let id = self.ability_names.intern(name);
        debug_assert_eq!(id.raw() as usize, self.abilities.len(), "ability name reused");
        desc.id = id;
        self.abilities.push(desc);
        id
    }
    /// Define a talent under `name`; its `id` is set to the minted handle.
    pub fn talent(&mut self, name: &str, mut desc: TalentDescriptor) -> TalentId {
        let id = self.talent_names.intern(name);
        debug_assert_eq!(id.raw() as usize, self.talents.len(), "talent name reused");
        desc.id = id;
        self.talents.push(desc);
        id
    }
    /// Define a buff under `name`; its `id` is set to the minted handle.
    pub fn buff(&mut self, name: &str, mut spec: BuffSpec) -> BuffId {
        let id = self.buff_names.intern(name);
        debug_assert_eq!(id.raw() as usize, self.buffs.len(), "buff name reused");
        spec.id = id;
        self.buffs.push(spec);
        id
    }
    /// Define a lookup curve under `name`, returning its handle.
    pub fn curve(&mut self, name: &str, curve: Curve) -> CurveId {
        let id = self.curve_names.intern(name);
        debug_assert_eq!(id.raw() as usize, self.curves.len(), "curve name reused");
        self.curves.push(curve);
        id
    }
    /// Define a spawnable unit under `name`; its `id` is set to the minted handle.
    pub fn unit(&mut self, name: &str, mut desc: UnitDescriptor) -> UnitId {
        let id = self.unit_names.intern(name);
        debug_assert_eq!(id.raw() as usize, self.units.len(), "unit name reused");
        desc.id = id;
        self.units.push(desc);
        id
    }

    /// The ABI version this context targets (always the SDK's [`ABI_VERSION`]).
    #[must_use]
    pub fn abi(&self) -> Version {
        ABI_VERSION
    }

    /// Finalize into the postcard-ready [`Registration`]: emit the [`Names`]
    /// tables from the interners and hand over the accumulated descriptors.
    #[must_use]
    pub fn finish(self) -> Registration {
        Registration {
            abi: ABI_VERSION,
            names: Names {
                stats: table(&self.stats),
                resources: table(&self.resources),
                stacks: table(&self.stacks),
                tags: table(&self.tags),
                tag_classes: table(&self.tag_class_names),
                params: table(&self.params),
                events: table(&self.events),
                buffs: table(&self.buff_names),
                curves: table(&self.curve_names),
                damage_types: table(&self.damage_types),
                dims: table(&self.dims),
                abilities: table(&self.ability_names),
                talents: table(&self.talent_names),
                handlers: table(&self.handlers),
                units: table(&self.unit_names),
            },
            abilities: self.abilities,
            talents: self.talents,
            buffs: self.buffs,
            tag_classes: self.tag_classes,
            curves: self.curves,
            units: self.units,
        }
    }
}
