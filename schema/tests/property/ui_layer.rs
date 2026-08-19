//! What a declared layer is, and what it is not (stormlight/server#110).
//!
//! Paint order used to be entirely implicit: within a tree it is declaration
//! order, and *between* trees it was whatever order the client happened to build
//! them in — which changes as roots come and go, so a summoned panel could land
//! over the console it is anchored to or under it, in the same session.
//!
//! [`Layout::layer`] makes it something a mod says. One field with one meaning at
//! both scales: **a layer orders a widget against its siblings**, and for a root
//! the other roots are its siblings. That is why it is on `Layout` rather than on
//! [`UiRoot`] — every construction site already goes through `Layout::default()`,
//! and a mod that has never heard of layers keeps the order it declared.
//!
//! What is pinned here:
//!
//!   - **the default is zero and it is the old behaviour** — an undeclared layer
//!     changes nothing, so this is additive to every HUD already written;
//!   - **it survives the wire** — a layer is part of the descriptor, not of the
//!     client's reading of it;
//!   - **it is not a validity rule** — any `i32` is a legal layer, including a
//!     negative one, which is how a mod puts something deliberately behind the
//!     interface everyone else declared at zero.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ui::{
    Layout, RootVisibility, Style, SummonGate, UiRoot, UiSubject, Widget, WidgetKind,
};

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// The layer the root declares.
    root: i32,
    /// The layer each of its children declares.
    children: Vec<i32>,
}

fn a_widget(name: &str, layer: i32, children: Vec<Widget>) -> Widget {
    Widget {
        name: name.to_string(),
        layout: Layout { layer, ..Layout::default() },
        style: Style::default(),
        kind: WidgetKind::Panel { children },
    }
}

fn a_root(s: &Scenario) -> UiRoot {
    let children = s
        .children
        .iter()
        .enumerate()
        .map(|(i, &layer)| a_widget(&format!("child{i}"), layer, Vec::new()))
        .collect();
    UiRoot {
        name: "layered".into(),
        when: RootVisibility::Always,
        summon: SummonGate::Ignored,
        subject: UiSubject::LocalPlayer,
        root: a_widget("root", s.root, children),
    }
}

#[test]
fn an_undeclared_layer_is_zero() {
    assert_eq!(Layout::default().layer, 0, "a HUD that never heard of layers is not at zero");
}

#[test]
fn a_layer_survives_the_trip_to_the_host() {
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = a_root(s);
        let bytes = postcard::to_allocvec(&declared).expect("a root encodes");
        let decoded: UiRoot = postcard::from_bytes(&bytes).expect("and decodes");
        assert_eq!(decoded, declared, "a declared layer did not survive the wire");
    });
}

#[test]
fn any_layer_is_a_legal_one() {
    check!().with_type::<Scenario>().for_each(|s| {
        assert_eq!(
            a_root(s).validate(),
            Ok(()),
            "a layer was refused; ordering is a decision, not a way to be wrong",
        );
    });
}

#[test]
fn declaring_a_layer_changes_the_layer_and_nothing_else() {
    check!().with_type::<Scenario>().for_each(|s| {
        let plain = Layout::default();
        let layered = Layout { layer: s.root, ..plain };
        assert_eq!(Layout { layer: 0, ..layered }, plain, "a layer moved something else too");
    });
}
