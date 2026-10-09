//! Declaring an effect with more to say than its look (stormlight/server#160,
//! stormlight/server#167) — the authoring half.
//!
//! ```ignore
//! use stormlight_mod_sdk::abi::lifetime::EffectLifetime;
//! use stormlight_mod_sdk::effects::{NamedEffectExt, effect, named_effect};
//!
//! // A burst that lives exactly as long as its art plays.
//! ctx.declare_effect(effect("bolt", EffectRole::Impact, burst).lasting(EffectLifetime::Art));
//! // A puff that stays on the ground where the step was, whatever the stepper does.
//! ctx.declare_notify_effect(named_effect("dust", puff).detached());
//! ```
//!
//! A value built up and then handed to the context, rather than a handle into the
//! context: the refinements compose in any order, and nothing is declared until the
//! declaration is whole.

use alloc::string::String;

use stormlight_mod_abi::attach::AttachPoint;
use stormlight_mod_abi::lifetime::{EffectLifetime, HostEnd};
use stormlight_mod_abi::visuals::{EffectRole, NamedEffect, VisualModel};

/// A piece of an ability's feedback being declared — see [`effect`], and
/// [`ClientContext::declare_effect`](crate::client::ClientContext::declare_effect)
/// to declare it.
#[derive(Clone, Debug)]
#[must_use = "an effect is declared only once it is handed to the context"]
pub struct EffectSpec<'a> {
    pub(crate) ability: &'a str,
    pub(crate) role: EffectRole,
    pub(crate) model: VisualModel,
    pub(crate) attach: Option<AttachPoint>,
    pub(crate) lifetime: EffectLifetime,
}

/// The visual for the ability named `ability` in `role`, hung where the role hangs
/// it and living the client's default.
pub fn effect(ability: &str, role: EffectRole, model: VisualModel) -> EffectSpec<'_> {
    EffectSpec { ability, role, model, attach: None, lifetime: EffectLifetime::Default }
}

impl EffectSpec<'_> {
    /// Hang it on a point of its unit's rig (stormlight/server#160). Build one with
    /// [`crate::attach::point`].
    pub fn at(mut self, attach: AttachPoint) -> Self {
        self.attach = Some(attach);
        self
    }

    /// Say how long it lives (stormlight/server#167). Read by the bursts nothing
    /// else ends; see [`EffectLifetime`].
    pub fn lasting(mut self, lifetime: EffectLifetime) -> Self {
        self.lifetime = lifetime;
        self
    }
}

/// An effect for an animation notify to spawn, under a name of this mod's own
/// choosing, living the client's default and following its host.
#[must_use]
pub fn named_effect(name: &str, model: VisualModel) -> NamedEffect {
    NamedEffect {
        name: String::from(name),
        model,
        lifetime: EffectLifetime::Default,
        on_host_end: HostEnd::Follow,
    }
}

/// The refining half of a [`NamedEffect`] declaration.
pub trait NamedEffectExt {
    /// How long it lives where the notify spawning it says nothing.
    #[must_use]
    fn lasting(self, lifetime: EffectLifetime) -> Self;
    /// What it does when the character it is hung on goes while it still lives.
    #[must_use]
    fn on_host_end(self, end: HostEnd) -> Self;
    /// Stay where it was when the character goes, and finish there — the same as
    /// `on_host_end(HostEnd::Detach)`.
    #[must_use]
    fn detached(self) -> Self;
}

impl NamedEffectExt for NamedEffect {
    fn lasting(mut self, lifetime: EffectLifetime) -> Self {
        self.lifetime = lifetime;
        self
    }
    fn on_host_end(mut self, end: HostEnd) -> Self {
        self.on_host_end = end;
        self
    }
    fn detached(self) -> Self {
        self.on_host_end(HostEnd::Detach)
    }
}
