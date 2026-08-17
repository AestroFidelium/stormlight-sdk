//! Invariants of the summon gate — the one *input* condition a declared root can
//! name (stormlight/server#98).
//!
//! A root already says when the game state warrants showing it
//! ([`RootVisibility`]); the gate says whether the player is asking to see it.
//! The two are orthogonal on purpose, and these pin what that has to mean in the
//! ABI itself:
//!   - **The gate is inert to validation**: a tree is well-formed or malformed for
//!     structural reasons only, so the same tree gets the same verdict under every
//!     gate. A gate that could turn a good tree bad would be a second, hidden rule
//!     an author has to discover by toggling a key.
//!   - **Round-trip**: a root carries its gate across the wasm boundary unchanged,
//!     and re-serializes to identical bytes.
//!   - **Tag stability**: a gate's wire tag is its declaration index. The ABI is
//!     unversioned, so a further gate may only be appended — and `Ignored` must
//!     stay tag `0`, since it is what every root that predates the gate means.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ui::{
    Anchor, Flow, Layout, Length, RootVisibility, Style, SummonGate, TextSource, UiRoot, UiSubject,
    Widget, WidgetKind,
};

extern crate alloc;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

/// Which gate a scenario declares — the generated counterpart of [`SummonGate`].
#[derive(Debug, TypeGenerator, Clone, Copy, PartialEq, Eq)]
enum Gate {
    Ignored,
    Held,
    Released,
}

impl Gate {
    fn gate(self) -> SummonGate {
        match self {
            Self::Ignored => SummonGate::Ignored,
            Self::Held => SummonGate::Held,
            Self::Released => SummonGate::Released,
        }
    }
}

/// Which state condition it declares beside the gate.
#[derive(Debug, TypeGenerator, Clone, Copy, PartialEq, Eq)]
enum When {
    Always,
    TalentPending,
    UnitHovered,
}

impl When {
    fn visibility(self) -> RootVisibility {
        match self {
            Self::Always => RootVisibility::Always,
            Self::TalentPending => RootVisibility::WhileTalentPending,
            Self::UnitHovered => RootVisibility::WhileUnitHovered,
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    gate: Gate,
    when: When,
    /// Empty for some cases, which is exactly the malformed root the verdict
    /// invariant needs: a break the gate must neither cause nor cure.
    name_len: u8,
    /// How many children the tree carries, so the walk is not always trivial.
    children: u8,
}

fn a_tree(children: u8) -> Widget {
    let kids: Vec<Widget> = (0..u16::from(children) % 4)
        .map(|_| Widget {
            name: "line".to_string(),
            layout: Layout::default(),
            style: Style::default(),
            kind: WidgetKind::Text { text: TextSource::Literal("choose".to_string()) },
        })
        .collect();
    Widget {
        name: "alert".to_string(),
        layout: Layout {
            anchor: Anchor::BottomLeft,
            offset: [Length::Px(0.0), Length::Px(0.0)],
            size: [Length::Px(64.0), Length::Px(64.0)],
            flow: Flow::Stack,
            gap: 0.0,
            padding: 0.0,
        },
        style: Style::default(),
        kind: WidgetKind::Panel { children: kids },
    }
}

fn a_root(s: &Scenario) -> UiRoot {
    UiRoot {
        name: "x".repeat(usize::from(s.name_len % 3)),
        when: s.when.visibility(),
        summon: s.gate.gate(),
        // The one subject with a declared partner condition, so the generated
        // pairs include the combination `validate` refuses on its own.
        subject: UiSubject::LocalPlayer,
        root: a_tree(s.children),
    }
}

#[test]
fn the_gate_never_changes_a_trees_verdict() {
    check!().with_type::<Scenario>().for_each(|s| {
        let baseline = a_root(s).validate();
        for gate in [SummonGate::Ignored, SummonGate::Held, SummonGate::Released] {
            let mut root = a_root(s);
            root.summon = gate;
            assert_eq!(
                root.validate(),
                baseline,
                "{gate:?} changed the verdict of a tree it does not describe",
            );
        }
    });
}

#[test]
fn a_root_carries_its_gate_across_the_wire() {
    check!().with_type::<Scenario>().for_each(|s| {
        let root = a_root(s);
        let bytes = postcard::to_allocvec(&root).expect("serialize");
        let back: UiRoot = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(back.summon, root.summon, "the gate did not survive the wire");
        assert_eq!(back, root, "the root did not round-trip");
        let again = postcard::to_allocvec(&back).expect("reserialize");
        assert_eq!(bytes, again, "serialization is not stable");
    });
}

#[test]
fn a_gates_wire_tag_is_its_declaration_index() {
    check!().with_type::<Scenario>().for_each(|_| {
        let gates = [SummonGate::Ignored, SummonGate::Held, SummonGate::Released];
        for (tag, gate) in gates.iter().enumerate() {
            let bytes = postcard::to_allocvec(gate).expect("serialize");
            assert_eq!(bytes, vec![tag as u8], "{gate:?} moved off wire tag {tag}",);
        }
        // What a root that never heard of the gate means, and the only default
        // that keeps such a declaration on screen.
        assert_eq!(SummonGate::default(), SummonGate::Ignored);
    });
}
