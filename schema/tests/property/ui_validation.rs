//! Structural validation of the UI ABI (stormlight/server#66).
//!
//! A widget tree is interpreted by a client that has never seen the interface: an
//! icon naming no image, a NaN width, a tree nested past what the renderer will
//! walk are each a blank rectangle at runtime and no explanation for the author.
//! `validate` is the load-time gate that turns every one into a named error, so a
//! break is reported where the mod is *loaded* rather than where it is *drawn*.
//!
//! Shape of the property: build a well-formed root from the generated counts,
//! apply exactly one structural break chosen by the scenario, and assert
//! `validate` accepts the untouched root and rejects each break with its own
//! variant. Directional — the *classification* is the invariant, never a
//! magnitude.
//!
//! A **cycle** is deliberately absent from the break list: a widget owns its
//! children by value, so a cyclic tree is unrepresentable in the ABI rather than
//! merely rejected by it. Unbounded *depth* is the reachable half of that hazard,
//! and it is what [`MAX_UI_DEPTH`] pins.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{Slot, StatId};
use stormlight_mod_abi::ui::{
    Anchor, Border, Flow, InteractionStyle, Layout, Length, MAX_UI_DEPTH, MAX_UI_WIDGETS,
    RootVisibility, Shown, Slice, StateStyle, Style, SummonGate, Sweep, TextSource, UiError,
    UiRoot, UiSubject, ValueBinding, Widget, WidgetKind,
};

extern crate alloc;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// One way to break a root. `None` leaves it well-formed.
#[derive(Debug, TypeGenerator, Clone, Copy, PartialEq, Eq)]
enum Break {
    /// Leave it alone — the control case.
    None,
    /// Strip the root's name.
    EmptyRootName,
    /// Read the hovered unit from a tree that is never gated on a hover.
    SubjectNeverPresent,
    /// Nest one level past what the renderer will walk.
    TooDeep,
    /// Put more widgets in one tree than the renderer will build.
    TooManyWidgets,
    /// An icon that names no image at all.
    IconWithoutImage,
    /// An image path that is present but empty.
    EmptyImagePath,
    /// A *state* override naming an empty image — the same break, hidden behind
    /// the pointer arriving (server#69).
    EmptyStateImagePath,
    /// A NaN in a state override's colour, likewise.
    NonFiniteState,
    /// Put a NaN in the root's width.
    NonFinite,
    /// Ask for a negative font size.
    NegativeMetric,
    /// A nine-slice inset that is negative — an extent like any other, and the
    /// one a stretched frame's corners depend on.
    NegativeSlice,
    /// And one that is not a number at all.
    NonFiniteSlice,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// 0..=3 children under the root.
    children: u8,
    brk: Break,
}

fn a_layout() -> Layout {
    Layout {
        anchor: Anchor::TopLeft,
        offset: [Length::Px(8.0), Length::Px(-8.0)],
        size: [Length::Fraction(0.25), Length::Auto],
        flow: Flow::Column,
        gap: 4.0,
        padding: 2.0,
        shown: Shown::Always,
    }
}

fn a_style() -> Style {
    Style {
        color: [1.0, 1.0, 1.0, 1.0],
        background: [0.0, 0.0, 0.0, 0.5],
        border: Border { color: [1.0, 1.0, 1.0, 1.0], width: 1.0 },
        font_size: 14.0,
        font: None,
        image: None,
        slice: None,
        flip_x: false,
        flip_y: false,
        states: InteractionStyle::default(),
        sweep: Sweep::default(),
        anim: Vec::new(),
        transition: None,
    }
}

fn a_widget(name: &str, kind: WidgetKind) -> Widget {
    Widget { name: name.to_string(), layout: a_layout(), style: a_style(), kind }
}

fn an_icon(name: &str) -> Widget {
    let mut w = a_widget(name, WidgetKind::Icon);
    w.style.image = Some("mod://m/icon.png".to_string());
    w
}

/// One well-formed child per index, cycling the leaf kinds so the control case
/// covers every one of them.
fn a_child(i: usize) -> Widget {
    let name = format!("child{i}");
    match i % 4 {
        0 => a_widget(&name, WidgetKind::Text { text: TextSource::Literal("hp".to_string()) }),
        1 => a_widget(&name, WidgetKind::Bar { value: ValueBinding::Health }),
        2 => an_icon(&name),
        _ => a_widget(
            &name,
            WidgetKind::AbilitySlot { slot: Slot(i as u8), key_hint: "Q".to_string() },
        ),
    }
}

/// A `depth`-deep chain of panels ending in one text leaf. Depth counts the
/// widgets on the path, so `depth == 1` is the leaf alone.
fn a_chain(depth: usize) -> Widget {
    let mut w = a_widget("leaf", WidgetKind::Text { text: TextSource::Literal("x".to_string()) });
    for level in 1..depth {
        w = a_widget(&format!("panel{level}"), WidgetKind::Panel { children: vec![w] });
    }
    w
}

fn a_root(children: usize) -> UiRoot {
    let kids: Vec<Widget> = (0..children).map(a_child).collect();
    UiRoot {
        name: "hud".to_string(),
        when: RootVisibility::Always,
        summon: SummonGate::Ignored,
        subject: UiSubject::LocalPlayer,
        root: a_widget("root", WidgetKind::Panel { children: kids }),
    }
}

fn apply(brk: Break, root: &mut UiRoot) {
    match brk {
        Break::None => {}
        Break::EmptyRootName => root.name = String::new(),
        Break::SubjectNeverPresent => {
            root.when = RootVisibility::Always;
            root.subject = UiSubject::HoveredUnit;
        }
        Break::TooDeep => root.root = a_chain(MAX_UI_DEPTH + 1),
        Break::TooManyWidgets => {
            let kids: Vec<Widget> = (0..=MAX_UI_WIDGETS).map(a_child).collect();
            root.root = a_widget("root", WidgetKind::Panel { children: kids });
        }
        Break::IconWithoutImage => {
            root.root = a_widget("icon", WidgetKind::Icon);
        }
        Break::EmptyImagePath => root.root.style.image = Some(String::new()),
        Break::EmptyStateImagePath => {
            root.root.style.states.hover =
                Some(StateStyle { image: Some(String::new()), ..StateStyle::default() });
        }
        Break::NonFiniteState => {
            root.root.style.states.disabled = Some(StateStyle {
                background: Some([f32::NAN, 0.0, 0.0, 1.0]),
                ..StateStyle::default()
            });
        }
        Break::NonFinite => root.root.layout.size[0] = Length::Px(f32::NAN),
        Break::NegativeMetric => root.root.style.font_size = -1.0,
        Break::NegativeSlice => {
            root.root.style.slice = Some(Slice { left: -1.0, top: 0.0, right: 0.0, bottom: 0.0 });
        }
        Break::NonFiniteSlice => {
            root.root.style.slice =
                Some(Slice { left: 0.0, top: f32::NAN, right: 0.0, bottom: 0.0 });
        }
    }
}

#[test]
fn a_well_formed_root_validates_and_each_break_is_classified() {
    check!().with_type::<Scenario>().for_each(|s| {
        let children = (s.children % 4) as usize;
        let mut root = a_root(children);
        apply(s.brk, &mut root);
        let result = root.validate();

        match s.brk {
            Break::None => assert!(result.is_ok(), "well-formed root rejected: {result:?}"),
            Break::EmptyRootName => assert_eq!(result, Err(UiError::EmptyRootName)),
            Break::SubjectNeverPresent => assert_eq!(result, Err(UiError::SubjectNeverPresent)),
            Break::TooDeep => {
                assert!(matches!(result, Err(UiError::TooDeep { .. })), "got {result:?}");
            }
            Break::TooManyWidgets => {
                assert_eq!(result, Err(UiError::TooManyWidgets), "got {result:?}");
            }
            Break::IconWithoutImage => {
                assert_eq!(result, Err(UiError::IconWithoutImage { widget: 0 }));
            }
            Break::EmptyImagePath | Break::EmptyStateImagePath => {
                assert_eq!(result, Err(UiError::EmptyImagePath { widget: 0 }));
            }
            Break::NonFiniteState => assert_eq!(result, Err(UiError::NonFinite { widget: 0 })),
            Break::NonFinite => assert_eq!(result, Err(UiError::NonFinite { widget: 0 })),
            Break::NegativeMetric | Break::NegativeSlice => {
                assert_eq!(result, Err(UiError::NegativeMetric { widget: 0 }));
            }
            Break::NonFiniteSlice => {
                assert_eq!(result, Err(UiError::NonFinite { widget: 0 }));
            }
        }
    });
}

#[test]
fn a_tree_exactly_at_the_depth_limit_is_accepted() {
    check!().with_type::<()>().for_each(|()| {
        let mut root = a_root(0);
        root.root = a_chain(MAX_UI_DEPTH);
        assert_eq!(root.validate(), Ok(()), "the limit itself must be reachable");
        // And one level past it is not — the limit is a boundary, not a hint.
        root.root = a_chain(MAX_UI_DEPTH + 1);
        assert!(matches!(root.validate(), Err(UiError::TooDeep { .. })));
    });
}

#[test]
fn a_break_is_reported_at_the_widget_it_sits_on() {
    check!().with_type::<u8>().for_each(|&seed| {
        // Pre-order index: the root is 0, its children follow in declaration
        // order — so an author reading the error can count to the widget.
        let at = (seed % 3) as usize;
        let mut root = a_root(3);
        let WidgetKind::Panel { children } = &mut root.root.kind else { unreachable!() };
        children[at] = a_widget("broken", WidgetKind::Icon);
        assert_eq!(root.validate(), Err(UiError::IconWithoutImage { widget: at as u16 + 1 }));
    });
}

#[test]
fn a_per_unit_tree_needs_no_visibility_gate() {
    check!().with_type::<u8>().for_each(|&seed| {
        // `HoveredUnit` reads a subject that exists only while something is
        // hovered, so it is refused without that gate. `EachUnit` is the opposite
        // case: it is instanced *per* unit, so its subject is present under every
        // visibility — and refusing a combination here would be refusing a
        // nameplate set that a talent panel happens to gate.
        let when = match seed % 3 {
            0 => RootVisibility::Always,
            1 => RootVisibility::WhileTalentPending,
            _ => RootVisibility::WhileUnitHovered,
        };
        let mut root = a_root(2);
        root.when = when;
        root.subject = UiSubject::EachUnit;
        assert_eq!(root.validate(), Ok(()), "a per-unit tree is valid under {when:?}");

        // The hovered-unit rule is untouched by the new variant.
        root.subject = UiSubject::HoveredUnit;
        assert_eq!(
            root.validate(),
            if when == RootVisibility::WhileUnitHovered {
                Ok(())
            } else {
                Err(UiError::SubjectNeverPresent)
            },
        );
    });
}

#[test]
fn a_bound_stat_does_not_affect_validity() {
    check!().with_type::<u16>().for_each(|&raw| {
        // Whether a bound handle *resolves* is adoption's business (the name
        // tables are not here); validation only ever judges the shape.
        let mut root = a_root(0);
        root.root = a_widget("bar", WidgetKind::Bar { value: ValueBinding::Stat(StatId(raw)) });
        assert_eq!(root.validate(), Ok(()));
    });
}
