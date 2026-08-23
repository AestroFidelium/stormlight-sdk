//! Declaring an interface about the *players* in a match rather than about a unit
//! (stormlight/server#145).
//!
//! Every subject this ABI could name before was a body: the one you drive, the one
//! under the cursor, every one on screen. A roster is not a body — it survives one,
//! and its whole point is that it answers for a player whose unit this client has
//! never received. So a fourth subject, and with it the two things a per-player
//! tree needs that a per-unit tree gets for free:
//!
//! - **where the copies go.** A nameplate is placed by its unit's own position, and
//!   there is no such answer here: a row of players is a *strip*, and how it runs
//!   is the mod's call. [`Strip`] says it once on the root, deliberately not reusing
//!   the root widget's own [`Layout::flow`] — that field already arranges the cell's
//!   children, and one field meaning two things is how a HUD ends up unable to
//!   express a column of rows;
//! - **what a player is drawn as.** A cell cannot name the picture: the interface
//!   mod does not know which hero anybody picked. [`WidgetKind::UnitIcon`] resolves
//!   it from the subject the same way an ability slot resolves its icon from
//!   whatever is bound to it.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::ui::{
    Flow, Layout, RootVisibility, RosterSide, Strip, Style, SummonGate, UiError, UiRoot, UiSubject,
    Widget, WidgetKind,
};
use stormlight_mod_abi::ui_event::{Coalesce, UiEvent};

#[derive(Debug, TypeGenerator)]
struct Scenario {
    side: u8,
    flow: u8,
    /// Raw gap seed. A gap between two rows may legitimately be negative — art
    /// that bleeds into its neighbour is how a real bar is drawn — so the sign is
    /// generated rather than clamped.
    gap: i8,
    /// Whether the cell declares a picture of its own to fall back on.
    fallback: bool,
}

fn side(byte: u8) -> RosterSide {
    match byte % 3 {
        0 => RosterSide::Everyone,
        1 => RosterSide::Own,
        _ => RosterSide::Other,
    }
}

fn flow(byte: u8) -> Flow {
    match byte % 3 {
        0 => Flow::Row,
        1 => Flow::Column,
        _ => Flow::Stack,
    }
}

fn a_cell(fallback: bool) -> Widget {
    Widget {
        name: "portrait".into(),
        layout: Layout::default(),
        style: Style { image: fallback.then(|| "mod://hud/empty.png".into()), ..Style::default() },
        kind: WidgetKind::UnitIcon,
    }
}

fn a_roster_root(s: &Scenario) -> UiRoot {
    UiRoot {
        name: "team".into(),
        when: RootVisibility::Always,
        summon: SummonGate::Ignored,
        subject: UiSubject::EachPlayer(side(s.side)),
        strip: Strip { flow: flow(s.flow), gap: f32::from(s.gap) },
        root: a_cell(s.fallback),
    }
}

#[test]
fn a_roster_root_survives_the_trip_to_the_host() {
    // The subject and the strip are both new tags on a type mods already ship, and
    // both are read by the client rather than by the mod that wrote them — so a
    // silent loss here is a HUD that lays itself out differently on the machine
    // that draws it than on the one that declared it.
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = a_roster_root(s);
        let bytes = postcard::to_allocvec(&declared).expect("a roster strip encodes");
        let decoded: UiRoot = postcard::from_bytes(&bytes).expect("and decodes");
        assert_eq!(decoded, declared, "a per-player root did not survive the wire");
        assert_eq!(
            decoded.subject,
            UiSubject::EachPlayer(side(s.side)),
            "which players the tree covers must survive exactly"
        );
        assert_eq!(decoded.strip, declared.strip, "the strip's own layout must survive exactly");
    });
}

#[test]
fn a_declared_roster_strip_is_accepted_and_a_broken_one_is_named() {
    // A gap is a distance *between* siblings, so below zero it means they overlap
    // and is perfectly legal — the same rule `Layout::gap` already states. A
    // non-finite one is not a wider gap, it is a number that reaches the layout
    // engine and takes the whole screen with it, so it is refused at load where an
    // author can read about it.
    check!().with_type::<Scenario>().for_each(|s| {
        assert_eq!(a_roster_root(s).validate(), Ok(()), "a declared strip must load");

        for broken in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut root = a_roster_root(s);
            root.strip.gap = broken;
            assert_eq!(
                root.validate(),
                Err(UiError::BadStrip),
                "a strip with a {broken} gap must be refused rather than laid out"
            );
        }
    });
}

#[test]
fn a_player_row_cannot_be_declared_as_an_occurrence() {
    // A transient is spawned by something *happening* and is anchored to the unit
    // the occurrence named — which is why it declares the per-unit subject. A
    // per-player root has no such unit and no occurrence to be about, so one
    // declared this way would be built by nothing and read nothing: refused, rather
    // than left as a tree an author waits forever to see.
    check!().with_type::<Scenario>().for_each(|s| {
        let mut root = a_roster_root(s);
        root.when = RootVisibility::OnEvent {
            event: UiEvent::Damaged,
            seconds: 1.0,
            coalesce: Coalesce::Never,
        };
        assert_eq!(
            root.validate(),
            Err(UiError::SubjectNeverPresent),
            "a per-player root that waits for an occurrence can never resolve a subject"
        );
    });
}

#[test]
fn a_unit_icon_needs_no_declared_picture_and_does_nothing_when_clicked() {
    // The two rules it inherits from the talent icon it is modelled on. A plain
    // `Icon` must name its picture because the interface owns it; this one's is
    // resolved from whichever hero the row's player picked, so declaring none is
    // how a cell says it wants no empty-socket art. And it draws only — a roster
    // row is not a button, and if a mod wants one it wraps the picture in a
    // `Button`, which is the one spelling of what a widget does.
    check!().with_type::<Scenario>().for_each(|s| {
        let mut root = a_roster_root(s);
        root.root.style.image = None;
        assert_eq!(root.validate(), Ok(()), "a unit icon must load without a declared picture");

        let cell = a_cell(s.fallback);
        assert!(cell.kind.children().is_empty(), "a unit icon is a leaf");
        assert_eq!(cell.kind.action(), None, "a unit icon must not act on its own");
    });
}

/// Every family fails. A declaration that survives it names no interned handle at
/// all — which is exactly what "a roster cell is authored without naming content"
/// means, spelled as a property.
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

#[test]
fn a_unit_icon_names_no_handle_to_remap() {
    // The picture travels with the *unit*, and which unit that is comes off the
    // roster at runtime — so unlike a trigger button, a roster cell carries nothing
    // in the declaring mod's id space and adoption has nothing to rewrite.
    check!().with_type::<Scenario>().for_each(|s| {
        let mut root = a_roster_root(s);
        assert_eq!(
            root.remap_ids(&FailEverything),
            Ok(()),
            "adoption asked a roster cell for a handle it does not carry"
        );
    });
}
