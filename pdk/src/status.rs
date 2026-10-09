//! Declaring the art a status wears on the unit it affects
//! (stormlight/server#171) — the authoring half.
//!
//! ```ignore
//! use stormlight_mod_sdk::status::status;
//!
//! // A frost shell around the chest, seen by everyone.
//! ctx.declare_status(status("chilled", shell).at(point("Ref_Center")));
//! // A mark only its victim's enemies see.
//! ctx.declare_status(status("marked", sigil).own(None));
//! ```
//!
//! Built up as a value and handed to the context, like an effect: the refinements
//! compose in any order, and nothing is declared until the declaration is whole.

use stormlight_mod_abi::attach::AttachPoint;
use stormlight_mod_abi::visuals::VisualModel;

/// A status visual being declared — see [`status`], and
/// [`ClientContext::declare_status`](crate::client::ClientContext::declare_status)
/// to declare it.
#[derive(Clone, Debug)]
#[must_use = "a status visual is declared only once it is handed to the context"]
pub struct StatusSpec<'a> {
    pub(crate) buff: &'a str,
    pub(crate) own: Option<VisualModel>,
    pub(crate) others: Option<VisualModel>,
    pub(crate) attach: Option<AttachPoint>,
}

/// The status visual for the buff named `buff`: `model`, seen by everyone, at the
/// unit's own origin.
pub fn status(buff: &str, model: VisualModel) -> StatusSpec<'_> {
    StatusSpec { buff, own: Some(model.clone()), others: Some(model), attach: None }
}

impl StatusSpec<'_> {
    /// What the affected unit's own player sees instead; `None` hides it from them.
    pub fn own(mut self, look: Option<VisualModel>) -> Self {
        self.own = look;
        self
    }

    /// What everyone else sees instead; `None` hides it from them.
    pub fn others(mut self, look: Option<VisualModel>) -> Self {
        self.others = look;
        self
    }

    /// Hang it on a point of the unit's rig (stormlight/server#160). Build one with
    /// [`crate::attach::point`].
    pub fn at(mut self, attach: AttachPoint) -> Self {
        self.attach = Some(attach);
        self
    }
}
