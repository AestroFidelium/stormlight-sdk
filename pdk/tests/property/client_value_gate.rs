//! Authoring a figure that says nothing rather than `0` (stormlight/server#230).
//!
//! The pdk only spells the gate, so the properties are about that spelling:
//!
//!   - [`WidgetExt::shown_while_positive`] reaches the **layout** with the binding it
//!     was given and leaves the paint and the kind alone;
//!   - a nameplate built with it — a held-damage bar and a count gated on the count
//!     — passes `validate` and survives the emit → decode the host receives it
//!     through, gate and all.
//!
//! Whether the gate holds for a given unit is the client's rule, pinned there.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::ui::{
    HeldQuantity, Length, RootVisibility, Shown, UiSubject, ValueBinding, ValuePart,
};
use stormlight_mod_sdk::abi::visuals::ClientRegistration;
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;
use stormlight_mod_sdk::ui::{WidgetExt, bar, bound_text, panel};

/// Which held number a widget reads.
#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Held {
    Count,
    Damage,
}

impl Held {
    fn binding(self) -> ValueBinding {
        ValueBinding::Held(match self {
            Self::Count => HeldQuantity::Count,
            Self::Damage => HeldQuantity::Damage,
        })
    }
}

#[derive(Debug, TypeGenerator)]
struct Nameplate {
    /// What the figure prints.
    figure: Held,
    /// What it waits on.
    gate: Held,
    /// Its size, in whole pixels.
    width: u8,
    height: u8,
}

#[test]
fn the_gate_lands_in_the_layout_and_nowhere_else() {
    check!().with_type::<Nameplate>().for_each(|s| {
        let plain = bound_text(s.figure.binding(), ValuePart::Current, 0)
            .sized(Length::Px(f32::from(s.width)), Length::Px(f32::from(s.height)));
        let gated = plain.clone().shown_while_positive(s.gate.binding());
        assert_eq!(gated.layout.shown, Shown::WhilePositive(s.gate.binding()));
        assert_eq!(gated.kind, plain.kind, "the gate changed what the widget is");
        assert_eq!(gated.style, plain.style, "the gate changed how it is painted");
        assert_eq!(gated.layout.size, plain.layout.size, "the gate moved the widget");
    });
}

#[test]
fn a_gated_nameplate_reaches_the_host_intact() {
    check!().with_type::<Nameplate>().for_each(|s| {
        let mut ctx = ClientContext::new();
        ctx.ui(
            "nameplate",
            RootVisibility::Always,
            UiSubject::EachUnit,
            panel(vec![
                bar(ValueBinding::Held(HeldQuantity::Damage)),
                bound_text(s.figure.binding(), ValuePart::Current, 0)
                    .shown_while_positive(s.gate.binding()),
            ]),
        );
        let sent = ctx.finish();
        for root in &sent.ui {
            assert_eq!(root.validate(), Ok(()), "the pdk built a nameplate the host refuses");
        }
        let bytes = to_bytes_client(&sent);
        let decoded: ClientRegistration = postcard::from_bytes(&bytes).expect("and decodes");
        assert_eq!(decoded, sent, "a value gate did not survive the trip to the host");
    });
}
