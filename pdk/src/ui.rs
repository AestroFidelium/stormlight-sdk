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
//! Registration stays on [`ClientContext::ui`](crate::client::ClientContext::ui),
//! alongside `unit_visual` and `unit_animation`, and the whole bundle still leaves
//! through the single `mod_register` export
//! [`register_client_mod!`](crate::register_client_mod) generates — a second
//! registration macro would mean a second export of the same name.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use stormlight_mod_abi::ids::{EventId, Slot};
use stormlight_mod_abi::ui::{
    Anchor, Flow, Layout, Length, ListBinding, Slice, StateStyle, Style, Sweep, SweepDirection,
    TextSource, UiAction, ValueBinding, ValuePart, Widget, WidgetKind, WidgetState,
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

/// A button that raises the mod-defined `event` into every gameplay guest
/// subscribed to it, against the clicking player's own unit.
#[must_use]
pub fn trigger_button(event: EventId, children: Vec<Widget>) -> Widget {
    button(UiAction::Trigger { event }, children)
}

/// An ability slot: icon, cooldown sweep and key hint. The sweep is a dark
/// clockwise wedge unless [`WidgetExt::sweep`] says otherwise.
#[must_use]
pub fn ability_slot(slot: Slot, key_hint: &str) -> Widget {
    widget(WidgetKind::AbilitySlot { slot, key_hint: key_hint.to_string() })
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
}
