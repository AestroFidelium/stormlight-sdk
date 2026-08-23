//! Two mods, one engine: a UI tree authored in a mod's **local** id space must
//! land in the global space the way every other descriptor family does
//! (stormlight/server#66).
//!
//! Adoption is modelled here the way the host performs it: each mod ships its own
//! dense `Names` tables, and the host re-interns every name into one global
//! [`Interner`], building a local→global [`IdMap`] per mod. Two mods that name the
//! same stat must converge on one handle; two that name different stats must not
//! collide — which is exactly what a mod loaded *second* would otherwise do, since
//! both authored from raw index 0.
//!
//! Cross-module rather than a pure property: it exercises the interner, the name
//! tables and the remap walk against each other, which is where a collision would
//! actually appear.

use core::cell::RefCell;

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::descriptors::Names;
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::impacts::PoolRef;
use stormlight_mod_abi::interner::Interner;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::ui::{
    Layout, RootVisibility, Strip, Style, SummonGate, UiRoot, UiSubject, ValueBinding, Widget,
    WidgetKind,
};

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// The names both mods draw from — deliberately overlapping, so a scenario can
/// produce shared and unshared names in the same load.
const STATS: [&str; 4] = ["power", "armor", "haste", "reach"];
const RESOURCES: [&str; 3] = ["energy", "fury", "focus"];

/// The engine's global id space, one interner per family the UI can bind.
#[derive(Default)]
struct Global {
    stats: Interner<StatId>,
    resources: Interner<ResourceId>,
}

/// A local handle that no name table explains — a bundle that lies about itself.
#[derive(Debug, PartialEq, Eq)]
struct Dangling;

/// The host's local→global map for one mod: resolve the local handle to the name
/// the mod gave it, then intern that name globally.
struct Adoption<'a> {
    names: &'a Names,
    global: &'a RefCell<Global>,
}

impl IdMap for Adoption<'_> {
    type Error = Dangling;
    fn stat(&self, id: StatId) -> Result<StatId, Dangling> {
        let name = self.names.stats.get(id.0 as usize).ok_or(Dangling)?;
        Ok(self.global.borrow_mut().stats.intern(name))
    }
    fn resource(&self, id: ResourceId) -> Result<ResourceId, Dangling> {
        let name = self.names.resources.get(id.0 as usize).ok_or(Dangling)?;
        Ok(self.global.borrow_mut().resources.intern(name))
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
    fn event(&self, id: EventId) -> Result<EventId, Dangling> {
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

/// One mod: the names it interned locally, and the HUD it authored against them.
struct Mod {
    names: Names,
    root: UiRoot,
}

fn a_widget(kind: WidgetKind) -> Widget {
    Widget { name: String::new(), layout: Layout::default(), style: Style::default(), kind }
}

/// A mod that binds one bar per stat it names and one per resource, in local
/// handle order — so the remapped tree can be read back positionally.
fn a_mod(stats: &[&str], resources: &[&str]) -> Mod {
    let mut children = Vec::new();
    for (raw, _) in stats.iter().enumerate() {
        children.push(a_widget(WidgetKind::Bar { value: ValueBinding::Stat(StatId(raw as u16)) }));
    }
    for (raw, _) in resources.iter().enumerate() {
        children.push(a_widget(WidgetKind::Bar {
            value: ValueBinding::Pool(PoolRef::Resource(ResourceId(raw as u16))),
        }));
    }
    Mod {
        names: Names {
            stats: stats.iter().map(ToString::to_string).collect(),
            resources: resources.iter().map(ToString::to_string).collect(),
            ..Names::default()
        },
        root: UiRoot {
            name: "hud".to_string(),
            when: RootVisibility::Always,
            summon: SummonGate::Ignored,
            subject: UiSubject::LocalPlayer,
            strip: Strip::default(),
            root: a_widget(WidgetKind::Panel { children }),
        },
    }
}

/// The bound handles of a remapped tree, in declaration order: stats first, then
/// resources — matching how [`a_mod`] laid them out.
fn bound(root: &UiRoot) -> (Vec<StatId>, Vec<ResourceId>) {
    let WidgetKind::Panel { children } = &root.root.kind else { unreachable!() };
    let mut stats = Vec::new();
    let mut resources = Vec::new();
    for child in children {
        match &child.kind {
            WidgetKind::Bar { value: ValueBinding::Stat(id) } => stats.push(*id),
            WidgetKind::Bar { value: ValueBinding::Pool(PoolRef::Resource(id)) } => {
                resources.push(*id);
            }
            _ => unreachable!(),
        }
    }
    (stats, resources)
}

/// Distinct names picked from `pool`, in the order the scenario chose them —
/// a local name table is dense and never repeats an entry.
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
    a_stats: [u8; 3],
    b_stats: [u8; 3],
    a_resources: [u8; 2],
    b_resources: [u8; 2],
}

#[test]
fn two_mods_agree_on_shared_names_and_never_collide_on_distinct_ones() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (sa, sb) = (pick(&STATS, &s.a_stats), pick(&STATS, &s.b_stats));
        let (ra, rb) = (pick(&RESOURCES, &s.a_resources), pick(&RESOURCES, &s.b_resources));

        let (mut a, mut b) = (a_mod(&sa, &ra), a_mod(&sb, &rb));
        let global = RefCell::new(Global::default());
        a.root.remap_ids(&Adoption { names: &a.names, global: &global }).expect("adopt a");
        b.root.remap_ids(&Adoption { names: &b.names, global: &global }).expect("adopt b");

        let (ga_stats, ga_res) = bound(&a.root);
        let (gb_stats, gb_res) = bound(&b.root);

        // Same name, same global handle — however differently the two mods
        // ordered their own local tables.
        for (i, name) in sa.iter().enumerate() {
            if let Some(j) = sb.iter().position(|other| other == name) {
                assert_eq!(ga_stats[i], gb_stats[j], "`{name}` adopted under two handles");
            }
        }
        for (i, name) in ra.iter().enumerate() {
            if let Some(j) = rb.iter().position(|other| other == name) {
                assert_eq!(ga_res[i], gb_res[j], "`{name}` adopted under two handles");
            }
        }

        // Different names, different global handles — the collision a second mod
        // authoring from raw 0 would otherwise cause.
        for (i, name) in sa.iter().enumerate() {
            for (j, other) in sb.iter().enumerate() {
                if name != other {
                    assert_ne!(ga_stats[i], gb_stats[j], "`{name}`/`{other}` share a handle");
                }
            }
        }
        for (i, name) in ra.iter().enumerate() {
            for (j, other) in rb.iter().enumerate() {
                if name != other {
                    assert_ne!(ga_res[i], gb_res[j], "`{name}`/`{other}` share a handle");
                }
            }
        }

        // Every adopted handle resolves back to the name the mod authored.
        let g = global.borrow();
        for (i, name) in sa.iter().enumerate() {
            assert_eq!(g.stats.resolve(ga_stats[i]), Some(*name));
        }
        for (i, name) in ra.iter().enumerate() {
            assert_eq!(g.resources.resolve(ga_res[i]), Some(*name));
        }
    });
}

#[test]
fn a_handle_the_name_table_does_not_explain_is_rejected() {
    check!().with_type::<u16>().for_each(|&raw| {
        // A bundle whose tree reaches past its own tables is a broken mod, not a
        // panic: the walk surfaces the map's error like every other family.
        let mut m = a_mod(&["power"], &[]);
        let WidgetKind::Panel { children } = &mut m.root.root.kind else { unreachable!() };
        let dangling = StatId(raw | 1); // never 0, the one handle the table has
        children[0].kind = WidgetKind::Bar { value: ValueBinding::Stat(dangling) };

        let global = RefCell::new(Global::default());
        let result = m.root.remap_ids(&Adoption { names: &m.names, global: &global });
        assert_eq!(result, Err(Dangling), "a dangling local handle must not adopt");
    });
}
