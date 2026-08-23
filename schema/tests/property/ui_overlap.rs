//! A flow's gap may be negative, and only a gap (stormlight/server#114).
//!
//! A container's `gap` is the distance *between* two siblings, not an extent of
//! either — so unlike a width, a padding or a border it has a meaningful negative
//! value: the siblings overlap. Real HUD art is authored that way. A column of
//! plates drawn to bleed into each other by a few pixels reads as one stack rather
//! than as a run of cards, and a strip of sockets laid along a plate is a run of
//! overlapping buttons; a layout language that cannot say so has to abandon the
//! flow and place every element absolutely, which throws away the one thing a flow
//! is for — closing the hole where an element was not laid out.
//!
//! Shape of the property: generate a container and a metric to make negative, and
//! assert that the classification splits exactly one way — a negative **gap** is
//! accepted, and a negative size, padding, border width or font size is still
//! [`UiError::NegativeMetric`]. Directional: which side of the line each metric
//! falls on, never the number itself.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ui::{
    Flow, Layout, Length, RootVisibility, Strip, Style, SummonGate, UiError, UiRoot, UiSubject,
    Widget, WidgetKind,
};

extern crate alloc;
use alloc::string::ToString;
use alloc::vec;

/// Which of a widget's numbers the scenario drives below zero.
#[derive(Debug, TypeGenerator, Clone, Copy, PartialEq, Eq)]
enum Metric {
    /// The distance between two of its children — the one that may be negative.
    Gap,
    /// Its own width.
    Width,
    /// The inset between its edge and its children.
    Padding,
    /// Its outline.
    Border,
    /// The height its text is set at.
    FontSize,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    metric: Metric,
    /// How far below zero, as a positive magnitude the scenario negates. Bounded
    /// so a generated `f32` cannot be a NaN, which is a *different* rejection.
    magnitude: u8,
    flow: FlowKind,
}

/// [`Flow`] is not `TypeGenerator`, and only a container's flow matters here.
#[derive(Debug, TypeGenerator, Clone, Copy)]
enum FlowKind {
    Row,
    Column,
    Stack,
}

impl From<FlowKind> for Flow {
    fn from(kind: FlowKind) -> Self {
        match kind {
            FlowKind::Row => Flow::Row,
            FlowKind::Column => Flow::Column,
            FlowKind::Stack => Flow::Stack,
        }
    }
}

fn a_root(scenario: &Scenario) -> UiRoot {
    let below = -(f32::from(scenario.magnitude) + 1.0);
    let mut layout = Layout {
        size: [Length::Px(100.0), Length::Px(100.0)],
        flow: scenario.flow.into(),
        ..Layout::default()
    };
    let mut style = Style { font_size: 14.0, ..Style::default() };
    match scenario.metric {
        Metric::Gap => layout.gap = below,
        Metric::Width => layout.size[0] = Length::Px(below),
        Metric::Padding => layout.padding = below,
        Metric::Border => style.border.width = below,
        Metric::FontSize => style.font_size = below,
    }
    UiRoot {
        name: "overlap".to_string(),
        when: RootVisibility::Always,
        summon: SummonGate::default(),
        subject: UiSubject::LocalPlayer,
        strip: Strip::default(),
        root: Widget {
            name: "container".to_string(),
            layout,
            style,
            kind: WidgetKind::Panel { children: vec![a_child("first"), a_child("second")] },
        },
    }
}

fn a_child(name: &str) -> Widget {
    Widget {
        name: name.to_string(),
        layout: Layout { size: [Length::Px(20.0), Length::Px(20.0)], ..Layout::default() },
        style: Style::default(),
        kind: WidgetKind::Panel { children: vec![] },
    }
}

#[test]
fn only_the_gap_between_siblings_may_be_negative() {
    check!().with_type::<Scenario>().for_each(|scenario| {
        let verdict = a_root(scenario).validate();
        match scenario.metric {
            Metric::Gap => assert_eq!(
                verdict,
                Ok(()),
                "a negative gap is an overlap, which is a thing HUD art really does",
            ),
            _ => assert!(
                matches!(verdict, Err(UiError::NegativeMetric { .. })),
                "a negative extent is still refused: {verdict:?}",
            ),
        }
    });
}
