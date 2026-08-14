//! A button's *action* crosses the local→global bridge too (stormlight/server#69).
//!
//! [`ui_remap`](super::ui_remap) pins the same property for what a widget
//! **reads**; this is about what it **does**. Only one action names a handle —
//! [`UiAction::Trigger`] — and it names one in the *gameplay* side's id space,
//! which is the whole hazard: a cosmetic mod authors `EventId(0)` for its own
//! first event, and two cosmetic mods loaded together both did. Left untranslated,
//! the second one's button would raise the first one's event.
//!
//! The other two actions carry no handle at all and must come through byte-identical
//! — a `Slot` is mod convention and a talent option is an index into the unit's own
//! tree, so "translating" either would be inventing a mapping nothing feeds. That
//! negative half is the more valuable one: a remap that helpfully rewrote a slot
//! index would silently re-bind every ability bar in the game.

use core::cell::RefCell;

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::descriptors::Names;
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, Slot, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::interner::Interner;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::ui::{
    Layout, RootVisibility, Style, UiAction, UiRoot, UiSubject, Widget, WidgetKind,
};

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Overlapping on purpose, so a scenario can produce two mods that share an event
/// name and two that do not, in the same load.
const EVENTS: [&str; 4] = ["banner", "rally", "recall", "salute"];

/// A local handle no name table explains.
#[derive(Debug, PartialEq, Eq)]
struct Dangling;

/// The host's map for one mod: resolve the local handle to the name that mod gave
/// it, then intern that name into the one global space.
struct Adoption<'a> {
    names: &'a Names,
    events: &'a RefCell<Interner<EventId>>,
}

impl IdMap for Adoption<'_> {
    type Error = Dangling;

    fn event(&self, id: EventId) -> Result<EventId, Dangling> {
        let name = self.names.events.get(id.0 as usize).ok_or(Dangling)?;
        Ok(self.events.borrow_mut().intern(name))
    }

    fn stat(&self, id: StatId) -> Result<StatId, Dangling> {
        Ok(id)
    }
    fn resource(&self, id: ResourceId) -> Result<ResourceId, Dangling> {
        Ok(id)
    }
    fn stack(&self, id: StackId) -> Result<StackId, Dangling> {
        Ok(id)
    }
    fn tag(&self, id: TagId) -> Result<TagId, Dangling> {
        Ok(id)
    }
    fn tag_class(&self, id: TagClassId) -> Result<TagClassId, Dangling> {
        Ok(id)
    }
    fn param(&self, id: ParamId) -> Result<ParamId, Dangling> {
        Ok(id)
    }
    fn buff(&self, id: BuffId) -> Result<BuffId, Dangling> {
        Ok(id)
    }
    fn curve(&self, id: CurveId) -> Result<CurveId, Dangling> {
        Ok(id)
    }
    fn damage_type(&self, id: DamageTypeId) -> Result<DamageTypeId, Dangling> {
        Ok(id)
    }
    fn ability(&self, id: AbilityId) -> Result<AbilityId, Dangling> {
        Ok(id)
    }
    fn talent(&self, id: TalentId) -> Result<TalentId, Dangling> {
        Ok(id)
    }
    fn handler(&self, id: HandlerId) -> Result<HandlerId, Dangling> {
        Ok(id)
    }
    fn unit(&self, id: UnitId) -> Result<UnitId, Dangling> {
        Ok(id)
    }
    fn navmesh(&self, id: NavMeshId) -> Result<NavMeshId, Dangling> {
        Ok(id)
    }
    fn anim_state(&self, id: AnimStateId) -> Result<AnimStateId, Dangling> {
        Ok(id)
    }
}

fn a_widget(kind: WidgetKind) -> Widget {
    Widget { name: String::new(), layout: Layout::default(), style: Style::default(), kind }
}

fn a_button(action: UiAction) -> Widget {
    a_widget(WidgetKind::Button { action, children: Vec::new() })
}

/// A HUD with one trigger button per event the mod names — in local handle order,
/// so the remapped tree reads back positionally — plus the two handle-free actions
/// and the composite, whose implied action must survive untouched as well.
fn a_root(events: &[&str], slot: u8, tier: u8, option: u8) -> UiRoot {
    let mut children: Vec<Widget> = (0..events.len())
        .map(|raw| a_button(UiAction::Trigger { event: EventId(raw as u16) }))
        .collect();
    children.push(a_button(UiAction::CastSlot(Slot(slot))));
    children.push(a_button(UiAction::PickTalent { tier, option }));
    children
        .push(a_widget(WidgetKind::AbilitySlot { slot: Slot(slot), key_hint: "Q".to_string() }));
    UiRoot {
        name: "hud".to_string(),
        when: RootVisibility::Always,
        subject: UiSubject::LocalPlayer,
        root: a_widget(WidgetKind::Panel { children }),
    }
}

/// Every action the tree declares, in declaration order — read through the same
/// accessor the client uses, so the composite's implied action is included.
fn actions(root: &UiRoot) -> Vec<UiAction> {
    let WidgetKind::Panel { children } = &root.root.kind else { unreachable!() };
    children.iter().filter_map(|child| child.kind.action()).collect()
}

fn names(events: &[&str]) -> Names {
    Names { events: events.iter().map(ToString::to_string).collect(), ..Names::default() }
}

/// Distinct names from `pool`, in the order chosen — a local name table is dense
/// and never repeats.
fn pick<'a>(pool: &[&'a str], choices: &[u8]) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    for &c in choices {
        let name = pool[c as usize % pool.len()];
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    a_events: [u8; 3],
    b_events: [u8; 3],
    slot: u8,
    tier: u8,
    option: u8,
}

#[test]
fn two_mods_agree_on_a_shared_event_and_never_collide_on_distinct_ones() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (ea, eb) = (pick(&EVENTS, &s.a_events), pick(&EVENTS, &s.b_events));
        let (mut a, mut b) =
            (a_root(&ea, s.slot, s.tier, s.option), a_root(&eb, s.slot, s.tier, s.option));
        let (na, nb) = (names(&ea), names(&eb));

        let global = RefCell::new(Interner::<EventId>::new());
        a.remap_ids(&Adoption { names: &na, events: &global }).expect("adopt a");
        b.remap_ids(&Adoption { names: &nb, events: &global }).expect("adopt b");

        let (ga, gb) = (actions(&a), actions(&b));

        // The global id a trigger landed on is the id its *name* interned to —
        // which is what makes the same event named by two mods one event.
        let global_of = |name: &str| global.borrow().get(name).expect("interned");
        for (i, name) in ea.iter().enumerate() {
            assert_eq!(ga[i], UiAction::Trigger { event: global_of(name) }, "mod a event {name}");
        }
        for (i, name) in eb.iter().enumerate() {
            assert_eq!(gb[i], UiAction::Trigger { event: global_of(name) }, "mod b event {name}");
        }

        // Distinct names never share a handle, however the two mods numbered them
        // locally — both authored from raw 0.
        for (i, na) in ea.iter().enumerate() {
            for (j, nb) in eb.iter().enumerate() {
                assert_eq!(
                    ga[i] == gb[j],
                    na == nb,
                    "`{na}` vs `{nb}` must share a handle iff they are the same name",
                );
            }
        }
    });
}

#[test]
fn the_handle_free_actions_come_through_byte_identical() {
    check!().with_type::<Scenario>().for_each(|s| {
        let events = pick(&EVENTS, &s.a_events);
        let mut root = a_root(&events, s.slot, s.tier, s.option);
        let before = actions(&root);
        let table = names(&events);
        let global = RefCell::new(Interner::<EventId>::new());
        root.remap_ids(&Adoption { names: &table, events: &global }).expect("adopt");
        let after = actions(&root);

        // The tail of the tree is: cast, pick, and the composite's implied cast.
        // None of the three names an interned handle, so adoption must be a no-op
        // on all of them — a slot index that shifted would re-bind the ability bar.
        assert_eq!(after[events.len()..], before[events.len()..]);
        assert_eq!(after[events.len()], UiAction::CastSlot(Slot(s.slot)));
        assert_eq!(
            after[events.len() + 1],
            UiAction::PickTalent { tier: s.tier, option: s.option }
        );
        assert_eq!(after[events.len() + 2], UiAction::CastSlot(Slot(s.slot)));
    });
}

#[test]
fn a_trigger_naming_an_event_the_mod_never_declared_is_rejected() {
    check!().with_type::<u8>().for_each(|&raw| {
        // A registration that contradicts itself: the button names a local event
        // its own table does not explain. Reported, never resolved to whatever
        // sits at that raw index globally — that would fire an unrelated mod's
        // event on every click.
        let table = names(&["banner"]);
        let global = RefCell::new(Interner::<EventId>::new());
        let mut root = UiRoot {
            name: "hud".to_string(),
            when: RootVisibility::Always,
            subject: UiSubject::LocalPlayer,
            root: a_button(UiAction::Trigger { event: EventId(u16::from(raw) + 1) }),
        };
        assert_eq!(
            root.remap_ids(&Adoption { names: &table, events: &global }),
            Err(Dangling),
            "local event {} has no name-table entry",
            raw as u16 + 1,
        );
    });
}
