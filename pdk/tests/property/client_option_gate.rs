//! Authoring a panel that draws as many choices as a tier really offers
//! (stormlight/server#118), and marks the one that was taken (stormlight/server#119).
//!
//! The pdk's job here is only to spell the schema addition, so the properties are
//! about that spelling:
//!
//!   - [`WidgetExt::shown_while_option`] reaches the **layout** and leaves the paint
//!     and the kind alone — the same widget, at a coordinate;
//!   - the coordinate it records is the coordinate it was given, in the same
//!     `(tier, option)` order the pick uses, so a row cannot be gated on its
//!     neighbour;
//!   - the two-node shape a real panel uses — a page container holding coordinate-
//!     gated rows — passes `validate`, so the authoring path cannot express a panel
//!     the host would refuse;
//!   - and it survives the emit → decode the host actually receives it through, with
//!     every gate still the one it was authored with.
//!
//! The **rule** — whether a given gate holds for a given tree and a given player —
//! is the client's, and is pinned there. Nothing in this file knows what a talent is.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::ui::{
    Anchor, Length, OptionState, RootVisibility, Shown, UiSubject, Widget,
};
use stormlight_mod_sdk::abi::visuals::ClientRegistration;
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;
use stormlight_mod_sdk::ui::{WidgetExt, icon, panel, talent_button, talent_icon, text};

/// The generated counterpart of [`OptionState`].
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

#[derive(Debug, TypeGenerator)]
struct Panel {
    /// 1..=8 tiers in the strip.
    tiers: u8,
    /// 1..=5 rows declared per tier — the panel's own width, not the tree's.
    options: u8,
}

impl Panel {
    fn tiers(&self) -> u8 {
        self.tiers % 8 + 1
    }
    fn options(&self) -> u8 {
        self.options % 5 + 1
    }
}

/// One choice: a row laid out only where its tier offers something, carrying the
/// mark of a choice taken and the veil over one passed over.
fn row(tier: u8, option: u8) -> Widget {
    talent_button(
        tier,
        option,
        vec![
            talent_icon(tier, option).image("mod://m/socket.png"),
            text("choice").font_size(14.0),
            icon("mod://m/taken.png").shown_while_option(tier, option, OptionState::Taken),
            icon("mod://m/veil.png").shown_while_option(tier, option, OptionState::PassedOver),
        ],
    )
    .shown_while_option(tier, option, OptionState::Offered)
}

/// The panel a scenario asks for: one page per tier, each holding its own rows.
fn author(p: &Panel) -> Widget {
    let mut children = Vec::new();
    for tier in 0..p.tiers() {
        let rows: Vec<_> = (0..p.options()).map(|option| row(tier, option)).collect();
        children.push(panel(rows).shown_while_tier(tier));
    }
    panel(children).at(Anchor::Center, Length::Px(0.0), Length::Px(0.0))
}

#[test]
fn a_coordinate_reaches_the_layout_and_nothing_else() {
    check!().with_type::<(u8, u8, State)>().for_each(|&(tier, option, is)| {
        let plain = text("choice").font_size(14.0).image("mod://m/row.png");
        let gated = plain.clone().shown_while_option(tier, option, is.state());
        assert_eq!(
            gated.layout.shown,
            Shown::WhileOption { tier, option, is: is.state() },
            "the gate recorded a coordinate other than the one it was given",
        );
        assert_eq!(gated.style, plain.style, "a coordinate gate reached the widget's paint");
        assert_eq!(gated.kind, plain.kind, "a coordinate gate reached what the widget is");
        assert_eq!(
            gated.layout,
            stormlight_mod_sdk::abi::ui::Layout { shown: gated.layout.shown, ..plain.layout },
            "a coordinate gate moved the widget as well as gating it",
        );
    });
}

/// `(tier, option)` and not `(option, tier)`. The two are both `u8`, so nothing but
/// a test can tell a transposition from correctness — and a transposed panel gates
/// every row on a coordinate that is *almost always* somebody else's.
#[test]
fn the_coordinate_is_not_transposed() {
    check!().with_type::<(u8, u8)>().for_each(|&(a, b)| {
        if a == b {
            return;
        }
        let gate = text("x").shown_while_option(a, b, OptionState::Offered).layout.shown;
        assert_eq!(gate, Shown::WhileOption { tier: a, option: b, is: OptionState::Offered });
        assert_ne!(
            gate,
            Shown::WhileOption { tier: b, option: a, is: OptionState::Offered },
            "the gate reads the same transposed, so nothing catches a swapped pair",
        );
    });
}

/// The socket's answer (server#120): the number steps aside while the tier still has
/// one to give, and what it steps aside *for* is said with coordinates, not with a
/// second polarity of this gate.
#[test]
fn a_tier_label_waits_on_its_own_tier() {
    check!().with_type::<u8>().for_each(|&tier| {
        let label = text("4").shown_while_tier_undecided(tier);
        assert_eq!(label.layout.shown, Shown::WhileTierUndecided(tier));
        assert_eq!(label.layout.shown.tier(), Some(tier), "the label named no tier");
    });
}

#[test]
fn an_authored_adapting_panel_is_one_the_host_accepts() {
    check!().with_type::<Panel>().for_each(|p| {
        let mut ctx = ClientContext::new();
        ctx.ui("talents", RootVisibility::Always, UiSubject::LocalPlayer, author(p));
        let bundle = ctx.finish();
        for root in &bundle.ui {
            assert_eq!(root.validate(), Ok(()), "the builders authored a panel the host refuses");
        }
    });
}

#[test]
fn every_gate_survives_the_trip_to_the_host() {
    check!().with_type::<Panel>().for_each(|p| {
        let mut ctx = ClientContext::new();
        ctx.ui("talents", RootVisibility::Always, UiSubject::LocalPlayer, author(p));
        let bundle = ctx.finish();
        let bytes = to_bytes_client(&bundle);
        let back: ClientRegistration = postcard::from_bytes(&bytes).expect("decode");
        assert_eq!(bundle, back, "an adapting panel did not survive the trip to the host");

        // Stated over the decoded tree rather than the built one: a page is a page,
        // and every row under it is gated on its own coordinate — the two facts the
        // whole panel rests on, and the ones a wire format could quietly transpose.
        let pages = back.ui[0].root.kind.children();
        for (tier, page) in pages.iter().enumerate() {
            let tier = u8::try_from(tier).expect("fewer than 256 pages");
            assert_eq!(page.layout.shown, Shown::WhileTierSelected(tier));
            for (option, row) in page.kind.children().iter().enumerate() {
                let option = u8::try_from(option).expect("fewer than 256 rows");
                assert_eq!(
                    row.layout.shown,
                    Shown::WhileOption { tier, option, is: OptionState::Offered },
                    "row {option} of page {tier} came back gated on somebody else",
                );
            }
        }
    });
}
