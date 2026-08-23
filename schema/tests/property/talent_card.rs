//! Invariants of the talent card (`TalentCard`, server#95) — what a talent is
//! *called*, what it *does* in words, and what it *looks like*, so a panel that
//! offers a pending tier can say what any of it means.
//!
//! Keyed by the talent's interned handle for the reason
//! [`AbilityIcon`](stormlight_mod_abi::visuals::AbilityIcon) is keyed by the
//! ability's: the presentation is a fact about the *talent*, and the interface mod
//! that lays out the panel names no content. Directional / structural only:
//!   - **Round-trip**: a bundle of cards survives a postcard serialize /
//!     deserialize unchanged and re-serializes to identical bytes — it crosses the
//!     wasm boundary with the rest of `ClientRegistration`.
//!   - **Identity remap is a no-op**: remapping through a map that returns every
//!     talent id unchanged leaves the bundle byte-identical. The walk rewrites the
//!     talent handle and nothing else — least of all the words, which are the
//!     mod's and must reach the screen exactly as authored.
//!   - **The handle is really walked**: a map that fails on any talent makes the
//!     walk return `Err` exactly when the bundle carried a card, never a panic. A
//!     card whose handle adoption never translated would sit in the declaring
//!     mod's local id space and describe another package's talent.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::descriptors::Names;
use stormlight_mod_abi::ids::{
    AbilityId, AnimStateId, BuffId, CurveId, DamageTypeId, EventId, HandlerId, NavMeshId, ParamId,
    ResourceId, StackId, StatId, TagClassId, TagId, TalentId, UnitId,
};
use stormlight_mod_abi::manifest::ABI_VERSION;
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::visuals::{ClientRegistration, TalentCard, TalentInfo};

/// Every family returns its id unchanged — the identity map.
struct Identity;

/// Every family but `talent` returns its id unchanged; a talent handle fails,
/// modelling a cosmetic mod carding a talent the gameplay side never declared.
struct FailTalent;

macro_rules! total_but {
    ($map:ty, $talent:expr) => {
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
            fn ability(&self, id: AbilityId) -> Result<AbilityId, ()> {
                Ok(id)
            }
            #[allow(clippy::redundant_closure_call)]
            fn talent(&self, id: TalentId) -> Result<TalentId, ()> {
                ($talent)(id)
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
total_but!(FailTalent, |_| Err(()));

const IDENT: &[u8] = b"abcdefghijklmnopqrstuvwxyz_0123456789/. ";

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// One `(raw talent handle, text seed)` per card the mod declares.
    cards: Vec<(u16, Vec<u8>)>,
    /// The talent names the bundle interns — a card names a talent, so the table
    /// travels with it.
    names: u8,
}

/// Words from a byte seed. The content never matters to these invariants; that it
/// is carried through untouched does.
fn words(seed: &[u8]) -> String {
    seed.iter().map(|b| IDENT[*b as usize % IDENT.len()] as char).collect()
}

fn build(s: &Scenario) -> ClientRegistration {
    ClientRegistration {
        abi: ABI_VERSION,
        names: Names {
            talents: (0..s.names).map(|i| format!("t{i}")).collect(),
            ..Names::default()
        },
        cards: s
            .cards
            .iter()
            .map(|(handle, seed)| TalentCard {
                talent: TalentId(u32::from(*handle)),
                info: TalentInfo {
                    name: words(seed),
                    description: words(seed),
                    image: format!("mod://pack/{}", words(seed)),
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
fn a_failing_talent_map_errors_exactly_when_cards_are_present() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut reg = build(s);
        let carried = !reg.cards.is_empty();
        let result = reg.remap_ids(&FailTalent);
        assert_eq!(result.is_err(), carried, "card talent handles are not walked by the remap");
    });
}
