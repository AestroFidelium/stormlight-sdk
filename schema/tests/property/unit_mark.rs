//! What lies under a unit because of what it is to the viewer
//! (stormlight/server#181). Laws, over any list of declared marks:
//!   - **the first declaration that fits wins**: a unit playing a role, at a
//!     relation, wears the look of the first mark naming that role and that
//!     relation (or every relation) — and none when no mark fits;
//!   - **a mark is never worn for a role it does not name**;
//!   - **a look a renderer cannot draw is refused**: no picture, a non-finite
//!     tint or turn, or a width that is not positive and finite;
//!   - **the declarations reach the host intact** through the registration bundle.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::decal::DecalBlend;
use stormlight_mod_abi::manifest::ABI_VERSION;
use stormlight_mod_abi::unit_mark::{MarkLook, MarkRole, Relation, UnitMark, mark_for};
use stormlight_mod_abi::visuals::ClientRegistration;

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Role {
    Driven,
    Selected,
    Target,
    Hovered,
}

impl Role {
    fn abi(self) -> MarkRole {
        match self {
            Self::Driven => MarkRole::Driven,
            Self::Selected => MarkRole::Selected,
            Self::Target => MarkRole::Target,
            Self::Hovered => MarkRole::Hovered,
        }
    }
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Side {
    Own,
    Ally,
    Enemy,
    Neutral,
}

impl Side {
    fn abi(self) -> Relation {
        match self {
            Self::Own => Relation::Own,
            Self::Ally => Relation::Ally,
            Self::Enemy => Relation::Enemy,
            Self::Neutral => Relation::Neutral,
        }
    }
}

/// One declared mark; its picture names its place in the list, so the look a
/// unit wears says which declaration it came from.
#[derive(Debug, Clone, Copy, TypeGenerator)]
struct Declared {
    role: Role,
    relation: Option<Side>,
    turns: bool,
}

fn marks(declared: &[Declared]) -> Vec<UnitMark> {
    declared
        .iter()
        .enumerate()
        .map(|(i, d)| UnitMark {
            role: d.role.abi(),
            relation: d.relation.map(Side::abi),
            look: MarkLook {
                asset: format!("mod://pack/mark{i}.png"),
                tint: [1.0; 4],
                blend: DecalBlend::Blend,
                scale: 1.2,
                turns: d.turns,
                yaw_offset: 0.0,
            },
        })
        .collect()
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    #[generator(bolero::produce::<Vec<Declared>>().with().len(0usize..=8))]
    declared: Vec<Declared>,
    role: Role,
    relation: Side,
}

#[test]
fn a_unit_wears_the_first_mark_that_fits() {
    check!().with_type::<Scenario>().for_each(|s| {
        let marks = marks(&s.declared);
        let (role, relation) = (s.role.abi(), s.relation.abi());
        let fits = |m: &UnitMark| m.role == role && m.relation.is_none_or(|r| r == relation);
        let worn = mark_for(&marks, role, relation);
        match marks.iter().position(fits) {
            None => assert!(worn.is_none(), "{s:?}: a mark was worn that nothing declared"),
            Some(first) => {
                let worn = worn.expect("a declaration fits");
                assert_eq!(worn.asset, format!("mod://pack/mark{first}.png"), "{s:?}");
                assert_eq!(marks[first].role, role, "{s:?}: worn for a role it does not name");
            }
        }
    });
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Fault {
    Sound,
    NoPicture,
    InfiniteTint,
    ZeroWidth,
    NegativeWidth,
    InfiniteWidth,
    InfiniteTurn,
}

#[derive(Debug, TypeGenerator)]
struct Look {
    fault: Fault,
    /// Tenths of the body's width.
    #[generator(1u8..=40)]
    scale: u8,
}

#[test]
fn a_look_no_renderer_can_draw_is_refused() {
    check!().with_type::<Look>().for_each(|l| {
        let mut look = MarkLook {
            asset: "mod://pack/ring.png".into(),
            tint: [0.2, 1.0, 0.3, 0.9],
            blend: DecalBlend::Add,
            scale: f32::from(l.scale) / 10.0,
            turns: true,
            yaw_offset: 3.0,
        };
        match l.fault {
            Fault::Sound => {}
            Fault::NoPicture => look.asset.clear(),
            Fault::InfiniteTint => look.tint[1] = f32::INFINITY,
            Fault::ZeroWidth => look.scale = 0.0,
            Fault::NegativeWidth => look.scale = -look.scale,
            Fault::InfiniteWidth => look.scale = f32::NAN,
            Fault::InfiniteTurn => look.yaw_offset = f32::INFINITY,
        }
        assert_eq!(look.is_valid(), matches!(l.fault, Fault::Sound), "{l:?}");
    });
}

#[test]
fn the_marks_reach_the_host_intact() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = ClientRegistration {
            abi: ABI_VERSION,
            unit_marks: marks(&s.declared),
            ..ClientRegistration::default()
        };
        let bytes = postcard::to_allocvec(&reg).expect("a bundle encodes");
        let back: ClientRegistration = postcard::from_bytes(&bytes).expect("and decodes");
        assert_eq!(back.unit_marks, reg.unit_marks, "{s:?}");
    });
}
