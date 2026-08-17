//! Invariants of the UI descriptor ABI (`UiRoot` / `Widget`, stormlight/server#66)
//! — what a cosmetic mod declares so the content-free client can build a HUD.
//! Directional/structural only:
//!   - **Round-trip**: any generated tree, including deeply nested containers,
//!     survives a postcard serialize / deserialize unchanged and re-serializes to
//!     identical bytes (this crosses the wasm boundary like every other bundle).
//!   - **Remap identity is a no-op**: remapping through a map that returns every
//!     handle unchanged leaves the tree byte-identical — the walk rewrites the
//!     bound `StatId`/`ResourceId`/`StackId` and touches nothing else (no layout
//!     number, no asset string, no `Slot`).
//!   - **Totality**: a map that fails on a family makes the walk return `Err`
//!     exactly when the tree bound something of that family, never a panic.
//!   - **Tag stability**: a widget kind's wire tag is its declaration index and
//!     does not depend on the payload — the ABI is unversioned, so a new kind may
//!     only be appended, never inserted before an existing one.

use core::cell::Cell;

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, Slot, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::impacts::PoolRef;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::ui::{
    Anchor, Border, Flow, InteractionStyle, Layout, Length, ListBinding, RootVisibility, Slice,
    StateStyle, Style, Sweep, SweepDirection, TextSource, UiAction, UiRoot, UiSubject,
    ValueBinding, ValuePart, Widget, WidgetKind, WidgetState,
};
use stormlight_mod_abi::ui_anim::{
    Ease, MAX_UI_TRACKS, Playback, Shape, UiKey, UiProperty, UiTrack, UiTransition, UiTrigger,
};

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// An identity map that counts the handles the walk touched per family, so a test
/// knows whether a tree bound any. The UI reaches exactly three families: a stat
/// binding, and a pool binding's resource / stack counter.
#[derive(Default)]
struct Counting {
    stats: Cell<u32>,
    resources: Cell<u32>,
    stacks: Cell<u32>,
}

impl IdMap for Counting {
    type Error = ();
    fn stat(&self, id: StatId) -> Result<StatId, ()> {
        self.stats.set(self.stats.get() + 1);
        Ok(id)
    }
    fn resource(&self, id: ResourceId) -> Result<ResourceId, ()> {
        self.resources.set(self.resources.get() + 1);
        Ok(id)
    }
    fn stack(&self, id: StackId) -> Result<StackId, ()> {
        self.stacks.set(self.stacks.get() + 1);
        Ok(id)
    }
    fn tag(&self, id: TagId) -> Result<TagId, ()> {
        Ok(id)
    }
    fn tag_class(&self, id: TagClassId) -> Result<TagClassId, ()> {
        Ok(id)
    }
    fn param(&self, id: ParamId) -> Result<ParamId, ()> {
        Ok(id)
    }
    fn event(&self, id: EventId) -> Result<EventId, ()> {
        Ok(id)
    }
    fn buff(&self, id: BuffId) -> Result<BuffId, ()> {
        Ok(id)
    }
    fn curve(&self, id: CurveId) -> Result<CurveId, ()> {
        Ok(id)
    }
    fn damage_type(&self, id: DamageTypeId) -> Result<DamageTypeId, ()> {
        Ok(id)
    }
    fn ability(&self, id: AbilityId) -> Result<AbilityId, ()> {
        Ok(id)
    }
    fn talent(&self, id: TalentId) -> Result<TalentId, ()> {
        Ok(id)
    }
    fn handler(&self, id: HandlerId) -> Result<HandlerId, ()> {
        Ok(id)
    }
    fn unit(&self, id: UnitId) -> Result<UnitId, ()> {
        Ok(id)
    }
    fn navmesh(&self, id: NavMeshId) -> Result<NavMeshId, ()> {
        Ok(id)
    }
    fn anim_state(&self, id: AnimStateId) -> Result<AnimStateId, ()> {
        Ok(id)
    }
}

/// Which family a [`Failing`] map refuses to translate — a mod binding a widget to
/// a stat / resource / stack the gameplay side never defined.
#[derive(Clone, Copy, Debug, TypeGenerator, PartialEq, Eq)]
enum Family {
    Stat,
    Resource,
    Stack,
}

/// A map that fails on exactly one family and passes everything else through.
struct Failing(Family);

impl IdMap for Failing {
    type Error = ();
    fn stat(&self, id: StatId) -> Result<StatId, ()> {
        if self.0 == Family::Stat { Err(()) } else { Ok(id) }
    }
    fn resource(&self, id: ResourceId) -> Result<ResourceId, ()> {
        if self.0 == Family::Resource { Err(()) } else { Ok(id) }
    }
    fn stack(&self, id: StackId) -> Result<StackId, ()> {
        if self.0 == Family::Stack { Err(()) } else { Ok(id) }
    }
    fn tag(&self, id: TagId) -> Result<TagId, ()> {
        Ok(id)
    }
    fn tag_class(&self, id: TagClassId) -> Result<TagClassId, ()> {
        Ok(id)
    }
    fn param(&self, id: ParamId) -> Result<ParamId, ()> {
        Ok(id)
    }
    fn event(&self, id: EventId) -> Result<EventId, ()> {
        Ok(id)
    }
    fn buff(&self, id: BuffId) -> Result<BuffId, ()> {
        Ok(id)
    }
    fn curve(&self, id: CurveId) -> Result<CurveId, ()> {
        Ok(id)
    }
    fn damage_type(&self, id: DamageTypeId) -> Result<DamageTypeId, ()> {
        Ok(id)
    }
    fn ability(&self, id: AbilityId) -> Result<AbilityId, ()> {
        Ok(id)
    }
    fn talent(&self, id: TalentId) -> Result<TalentId, ()> {
        Ok(id)
    }
    fn handler(&self, id: HandlerId) -> Result<HandlerId, ()> {
        Ok(id)
    }
    fn unit(&self, id: UnitId) -> Result<UnitId, ()> {
        Ok(id)
    }
    fn navmesh(&self, id: NavMeshId) -> Result<NavMeshId, ()> {
        Ok(id)
    }
    fn anim_state(&self, id: AnimStateId) -> Result<AnimStateId, ()> {
        Ok(id)
    }
}

const IDENT: &[u8] = b"abcdefghijklmnopqrstuvwxyz_0123456789/";

/// A cursor over a generated `u16` stream — the established generator shape.
struct Gen<'a> {
    ops: &'a [u16],
    pos: usize,
}

impl Gen<'_> {
    fn next(&mut self) -> u16 {
        let v = self.ops.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        v
    }
    fn count(&mut self, ceil: u16) -> u16 {
        self.next() % ceil
    }
    fn f32(&mut self) -> f32 {
        // Always finite: value-equality after a round-trip is meaningful.
        f32::from(self.next()) / f32::from(u16::MAX) * 200.0 - 100.0
    }
    fn string(&mut self) -> String {
        let len = self.count(8) as usize;
        let mut s = String::new();
        for _ in 0..len {
            s.push(IDENT[self.next() as usize % IDENT.len()] as char);
        }
        s
    }
    fn length(&mut self) -> Length {
        match self.next() % 3 {
            0 => Length::Px(self.f32()),
            1 => Length::Fraction(self.f32()),
            _ => Length::Auto,
        }
    }
    fn anchor(&mut self) -> Anchor {
        match self.next() % 9 {
            0 => Anchor::TopLeft,
            1 => Anchor::TopCenter,
            2 => Anchor::TopRight,
            3 => Anchor::CenterLeft,
            4 => Anchor::Center,
            5 => Anchor::CenterRight,
            6 => Anchor::BottomLeft,
            7 => Anchor::BottomCenter,
            _ => Anchor::BottomRight,
        }
    }
    fn layout(&mut self) -> Layout {
        Layout {
            anchor: self.anchor(),
            offset: [self.length(), self.length()],
            size: [self.length(), self.length()],
            flow: match self.next() % 3 {
                0 => Flow::Row,
                1 => Flow::Column,
                _ => Flow::Stack,
            },
            gap: self.f32(),
            padding: self.f32(),
        }
    }
    fn rgba(&mut self) -> [f32; 4] {
        [self.f32(), self.f32(), self.f32(), self.f32()]
    }
    /// One state override, each property present about half the time — so the
    /// round-trip sees every combination of declared and undeclared.
    fn state_style(&mut self) -> Option<StateStyle> {
        if !self.next().is_multiple_of(2) {
            return None;
        }
        Some(StateStyle {
            color: self.next().is_multiple_of(2).then(|| self.rgba()),
            background: self.next().is_multiple_of(2).then(|| self.rgba()),
            image: self.next().is_multiple_of(2).then(|| self.string()),
        })
    }
    fn states(&mut self) -> InteractionStyle {
        InteractionStyle {
            hover: self.state_style(),
            press: self.state_style(),
            disabled: self.state_style(),
        }
    }
    fn style(&mut self) -> Style {
        Style {
            color: self.rgba(),
            background: self.rgba(),
            border: Border { color: self.rgba(), width: self.f32() },
            font_size: self.f32(),
            font: if self.next().is_multiple_of(2) { Some(self.string()) } else { None },
            image: if self.next().is_multiple_of(2) { Some(self.string()) } else { None },
            // Half the styles carry slice insets, so the round trip covers both
            // the sliced and the stretched frame.
            slice: if self.next().is_multiple_of(2) {
                Some(Slice {
                    left: self.f32(),
                    top: self.f32(),
                    right: self.f32(),
                    bottom: self.f32(),
                })
            } else {
                None
            },
            flip_x: self.next().is_multiple_of(2),
            flip_y: self.next().is_multiple_of(3),
            states: self.states(),
            sweep: Sweep {
                color: self.rgba(),
                direction: if self.next().is_multiple_of(2) {
                    SweepDirection::Clockwise
                } else {
                    SweepDirection::CounterClockwise
                },
            },
            anim: self.anim(),
            transition: self.next().is_multiple_of(3).then(|| UiTransition {
                seconds: self.f32(),
                shape: Shape { leave: self.ease(), arrive: self.ease() },
            }),
        }
    }
    fn ease(&mut self) -> Ease {
        match self.next() % 4 {
            0 => Ease::Linear,
            1 => Ease::Slow,
            2 => Ease::Fast,
            _ => Ease::Step,
        }
    }
    /// The tracks a widget declares (server#97) — usually none, which is the case
    /// a HUD is mostly made of, and never more than the interpreter's budget.
    fn anim(&mut self) -> Vec<UiTrack> {
        let count = usize::from(self.next()) % (MAX_UI_TRACKS + 1);
        (0..count)
            .map(|_| UiTrack {
                property: match self.next() % 4 {
                    0 => UiProperty::TranslateX,
                    1 => UiProperty::TranslateY,
                    2 => UiProperty::Scale,
                    _ => UiProperty::Opacity,
                },
                on: match self.next() % 3 {
                    0 => UiTrigger::Built,
                    1 => UiTrigger::State(WidgetState::Hovered),
                    _ => UiTrigger::Hidden,
                },
                playback: match self.next() % 3 {
                    0 => Playback::Once,
                    1 => Playback::Loop,
                    _ => Playback::PingPong,
                },
                keys: (0..=usize::from(self.next()) % 3)
                    .map(|index| UiKey {
                        #[allow(clippy::cast_precision_loss)] // Three keys at most.
                        time: index as f32,
                        value: self.f32(),
                        arrive: self.ease(),
                        leave: self.ease(),
                    })
                    .collect(),
            })
            .collect()
    }
    fn pool(&mut self) -> PoolRef {
        match self.next() % 6 {
            0 => PoolRef::Shield,
            1 => PoolRef::Resource(ResourceId(self.next())),
            2 => PoolRef::Stacks(StackId(self.next())),
            3 => PoolRef::Cooldown(Slot(self.next() as u8)),
            4 => PoolRef::Charges(Slot(self.next() as u8)),
            _ => PoolRef::Xp,
        }
    }
    fn binding(&mut self) -> ValueBinding {
        match self.next() % 5 {
            0 => ValueBinding::Health,
            1 => ValueBinding::Pool(self.pool()),
            2 => ValueBinding::Stat(StatId(self.next())),
            3 => ValueBinding::Level,
            _ => ValueBinding::CastProgress,
        }
    }
    fn text(&mut self) -> TextSource {
        match self.next() % 3 {
            0 => TextSource::Literal(self.string()),
            1 => TextSource::Value {
                binding: self.binding(),
                part: match self.next() % 3 {
                    0 => ValuePart::Current,
                    1 => ValuePart::Max,
                    _ => ValuePart::Fraction,
                },
                decimals: self.next() as u8,
            },
            _ => TextSource::List(ListBinding::ChosenTalents),
        }
    }
    fn action(&mut self) -> UiAction {
        match self.next() % 3 {
            0 => UiAction::CastSlot(Slot(self.next() as u8)),
            1 => UiAction::PickTalent { tier: self.next() as u8, option: self.next() as u8 },
            _ => UiAction::Trigger { event: EventId(self.next()) },
        }
    }
    /// A widget, nesting until `depth` runs out — the deeply nested container the
    /// round-trip invariant is about.
    fn widget(&mut self, depth: u8) -> Widget {
        let kind = match self.next() % 6 {
            0 if depth > 0 => WidgetKind::Panel { children: self.children(depth) },
            1 => WidgetKind::Text { text: self.text() },
            2 => WidgetKind::Bar { value: self.binding() },
            3 => WidgetKind::Icon,
            4 if depth > 0 => {
                WidgetKind::Button { action: self.action(), children: self.children(depth) }
            }
            _ => WidgetKind::AbilitySlot { slot: Slot(self.next() as u8), key_hint: self.string() },
        };
        Widget { name: self.string(), layout: self.layout(), style: self.style(), kind }
    }
    fn children(&mut self, depth: u8) -> Vec<Widget> {
        let n = self.count(3);
        (0..n).map(|_| self.widget(depth - 1)).collect()
    }
    fn root(&mut self) -> UiRoot {
        let when = match self.next() % 3 {
            0 => RootVisibility::Always,
            1 => RootVisibility::WhileTalentPending,
            _ => RootVisibility::WhileUnitHovered,
        };
        UiRoot {
            name: self.string(),
            when,
            subject: match self.next() % 3 {
                0 => UiSubject::LocalPlayer,
                1 => UiSubject::HoveredUnit,
                _ => UiSubject::EachUnit,
            },
            root: self.widget(4),
        }
    }
    fn roots(&mut self) -> Vec<UiRoot> {
        let n = self.count(3);
        (0..n).map(|_| self.root()).collect()
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    ops: Vec<u16>,
    family: Family,
}

fn build(s: &Scenario) -> Vec<UiRoot> {
    Gen { ops: &s.ops, pos: 0 }.roots()
}

#[test]
fn any_ui_tree_survives_a_postcard_round_trip() {
    check!().with_type::<Scenario>().for_each(|s| {
        let roots = build(s);
        let bytes = postcard::to_allocvec(&roots).expect("serialize");
        let back: Vec<UiRoot> = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(roots, back, "ui tree did not round-trip");
        let again = postcard::to_allocvec(&back).expect("reserialize");
        assert_eq!(bytes, again, "ui tree serialization is not stable");
    });
}

#[test]
fn identity_remap_is_a_noop() {
    check!().with_type::<Scenario>().for_each(|s| {
        let roots = build(s);
        let mut out = roots.clone();
        out.remap_ids(&Counting::default()).expect("identity map never fails");
        let a = postcard::to_allocvec(&roots).expect("serialize");
        let b = postcard::to_allocvec(&out).expect("serialize");
        assert_eq!(a, b, "identity remap changed the tree");
    });
}

#[test]
fn a_failing_map_errors_exactly_when_that_family_is_bound() {
    check!().with_type::<Scenario>().for_each(|s| {
        let counter = Counting::default();
        let mut probe = build(s);
        probe.remap_ids(&counter).expect("total map succeeds");
        let bound = match s.family {
            Family::Stat => counter.stats.get(),
            Family::Resource => counter.resources.get(),
            Family::Stack => counter.stacks.get(),
        } > 0;

        let mut roots = build(s);
        let result = roots.remap_ids(&Failing(s.family));
        assert_eq!(
            result.is_err(),
            bound,
            "error propagation disagrees with {:?} binding presence",
            s.family,
        );
    });
}

#[test]
fn a_widget_kinds_wire_tag_is_its_declaration_index() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut g = Gen { ops: &s.ops, pos: 0 };
        // The tag must depend on the variant alone, never on the payload it
        // carries — that is what makes appending a kind safe on an unversioned
        // ABI, and inserting one before an existing kind unsafe.
        let kinds = [
            WidgetKind::Panel { children: g.children(2) },
            WidgetKind::Text { text: g.text() },
            WidgetKind::Bar { value: g.binding() },
            WidgetKind::Icon,
            WidgetKind::Button { action: g.action(), children: g.children(2) },
            WidgetKind::AbilitySlot { slot: Slot(g.next() as u8), key_hint: g.string() },
        ];
        for (tag, kind) in kinds.iter().enumerate() {
            let bytes = postcard::to_allocvec(kind).expect("serialize");
            assert_eq!(
                bytes.first().copied(),
                Some(tag as u8),
                "widget kind {kind:?} moved off wire tag {tag}",
            );
        }
    });
}
