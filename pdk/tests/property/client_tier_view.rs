//! Authoring a talent panel that is read one tier at a time
//! (stormlight/server#104).
//!
//! The pdk's job here is only to spell the two schema additions, so the properties
//! are about that spelling and nothing else:
//!   - [`select_tier_button`] produces a button carrying exactly the tier it was
//!     given, which is what the client's single reading of "what does this widget
//!     do" will report;
//!   - [`WidgetExt::shown_while_tier`] reaches the **layout** and leaves the paint
//!     and the kind alone — the same widget, on a page;
//!   - a whole paged panel authored through the builders passes `validate`, so the
//!     authoring path cannot express a panel the host would refuse;
//!   - and it survives the emit → decode the host actually receives it through.
//!
//! The page a widget is on is deliberately **not** the page its button selects in
//! the generator below: a tier button is on screen whatever page is open, and a
//! generator that tied the two together would never produce that ordinary case.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::ui::{Anchor, Length, RootVisibility, Shown, UiAction, UiSubject};
use stormlight_mod_sdk::abi::visuals::ClientRegistration;
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;
use stormlight_mod_sdk::ui::{
    WidgetExt, panel, select_tier_button, talent_button, talent_icon, text,
};

#[derive(Debug, TypeGenerator)]
struct Panel {
    /// 1..=8 tiers in the strip.
    tiers: u8,
    /// 1..=5 choices offered per tier.
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

/// The panel a scenario asks for: one column of choices per tier, all gated on
/// their own tier, over a strip of buttons that page between them.
fn author(p: &Panel) -> stormlight_mod_sdk::abi::ui::Widget {
    let mut children = Vec::new();
    for tier in 0..p.tiers() {
        let rows: Vec<_> = (0..p.options())
            .map(|option| {
                talent_button(
                    tier,
                    option,
                    vec![
                        talent_icon(tier, option).image("mod://m/socket.png"),
                        text("choice").font_size(14.0),
                    ],
                )
                .shown_while_tier(tier)
            })
            .collect();
        children.push(panel(rows).shown_while_tier(tier));
        children.push(select_tier_button(tier, vec![text("tier")]).image("mod://m/tier.png"));
    }
    panel(children).at(Anchor::Center, Length::Px(0.0), Length::Px(0.0))
}

#[test]
fn a_tier_button_carries_the_tier_it_pages_to() {
    check!().with_type::<u8>().for_each(|&tier| {
        let widget = select_tier_button(tier, Vec::new());
        assert_eq!(widget.kind.action(), Some(UiAction::SelectTier(tier)));
        // Paging is not a page: a button that selects a tier is on screen whatever
        // page is open, and gating it would hide the only way back.
        assert_eq!(widget.layout.shown, Shown::Always, "a tier button gated itself");
    });
}

#[test]
fn a_page_reaches_the_layout_and_nothing_else() {
    check!().with_type::<u8>().for_each(|&tier| {
        let plain = text("choice").font_size(14.0).image("mod://m/row.png");
        let paged = plain.clone().shown_while_tier(tier);
        assert_eq!(paged.layout.shown, Shown::WhileTierSelected(tier));
        assert_eq!(paged.style, plain.style, "a page reached the widget's paint");
        assert_eq!(paged.kind, plain.kind, "a page reached what the widget is");
        assert_eq!(
            paged.layout,
            stormlight_mod_sdk::abi::ui::Layout { shown: paged.layout.shown, ..plain.layout },
            "a page moved the widget as well as gating it",
        );
    });
}

#[test]
fn an_authored_paged_panel_is_one_the_host_accepts() {
    check!().with_type::<Panel>().for_each(|p| {
        let mut ctx = ClientContext::new();
        ctx.ui("talents", RootVisibility::WhileTalentPending, UiSubject::LocalPlayer, author(p));
        let bundle = ctx.finish();
        for root in &bundle.ui {
            assert_eq!(root.validate(), Ok(()), "the builders authored a panel the host refuses");
        }
    });
}

#[test]
fn a_paged_panel_survives_the_trip_to_the_host() {
    check!().with_type::<Panel>().for_each(|p| {
        let mut ctx = ClientContext::new();
        ctx.ui("talents", RootVisibility::WhileTalentPending, UiSubject::LocalPlayer, author(p));
        let bundle = ctx.finish();
        let bytes = to_bytes_client(&bundle);
        let back: ClientRegistration = postcard::from_bytes(&bytes).expect("decode");
        assert_eq!(bundle, back, "a paged panel did not survive the trip to the host");

        // And every page is still the one it was authored on — the property the
        // whole panel rests on, stated over the decoded tree rather than the built
        // one.
        let root = &back.ui[0].root;
        let gates: Vec<_> = root.kind.children().iter().map(|child| child.layout.shown).collect();
        for tier in 0..p.tiers() {
            assert_eq!(gates[usize::from(tier) * 2], Shown::WhileTierSelected(tier));
            assert_eq!(gates[usize::from(tier) * 2 + 1], Shown::Always);
        }
    });
}
