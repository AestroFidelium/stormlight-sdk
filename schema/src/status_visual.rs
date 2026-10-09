//! The art a status wears on the unit it affects (stormlight/server#171).
//!
//! A buff is a fact about a unit the server keeps — what it modifies, how long it
//! has left, who applied it. The client is told only which buffs a unit carries
//! and how many stacks of each, which is all a drawing needs and none of the
//! privileged rest. This is the drawing: keyed by the buff's interned handle, so
//! the mod that authored the buff need not be the one that dresses it, exactly as
//! an ability's feedback is keyed by the ability.
//!
//! A status visual is the archetypal **hold-open** piece of art: it appears when the
//! status does, plays its opening and then its loop for as long as the status
//! lasts, and closes — its [`ModelClips::death`](crate::visuals::ModelClips::death)
//! clip, if it has one — when the status is gone. It never expires on a clock of
//! its own, because it draws something that has one.

use serde::{Deserialize, Serialize};

use crate::attach::AttachPoint;
use crate::ids::BuffId;
use crate::visuals::VisualModel;

/// Who is looking at the affected unit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Viewer {
    /// The player the affected unit belongs to.
    Owner,
    /// Anyone else who can see it.
    Other,
}

/// How a status is drawn on the unit carrying it.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct StatusVisual {
    /// The buff whose presence on a unit this draws.
    pub buff: BuffId,
    /// What it looks like.
    pub look: StatusLook,
}

/// The drawing half of a [`StatusVisual`], with no handle attached — carried on its
/// own so every table downstream is keyed by the id the *host* resolved the buff
/// to, and holds no copy of the local handle the mod authored it with (the reason
/// [`CardInfo`](crate::visuals::CardInfo) is split off a card).
///
/// **Per viewer**: the same status may look one way to the player it is affecting
/// and another — or not at all — to everyone else. A mark the victim must notice
/// and its enemies need not, or a buff whose holder sees a subtle shimmer while
/// opponents see the full warning, is one declaration, not two statuses.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct StatusLook {
    /// What the affected unit's own player sees. `None`: nothing.
    pub own: Option<VisualModel>,
    /// What every other player sees. `None`: nothing.
    pub others: Option<VisualModel>,
    /// Where on the unit's rig it hangs — a chest or centre point, so the shell
    /// sits around the body rather than at its feet. `None`: the unit's own origin.
    pub attach: Option<AttachPoint>,
}

impl StatusLook {
    /// What `viewer` sees. Neither half falls back to the other: a status that
    /// shows its holder nothing is a declaration, not a gap to fill.
    #[must_use]
    pub fn seen_by(&self, viewer: Viewer) -> Option<&VisualModel> {
        match viewer {
            Viewer::Owner => self.own.as_ref(),
            Viewer::Other => self.others.as_ref(),
        }
    }

    /// Whether anyone sees anything at all. One that shows nothing to anyone can
    /// never be seen, and is refused at adoption rather than carried.
    #[must_use]
    pub fn shows_anything(&self) -> bool {
        self.own.is_some() || self.others.is_some()
    }
}
