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
    Anchor, Flow, Layout, Length, ListBinding, Shown, Slice, StateStyle, Style, SummonRequest,
    Sweep, SweepDirection, TalentText, TextSource, UiAction, ValueBinding, ValuePart, Widget,
    WidgetKind, WidgetState,
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
    fn layered(mut self, layer: i32) -> Self {
        self.layout.layer = layer;
        self
    }
}
