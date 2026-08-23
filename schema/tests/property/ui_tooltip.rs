//! What a widget says about itself under the pointer (stormlight/server#112).
//!
//! A tooltip is a **subtree of the widget it belongs to**, not a root with a subject
//! of its own. That shape is the reason nothing new had to be invented for it: a
//! talent's tooltip is `TextSource::Talent` at the coordinate its row already names,
//! so it reads the same bindings everything else does and needs no new source.
//!
//! It sits on [`Style`] rather than on [`Widget`] for a duller reason and a real
//! one. The dull one is that `Style` has a `Default` a hundred construction sites go
//! through, and `Widget` does not. The real one is that it belongs beside
//! [`Style::states`]: both answer "what does this widget do while the pointer is on
//! it".
//!
//! What is pinned:
//!
//!   - **it is a real subtree** — the walk that validates a tree descends into it,
//!     so a tooltip cannot carry an icon with no picture or a NaN width that the
//!     rest of the tree would have been refused for;
//!   - **it counts against the budget** — a tooltip is widgets that are really laid
//!     out, so a HUD cannot smuggle a thousand of them past [`MAX_UI_WIDGETS`];
//!   - **empty is the ordinary case** and survives the wire, because that is what
//!     every widget in every existing HUD now carries.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ui::{
    Layout, MAX_UI_WIDGETS, RootVisibility, Strip, Style, SummonGate, TextSource, Tooltip,
    UiAction, UiError, UiRoot, UiSubject, Widget, WidgetKind,
};

// Its handles crossing the local→global bridge is pinned where the rest of that
// property lives, in `tests/integration/ui_action_remap.rs`.

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// How many widgets the tooltip's own tree holds.
    inside: u8,
    /// Whether the tooltip holds something that would be refused on its own — an
    /// icon with no picture.
    broken: bool,
    /// How many widgets the tree outside the tooltip holds.
    outside: u8,
}

fn a_widget(name: &str, kind: WidgetKind) -> Widget {
    Widget { name: name.to_string(), layout: Layout::default(), style: Style::default(), kind }
}

fn a_label(i: usize) -> Widget {
    a_widget(&format!("line{i}"), WidgetKind::Text { text: TextSource::Literal("what".into()) })
}

/// A row with a tooltip on it, and `outside` plain siblings beside it.
fn a_root(s: &Scenario) -> UiRoot {
    let mut content: Vec<Widget> = (0..usize::from(s.inside)).map(a_label).collect();
    if s.broken {
        // An icon with no picture: legal to write, refused by the walk. Whether the
        // walk reaches *into* a tooltip is the whole question.
        content.push(a_widget("blank", WidgetKind::Icon));
    }
    let row = Widget {
        style: Style {
            tooltip: Tooltip {
                content,
                anchor: Default::default(),
                offset: [12.0, 8.0],
                delay: 0.25,
            },
            ..Style::default()
        },
        ..a_widget(
            "row",
            WidgetKind::Button {
                action: UiAction::PickTalent { tier: 0, option: 0 },
                children: Vec::new(),
            },
        )
    };
    let mut children = vec![row];
    children.extend((0..usize::from(s.outside)).map(|i| a_label(100 + i)));
    UiRoot {
        name: "panel".into(),
        when: RootVisibility::Always,
        summon: SummonGate::Ignored,
        subject: UiSubject::LocalPlayer,
        strip: Strip::default(),
        root: a_widget("panel", WidgetKind::Panel { children }),
    }
}

#[test]
fn a_tooltip_survives_the_trip_to_the_host() {
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = a_root(s);
        let bytes = postcard::to_allocvec(&declared).expect("a panel encodes");
        let decoded: UiRoot = postcard::from_bytes(&bytes).expect("and decodes");
        assert_eq!(decoded, declared, "a tooltip did not survive the wire");
    });
}

#[test]
fn a_tooltips_own_tree_is_held_to_every_rule_the_rest_of_the_tree_is() {
    check!().with_type::<Scenario>().for_each(|s| {
        let total = 2 + usize::from(s.inside) + usize::from(s.broken) + usize::from(s.outside);
        if total > MAX_UI_WIDGETS {
            return;
        }
        match a_root(s).validate() {
            Ok(()) => assert!(
                !s.broken,
                "a tooltip holding an icon with no picture was accepted; the walk that \
                 validates a tree does not reach inside one",
            ),
            Err(UiError::IconWithoutImage { .. }) => {
                assert!(s.broken, "a sound tooltip was refused for a picture it has");
            }
            Err(other) => panic!("a tooltip was refused for something else entirely: {other:?}"),
        }
    });
}

#[test]
fn a_tooltip_counts_against_the_widget_budget() {
    // Enough tooltip content to blow the budget on its own. Counted, this is
    // refused; uncounted, a HUD ships a thousand widgets the interpreter has to lay
    // out and the cap means nothing.
    let content: Vec<Widget> = (0..=MAX_UI_WIDGETS).map(a_label).collect();
    let row = Widget {
        style: Style { tooltip: Tooltip { content, ..Tooltip::default() }, ..Style::default() },
        ..a_widget("row", WidgetKind::Panel { children: Vec::new() })
    };
    let root = UiRoot {
        name: "panel".into(),
        when: RootVisibility::Always,
        summon: SummonGate::Ignored,
        subject: UiSubject::LocalPlayer,
        strip: Strip::default(),
        root: a_widget("panel", WidgetKind::Panel { children: vec![row] }),
    };
    assert_eq!(
        root.validate(),
        Err(UiError::TooManyWidgets),
        "a tooltip's tree is not counted, so the widget cap can be walked straight past",
    );
}

#[test]
fn an_empty_tooltip_is_what_a_widget_carries_by_default() {
    assert!(
        Style::default().tooltip.is_empty(),
        "every widget now says something under the pointer"
    );
    // And it round-trips, which is the case every existing HUD is made of.
    let bytes = postcard::to_allocvec(&Style::default()).expect("a style encodes");
    let decoded: Style = postcard::from_bytes(&bytes).expect("and decodes");
    assert_eq!(decoded, Style::default(), "an empty tooltip did not survive the wire");
}
