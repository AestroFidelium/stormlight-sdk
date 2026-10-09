//! Authoring what an ability is called and does (stormlight/server#116).
//!
//! The pdk only spells the card, so the properties are about that spelling:
//!   - [`ClientContext::ability_card`] lands its three strings on the handle of the
//!     ability it names — the same handle that ability's feedback and icon use;
//!   - [`ability_text`] addresses a **slot**, never an ability, so an interface
//!     names no content; and a tooltip built from it passes the host's validation
//!     and survives the trip to the host.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::ids::Slot;
use stormlight_mod_sdk::abi::ui::{AbilityText, RootVisibility, TextSource, UiSubject, WidgetKind};
use stormlight_mod_sdk::abi::visuals::ClientRegistration;
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;
use stormlight_mod_sdk::ui::{WidgetExt, ability_slot, ability_text};

#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Field {
    Name,
    Description,
    Hotkey,
}

impl Field {
    fn text(self) -> AbilityText {
        match self {
            Self::Name => AbilityText::Name,
            Self::Description => AbilityText::Description,
            Self::Hotkey => AbilityText::Hotkey,
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    #[generator(0u8..=7)]
    slot: u8,
    field: Field,
    /// How many abilities are carded.
    #[generator(0u8..=5)]
    cards: u8,
}

#[test]
fn a_card_lands_its_words_on_its_abilitys_handle() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ClientContext::new();
        let mut handles = Vec::new();
        for i in 0..s.cards {
            let icon = ctx.ability_icon(&format!("a{i}"), "mod://pack/old.png");
            let card = ctx.ability_card(
                &format!("a{i}"),
                &format!("Name {i}"),
                &format!("Does {i}"),
                &format!("mod://pack/a{i}.png"),
            );
            assert_eq!(icon, card, "an ability's icon and card took two handles");
            handles.push(card);
        }
        let reg = ctx.finish();
        for (i, handle) in handles.iter().enumerate() {
            let last = reg
                .ability_cards
                .iter()
                .rev()
                .find(|c| c.ability == *handle)
                .expect("the card is carried");
            assert_eq!(last.info.name, format!("Name {i}"));
            assert_eq!(last.info.description, format!("Does {i}"));
            assert_eq!(last.info.image, format!("mod://pack/a{i}.png"));
        }
    });
}

#[test]
fn a_slot_tooltip_names_no_ability_and_reaches_the_host() {
    check!().with_type::<Scenario>().for_each(|s| {
        let slot = Slot(s.slot);
        let words = ability_text(slot, s.field.text());
        assert_eq!(
            words.kind,
            WidgetKind::Text { text: TextSource::Ability { slot, field: s.field.text() } },
        );
        let mut ctx = ClientContext::new();
        ctx.ui(
            "bar",
            RootVisibility::Always,
            UiSubject::LocalPlayer,
            ability_slot(slot, "Q").tooltip(
                stormlight_mod_sdk::abi::ui::Anchor::BottomLeft,
                [8.0, 8.0],
                0.25,
                vec![words],
            ),
        );
        let reg = ctx.finish();
        for root in &reg.ui {
            assert_eq!(root.validate(), Ok(()), "a slot tooltip was refused");
        }
        let back: ClientRegistration =
            postcard::from_bytes(&to_bytes_client(&reg)).expect("decodes");
        assert_eq!(back, reg, "a slot tooltip did not survive the trip to the host");
    });
}
