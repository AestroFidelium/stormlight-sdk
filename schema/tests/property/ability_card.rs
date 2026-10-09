//! Invariants of the ability card (`AbilityCard`, server#94, server#116) — what a
//! cosmetic mod says about an *ability* — its name, what it does, and its picture —
//! so an interface can draw and describe it in whichever slot that ability
//! occupies. The same shape a talent's card has. Keyed by the ability's interned handle,
//! exactly like the feedback visuals beside it, because the interface mod that
//! draws the bar knows no name of the gameplay mod's content. Directional /
//! structural only:
//!   - **Round-trip**: a bundle of cards survives a postcard serialize /
//!     deserialize unchanged and re-serializes to identical bytes — this crosses
//!     the wasm boundary like the rest of `ClientRegistration`.
//!   - **Identity remap is a no-op**: remapping through a map that returns every
//!     ability id unchanged leaves the bundle byte-identical (the walk rewrites
//!     the ability handle and touches nothing else — least of all the words).
//!   - **The handle is really walked**: a map that fails on any ability makes the
//!     walk return `Err` exactly when the bundle carried a card, never a panic.
//!     A card whose handle adoption never translated would resolve against the
//!     declaring mod's local id space and draw another package's picture.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::descriptors::Names;
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::manifest::ABI_VERSION;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::visuals::{AbilityCard, CardInfo, ClientRegistration};

/// Every family returns its id unchanged — the identity map.
struct Identity;

/// Every family but `ability` returns its id unchanged; an ability handle fails,
/// modelling a cosmetic mod naming an ability the gameplay side never defined.
struct FailAbility;

macro_rules! total_but {
    ($map:ty, $ability:expr) => {
        impl IdMap for $map {
            type Error = ();
            fn stat(&self, id: StatId) -> Result<StatId, ()> {
                Ok(id)
            }
            fn resource(&self, id: ResourceId) -> Result<ResourceId, ()> {
                Ok(id)
            }
            fn stack(&self, id: StackId) -> Result<StackId, ()> {
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
            #[allow(clippy::redundant_closure_call)]
            fn ability(&self, id: AbilityId) -> Result<AbilityId, ()> {
                ($ability)(id)
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
    };
}

total_but!(Identity, Ok);
total_but!(FailAbility, |_| Err(()));

const IDENT: &[u8] = b"abcdefghijklmnopqrstuvwxyz_0123456789/.";

/// One card a mod declares.
#[derive(Debug, TypeGenerator)]
struct Card {
    /// The raw ability handle it names.
    handle: u16,
    /// Seeds for its name, its sentence and its picture.
    name: Vec<u8>,
    words: Vec<u8>,
    picture: Vec<u8>,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    cards: Vec<Card>,
    /// The ability names the bundle interns — an icon names an ability, so the
    /// table travels with it.
    names: u8,
}

/// A `mod://` looking path from a byte seed. The content never matters to these
/// invariants; that it is carried through untouched does.
fn path(seed: &[u8]) -> String {
    let tail: String = seed.iter().map(|b| IDENT[*b as usize % IDENT.len()] as char).collect();
    format!("mod://pack/{tail}")
}

fn build(s: &Scenario) -> ClientRegistration {
    ClientRegistration {
        abi: ABI_VERSION,
        names: Names {
            abilities: (0..s.names).map(|i| format!("a{i}")).collect(),
            ..Names::default()
        },
        ability_cards: s
            .cards
            .iter()
            .map(|card| AbilityCard {
                ability: AbilityId(u32::from(card.handle)),
                info: CardInfo {
                    name: path(&card.name),
                    description: path(&card.words),
                    image: path(&card.picture),
                },
            })
            .collect(),
        ..ClientRegistration::default()
    }
}

#[test]
fn declared_cards_survive_a_postcard_round_trip() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = build(s);
        let bytes = postcard::to_allocvec(&reg).expect("serialize");
        let back: ClientRegistration = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(reg, back, "a card bundle did not round-trip");
        let again = postcard::to_allocvec(&back).expect("reserialize");
        assert_eq!(bytes, again, "card serialization is not stable");
    });
}

#[test]
fn identity_remap_leaves_every_card_untouched() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = build(s);
        let mut out = reg.clone();
        out.remap_ids(&Identity).expect("identity map never fails");
        assert_eq!(reg, out, "identity remap changed a card");
    });
}

#[test]
fn a_failing_ability_map_errors_exactly_when_cards_are_present() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut reg = build(s);
        let carried = !reg.ability_cards.is_empty();
        let result = reg.remap_ids(&FailAbility);
        assert_eq!(result.is_err(), carried, "card ability handles are not walked by the remap",);
    });
}
