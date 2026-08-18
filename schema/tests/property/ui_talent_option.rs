//! The offered options of a talent tier (stormlight/server#95) — the middle the
//! ABI was missing between "the talents already taken"
//! ([`ListBinding::ChosenTalents`]) and "take option *n* of tier *t*"
//! ([`UiAction::PickTalent`]).
//!
//! A panel could offer the choice and could not say what any of it did. Three
//! declarations close that, and all three are addressed the way a pick already is
//! — **by tier and option index**, never by naming a talent:
//!
//! | declaration | yields |
//! | --- | --- |
//! | [`TextSource::Talent`] | one offered option's name or description |
//! | [`ListBinding::TierOptions`] | every offered option of a tier, one per line |
//! | [`WidgetKind::TalentIcon`] | the picture the offered option wears |
//!
//! ## Coordinates carry no handle
//!
//! That is the property this file exists for. A tier and an option index are
//! positions in the *unit's own* declared tree — pure mod convention, exactly like
//! [`UiAction::PickTalent`], which is why a cosmetic mod can lay out a talent panel
//! while naming no content at all. So a tree built out of nothing but these three
//! must survive a remap through a map that fails **every** family: there is
//! nothing in them for adoption to translate. If that ever stops holding, some
//! coordinate has quietly become an id and the panel has started naming content.
//!
//! ## A talent icon is the one picture its mod cannot name
//!
//! [`WidgetKind::Icon`] with no image is a break — a mod asked for a picture and
//! supplied none. A [`WidgetKind::TalentIcon`] with no image is ordinary: the
//! picture is resolved from whichever talent the tier offers at that index, and the
//! declared one is only the fallback for a cell offering nothing. Validation must
//! tell the two apart, or every talent panel is refused at load.
//!
//! ## It is not a button
//!
//! The pick stays [`UiAction::PickTalent`] on an enclosing button. A picture that
//! also submitted a choice would give the ABI two ways to spell one action, and the
//! whole point of [`WidgetKind::action`] is that there is exactly one.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::ui::{
    Layout, ListBinding, RootVisibility, Style, SummonGate, TalentText, TextSource, UiError,
    UiRoot, UiSubject, Widget, WidgetKind,
};

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// Every family fails. A declaration that survives it names no interned handle at
/// all — which is exactly what "a cosmetic mod authors a talent panel without
/// naming content" means, spelled as a property.
struct FailEverything;

impl IdMap for FailEverything {
    type Error = ();
    fn stat(&self, _: StatId) -> Result<StatId, ()> {
        Err(())
    }
    fn resource(&self, _: ResourceId) -> Result<ResourceId, ()> {
        Err(())
    }
    fn stack(&self, _: StackId) -> Result<StackId, ()> {
        Err(())
    }
    fn tag(&self, _: TagId) -> Result<TagId, ()> {
        Err(())
    }
    fn tag_class(&self, _: TagClassId) -> Result<TagClassId, ()> {
        Err(())
    }
    fn param(&self, _: ParamId) -> Result<ParamId, ()> {
        Err(())
    }
    fn event(&self, _: EventId) -> Result<EventId, ()> {
        Err(())
    }
    fn buff(&self, _: BuffId) -> Result<BuffId, ()> {
        Err(())
    }
    fn curve(&self, _: CurveId) -> Result<CurveId, ()> {
        Err(())
    }
    fn damage_type(&self, _: DamageTypeId) -> Result<DamageTypeId, ()> {
        Err(())
    }
    fn ability(&self, _: AbilityId) -> Result<AbilityId, ()> {
        Err(())
    }
    fn talent(&self, _: TalentId) -> Result<TalentId, ()> {
        Err(())
    }
    fn handler(&self, _: HandlerId) -> Result<HandlerId, ()> {
        Err(())
    }
    fn unit(&self, _: UnitId) -> Result<UnitId, ()> {
        Err(())
    }
    fn navmesh(&self, _: NavMeshId) -> Result<NavMeshId, ()> {
        Err(())
    }
    fn anim_state(&self, _: AnimStateId) -> Result<AnimStateId, ()> {
        Err(())
    }
}

/// One cell of a declared panel: which coordinate it names and which of the three
/// new declarations it makes.
#[derive(Debug, TypeGenerator, Clone, Copy)]
enum Cell {
    /// A text reading one offered option's name.
    Name { tier: u8, option: u8 },
    /// A text reading one offered option's description.
    Description { tier: u8, option: u8 },
    /// A text listing a whole tier's offered options.
    Options { tier: u8 },
    /// The picture an offered option wears.
    Icon { tier: u8, option: u8 },
}

fn a_widget(kind: WidgetKind) -> Widget {
    Widget { name: "w".to_string(), layout: Layout::default(), style: Style::default(), kind }
}

fn build(cell: Cell) -> Widget {
    a_widget(match cell {
        Cell::Name { tier, option } => {
            WidgetKind::Text { text: TextSource::Talent { tier, option, field: TalentText::Name } }
        }
        Cell::Description { tier, option } => WidgetKind::Text {
            text: TextSource::Talent { tier, option, field: TalentText::Description },
        },
        Cell::Options { tier } => {
            WidgetKind::Text { text: TextSource::List(ListBinding::TierOptions(tier)) }
        }
        Cell::Icon { tier, option } => WidgetKind::TalentIcon { tier, option },
    })
}

/// A whole declared panel out of the generated cells, so the properties are about
/// a tree rather than about one node.
fn a_panel(cells: &[Cell]) -> UiRoot {
    UiRoot {
        name: "talents".to_string(),
        when: RootVisibility::WhileTalentPending,
        summon: SummonGate::Held,
        subject: UiSubject::LocalPlayer,
        root: a_widget(WidgetKind::Panel {
            children: cells.iter().copied().map(build).collect::<Vec<_>>(),
        }),
    }
}

#[test]
fn a_tier_coordinate_names_no_interned_handle() {
    check!().with_type::<Vec<Cell>>().for_each(|cells| {
        let declared = a_panel(cells);
        let mut out = declared.clone();
        // Every family fails, so anything that reached the map at all would error.
        out.remap_ids(&FailEverything).expect("a talent coordinate is not a handle");
        assert_eq!(declared, out, "adoption rewrote a tier/option coordinate");
    });
}

#[test]
fn the_new_declarations_survive_a_postcard_round_trip() {
    check!().with_type::<Vec<Cell>>().for_each(|cells| {
        let declared = a_panel(cells);
        let bytes = postcard::to_allocvec(&declared).expect("serialize");
        let back: UiRoot = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(declared, back, "a talent panel did not round-trip");
        let again = postcard::to_allocvec(&back).expect("reserialize");
        assert_eq!(bytes, again, "talent panel serialization is not stable");
    });
}

#[test]
fn a_talent_icon_needs_no_declared_picture_but_a_plain_icon_still_does() {
    check!().with_type::<(u8, u8)>().for_each(|&(tier, option)| {
        // The picture comes from whichever talent the tier offers there, so a cell
        // that declares none is complete as authored.
        let resolved = a_panel(&[Cell::Icon { tier, option }]);
        assert_eq!(resolved.validate(), Ok(()), "a talent icon was refused for having no image");

        // The kind whose picture *is* the mod's own is unchanged: still a break.
        let bare = UiRoot {
            root: a_widget(WidgetKind::Panel { children: vec![a_widget(WidgetKind::Icon)] }),
            ..a_panel(&[])
        };
        assert_eq!(
            bare.validate(),
            Err(UiError::IconWithoutImage { widget: 1 }),
            "a plain icon with no picture stopped being a break",
        );
    });
}

#[test]
fn a_talent_icon_acts_on_nothing_and_holds_nothing() {
    check!().with_type::<(u8, u8)>().for_each(|&(tier, option)| {
        let kind = WidgetKind::TalentIcon { tier, option };
        assert_eq!(kind.action(), None, "a talent icon grew an action of its own");
        assert!(kind.children().is_empty(), "a talent icon grew children");
    });
}

/// The declared picture is not *ignored* — it is the fallback a cell offering
/// nothing wears, so an author who names one must still have it validated like
/// every other path.
#[test]
fn a_talent_icons_fallback_picture_is_still_checked() {
    check!().with_type::<(u8, u8)>().for_each(|&(tier, option)| {
        let mut declared = a_panel(&[Cell::Icon { tier, option }]);
        let WidgetKind::Panel { children } = &mut declared.root.kind else { unreachable!() };
        children[0].style.image = Some(String::new());
        assert_eq!(
            declared.validate(),
            Err(UiError::EmptyImagePath { widget: 1 }),
            "a talent icon's empty fallback path slipped past validation",
        );
    });
}
