//! The authoring path for the summon gate (stormlight/server#98): a cosmetic mod
//! declares which half of a handover a tree is, and the bundle carries it.
//!
//! Laws:
//!   - **Ungated by default**: a root declared with [`ClientContext::ui`] carries
//!     [`SummonGate::Ignored`], so every HUD authored before the gate existed keeps
//!     meaning exactly what it meant.
//!   - **Carried verbatim**: [`ClientContext::ui_gated`] changes the gate and
//!     nothing else — the same declaration under all three gates differs in that
//!     one field and is otherwise byte-identical, so no gate can quietly alter a
//!     tree.
//!   - **Across the boundary**: the gate survives `finish` → postcard → decode,
//!     which is the trip every declaration makes to reach the host.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::ui::{Anchor, Length, RootVisibility, SummonGate, UiSubject};
use stormlight_mod_sdk::abi::visuals::ClientRegistration;
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;
use stormlight_mod_sdk::ui::{WidgetExt, panel, text};

/// Which gate a scenario asks for.
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

#[derive(Debug, TypeGenerator)]
struct Scenario {
    gate: Gate,
    /// 0..=3 lines in the tree, so the comparison is over more than a leaf.
    lines: u8,
}

fn a_tree(s: &Scenario) -> stormlight_mod_sdk::abi::ui::Widget {
    let lines: Vec<_> = (0..u16::from(s.lines) % 4)
        .map(|i| text("waiting").font_size(f32::from(i) + 12.0))
        .collect();
    panel(lines).at(Anchor::BottomLeft, Length::Px(24.0), Length::Px(-120.0))
}

#[test]
fn an_ungated_declaration_ignores_the_summon_input() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ClientContext::new();
        ctx.ui("hud", RootVisibility::Always, UiSubject::LocalPlayer, a_tree(s));
        let reg = ctx.finish();
        assert_eq!(
            reg.ui[0].summon,
            SummonGate::Ignored,
            "a root that never mentions the summon input must not wait on it",
        );
    });
}

#[test]
fn the_gate_is_the_only_thing_ui_gated_changes() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut plain = ClientContext::new();
        plain.ui("alert", RootVisibility::WhileTalentPending, UiSubject::LocalPlayer, a_tree(s));
        let mut gated = ClientContext::new();
        gated.ui_gated(
            "alert",
            RootVisibility::WhileTalentPending,
            s.gate.gate(),
            UiSubject::LocalPlayer,
            a_tree(s),
        );

        let (mut a, b) = (plain.finish().ui, gated.finish().ui);
        assert_eq!(b[0].summon, s.gate.gate(), "the declared gate did not arrive");
        // Same tree, same name, same condition, same subject: the gate is the one
        // difference, so equating them once it is copied over proves nothing else
        // moved.
        a[0].summon = b[0].summon;
        assert_eq!(a, b, "ui_gated changed more than the gate");
        assert_eq!(b[0].validate(), Ok(()), "a gated tree must still validate");
    });
}

#[test]
fn a_gated_bundle_decodes_equal() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ClientContext::new();
        ctx.ui_gated(
            "picker",
            RootVisibility::WhileTalentPending,
            s.gate.gate(),
            UiSubject::LocalPlayer,
            a_tree(s),
        );
        let reg = ctx.finish();
        let bytes = to_bytes_client(&reg);
        let back: ClientRegistration = postcard::from_bytes(&bytes).expect("decode");
        assert_eq!(back.ui, reg.ui, "the gated bundle did not survive the boundary");
    });
}
