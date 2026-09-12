//! The HUD authoring path (stormlight/server#66) — constructors and a chaining
//! extension for [`Widget`], so declaring an interface in a `*_client` mod reads
//! the way declaring visuals does.
//!
//! The ABI itself is plain data: a [`Widget`] is four fields and one of them is a
//! `Vec` of more widgets. Written out literally that is three lines of
//! boilerplate per node, and a HUD is thirty nodes. So the pdk supplies one
//! constructor per widget kind — each returning a node with a neutral
//! [`Layout`]/[`Style`] — and a [`WidgetExt`] trait to override only what the
//! author cares about:
//!
//! ```ignore
//! use stormlight_mod_sdk::ui::{WidgetExt, bar, panel};
//! use stormlight_mod_sdk::abi::ui::{Anchor, Length, RootVisibility, UiSubject, ValueBinding};
//!
//! ctx.ui("vitals", RootVisibility::Always, UiSubject::LocalPlayer,
//!     panel(vec![bar(ValueBinding::Health).sized(Length::Px(180.0), Length::Px(12.0))])
//!         .at(Anchor::BottomLeft, Length::Px(16.0), Length::Px(-16.0)));
//! ```
//!
//! An interface that *moves* is the same story one level along: [`track`] and
//! [`key`] build a curve, [`WidgetExt::animated`] hangs it on the widget, and
//! [`WidgetExt::transition`] says how a bound number catches up
//! (stormlight/server#97):
//!
//! ```ignore
//! use stormlight_mod_sdk::ui::{KeyExt, WidgetExt, key, panel, track};
//! use stormlight_mod_sdk::abi::ui_anim::{Ease, Playback, UiProperty, UiTrigger};
//!
//! // Slides up into place when the tree is built, settling rather than stopping.
//! panel(vec![]).animated(track(
//!     UiProperty::TranslateY,
//!     UiTrigger::Built,
//!     Playback::Once,
//!     vec![key(0.0, 48.0), key(0.35, 0.0).arriving(Ease::Slow)],
//! ));
//! ```
//!
//! Registration stays on [`ClientContext::ui`](crate::client::ClientContext::ui),
//! alongside `unit_visual` and `unit_animation`, and the whole bundle still leaves
//! through the single `mod_register` export
//! [`register_client_mod!`](crate::register_client_mod) generates — a second
//! registration macro would mean a second export of the same name.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use stormlight_mod_abi::ids::{EventId, Slot};
use stormlight_mod_abi::ui::{
    Anchor, Flow, Layout, Length, ListBinding, OptionState, QuestSpan, Shown, Slice, StateStyle,
    Style, SummonRequest, Sweep, SweepDirection, TalentText, TaskState, TextSource, Tooltip,
    UiAction, ValueBinding, ValuePart, Widget, WidgetKind, WidgetState,
};
use stormlight_mod_abi::ui_anim::{
    Ease, Playback, Shape, UiKey, UiProperty, UiTrack, UiTransition, UiTrigger,
};

/// A node of the given kind with a neutral layout and style.
fn widget(kind: WidgetKind) -> Widget {
    Widget { name: String::new(), layout: Layout::default(), style: Style::default(), kind }
}

/// A layout container holding `children`.
#[must_use]
pub fn panel(children: Vec<Widget>) -> Widget {
    widget(WidgetKind::Panel { children })
}

/// A line of authored text.
#[must_use]
pub fn text(literal: &str) -> Widget {
    widget(WidgetKind::Text { text: TextSource::Literal(literal.to_string()) })
}

/// A number read from generic state, with `decimals` fractional digits.
#[must_use]
pub fn bound_text(binding: ValueBinding, part: ValuePart, decimals: u8) -> Widget {
    widget(WidgetKind::Text { text: TextSource::Value { binding, part, decimals } })
}

/// How far the subject has got with the task the talent at one coordinate sets
/// (stormlight/server#139).
///
/// `span` picks which of a staged task's two honest targets the numbers are against:
/// the whole objective, or the rung being worked at. A socket's ring usually wants
/// the second and a panel's line the first, and neither is derivable from the other
/// without the client deciding something it should be told.
#[must_use]
pub fn talent_task(tier: u8, option: u8, span: QuestSpan) -> ValueBinding {
    ValueBinding::TalentQuest { tier, option, span }
}

/// How far the subject has got with one of its **own** tasks
/// (stormlight/server#139) — one nobody chose, addressed by its position in the
/// unit's descriptor.
#[must_use]
pub fn unit_task(index: u8, span: QuestSpan) -> ValueBinding {
    ValueBinding::UnitTask { index, span }
}

/// The subject's chosen talents, one per line.
#[must_use]
pub fn talent_list() -> Widget {
    widget(WidgetKind::Text { text: TextSource::List(ListBinding::ChosenTalents) })
}

/// What tier `tier` *offers*, one option per line — the counterpart of
/// [`talent_list`] (stormlight/server#95).
#[must_use]
pub fn tier_options(tier: u8) -> Widget {
    widget(WidgetKind::Text { text: TextSource::List(ListBinding::TierOptions(tier)) })
}

/// One offered talent's name or description (stormlight/server#95), addressed by
/// the same `(tier, option)` coordinate [`talent_button`] takes — so a cell says
/// what it is and what it does without this mod naming a single talent.
#[must_use]
pub fn talent_text(tier: u8, option: u8, field: TalentText) -> Widget {
    widget(WidgetKind::Text { text: TextSource::Talent { tier, option, field } })
}

/// The picture the talent offered at `(tier, option)` wears, falling back to
/// [`WidgetExt::image`] for a coordinate that offers nothing (server#95).
///
/// It draws and does not act: put it inside a [`talent_button`] with the same
/// coordinate, which is what the player clicks.
#[must_use]
pub fn talent_icon(tier: u8, option: u8) -> Widget {
    widget(WidgetKind::TalentIcon { tier, option })
}

/// The portrait of the unit the subject is driving (stormlight/server#145),
/// falling back to [`WidgetExt::image`] for a subject driving nothing or a unit
/// nobody gave a picture to.
///
/// The roster counterpart of [`talent_icon`], and it names no content for the same
/// reason: a row is instanced per *player*, and which hero any of them picked is
/// not something an interface mod can know. It draws and does not act — wrap it in
/// a [`button`] if the row should be clickable.
#[must_use]
pub fn unit_icon() -> Widget {
    widget(WidgetKind::UnitIcon)
}

/// The unit level at which tier `tier` of the subject's tree becomes choosable
/// (stormlight/server#111).
///
/// A tier *index*, like every other talent coordinate in this ABI, so a strip of
/// buttons labelled with these reads `1 4 7 10 …` for one tree and whatever a
/// different tree declares — and names no content either way. A tier no tree
/// declares prints nothing.
#[must_use]
pub fn tier_level(tier: u8) -> Widget {
    bound_text(ValueBinding::TierLevel(tier), ValuePart::Current, 0)
}

/// A fill bar bound to `value`.
#[must_use]
pub fn bar(value: ValueBinding) -> Widget {
    widget(WidgetKind::Bar { value })
}

/// A picture from a `mod://<id>/<path>` URL.
#[must_use]
pub fn icon(asset: &str) -> Widget {
    widget(WidgetKind::Icon).image(asset)
}

/// A clickable container that sends `action`.
#[must_use]
pub fn button(action: UiAction, children: Vec<Widget>) -> Widget {
    widget(WidgetKind::Button { action, children })
}

/// A button that casts the ability bound in `slot` — the same request that slot's
/// keybind sends (stormlight/server#69).
#[must_use]
pub fn cast_button(slot: Slot, children: Vec<Widget>) -> Widget {
    button(UiAction::CastSlot(slot), children)
}

/// A button that takes option `option` of talent tier `tier`. An index into the
/// *unit's* declared tree, so a cosmetic mod lays out a talent panel without
/// naming a single talent.
#[must_use]
pub fn talent_button(tier: u8, option: u8, children: Vec<Widget>) -> Widget {
    button(UiAction::PickTalent { tier, option }, children)
}

/// A button that marks option `option` of tier `tier` as the one this player means
/// to take, or clears the mark by naming it again (stormlight/server#133).
///
/// A plan, not a pick: it asks the server for nothing, changes nothing about the
/// unit, and works for a tier the player has not reached — which is the whole of
/// why it is useful, since a plan stops being a plan the moment it is actionable.
///
/// Declare it as a **widget of its own**, inside the row rather than as the row.
/// A row of a locked tier is refused a pick and must still accept a mark, so
/// reading one click two ways would tie the two refusals together.
#[must_use]
pub fn prepick_button(tier: u8, option: u8, children: Vec<Widget>) -> Widget {
    button(UiAction::PrepickTalent { tier, option }, children)
}

/// A button that pages this client's own talent panel to `tier`
/// (stormlight/server#104).
///
/// The one button that asks the server for nothing: it moves the player's eyes,
/// not their unit. Pair it with [`WidgetExt::shown_while_tier`] on the rows it
/// pages to.
#[must_use]
pub fn select_tier_button(tier: u8, children: Vec<Widget>) -> Widget {
    button(UiAction::SelectTier(tier), children)
}

/// A button that raises the mod-defined `event` into every gameplay guest
/// subscribed to it, against the clicking player's own unit.
#[must_use]
pub fn trigger_button(event: EventId, children: Vec<Widget>) -> Widget {
    button(UiAction::Trigger { event }, children)
}

/// A button that latches this client's own interface summoned, lets go of it, or
/// flips between the two (stormlight/server#107).
///
/// The other button that asks the server for nothing. Pair it with a root that
/// declares [`SummonGate::Held`](stormlight_mod_abi::ui::SummonGate::Held): the
/// summon *input* is still a peek the player holds, and this is the same interface
/// asked for in a way they can let go of.
#[must_use]
pub fn summon_button(request: SummonRequest, children: Vec<Widget>) -> Widget {
    button(UiAction::Summon(request), children)
}

/// An ability slot: icon, cooldown sweep and key hint. The sweep is a dark
/// clockwise wedge unless [`WidgetExt::sweep`] says otherwise.
#[must_use]
pub fn ability_slot(slot: Slot, key_hint: &str) -> Widget {
    widget(WidgetKind::AbilitySlot { slot, key_hint: key_hint.to_string() })
}

/// One keyframe: `value` at `time` seconds, passed through at constant speed.
///
/// Pair it with [`KeyExt`] to shape the segments around it — a key is *arrived at*
/// with one velocity and *left* with another, and every ordinary easing is a pair
/// of those (see [`Shape`]).
#[must_use]
pub fn key(time: f32, value: f32) -> UiKey {
    UiKey { time, value, arrive: Ease::Linear, leave: Ease::Linear }
}

/// A curve over one property, started by `on` and repeated by `playback`
/// (stormlight/server#97).
///
/// The track's length is its last key's time; there is no separate duration to
/// keep in step with the keys.
#[must_use]
pub fn track(property: UiProperty, on: UiTrigger, playback: Playback, keys: Vec<UiKey>) -> UiTrack {
    UiTrack { property, on, playback, keys }
}

/// Shaping a key's two ends — the velocities the value passes through it with.
pub trait KeyExt: Sized {
    /// How the value *arrives* at this key: the tail of the segment before it.
    #[must_use]
    fn arriving(self, ease: Ease) -> Self;
    /// How it *leaves* this key: the head of the segment after it.
    #[must_use]
    fn leaving(self, ease: Ease) -> Self;
}

impl KeyExt for UiKey {
    fn arriving(mut self, ease: Ease) -> Self {
        self.arrive = ease;
        self
    }
    fn leaving(mut self, ease: Ease) -> Self {
        self.leave = ease;
        self
    }
}

/// Chaining overrides for a widget's layout and style — an author states only the
/// properties that differ from the neutral base.
pub trait WidgetExt: Sized {
    /// Name it, for diagnostics.
    #[must_use]
    fn named(self, name: &str) -> Self;
    /// Anchor it and offset it from that anchor.
    #[must_use]
    fn at(self, anchor: Anchor, x: Length, y: Length) -> Self;
    /// Give it an explicit extent.
    #[must_use]
    fn sized(self, width: Length, height: Length) -> Self;
    /// Arrange its children, `gap` pixels apart.
    #[must_use]
    fn flow(self, flow: Flow, gap: f32) -> Self;
    /// Inset its children from its own edge.
    #[must_use]
    fn padded(self, padding: f32) -> Self;
    /// Set the foreground (text colour, icon tint, bar fill).
    #[must_use]
    fn color(self, rgba: [f32; 4]) -> Self;
    /// Set the background (panel fill, bar trough).
    #[must_use]
    fn background(self, rgba: [f32; 4]) -> Self;
    /// Outline it.
    #[must_use]
    fn border(self, rgba: [f32; 4], width: f32) -> Self;
    /// Set the text height.
    #[must_use]
    fn font_size(self, size: f32) -> Self;
    /// Set the face, a `mod://<id>/<path>` font this package ships.
    #[must_use]
    fn font(self, asset: &str) -> Self;
    /// Attach a `mod://<id>/<path>` image.
    #[must_use]
    fn image(self, asset: &str) -> Self;
    /// Cut whatever picture this widget draws to the **alpha of `asset`**
    /// (server#121) — the shape of the socket it sits in.
    ///
    /// For square content art in a plate that is not square. The mask's own colour
    /// is ignored, so this can point at a plate the package already ships rather
    /// than at a second black-and-white copy of it, and it belongs to the widget
    /// rather than to the picture — a slot cut to its socket stays cut whichever
    /// ability is bound to it.
    #[must_use]
    fn mask(self, asset: &str) -> Self;
    /// Cut that image nine-slice, `left`/`top`/`right`/`bottom` insets in the
    /// art's own pixels — so a plate keeps its corners at any width.
    #[must_use]
    fn sliced(self, left: f32, top: f32, right: f32, bottom: f32) -> Self;
    /// Mirror that image left-to-right and/or top-to-bottom.
    #[must_use]
    fn flipped(self, x: bool, y: bool) -> Self;
    /// Recolour its foreground while it is in `state` (server#69).
    #[must_use]
    fn state_color(self, state: WidgetState, rgba: [f32; 4]) -> Self;
    /// Recolour its background while it is in `state`.
    #[must_use]
    fn state_background(self, state: WidgetState, rgba: [f32; 4]) -> Self;
    /// Swap its picture while it is in `state` — the `_hover` / `_disabled`
    /// texture a real HUD's buttons ship beside their resting one.
    #[must_use]
    fn state_image(self, state: WidgetState, asset: &str) -> Self;
    /// Recolour the cooldown wedge wiped over it, and say which way it goes
    /// (stormlight/server#99).
    ///
    /// Only an [`ability_slot`] has a cooldown, so this is inert anywhere else.
    /// A fully transparent colour draws no sweep at all, which is how a HUD that
    /// prints its cooldowns instead says so.
    #[must_use]
    fn sweep(self, rgba: [f32; 4], direction: SweepDirection) -> Self;
    /// Give it a curve to play (stormlight/server#97). Called again for another
    /// property — one track each, and the last one declared for a property is the
    /// one that drives it.
    #[must_use]
    fn animated(self, track: UiTrack) -> Self;
    /// Say how the number it draws catches up when that number steps: `seconds` to
    /// cover the distance, on the given curve. Zero seconds draws it exactly.
    #[must_use]
    fn transition(self, seconds: f32, shape: Shape) -> Self;
    /// Lay it out only while the panel is paged to `tier` (stormlight/server#104).
    ///
    /// Absent, not transparent: off its page the widget reserves no space, takes no
    /// pointer and reads no binding — so a column of choices sits where the panel's
    /// art expects it instead of after a run of hidden rows. Which page is open is
    /// the client's answer, from [`select_tier_button`] and from whichever tier is
    /// waiting on a choice.
    #[must_use]
    fn shown_while_tier(self, tier: u8) -> Self;
    /// Lay it out only while one coordinate of one tier is in `is`
    /// (stormlight/server#118).
    ///
    /// The same `(tier, option)` a [`talent_button`] picks and a [`talent_icon`]
    /// draws, so a panel gates a row on the row it already declared and still names
    /// no talent. Three uses, one per state:
    ///
    /// - [`OptionState::Offered`] — a tier that offers two choices draws two rows
    ///   rather than however many the panel happened to declare;
    /// - [`OptionState::Taken`] — the mark on the one that was chosen;
    /// - [`OptionState::PassedOver`] — the veil over the ones it beat.
    ///
    /// A widget carries one gate, so a row that is also on a page says the page part
    /// with a container: one per tier, [`shown_while_tier`](Self::shown_while_tier),
    /// holding rows gated on their own coordinate.
    #[must_use]
    fn shown_while_option(self, tier: u8, option: u8, is: OptionState) -> Self;
    /// Lay it out only while the task the talent at one coordinate sets is in a
    /// given state (stormlight/server#139).
    ///
    /// What a socket's progress ring and its completed mark are gated on. Every one
    /// of these is **told** by the server: a client never works out whether a rung
    /// has been paid, because a shortcut finishes a task the count never got to the
    /// top of.
    #[must_use]
    fn shown_while_talent_task(self, tier: u8, option: u8, is: TaskState) -> Self;
    /// Lay it out only while one of the subject's **own** tasks is in a given state
    /// (stormlight/server#139).
    ///
    /// A unit carries tasks with nothing chosen — a hero's baseline objective, an
    /// event's, a map's — and those have no coordinate, so one is addressed by its
    /// position in the unit's descriptor.
    #[must_use]
    fn shown_while_unit_task(self, index: u8, is: TaskState) -> Self;
    /// Lay it out only while `tier` is still waiting on a choice
    /// (stormlight/server#120).
    ///
    /// What a socket's number is for: a tier nobody has answered is worth labelling
    /// with the level that opens it, and one that has been answered is worth
    /// labelling with the answer. Draw the answer with [`talent_icon`]s gated
    /// [`OptionState::Taken`] — at most one is ever on screen — and step the number
    /// aside with this.
    #[must_use]
    fn shown_while_tier_undecided(self, tier: u8) -> Self;
    /// Lay it out only while `tier` is the decision the player is being asked for —
    /// reached and unanswered (stormlight/server#114).
    ///
    /// What a strip points at. Not [`WidgetExt::shown_while_tier_undecided`], which
    /// is true of every tier still ahead of the player, and not
    /// [`WidgetExt::shown_while_tier`], which is wherever they happen to be looking.
    #[must_use]
    fn shown_while_tier_waiting(self, tier: u8) -> Self;
    /// Draw it in front of the siblings that declare a lower layer
    /// (stormlight/server#110) — and for a root, the other roots are its siblings.
    ///
    /// Undeclared is `0`, which is declaration order, so this is only ever worth
    /// reaching for where declaration order cannot say it: a summoned panel that
    /// has to rise out from *behind* an always-on console, or a nameplate that has
    /// to stay over both. Negative puts something deliberately behind an interface
    /// declared at zero.
    #[must_use]
    fn layered(self, layer: i32) -> Self;
    /// Say what it is, at length, while the pointer rests on it
    /// (stormlight/server#112).
    ///
    /// `content` is an ordinary widget tree and reads the ordinary bindings, so a
    /// talent's tooltip is [`talent_text`] at the coordinate its row already names —
    /// this mod still names no content. `anchor` says which corner of the box sits
    /// at the pointer, `offset` moves it clear of the cursor, and `delay` is how
    /// long the pointer has to rest first. All three are the interface's decisions,
    /// like every other duration and offset it declares.
    #[must_use]
    fn tooltip(self, anchor: Anchor, offset: [f32; 2], delay: f32, content: Vec<Widget>) -> Self;
}

/// The override slot for one state, created empty on first use so the three
/// `state_*` setters compose on the same widget.
///
/// `None` for [`WidgetState::Idle`], which has no slot: the declared style *is*
/// the resting appearance, so the setters no-op there rather than quietly
/// rewriting the base an author is measuring their states against.
fn slot(style: &mut Style, state: WidgetState) -> Option<&mut StateStyle> {
    let entry = match state {
        WidgetState::Idle => return None,
        WidgetState::Hovered => &mut style.states.hover,
        WidgetState::Pressed => &mut style.states.press,
        WidgetState::Disabled => &mut style.states.disabled,
    };
    Some(entry.get_or_insert_with(StateStyle::default))
}

impl WidgetExt for Widget {
    fn named(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }
    fn at(mut self, anchor: Anchor, x: Length, y: Length) -> Self {
        self.layout.anchor = anchor;
        self.layout.offset = [x, y];
        self
    }
    fn sized(mut self, width: Length, height: Length) -> Self {
        self.layout.size = [width, height];
        self
    }
    fn flow(mut self, flow: Flow, gap: f32) -> Self {
        self.layout.flow = flow;
        self.layout.gap = gap;
        self
    }
    fn padded(mut self, padding: f32) -> Self {
        self.layout.padding = padding;
        self
    }
    fn color(mut self, rgba: [f32; 4]) -> Self {
        self.style.color = rgba;
        self
    }
    fn background(mut self, rgba: [f32; 4]) -> Self {
        self.style.background = rgba;
        self
    }
    fn border(mut self, rgba: [f32; 4], width: f32) -> Self {
        self.style.border.color = rgba;
        self.style.border.width = width;
        self
    }
    fn font_size(mut self, size: f32) -> Self {
        self.style.font_size = size;
        self
    }
    fn font(mut self, asset: &str) -> Self {
        self.style.font = Some(asset.to_string());
        self
    }
    fn image(mut self, asset: &str) -> Self {
        self.style.image = Some(asset.to_string());
        self
    }
    fn mask(mut self, asset: &str) -> Self {
        self.style.mask = Some(asset.to_string());
        self
    }
    fn sliced(mut self, left: f32, top: f32, right: f32, bottom: f32) -> Self {
        self.style.slice = Some(Slice { left, top, right, bottom });
        self
    }
    fn flipped(mut self, x: bool, y: bool) -> Self {
        self.style.flip_x = x;
        self.style.flip_y = y;
        self
    }
    fn state_color(mut self, state: WidgetState, rgba: [f32; 4]) -> Self {
        if let Some(over) = slot(&mut self.style, state) {
            over.color = Some(rgba);
        }
        self
    }
    fn state_background(mut self, state: WidgetState, rgba: [f32; 4]) -> Self {
        if let Some(over) = slot(&mut self.style, state) {
            over.background = Some(rgba);
        }
        self
    }
    fn state_image(mut self, state: WidgetState, asset: &str) -> Self {
        if let Some(over) = slot(&mut self.style, state) {
            over.image = Some(asset.to_string());
        }
        self
    }
    fn sweep(mut self, rgba: [f32; 4], direction: SweepDirection) -> Self {
        self.style.sweep = Sweep { color: rgba, direction };
        self
    }
    fn animated(mut self, track: UiTrack) -> Self {
        self.style.anim.push(track);
        self
    }
    fn transition(mut self, seconds: f32, shape: Shape) -> Self {
        self.style.transition = Some(UiTransition { seconds, shape });
        self
    }
    fn shown_while_tier(mut self, tier: u8) -> Self {
        self.layout.shown = Shown::WhileTierSelected(tier);
        self
    }
    fn shown_while_option(mut self, tier: u8, option: u8, is: OptionState) -> Self {
        self.layout.shown = Shown::WhileOption { tier, option, is };
        self
    }
    fn shown_while_talent_task(mut self, tier: u8, option: u8, is: TaskState) -> Self {
        self.layout.shown = Shown::WhileTalentTask { tier, option, is };
        self
    }
    fn shown_while_unit_task(mut self, index: u8, is: TaskState) -> Self {
        self.layout.shown = Shown::WhileUnitTask { index, is };
        self
    }
    fn shown_while_tier_undecided(mut self, tier: u8) -> Self {
        self.layout.shown = Shown::WhileTierUndecided(tier);
        self
    }
    fn shown_while_tier_waiting(mut self, tier: u8) -> Self {
        self.layout.shown = Shown::WhileTierWaiting(tier);
        self
    }
    fn layered(mut self, layer: i32) -> Self {
        self.layout.layer = layer;
        self
    }
    fn tooltip(
        mut self,
        anchor: Anchor,
        offset: [f32; 2],
        delay: f32,
        content: Vec<Widget>,
    ) -> Self {
        self.style.tooltip = Tooltip { content, anchor, offset, delay };
        self
    }
}
