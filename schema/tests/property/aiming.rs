//! Invariants of how an ability declares its **aim** — the ABI half of
//! stormlight/server#59.
//!
//! Two independent pieces of vocabulary, both of which have to be stable for a
//! client and a server compiled from different mod loads to agree:
//!   - [`Targeting::shape`] — the mode → the *kind* of aim it admits. It is the
//!     single rule both ends read: the client builds an aim of that shape, the
//!     server accepts exactly that shape. A mode must therefore map to one shape
//!     and always the same one, and the mapping must survive a round trip.
//!   - [`params::RESERVED`] — the engine-reserved param names. Their **order is
//!     the id order** the engine seeds its interner with, so a duplicate or a
//!     reordering silently re-points every mod's `range` at something else.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::abilities::{AimShape, Targeting};
use stormlight_mod_abi::common::{Affiliation, TargetFilter};
use stormlight_mod_abi::ids::TagId;
use stormlight_mod_abi::params;

#[derive(Debug, TypeGenerator)]
enum Mode {
    NoTarget,
    SelfCast,
    Unit,
    Point,
    Vector,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    mode: Mode,
    /// The filter a unit-target mode carries — irrelevant to the shape, which is
    /// exactly what the invariant below pins down.
    affiliation: Aff,
    require: Vec<u16>,
    exclude: Vec<u16>,
    include_dead: bool,
}

#[derive(Debug, TypeGenerator)]
enum Aff {
    Enemies,
    Allies,
    All,
}

fn filter(s: &Scenario) -> TargetFilter {
    TargetFilter {
        affiliation: match s.affiliation {
            Aff::Enemies => Affiliation::Enemies,
            Aff::Allies => Affiliation::Allies,
            Aff::All => Affiliation::All,
        },
        require_tags: s.require.iter().map(|&t| TagId(t)).collect(),
        exclude_tags: s.exclude.iter().map(|&t| TagId(t)).collect(),
        include_dead: s.include_dead,
    }
}

fn targeting(s: &Scenario) -> Targeting {
    match s.mode {
        Mode::NoTarget => Targeting::NoTarget,
        Mode::SelfCast => Targeting::SelfCast,
        Mode::Unit => Targeting::Unit { filter: filter(s) },
        Mode::Point => Targeting::Point,
        Mode::Vector => Targeting::Vector,
    }
}

/// The shape a mode admits is a property of the *mode alone*: two modes that
/// differ only in their target filter admit the same aim. Otherwise the client
/// would have to understand filters to know what to send.
#[test]
fn shape_depends_only_on_the_mode() {
    check!().with_type::<(Scenario, Scenario)>().for_each(|(a, b)| {
        let (ta, tb) = (targeting(a), targeting(b));
        if core::mem::discriminant(&ta) == core::mem::discriminant(&tb) {
            assert_eq!(ta.shape(), tb.shape(), "same mode, different admitted aim");
        }
    });
}

/// A serialized descriptor decodes to a mode admitting the same aim — a client
/// that learned the mode from a mod package agrees with the server that ran it.
#[test]
fn shape_survives_a_round_trip() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mode = targeting(s);
        let bytes = postcard::to_allocvec(&mode).expect("targeting serializes");
        let back: Targeting = postcard::from_bytes(&bytes).expect("targeting decodes");
        assert_eq!(back, mode);
        assert_eq!(back.shape(), mode.shape());
    });
}

/// Only a real target carries an entity or a position: the two no-aim modes must
/// map to the empty shape, and no aimed mode may.
#[test]
fn only_untargeted_modes_admit_no_aim() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mode = targeting(s);
        let untargeted = matches!(mode, Targeting::NoTarget | Targeting::SelfCast);
        assert_eq!(untargeted, mode.shape() == AimShape::None);
    });
}

/// The reserved param names are an id table: each name appears once, and
/// [`params::reserved_index`] agrees with the position the engine interns it at.
#[test]
fn reserved_param_names_are_a_dense_unique_table() {
    for (i, name) in params::RESERVED.iter().enumerate() {
        assert_eq!(
            params::RESERVED.iter().filter(|n| *n == name).count(),
            1,
            "reserved param `{name}` listed twice — every id after it shifts"
        );
        assert_eq!(params::reserved_index(name), Some(i));
    }
    assert_eq!(params::reserved_index("a name no engine reserves"), None);
}
