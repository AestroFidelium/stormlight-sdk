//! The time axis of the interface ABI (stormlight/server#97) — what a mod says
//! about an interface that *moves*.
//!
//! [`ui`](crate::ui) describes one frame: a widget sits somewhere, is painted some
//! way, and stays there. Every HUD worth porting does more than that — a console
//! slides up when it is built, an alert that wants attention pulses while it is
//! shown, a bar catches up with the value it reads instead of teleporting to it.
//! None of that was expressible, so this is the axis the descriptor was missing.
//!
//! ## Two mechanisms, because there are two questions
//!
//! - A [`UiTrack`] answers "what does this widget do **when** something happens":
//!   a keyed curve over one property, started by a [`UiTrigger`] and run under a
//!   [`Playback`] rule. Entrances, exits and pulses are all this.
//! - A [`UiTransition`] answers "how does this widget **catch up** with the number
//!   it draws": a duration and a shape for the value a binding moved. A bar has
//!   nothing to key — it does not know where its value will go — so a track cannot
//!   express it and a transition cannot express an entrance.
//!
//! ## The engine owns no duration and no curve
//!
//! Every number here is the mod's. The interpreter samples a declared track and
//! writes the result; it has no library of named animations, no default duration
//! and no opinion about what a HUD's entrance looks like. A widget that declares
//! nothing is still, which is exactly what the ABI meant before this existed.
//!
//! ## Wall-clock, never the simulation's
//!
//! An interface is cosmetic, so a track runs on real time. A rewind, a slowed or
//! paused simulation and a rollback do not touch it — a panel does not re-play its
//! entrance because the server corrected a position, and a pulse does not stall
//! because the sim did. That is also why nothing in this module is reversible: it
//! is not state the simulation can be asked to restore.
//!
//! ## The shape of a segment is two tangents, not one name
//!
//! An eased segment is a cubic Hermite curve between two keys whose end slopes come
//! from the keys themselves: the velocity the left key is *left* with and the
//! velocity the right key is *arrived at* with. Naming a whole segment instead
//! ("ease-in-out") cannot say that a key is approached gently and left sharply,
//! which is most of what an authored curve is made of — and the shape a real
//! layout's keys carry is exactly this pair.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::ui::WidgetState;

/// How many tracks one widget may declare.
///
/// [`MAX_UI_WIDGETS`](crate::ui::MAX_UI_WIDGETS) bounds how big a tree is; this and
/// [`MAX_UI_KEYS`] bound how much of it moves, which is the cost that is paid every
/// frame rather than once at build. Four is one per property, which is the most a
/// widget can animate without two tracks fighting over the same number.
pub const MAX_UI_TRACKS: usize = 4;

/// How many keys one track may hold. A HUD's curves are a handful of keys — a
/// slide is two, a pulse is three — and a track with hundreds is a mod streaming an
/// animation through a descriptor rather than declaring one.
pub const MAX_UI_KEYS: usize = 16;

/// The velocity a key is approached or left with — one end of a segment's shape.
///
/// Deliberately velocities rather than named curves: paired across a segment they
/// compose into every ordinary easing (`Linear` → `Slow` is an ease-out, `Slow` →
/// `Linear` an ease-in, `Slow` → `Slow` a smoothstep) and into the asymmetric ones
/// a named set cannot spell at all.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Ease {
    /// Constant speed through this end of the segment.
    #[default]
    Linear,
    /// Comes to rest at this end — the flat tangent that reads as a settle.
    Slow,
    /// Rushes through this end.
    Fast,
    /// No interpolation at all: the segment holds the earlier key's value until the
    /// later key's time, then jumps. A flicker, a blink, a two-frame flash.
    Step,
}

impl Ease {
    /// The end slope this tangent contributes to a segment's Hermite curve.
    #[must_use]
    pub fn slope(self) -> f32 {
        match self {
            Self::Linear => 1.0,
            Self::Slow => 0.0,
            Self::Fast => 2.0,
            // Handled before a curve is built; the value is never read.
            Self::Step => 1.0,
        }
    }
}

/// The shape of one segment: the velocity it leaves the earlier value with and the
/// velocity it arrives at the later one with.
///
/// Its [`Default`] is linear at both ends, so a mod that says nothing about shape
/// gets a straight line rather than a curve the engine picked.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Shape {
    /// How the earlier value is left.
    pub leave: Ease,
    /// How the later value is arrived at.
    pub arrive: Ease,
}

impl Shape {
    /// Reshape a segment's linear progress `u` into its eased progress.
    ///
    /// Total, clamped and anchored: `0.0` in gives `0.0` out and `1.0` in gives
    /// `1.0` out for every shape, so a segment always *lands* on its later key
    /// whatever curve was asked for. A [`Ease::Step`] at either end holds the
    /// earlier value for the whole segment, which is the one shape that does not
    /// pass through the middle.
    #[must_use]
    pub fn ease(self, u: f32) -> f32 {
        if !u.is_finite() {
            return 0.0;
        }
        let u = u.clamp(0.0, 1.0);
        if self.leave == Ease::Step || self.arrive == Ease::Step {
            // Hold, then jump: the later key only arrives at the segment's end.
            return if u >= 1.0 { 1.0 } else { 0.0 };
        }
        let (m0, m1) = (self.leave.slope(), self.arrive.slope());
        let (t2, t3) = (u * u, u * u * u);
        // Hermite basis for p0 = 0, p1 = 1 with end slopes m0 and m1.
        let v = (t3 - 2.0 * t2 + u) * m0 + (-2.0 * t3 + 3.0 * t2) + (t3 - t2) * m1;
        v.clamp(0.0, 1.0)
    }
}

/// What a track drives.
///
/// Four scalars, and every one of them is *paint or placement* — never layout. An
/// animated widget moves over its neighbours instead of pushing them around, so a
/// panel sliding in cannot re-flow the row it sits in and a pulse cannot make the
/// text beside it jump. That is the difference between an interface that moves and
/// one that is unreadable while it moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum UiProperty {
    /// Horizontal displacement in logical pixels, positive right. Added to wherever
    /// the widget's declared layout put it.
    #[default]
    TranslateX,
    /// Vertical displacement in logical pixels, positive down.
    TranslateY,
    /// Uniform scale about the widget's own centre; `1.0` is its declared size.
    Scale,
    /// A multiplier on every alpha the widget paints with — its fill, its outline,
    /// its text and the tint of its picture. `1.0` is exactly as declared.
    Opacity,
}

impl UiProperty {
    /// What this property reads as when nothing is driving it. A track that is not
    /// playing leaves the widget here, which is the widget exactly as declared.
    #[must_use]
    pub fn rest(self) -> f32 {
        match self {
            Self::TranslateX | Self::TranslateY => 0.0,
            Self::Scale | Self::Opacity => 1.0,
        }
    }
}

/// What starts a track.
///
/// Closed, and each variant is a fact the interpreter already holds for its own
/// reasons — there is no event bus here and no way for a mod to start an animation
/// from its gameplay guest. A HUD animates in response to *being an interface*:
/// appearing, being pointed at, and going away.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum UiTrigger {
    /// The widget was built — its tree came on screen. An entrance, and, with
    /// [`Playback::Loop`], the idle animation that runs for as long as the tree is
    /// shown.
    #[default]
    Built,
    /// The widget entered that interaction state (stormlight/server#96). All four
    /// states are reachable, so *leaving* a state needs no variant of its own:
    /// returning to [`WidgetState::Idle`] is the pointer having left.
    State(WidgetState),
    /// The tree the widget belongs to stopped being shown, and is playing out
    /// before it goes.
    ///
    /// The only trigger that costs anything by existing: a tree with one of these
    /// anywhere in it is held on screen for as long as its longest exit runs, and
    /// stops taking the pointer the moment it starts leaving. A tree that declares
    /// none is despawned the instant its condition turns false, exactly as before.
    Hidden,
}

/// What a track does when it reaches its last key.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Playback {
    /// Stop there and hold the last key's value — an entrance stays where it
    /// arrived.
    #[default]
    Once,
    /// Start over from the first key. The value jumps back unless the first and
    /// last keys agree, which is the author's business and not the engine's.
    Loop,
    /// Run back to the first key, then forward again, forever — the pulse that does
    /// not tick.
    PingPong,
}

/// One keyframe: when, what, and the velocities the value passes through it with.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct UiKey {
    /// Seconds from the track's start. Keys are read in the order declared, and
    /// [`UiTrack::validate`] refuses a track whose times go backwards.
    pub time: f32,
    /// The property's value at that moment, in the property's own units.
    pub value: f32,
    /// How this key is arrived at (the shape of the segment *before* it).
    pub arrive: Ease,
    /// How it is left (the shape of the segment *after* it).
    pub leave: Ease,
}

/// A keyed curve over one property, and the rule that starts and repeats it.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct UiTrack {
    /// What it drives.
    pub property: UiProperty,
    /// What starts it.
    pub on: UiTrigger,
    /// What it does at the end.
    pub playback: Playback,
    /// Its keys, in time order. A track with no keys drives nothing.
    pub keys: Vec<UiKey>,
}

impl UiTrack {
    /// How long one pass through it takes: its last key's time.
    ///
    /// Derived rather than declared, so a track cannot claim a length its keys do
    /// not fill — the gap between the two would be a stretch of nothing that only
    /// [`Playback::Loop`] could reveal.
    #[must_use]
    pub fn duration(&self) -> f32 {
        self.keys.last().map_or(0.0, |key| key.time)
    }

    /// The value this track drives `elapsed` seconds after it started.
    ///
    /// Total over anything: a track with no keys reads its property's rest value, a
    /// zero-length one reads its single key, and a time before the first key or
    /// after the last (under [`Playback::Once`]) holds that key — which is the
    /// "degrades, never panics" rule this ABI applies to every malformed thing.
    #[must_use]
    pub fn sample(&self, elapsed: f32) -> f32 {
        let [first, .., last] = self.keys.as_slice() else {
            return self.keys.first().map_or_else(|| self.property.rest(), |key| key.value);
        };
        let local = self.local_time(elapsed);
        // The end is tested first, and that order is the rule for keys that share a
        // time: the *later* declared key wins. A track whose keys all sit at one
        // moment is a curve with no length, and what it holds must be where its
        // author left it rather than where they started.
        if local >= last.time {
            return last.value;
        }
        if local <= first.time {
            return first.value;
        }
        for pair in self.keys.windows(2) {
            let [a, b] = pair else { continue };
            if local >= a.time && local <= b.time {
                let span = b.time - a.time;
                if span <= 0.0 {
                    return b.value;
                }
                let shape = Shape { leave: a.leave, arrive: b.arrive };
                let u = shape.ease((local - a.time) / span);
                return a.value + (b.value - a.value) * u;
            }
        }
        last.value
    }

    /// Where `elapsed` lands on the track's own timeline under its playback rule.
    fn local_time(&self, elapsed: f32) -> f32 {
        let duration = self.duration();
        if !elapsed.is_finite() || elapsed <= 0.0 {
            return 0.0;
        }
        if duration <= 0.0 {
            return 0.0;
        }
        match self.playback {
            Playback::Once => elapsed.min(duration),
            Playback::Loop => elapsed % duration,
            Playback::PingPong => {
                let cycle = elapsed % (2.0 * duration);
                if cycle <= duration { cycle } else { 2.0 * duration - cycle }
            }
        }
    }

    /// Whether it ever stops. A held entrance can be forgotten once it has landed;
    /// a repeating one has to be sampled for as long as its widget is on screen.
    #[must_use]
    pub fn repeats(&self) -> bool {
        matches!(self.playback, Playback::Loop | Playback::PingPong)
    }

    /// Why this track cannot be played as declared, or `None`.
    ///
    /// Checked by [`UiRoot::validate`](crate::ui::UiRoot::validate) at load, at the
    /// widget that carries it, so an author hears about a broken curve when their
    /// mod is loaded rather than when the pointer finally reaches the widget.
    #[must_use]
    pub fn fault(&self) -> Option<TrackFault> {
        if self.keys.is_empty() {
            return Some(TrackFault::NoKeys);
        }
        if self.keys.len() > MAX_UI_KEYS {
            return Some(TrackFault::TooManyKeys);
        }
        let mut previous = f32::NEG_INFINITY;
        for key in &self.keys {
            if !key.time.is_finite() || !key.value.is_finite() {
                return Some(TrackFault::NonFinite);
            }
            if key.time < 0.0 {
                return Some(TrackFault::NegativeTime);
            }
            if key.time < previous {
                return Some(TrackFault::OutOfOrder);
            }
            previous = key.time;
        }
        None
    }
}

/// How a value a widget *draws* catches up when the state behind it steps
/// (stormlight/server#97).
///
/// A bar reads a number nothing declared and cannot key it, so this is the other
/// half of the time axis: not "play this curve" but "take this long to get there".
/// Declared on the widget that draws the value; inert on a widget that draws no
/// continuous quantity of its own.
///
/// **Zero seconds is a snap**, and that is the point of allowing it: a HUD that
/// wants its numbers exact — a cooldown readout, a hitpoint count in a competitive
/// interface — says so with a number rather than by not being able to say it.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct UiTransition {
    /// How long the catch-up takes, in seconds, however far it has to go.
    pub seconds: f32,
    /// The curve it takes to get there.
    pub shape: Shape,
}

impl UiTransition {
    /// How much of the way from the old value to the new one has been covered
    /// `elapsed` seconds in. Clamped to `0.0..=1.0`, and exactly `1.0` once the
    /// duration is up — a catch-up that never quite lands is an interface that
    /// never stops re-laying itself out.
    #[must_use]
    pub fn progress(&self, elapsed: f32) -> f32 {
        if !elapsed.is_finite() || !self.seconds.is_finite() || self.seconds <= 0.0 {
            return 1.0;
        }
        if elapsed <= 0.0 {
            return 0.0;
        }
        self.shape.ease((elapsed / self.seconds).clamp(0.0, 1.0))
    }

    /// Whether it can be run at all — a negative or non-finite duration is not a
    /// slower transition, it is a break.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.seconds.is_finite() && self.seconds >= 0.0
    }
}

/// Why one declared track cannot be played.
///
/// Reported by [`UiTrack::fault`] and carried up into
/// [`UiError`](crate::ui::UiError) with the widget that declared it — the same
/// treatment a NaN width gets, because a curve with a NaN in it drives a widget to
/// a position no layout can resolve.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrackFault {
    /// No keys: a track that drives nothing, which is a declaration the author
    /// meant to finish.
    NoKeys,
    /// More keys than [`MAX_UI_KEYS`].
    TooManyKeys,
    /// A NaN or an infinity in a key's time or value.
    NonFinite,
    /// A key before the track started.
    NegativeTime,
    /// Keys whose times go backwards, so which segment a moment belongs to would
    /// depend on the order they happened to be walked in.
    OutOfOrder,
}

impl core::fmt::Display for TrackFault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoKeys => f.write_str("has no keys"),
            Self::TooManyKeys => write!(f, "holds more than {MAX_UI_KEYS} keys"),
            Self::NonFinite => f.write_str("carries a non-finite time or value"),
            Self::NegativeTime => f.write_str("carries a key before the track starts"),
            Self::OutOfOrder => f.write_str("carries keys whose times go backwards"),
        }
    }
}
