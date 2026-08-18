//! The occurrence axis of the interface ABI (stormlight/server#93) — what a mod
//! says about an interface that appears *because something happened*.
//!
//! [`ui`](crate::ui) describes a tree that is on screen while a condition holds, and
//! [`ui_anim`](crate::ui_anim) describes how that tree moves once it is there. Every
//! condition the first of those can name is a state that *lasts*: the player has a
//! unit, a tier is waiting to be picked, the cursor is over something. Floating
//! combat text is none of them. It is one instance per authoritative hit, over the
//! unit that was hit, alive for a moment and then gone — so this is the axis the
//! descriptor was missing, and the reason the HUD of stormlight/server#70 shipped
//! with the effect of an ability legible from a bar dropping and nothing else.
//!
//! ## The engine reports; the mod decides what that looks like
//!
//! What arrives from the server is a fact — this unit lost this much, and here is
//! the opaque key of what caused it. Everything after that is the mod's: whether a
//! popup appears at all, how long it lives, whether a second hit joins the first or
//! starts its own number, and what it does while it is on screen (which is an
//! ordinary [`UiTrack`](crate::ui_anim::UiTrack) keyed on
//! [`UiTrigger::Built`](crate::ui_anim::UiTrigger::Built), so a rise-and-fade needs
//! nothing new). The engine picks no duration, no curve and no colour.
//!
//! ## Why merging is in the ABI at all
//!
//! Because the alternative is unreadable. A unit standing in something that ticks
//! produces an occurrence every time it ticks, and a HUD that spawns a number for
//! each of them shows a column of illegible ones instead of a fight. [`Coalesce`]
//! lets one popup *absorb* the occurrences that keep landing — its
//! [`EventQuantity::Latest`] stays the newest hit and its [`EventQuantity::Total`]
//! climbs — so a channelled beam reads as one growing number and a burst that lands
//! beside it still gets its own.
//!
//! Note what is **not** here: no notion of "this was a damage-over-time". The engine
//! does not have one and does not need one — [`Coalesce::PerCause`] groups by what
//! *caused* the damage, so a beam's ticks merge with each other because they share a
//! cause, not because they were labelled periodic. That is strictly more general and
//! stays content-free: the cause is an opaque id, never an ability's name.

use serde::{Deserialize, Serialize};

/// Which authoritative occurrence a transient root listens for.
///
/// Closed, and both variants are things the *simulation* did — never something the
/// interface noticed. A mod that wants a popup for an occasion the engine does not
/// report raises its own event through its gameplay guest and shows an ordinary
/// root; it does not get a variant here.
///
/// Damage and healing are separate rather than one signed quantity because they are
/// separate on the gameplay side too — they are their own verbs in the ISA — and
/// because a HUD paints them differently. Two roots, two declarations, and an
/// occurrence of one kind can never merge into a popup of the other.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum UiEvent {
    /// A unit took damage. The amount is what actually landed **after mitigation
    /// and including whatever a shield absorbed** — the number a player checks
    /// against the bar they just watched drop.
    #[default]
    Damaged,
    /// A unit was healed, by the amount that was actually restored.
    Healed,
}

/// Whether a fresh occurrence joins a popup already on screen, and what counts as
/// "the same" popup.
///
/// The rule is applied at the moment an occurrence arrives: if a live instance
/// matches, it absorbs the occurrence and its lifetime starts over, so a unit under
/// continuous fire carries one number that grows for as long as the fire lasts and
/// fades when it stops. If none matches, a new instance is spawned.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Coalesce {
    /// Every occurrence gets an instance of its own — the classic popup that
    /// floats off on its own path. Honest and unreadable under anything that
    /// ticks, which is exactly what the other two are for.
    Never,
    /// Occurrences that share a *cause* on the same unit join one instance. The
    /// default, and the one that reads best: a beam's ticks merge into a single
    /// climbing number while a hit that lands from somewhere else at the same
    /// moment pops separately, with no notion of "periodic" anywhere in the engine.
    #[default]
    PerCause,
    /// Every occurrence on the same unit joins one instance, whatever caused it —
    /// one number per unit per kind, for a HUD that wants a damage *readout* rather
    /// than combat text.
    PerUnit,
}

/// Which of a live popup's numbers a binding reads.
///
/// A popup is not a snapshot of one hit: under [`Coalesce`] it is an accumulator,
/// and these are the things it accumulates. A beam that reads `24` beside a running
/// `312` says what is happening to you *and* how much it has cost, which neither
/// number says alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum EventQuantity {
    /// The most recent occurrence's amount — the tick that just landed.
    #[default]
    Latest,
    /// Everything this instance has absorbed since it appeared. Equal to
    /// [`Self::Latest`] under [`Coalesce::Never`], where an instance only ever holds
    /// one occurrence.
    ///
    /// Always present, which is what a readout wants: a panel that prints "damage
    /// taken" must say `42` for one hit rather than falling silent. A *popup* that
    /// prints this beside [`Self::Latest`] wants [`Self::Toll`] instead — see there.
    Total,
    /// How many occurrences it has absorbed — the "×7" on a merged popup. Always at
    /// least `1`, since an instance exists because something happened.
    Hits,
    /// Of [`Self::Total`], the part a shield absorbed rather than health.
    ///
    /// Zero for healing and for a unit with no shield up. It is reported separately
    /// because a hit taken entirely on a shield still costs exactly what it says —
    /// the popup must show it — while a HUD that wants to tint that part, or to
    /// print what got through as `Total - Absorbed`, cannot recover it from one
    /// number.
    Absorbed,
    /// The same running total as [`Self::Total`], but **absent while the popup
    /// holds a single occurrence**.
    ///
    /// This exists because of what a popup looks like on screen. The natural way to
    /// author combat text is the blow that just landed with the toll beneath it —
    /// and until something has been merged those two are, by definition, the same
    /// figure, so a player hit once reads their damage twice. Every way out of that
    /// is worse than a second quantity: printing only the total loses the tick a
    /// beam is doing right now, printing only the latest loses what it has cost, and
    /// a rule that hid a widget whose value duplicated its sibling's would be the
    /// engine deciding what a mod's interface says.
    ///
    /// Absent means absent, not zero: a bound text draws nothing at all, so the
    /// second line simply is not there for a lone hit and appears the moment the
    /// popup becomes a sum of more than one thing — which is the moment it starts
    /// saying something the first line does not.
    Toll,
}
