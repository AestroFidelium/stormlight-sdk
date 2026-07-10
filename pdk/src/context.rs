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

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use stormlight_mod_abi::abilities::AbilityDescriptor;
use stormlight_mod_abi::behaviors::BuffSpec;
use stormlight_mod_abi::descriptors::{Curve, Names, Registration};
use stormlight_mod_abi::ids::{
    AbilityId, BuffId, CurveId, DamageTypeId, DimId, EventId, Handle, HandlerId, ParamId,
    ResourceId, StackId, StatId, TagClassId, TagId, TalentId,
};
use stormlight_mod_abi::interner::Interner;
use stormlight_mod_abi::manifest::{ABI_VERSION, Version};
use stormlight_mod_abi::talents::TalentDescriptor;

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

    abilities: Vec<AbilityDescriptor>,
    talents: Vec<TalentDescriptor>,
    buffs: Vec<BuffSpec>,
    tag_classes: Vec<(TagId, TagClassId)>,
    curves: Vec<Curve>,
}

/// Collect an interner's contents as a dense `raw index -> name` table.
fn table<H: Handle>(interner: &Interner<H>) -> Vec<String> {
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
            },
            abilities: self.abilities,
            talents: self.talents,
            buffs: self.buffs,
            tag_classes: self.tag_classes,
            curves: self.curves,
        }
    }
}
