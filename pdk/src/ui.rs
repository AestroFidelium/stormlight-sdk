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

use stormlight_mod_abi::ids::Slot;
use stormlight_mod_abi::ui::{
    Anchor, Flow, Layout, Length, ListBinding, Style, TextSource, UiAction, ValueBinding,
    ValuePart, Widget, WidgetKind,
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

/// An ability slot: icon, cooldown sweep and key hint.
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
    /// Attach a `mod://<id>/<path>` image.
    #[must_use]
    fn image(self, asset: &str) -> Self;
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
    fn image(mut self, asset: &str) -> Self {
        self.style.image = Some(asset.to_string());
        self
    }
}
