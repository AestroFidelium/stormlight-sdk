//! What a tier says about one of its options (stormlight/server#118,
//! stormlight/server#119) — [`Shown::WhileOption`] and the three states it asks for.
//!
//! A panel is a Rust program looping over coordinates, so it declares a *rectangle*:
//! however many tiers by however many options its author wrote. What it cannot know
//! at compile time is how many options *this* unit's tier really offers, or which of
//! them the player took. Both are questions about a coordinate the panel has already
//! named, so both are answered by a gate on that coordinate rather than by a new
//! addressing scheme.
//!
//! | state | laid out while |
//! | --- | --- |
//! | [`OptionState::Offered`] | the tier has an option at this index at all |
//! | [`OptionState::Taken`] | this is the choice made in this tier |
//! | [`OptionState::PassedOver`] | the tier is decided and this is not what was taken |
//!
//! Beside them sits the one question about a tier no coordinate can answer —
//! [`Shown::WhileTierUndecided`] (stormlight/server#120), which is what steps a
//! socket's number aside once the tier it labels has an answer to show instead.
//!
//! ## What is pinned here, and what is not
//!
//! The **client** decides whether a gate holds — it is the only side that has the
//! player's own view of their own tree — so the rule itself is pinned there. What is
//! pinned here is everything the two sides have to agree about *before* the rule can
//! run: that a gate survives the wire byte for byte, that it names no interned
//! handle, that it never reaches what the widget is or does, and that a declaration
//! written before it existed still means "always".
//!
//! ## One gate per widget
//!
//! A widget carries one condition. A row that is both on a page and at a coordinate
//! says so with two nodes — a container per page holding rows gated on their own
//! coordinate — which is pinned here as the *structural* fact that a gate on a
//! parent and a gate on a child are independent declarations.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::remap::RemapIds;
use stormlight_mod_abi::ui::{
    Layout, OptionState, RootVisibility, Shown, Strip, Style, SummonGate, TextSource, UiRoot,
    UiSubject, Widget, WidgetKind,
};

extern crate alloc;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use crate::ui_tier_view::FailEverything;

/// The generated counterpart of [`OptionState`], so a scenario can name one.
#[derive(Debug, TypeGenerator, Clone, Copy, PartialEq, Eq)]
enum State {
    Offered,
    Taken,
    PassedOver,
}

impl State {
    fn state(self) -> OptionState {
        match self {
            Self::Offered => OptionState::Offered,
            Self::Taken => OptionState::Taken,
            Self::PassedOver => OptionState::PassedOver,
        }
    }
}

/// One declared row: the coordinate it sits at, the state it waits on, and the page
/// its container is on.
#[derive(Debug, TypeGenerator, Clone, Copy)]
struct Row {
    page: u8,
    tier: u8,
    option: u8,
    is: State,
}

impl Row {
    fn gate(self) -> Shown {
        Shown::WhileOption { tier: self.tier, option: self.option, is: self.is.state() }
    }
}

fn a_widget(kind: WidgetKind) -> Widget {
    Widget { name: "w".to_string(), layout: Layout::default(), style: Style::default(), kind }
}

/// A page container holding one gated row — the two-node shape a panel uses to say
/// "on this page *and* at this coordinate" — and beside it the label a decided tier
/// steps aside (server#120).
fn build(row: Row) -> Widget {
    let mut child = a_widget(WidgetKind::Text { text: TextSource::Literal("row".to_string()) });
    child.layout.shown = row.gate();
    let mut label = a_widget(WidgetKind::Text { text: TextSource::Literal("lvl".to_string()) });
    label.layout.shown = Shown::WhileTierUndecided(row.tier);
    let mut page = a_widget(WidgetKind::Panel { children: vec![child, label] });
    page.layout.shown = Shown::WhileTierSelected(row.page);
    page
}

fn a_panel(rows: &[Row]) -> UiRoot {
    UiRoot {
        name: "talents".to_string(),
        when: RootVisibility::Always,
        summon: SummonGate::Held,
        subject: UiSubject::LocalPlayer,
        strip: Strip::default(),
        root: a_widget(WidgetKind::Panel {
            children: rows.iter().copied().map(build).collect::<Vec<_>>(),
        }),
    }
}

#[test]
fn a_coordinate_gate_names_no_interned_handle() {
    check!().with_type::<Vec<Row>>().for_each(|rows| {
        let declared = a_panel(rows);
        let mut out = declared.clone();
        out.remap_ids(&FailEverything).expect("a coordinate is not a handle");
        assert_eq!(declared, out, "adoption rewrote a coordinate gate");
    });
}

#[test]
fn a_gated_panel_survives_a_postcard_round_trip() {
    check!().with_type::<Vec<Row>>().for_each(|rows| {
        let declared = a_panel(rows);
        let bytes = postcard::to_allocvec(&declared).expect("serialize");
        let back: UiRoot = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(declared, back, "a coordinate-gated panel did not round-trip");
        let again = postcard::to_allocvec(&back).expect("reserialize");
        assert_eq!(bytes, again, "coordinate-gated panel serialization is not stable");
    });
}

#[test]
fn every_conditional_gate_names_exactly_one_tier() {
    check!().with_type::<Row>().for_each(|&row| {
        for gate in
            [row.gate(), Shown::WhileTierSelected(row.tier), Shown::WhileTierUndecided(row.tier)]
        {
            assert_eq!(
                gate.tier(),
                Some(row.tier),
                "{gate:?} did not report the tier it is about, so a client cannot tell \
                 which row of which tree to ask",
            );
        }
        assert_eq!(Shown::Always.tier(), None, "the ungated case named a tier");
    });
}

#[test]
fn a_coordinate_gate_changes_neither_what_a_widget_is_nor_what_it_does() {
    check!().with_type::<Row>().for_each(|&row| {
        let ungated = a_widget(WidgetKind::Text { text: TextSource::Literal("row".to_string()) });
        let mut gated = ungated.clone();
        gated.layout.shown = row.gate();
        assert_eq!(ungated.kind, gated.kind, "a layout gate reached the widget's kind");
        assert_eq!(ungated.kind.action(), gated.kind.action(), "a layout gate reached the action");
        assert_eq!(ungated.style, gated.style, "a layout gate reached the widget's paint");
        assert_eq!(ungated.name, gated.name, "a layout gate reached the widget's name");
    });
}

/// The two-node shape has to stay two declarations. If a container's gate reached
/// into its children, a panel could no longer say "on this page" and "at this
/// coordinate" as separate facts, which is the whole reason one gate per widget is
/// enough.
#[test]
fn a_pages_gate_and_a_rows_gate_are_independent() {
    check!().with_type::<Row>().for_each(|&row| {
        let page = build(row);
        assert_eq!(page.layout.shown, Shown::WhileTierSelected(row.page));
        let WidgetKind::Panel { children } = &page.kind else { panic!("a page is a panel") };
        assert_eq!(children.len(), 2, "a page holds the row and the label it was built around");
        assert_eq!(children[0].layout.shown, row.gate(), "the page's gate overwrote the row's");
        assert_eq!(
            children[1].layout.shown,
            Shown::WhileTierUndecided(row.tier),
            "the page's gate overwrote its label's",
        );
    });
}

/// The default is the whole HUD: every widget declared before this existed goes
/// through `Layout::default()`.
#[test]
fn an_undeclared_gate_is_still_always_shown() {
    assert_eq!(Layout::default().shown, Shown::Always);
    assert_eq!(OptionState::default(), OptionState::Offered);
}
