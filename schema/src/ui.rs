//! The UI descriptor ABI (stormlight/server#66) — what a mod is allowed to say
//! about an interface, so the content-free client can present a full HUD while
//! shipping no widget of its own.
//!
//! The third leg of the cosmetic bundle: [`VisualDescriptor`](crate::visuals::VisualDescriptor)
//! says how a unit *looks*, [`AnimationDescriptor`](crate::animation::AnimationDescriptor)
//! how it *moves*, and this says what the player *reads and clicks*. Like both of
//! those it is pure serializable data, carried in
//! [`ClientRegistration`](crate::visuals::ClientRegistration) and interpreted by
//! the client.
//!
//! ## Deliberately small
//!
//! This is not a general-purpose UI language, and growing it into one is a
//! non-goal: it is the smallest vocabulary that expresses a real HUD. Six widget
//! kinds, one anchor-plus-offset layout model, five style properties, and a
//! closed set of bindings. A mod that needs a *seventh* kind of thing on screen
//! composes it out of [`WidgetKind::Panel`] and the leaves, the way an ability is
//! composed out of the effect ISA rather than given a new leaf.
//!
//! ## Bindings name *what*, never *where*
//!
//! A widget's dynamic value is a [`ValueBinding`]: "the health of the subject",
//! "this resource pool", "that slot's cooldown". It never names a component, a
//! field or an address — the client resolves it against whatever generic state it
//! holds, and drops the widget cleanly when that state is absent (a bar bound to
//! a pool a unit does not have). This is what lets the engine change how it
//! stores vitals without breaking a single mod.
//!
//! ## One declaration, however many copies
//!
//! A tree is declared once. [`UiSubject::EachUnit`] is what turns that one
//! declaration into a nameplate over every unit on screen: the client instances
//! it per unit and reads each instance's bindings against the unit it hangs on.
//! There is no repeater and no per-unit authoring — a mod that wants health over
//! heads declares a bar exactly once.
//!
//! ## A tree, not a graph
//!
//! A widget owns its children by value, so a *cyclic* interface is unrepresentable
//! rather than merely rejected. What is representable — and what
//! [`UiRoot::validate`] rejects at load — is a tree deeper or wider than the
//! renderer will walk ([`MAX_UI_DEPTH`], [`MAX_UI_WIDGETS`]).
//!
//! ## A widget asks; it never acts
//!
//! An interactive widget carries a [`UiAction`], and every variant of that is a
//! *request* the server validates — a cast of a slot, a pick from a tier, or a
//! mod-defined event raised into that mod's own gameplay guest. A declared
//! interface therefore cannot do anything a player could not do with a keypress,
//! and cannot touch simulation state at all. [`WidgetKind::action`] is the one
//! place "what does this widget do" is answered, so a bar clicked with the mouse
//! and the same bar pressed with a key cannot drift apart (server#69).
//!
//! Handles inside bindings are authored in the mod's **local** id space and
//! remapped to global at adoption like every other family (see [`crate::remap`]).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::{EventId, Slot, StatId};
use crate::impacts::PoolRef;
use crate::remap::{IdMap, RemapIds};

/// How deeply one widget tree may nest, counting the root as level 1.
///
/// A HUD is a shallow thing — a panel of rows of bars is four levels — so the
/// limit is generous for anything an author means to build and still bounds the
/// walk the renderer performs every time the tree is (re)built.
pub const MAX_UI_DEPTH: usize = 16;

/// How many widgets one tree may hold in total.
///
/// The interface is rebuilt from the descriptor, so an unbounded tree is an
/// unbounded amount of work on the frame it appears. A bundle that wants more
/// than this splits it across several [`UiRoot`]s, which the client can show and
/// hide independently.
pub const MAX_UI_WIDGETS: usize = 256;

/// A distance along one axis.
///
/// Two units, because both are needed and neither substitutes for the other: art
/// and text want absolute pixels, while a bar that should span a quarter of the
/// screen wants a fraction of its parent. [`Self::Auto`] is the third case a HUD
/// cannot do without — a label or an icon sized by its own content.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum Length {
    /// Absolute logical pixels.
    Px(f32),
    /// A fraction of the parent's extent on the same axis (`1.0` is all of it).
    Fraction(f32),
    /// Sized by content — the text's extent, the image's size, the children.
    #[default]
    Auto,
}

impl Length {
    /// Whether the number behind it is finite. [`Self::Auto`] carries none.
    #[must_use]
    pub fn is_finite(self) -> bool {
        match self {
            Self::Px(v) | Self::Fraction(v) => v.is_finite(),
            Self::Auto => true,
        }
    }

    /// Whether it denotes a negative extent — meaningless as a *size*, ordinary
    /// as an *offset*.
    #[must_use]
    pub fn is_negative(self) -> bool {
        match self {
            Self::Px(v) | Self::Fraction(v) => v < 0.0,
            Self::Auto => false,
        }
    }
}

/// Which point of the parent a widget's offset is measured from.
///
/// Corner-anchoring rather than absolute coordinates is what makes a HUD survive
/// a different resolution: "16px in from the bottom-left" stays where the author
/// put it on every screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Anchor {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// How a container arranges its children.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Flow {
    /// Left to right.
    Row,
    /// Top to bottom.
    #[default]
    Column,
    /// All at the same place, in declaration order — the back-to-front stack a
    /// cooldown sweep over an icon needs.
    Stack,
}

/// Where a widget sits and how big it is. Enough for corners, rows, columns and
/// stacks; deliberately not a flexbox surface.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Layout {
    /// Which point of the parent [`Self::offset`] is measured from.
    pub anchor: Anchor,
    /// Displacement from the anchor, `[x, y]`, positive right and down. May be
    /// negative — that is how a bottom-anchored widget moves *up* off the edge.
    pub offset: [Length; 2],
    /// Extent, `[width, height]`.
    pub size: [Length; 2],
    /// How this widget arranges its own children (ignored by the leaf kinds).
    pub flow: Flow,
    /// Pixels between adjacent children.
    pub gap: f32,
    /// Pixels between this widget's edge and its children.
    pub padding: f32,
}

/// A widget's outline.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Border {
    /// Linear RGBA, `0.0..=1.0`.
    pub color: [f32; 4],
    /// Thickness in pixels; `0.0` draws none.
    pub width: f32,
}

/// Which of the four appearances an interactive widget is currently wearing
/// (stormlight/server#69).
///
/// Exactly four, because they are the four *facts* a pointer and a gate can
/// produce between them, not a palette of moods: the pointer is over it or not,
/// the button is held or not, and the request it would send is one the client
/// already knows would be refused. Anything finer — a focus ring, a toggled-on
/// tab — is state the descriptor would have to carry, and this ABI does not hold
/// widget state.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum WidgetState {
    /// At rest: the declared [`Style`] exactly as authored.
    #[default]
    Idle,
    /// The pointer is over it.
    Hovered,
    /// It is being held down.
    Pressed,
    /// Its action cannot be taken right now — a slot on cooldown, a talent tier
    /// the unit has not reached.
    Disabled,
}

/// What changes about a widget's painting while it is in one interaction state.
///
/// Every property is optional **on its own**, and that is the whole design: a mod
/// that only brightens a button on hover keeps its declared picture, and one that
/// only swaps the picture keeps its declared colours. A whole replacement
/// [`Style`] would make every author restate the properties they did not mean to
/// change, and the first one they forgot would flicker.
///
/// Three properties rather than five: colour, background and picture are what a
/// HUD's states actually differ in. A border that thickened on hover or text that
/// grew on press would move the layout under the pointer, which is a worse
/// interface than one that does not.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct StateStyle {
    /// Replaces [`Style::color`] while the state holds.
    pub color: Option<[f32; 4]>,
    /// Replaces [`Style::background`].
    pub background: Option<[f32; 4]>,
    /// Replaces [`Style::image`] — a whole different picture, which is how a real
    /// HUD draws a hovered or unavailable button.
    pub image: Option<String>,
}

impl StateStyle {
    /// Whether this override names a picture that is present but empty — a
    /// `mod://` URL to nothing, the same break [`UiError::EmptyImagePath`] catches
    /// on a base style.
    #[must_use]
    pub fn has_empty_image(&self) -> bool {
        self.image.as_ref().is_some_and(String::is_empty)
    }

    /// Whether every colour it declares is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        let finite = |c: &Option<[f32; 4]>| c.iter().flatten().all(|v| v.is_finite());
        finite(&self.color) && finite(&self.background)
    }
}

/// A widget's three non-resting appearances. All optional: a widget that declares
/// none does not react to the pointer at all, which is the right default for the
/// text and bars that make up most of a HUD.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct InteractionStyle {
    /// While the pointer is over it.
    pub hover: Option<StateStyle>,
    /// While it is held down.
    pub press: Option<StateStyle>,
    /// While its action would be refused.
    pub disabled: Option<StateStyle>,
}

impl InteractionStyle {
    /// The override for one state, or `None` for [`WidgetState::Idle`] and for a
    /// state this widget never declared.
    ///
    /// States never *inherit* from one another: a widget with a hover style and no
    /// press style stays hovered-looking while held, rather than the engine
    /// darkening the hover into a press it was never given. Borrowing one state's
    /// appearance for another is the engine deciding how a mod's button looks.
    #[must_use]
    pub fn get(&self, state: WidgetState) -> Option<&StateStyle> {
        match state {
            WidgetState::Idle => None,
            WidgetState::Hovered => self.hover.as_ref(),
            WidgetState::Pressed => self.press.as_ref(),
            WidgetState::Disabled => self.disabled.as_ref(),
        }
    }

    /// Every override declared, in a fixed order — what validation walks.
    pub fn declared(&self) -> impl Iterator<Item = &StateStyle> {
        [self.hover.as_ref(), self.press.as_ref(), self.disabled.as_ref()].into_iter().flatten()
    }

    /// Whether any declared override names an empty picture.
    #[must_use]
    pub fn has_empty_image(&self) -> bool {
        self.declared().any(StateStyle::has_empty_image)
    }

    /// Whether every declared override's colours are finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.declared().all(StateStyle::is_finite)
    }
}

/// How a widget is painted. The five properties a HUD actually uses, plus what
/// changes about three of them while the pointer is on it.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Style {
    /// Foreground: text colour, icon tint, the *filled* part of a bar. Linear
    /// RGBA, `0.0..=1.0`.
    pub color: [f32; 4],
    /// Background: a panel's fill, the *empty* part of a bar.
    pub background: [f32; 4],
    /// The outline.
    pub border: Border,
    /// Text height in pixels, for the kinds that draw text.
    pub font_size: f32,
    /// A `mod://<id>/<path>` image, resolved within the declaring package.
    /// Required by [`WidgetKind::Icon`]; optional decoration on anything else.
    pub image: Option<String>,
    /// What changes while the pointer is over it, holding it, or while its action
    /// would be refused (stormlight/server#69). Inert on a widget that does
    /// nothing when clicked — there is no state for it to be in.
    #[serde(default)]
    pub states: InteractionStyle,
}

impl Default for Style {
    /// Opaque white foreground on nothing, no border, ordinary text size, and no
    /// reaction to the pointer — the neutral base an author overrides one
    /// property at a time.
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            background: [0.0, 0.0, 0.0, 0.0],
            border: Border::default(),
            font_size: 16.0,
            image: None,
            states: InteractionStyle::default(),
        }
    }
}

impl Style {
    /// This style as it is painted in `state`: the declared base with that state's
    /// overrides applied, and every property it did not name left alone.
    ///
    /// Total and allocating a fresh style rather than mutating: a widget's
    /// *declared* appearance is what it returns to when the pointer leaves, so the
    /// base must survive every state it passes through.
    #[must_use]
    pub fn resolve(&self, state: WidgetState) -> Self {
        let mut out = self.clone();
        let Some(over) = self.states.get(state) else { return out };
        if let Some(color) = over.color {
            out.color = color;
        }
        if let Some(background) = over.background {
            out.background = background;
        }
        if let Some(image) = &over.image {
            out.image = Some(image.clone());
        }
        out
    }
}

/// The piece of generic state a widget reads.
///
/// Closed on purpose: every variant is state the engine holds for *any* unit,
/// whatever mod defined it. Adding one means the engine gained a new generic
/// quantity, not that a mod needs a new field.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ValueBinding {
    /// Current hit points against the maximum — the one vital every unit has.
    /// Separate from [`Self::Pool`] because health is not an
    /// [`AdjustPool`](crate::impacts::Impact::AdjustPool) reserve on the gameplay
    /// side either; damage and healing are their own verbs.
    Health,
    /// A bounded reserve: a mod-defined resource, a stack counter, shield, XP, or
    /// a slot's cooldown / charges. Reuses the ISA's own
    /// [`PoolRef`] so a bar names a pool exactly as an effect does.
    Pool(PoolRef),
    /// An aggregated unit stat. It has no ceiling, so a bar bound to one reads
    /// full and only its [`ValuePart::Current`] is meaningful.
    Stat(StatId),
    /// The subject's level.
    Level,
    /// How far the cast or channel currently running has progressed, `0.0..=1.0`.
    /// Reads empty when nothing is casting.
    CastProgress,
}

/// Which number of a binding a text reads. A bar always reads the fraction.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ValuePart {
    /// The present value.
    #[default]
    Current,
    /// The ceiling (the same as [`Self::Current`] for an unbounded binding).
    Max,
    /// `current / max`, `0.0..=1.0`.
    Fraction,
}

/// A binding that yields a *list* rather than a number.
///
/// One variant, and no widget kind repeats a template over it: a
/// [`TextSource::List`] renders the entries as lines. That is the whole of the
/// list support, deliberately — a repeater is the point where a descriptor ABI
/// turns into a template language.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ListBinding {
    /// The talents the subject has chosen so far, in tier order.
    ChosenTalents,
}

/// What a [`WidgetKind::Text`] displays.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum TextSource {
    /// Text the mod authored. The engine ships none of its own.
    Literal(String),
    /// A number read from generic state, rendered with `decimals` fractional
    /// digits.
    Value { binding: ValueBinding, part: ValuePart, decimals: u8 },
    /// Every entry of a list binding, one per line.
    List(ListBinding),
}

/// What activating an interactive widget asks the server to do
/// (stormlight/server#69).
///
/// Every variant is a *request*, not an effect: the widget says what the player
/// wants and the server decides, exactly as it does for a keypress. That is the
/// whole safety property of the interface ABI — a mod's HUD can never do anything
/// a player could not do with a key, and it can never change simulation state at
/// all. The client refuses an impossible action locally only as a **courtesy**,
/// to save a round trip; the server validates every one of them regardless.
///
/// Closed, and small on purpose: these are the three things a player *does* to
/// their own unit. Anything a mod wants beyond them goes through
/// [`Self::Trigger`], where it is that mod's own gameplay guest — running
/// server-side, under the engine's rules — that decides what happens.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum UiAction {
    /// Cast the ability bound in this slot — byte-for-byte the request the
    /// keybind for that slot sends, aimed the same way.
    CastSlot(Slot),
    /// Take option `option` of tier `tier` of the subject's talent tree.
    ///
    /// An **option index**, not a talent handle, because the tree is the *unit's*
    /// declaration and a cosmetic mod that named a talent directly would be
    /// authoring gameplay content in a HUD. The client resolves the index against
    /// the tree its gameplay mods declared, and a tier or option that names
    /// nothing sends nothing at all.
    PickTalent { tier: u8, option: u8 },
    /// Raise a mod-defined event, routed to every gameplay guest subscribed to it
    /// (the `mod_trigger` fan-out of stormlight/server#34/#39).
    ///
    /// The extension point: a mod that wants a button doing something the three
    /// closed verbs do not cover declares an event, subscribes its gameplay guest
    /// to it, and gets back the whole effect ISA — **on the server**, against the
    /// clicking player's own unit. Nothing about that path is client-authoritative;
    /// the click is a request like the other two.
    Trigger { event: EventId },
}

/// What a widget *is*. Five leaves and one container, plus the one composite
/// worth blessing.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum WidgetKind {
    /// A layout container. Draws its own [`Style`] and arranges its children by
    /// its [`Layout`].
    Panel { children: Vec<Widget> },
    /// A line (or lines) of text.
    Text { text: TextSource },
    /// A value against its maximum, drawn as a fill: [`Style::color`] fills the
    /// bound fraction of [`Style::background`].
    Bar { value: ValueBinding },
    /// A picture, from [`Style::image`].
    Icon,
    /// A clickable container: a [`Self::Panel`] that sends a [`UiAction`].
    Button { action: UiAction, children: Vec<Widget> },
    /// An ability slot: its icon ([`Style::image`]), a cooldown sweep over it,
    /// and its key hint. The one composite blessed as a kind of its own, because
    /// every HUD needs it and composing it out of a stacked panel, a bar bound to
    /// [`PoolRef::Cooldown`] and a text would put the same twenty lines in every
    /// mod.
    AbilitySlot {
        /// Which loadout slot it shows.
        slot: Slot,
        /// The key to print on it (empty draws none). A hint, not a binding —
        /// input mapping is the player's, not the mod's.
        key_hint: String,
    },
}

impl WidgetKind {
    /// The children this kind contains — empty for every leaf.
    #[must_use]
    pub fn children(&self) -> &[Widget] {
        match self {
            Self::Panel { children } | Self::Button { children, .. } => children,
            Self::Text { .. } | Self::Bar { .. } | Self::Icon | Self::AbilitySlot { .. } => &[],
        }
    }

    /// What activating this widget asks for, or `None` for a kind that does
    /// nothing when clicked (stormlight/server#69).
    ///
    /// Two kinds are interactive and they arrive at their action differently: a
    /// [`Self::Button`] carries one explicitly, while a [`Self::AbilitySlot`]
    /// *is* a cast of the slot it draws — an ability bar you cannot click is not
    /// an ability bar. Stating that equivalence here, once, is what keeps a
    /// client from re-deriving it and letting a moused slot drift from the
    /// pressed one.
    #[must_use]
    pub fn action(&self) -> Option<UiAction> {
        match self {
            Self::Button { action, .. } => Some(*action),
            Self::AbilitySlot { slot, .. } => Some(UiAction::CastSlot(*slot)),
            Self::Panel { .. } | Self::Text { .. } | Self::Bar { .. } | Self::Icon => None,
        }
    }
}

/// One node of an interface: where it sits, how it is painted, and what it is.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Widget {
    /// Author-facing name, for diagnostics. May be empty.
    pub name: String,
    /// Where it sits and how big it is.
    pub layout: Layout,
    /// How it is painted.
    pub style: Style,
    /// What it is.
    pub kind: WidgetKind,
}

/// When a widget tree is on screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum RootVisibility {
    /// Whenever the player has a unit to read — the ordinary HUD.
    #[default]
    Always,
    /// While a talent tier is waiting to be picked.
    WhileTalentPending,
    /// While the cursor is over a unit — the frame that describes it.
    WhileUnitHovered,
}

/// Whose state a tree's bindings read — and, for the last variant, *where the
/// tree sits*, because a tree that reads a unit in the world is drawn over that
/// unit rather than in a corner.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum UiSubject {
    /// The unit this player drives.
    #[default]
    LocalPlayer,
    /// The unit under the cursor. Only ever present under
    /// [`RootVisibility::WhileUnitHovered`], which [`UiRoot::validate`] enforces.
    HoveredUnit,
    /// Every unit on screen, one instance of the tree each, anchored to the unit
    /// it reads — the nameplate case (stormlight/server#67).
    ///
    /// This is the one subject that also decides placement. A screen-anchored
    /// tree measures its [`Layout::anchor`] against the screen; a per-unit
    /// instance measures it against the point its unit projects to, so the same
    /// anchor-plus-offset vocabulary puts a bar above a head
    /// ([`Anchor::BottomCenter`], a negative `y`) with nothing new to learn. An
    /// instance whose unit is behind the camera or off screen is not drawn at
    /// all — never clamped to the edge, which would fill the border with the
    /// nameplates of things the player cannot see.
    EachUnit,
}

/// One declared widget tree: what it is called, when it is shown, whose state it
/// reads, and the tree itself.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct UiRoot {
    /// Stable name within the declaring mod, for diagnostics and for a host that
    /// wants to address one tree.
    pub name: String,
    /// When it is on screen.
    pub when: RootVisibility,
    /// Whose state its bindings read — and, for [`UiSubject::EachUnit`], that it
    /// is instanced per unit and anchored to it rather than to the screen.
    pub subject: UiSubject,
    /// The tree.
    pub root: Widget,
}

/// Why a [`UiRoot`] cannot be rendered as declared.
///
/// Each variant is a break the client would otherwise absorb silently — an icon
/// with no picture, a NaN width collapsing a panel — leaving the author with a
/// blank screen and nothing to read. `widget` is the tree's **pre-order** index:
/// the root is `0`, its first child `1`, that child's own children next.
#[derive(Clone, PartialEq, Debug)]
pub enum UiError {
    /// The root has no name, so nothing can refer to it in a diagnostic.
    EmptyRootName,
    /// A tree reads the hovered unit but is not gated on a hover, so its
    /// bindings could never resolve to anything.
    SubjectNeverPresent,
    /// Nested past [`MAX_UI_DEPTH`].
    TooDeep { widget: u16 },
    /// More than [`MAX_UI_WIDGETS`] widgets in one tree.
    TooManyWidgets,
    /// An [`WidgetKind::Icon`] whose style names no image.
    IconWithoutImage { widget: u16 },
    /// An image path that is present but empty — a `mod://` URL to nothing.
    EmptyImagePath { widget: u16 },
    /// A non-finite length, gap, padding, colour, border width or font size.
    NonFinite { widget: u16 },
    /// A negative size, gap, padding, border width or font size. (An *offset*
    /// may be negative; an extent may not.)
    NegativeMetric { widget: u16 },
}

impl fmt::Display for UiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRootName => f.write_str("ui root has no name"),
            Self::SubjectNeverPresent => {
                f.write_str("ui root reads the hovered unit but is not shown on hover")
            }
            Self::TooDeep { widget } => {
                write!(f, "widget {widget} nests past the {MAX_UI_DEPTH}-level limit")
            }
            Self::TooManyWidgets => {
                write!(f, "ui root holds more than {MAX_UI_WIDGETS} widgets")
            }
            Self::IconWithoutImage { widget } => write!(f, "icon {widget} names no image"),
            Self::EmptyImagePath { widget } => write!(f, "widget {widget} names an empty image"),
            Self::NonFinite { widget } => write!(f, "widget {widget} carries a non-finite number"),
            Self::NegativeMetric { widget } => {
                write!(f, "widget {widget} carries a negative extent")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for UiError {}

/// Whether every number a widget carries is finite.
fn is_finite(widget: &Widget) -> bool {
    let l = &widget.layout;
    let s = &widget.style;
    l.offset.iter().chain(l.size.iter()).all(|v| v.is_finite())
        && l.gap.is_finite()
        && l.padding.is_finite()
        && s.color
            .iter()
            .chain(s.background.iter())
            .chain(s.border.color.iter())
            .all(|v| v.is_finite())
        && s.border.width.is_finite()
        && s.font_size.is_finite()
        // A state's colours reach the same layout/paint path the base ones do,
        // so a NaN hidden in a hover override is the same break — it just waits
        // for the pointer to arrive before it costs the screen.
        && s.states.is_finite()
}

/// Whether any extent a widget carries is negative. Offsets are excluded: moving
/// a bottom-anchored widget up the screen is a negative `y` and nothing else.
fn is_negative(widget: &Widget) -> bool {
    widget.layout.size.iter().any(|v| v.is_negative())
        || widget.layout.gap < 0.0
        || widget.layout.padding < 0.0
        || widget.style.border.width < 0.0
        || widget.style.font_size < 0.0
}

impl UiRoot {
    /// Full structural validation, run by the host at load.
    ///
    /// Checks only what is decidable from the descriptor itself. Whether a bound
    /// handle has a name-table entry belongs to adoption, and whether a named
    /// image exists belongs to asset loading — neither is knowable here.
    ///
    /// The walk is iterative: a descriptor arriving from a mod may be nested far
    /// past what it is allowed to be, and the gate that says so must not be the
    /// thing that overflows the stack.
    pub fn validate(&self) -> Result<(), UiError> {
        if self.name.is_empty() {
            return Err(UiError::EmptyRootName);
        }
        if self.subject == UiSubject::HoveredUnit && self.when != RootVisibility::WhileUnitHovered {
            return Err(UiError::SubjectNeverPresent);
        }

        // Pre-order walk over an explicit stack, so an error's index is the
        // widget an author counts to in the source.
        let mut stack: Vec<(&Widget, usize)> = Vec::new();
        stack.push((&self.root, 1));
        let mut index: u16 = 0;
        let mut seen: usize = 0;

        while let Some((widget, depth)) = stack.pop() {
            seen += 1;
            if seen > MAX_UI_WIDGETS {
                return Err(UiError::TooManyWidgets);
            }
            if depth > MAX_UI_DEPTH {
                return Err(UiError::TooDeep { widget: index });
            }
            if !is_finite(widget) {
                return Err(UiError::NonFinite { widget: index });
            }
            if is_negative(widget) {
                return Err(UiError::NegativeMetric { widget: index });
            }
            match &widget.style.image {
                Some(path) if path.is_empty() => {
                    return Err(UiError::EmptyImagePath { widget: index });
                }
                None if matches!(widget.kind, WidgetKind::Icon) => {
                    return Err(UiError::IconWithoutImage { widget: index });
                }
                _ => {}
            }
            // The same rule for the pictures a state swaps in. Checked here rather
            // than left to the loader: a hover image that resolves to nothing is a
            // break the author only ever sees by pointing at the widget, which is
            // the worst possible time to discover it.
            if widget.style.states.has_empty_image() {
                return Err(UiError::EmptyImagePath { widget: index });
            }
            // Reversed, so popping yields declaration order and the index an
            // error reports is the one the author reads down the file.
            for child in widget.kind.children().iter().rev() {
                stack.push((child, depth + 1));
            }
            index += 1;
        }
        Ok(())
    }
}

impl RemapIds for ValueBinding {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            // Health, level and cast progress are the engine's own quantities;
            // no mod ever named them, so there is no handle to rewrite.
            Self::Health | Self::Level | Self::CastProgress => {}
            Self::Pool(pool) => pool.remap_ids(m)?,
            Self::Stat(id) => *id = m.stat(*id)?,
        }
        Ok(())
    }
}

impl RemapIds for TextSource {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            // Authored text is content; a list binding names no family.
            Self::Literal(_) | Self::List(_) => {}
            Self::Value { binding, .. } => binding.remap_ids(m)?,
        }
        Ok(())
    }
}

impl RemapIds for UiAction {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            // A slot is a non-interned `Slot` and a talent option is an index into
            // the *unit's* tree — pure mod convention, like everywhere else. Only
            // the event names a handle, and it names one in the gameplay side's id
            // space, which is why the cosmetic bundle's map has to reach it.
            Self::CastSlot(_) | Self::PickTalent { .. } => {}
            Self::Trigger { event } => *event = m.event(*event)?,
        }
        Ok(())
    }
}

impl RemapIds for WidgetKind {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        match self {
            // An icon is an asset path; an ability slot's own `Slot` is not
            // interned, and the action it implies carries nothing else.
            Self::Icon | Self::AbilitySlot { .. } => {}
            Self::Panel { children } => children.remap_ids(m)?,
            Self::Button { action, children } => {
                action.remap_ids(m)?;
                children.remap_ids(m)?;
            }
            Self::Text { text } => text.remap_ids(m)?,
            Self::Bar { value } => value.remap_ids(m)?,
        }
        Ok(())
    }
}

impl RemapIds for Widget {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        // `name`, `layout` and `style` are diagnostics, geometry and asset
        // strings — never a handle.
        self.kind.remap_ids(m)
    }
}

impl RemapIds for UiRoot {
    fn remap_ids<M: IdMap>(&mut self, m: &M) -> Result<(), M::Error> {
        self.root.remap_ids(m)
    }
}
