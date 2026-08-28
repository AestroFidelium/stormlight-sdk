//! Invariants of the progression ABI (stormlight/server#62) — how a mod declares
//! that a unit levels, and what killing it is worth.
//!
//! Talents are gated by level, so a talent experience needs a real economy behind
//! it. That economy is *entirely* content: how much XP a kill yields, how the yield
//! is divided, how much is needed for the next level, and what the level grants.
//! The engine knows only how to run the loop around those numbers, which is why
//! every one of them lives here rather than in a constant somewhere in the server.
//!
//! The contract this pins:
//!
//! - a declaration survives the postcard round trip the guest→host boundary uses,
//!   thresholds, grants, sources and bounty intact — a share policy that flipped
//!   crossing the ABI would hand a whole team XP a mod meant for one killer;
//! - the remap walk rewrites **every** interned handle it carries (the threshold
//!   curve, a grant's tags and abilities, the handles inside a bounty's or a
//!   source's `Value`) and leaves the plain numbers alone. A missed handle here is
//!   a dangling reference into another mod's id space;
//! - the sanitizing accessors are **total**: whatever a mod declares — zero, a
//!   negative period, an infinite window, NaN — reads as something the engine can
//!   act on, because none of these numbers is validated anywhere else.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{AbilityId, CurveId, Slot, TagId, UnitId};
use stormlight_mod_abi::math::{Value, Var, Who};
use stormlight_mod_abi::progression::{
    LevelGrants, ProgressionSpec, XpBounty, XpCurve, XpShare, XpSource, XpTrigger,
};
use stormlight_mod_abi::remap::{IdMap, RemapIds};
use stormlight_mod_abi::units::UnitDescriptor;

mod ids {
    pub use stormlight_mod_abi::ids::*;
}

/// A map that shifts every family a progression declaration can reference and
/// leaves the rest alone — so a handle the walk forgets shows up as an unshifted
/// number rather than as a silent pass.
struct ShiftReferenced;

macro_rules! identity_families {
    ($( $fn_name:ident($ty:ident) ),+ $(,)?) => {$(
        fn $fn_name(&self, id: ids::$ty) -> Result<ids::$ty, ()> {
            Ok(id)
        }
    )+};
}

macro_rules! shift_families {
    ($( $fn_name:ident($ty:ident) ),+ $(,)?) => {$(
        fn $fn_name(&self, id: ids::$ty) -> Result<ids::$ty, ()> {
            Ok(ids::$ty(id.0.wrapping_add(1)))
        }
    )+};
}

impl IdMap for ShiftReferenced {
    type Error = ();

    identity_families!(
        resource(ResourceId),
        stack(StackId),
        tag_class(TagClassId),
        param(ParamId),
        event(EventId),
        buff(BuffId),
        damage_type(DamageTypeId),
        talent(TalentId),
        handler(HandlerId),
        navmesh(NavMeshId),
        anim_state(AnimStateId),
    );

    shift_families!(stat(StatId), tag(TagId), curve(CurveId), ability(AbilityId), unit(UnitId),);
}

/// How a generated number is chosen: ordinary declarations plus the ways a mod can
/// hand the engine a quantity no loop can act on.
#[derive(Debug, TypeGenerator)]
enum Amount {
    /// A plain declaration, in 1/64ths.
    Declared(u16),
    Zero,
    Negative(u16),
    Infinite,
    NotANumber,
}

impl Amount {
    fn value(&self) -> f32 {
        match self {
            Amount::Declared(v) => f32::from(*v) / 64.0,
            Amount::Zero => 0.0,
            Amount::Negative(v) => -f32::from(*v) / 64.0,
            Amount::Infinite => f32::INFINITY,
            Amount::NotANumber => f32::NAN,
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    max_level: u8,
    curve: u16,
    table: Vec<u16>,
    by_table: bool,
    grant_level: u8,
    tag: u16,
    slot: u8,
    ability: u32,
    stat: u16,
    bounty: Option<(Amount, Share, Option<Amount>)>,
    period: Amount,
    timed: bool,
    unit: u16,
    /// The level the unit declares it begins at — unconstrained, so a start above
    /// the ceiling and a start of zero are both ordinary cases here.
    starting_level: u8,
}

/// The share policies, generated structurally so the round trip sees each one.
#[derive(Clone, Copy, Debug, TypeGenerator)]
enum Share {
    Killer,
    ByDamage,
    Team,
}

impl From<Share> for XpShare {
    fn from(s: Share) -> Self {
        match s {
            Share::Killer => XpShare::Killer,
            Share::ByDamage => XpShare::ByDamage,
            Share::Team => XpShare::Team,
        }
    }
}

/// A `Value` carrying one interned handle, so the remap walk has something to
/// prove it descended into an amount expression rather than skipping it.
fn stat_value(stat: u16) -> Value {
    Value::Read(Var::Stat(ids::StatId(stat), Who::Caster))
}

fn spec(s: &Scenario) -> ProgressionSpec {
    ProgressionSpec {
        thresholds: if s.by_table {
            XpCurve::Table(s.table.iter().map(|v| f32::from(*v)).collect())
        } else {
            XpCurve::Curve(CurveId(s.curve))
        },
        max_level: s.max_level,
        grants: vec![(
            s.grant_level,
            LevelGrants {
                tags: vec![TagId(s.tag)],
                abilities: vec![(Slot(s.slot), AbilityId(s.ability))],
            },
        )],
        sources: vec![XpSource {
            trigger: if s.timed {
                XpTrigger::Timed { period: s.period.value() }
            } else {
                XpTrigger::DamageDealt
            },
            amount: stat_value(s.stat),
        }],
        bounty: s.bounty.as_ref().map(|(amount, share, window)| XpBounty {
            amount: Value::Bin(
                stormlight_mod_abi::math::BinOp::Mul,
                Box::new(Value::Const(amount.value())),
                Box::new(stat_value(s.stat)),
            ),
            share: XpShare::from(*share),
            credit_window: window.as_ref().map(Amount::value),
        }),
        starting_level: s.starting_level,
    }
}

/// A unit descriptor carrying `progression`, otherwise as bare as one can be.
fn unit(id: UnitId, progression: Option<ProgressionSpec>) -> UnitDescriptor {
    UnitDescriptor {
        id,
        health: Value::Const(100.0),
        stats: Vec::new(),
        tags: Vec::new(),
        abilities: Vec::new(),
        resources: Vec::new(),
        talents: Vec::new(),
        talent_tree: None,
        respawn: None,
        progression,
        turn_rate: None,
        attack: None,
    }
}

#[test]
fn a_progression_declaration_survives_the_wire_round_trip_intact() {
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = spec(s);
        let bytes = postcard::to_allocvec(&declared).expect("a spec should encode");
        let back: ProgressionSpec = postcard::from_bytes(&bytes).expect("a spec should decode");

        assert_eq!(
            back.max_level, declared.max_level,
            "the level ceiling changed crossing the ABI"
        );
        assert_eq!(back.grants, declared.grants, "a level's grants changed crossing the ABI");
        assert_eq!(
            back.bounty.as_ref().map(|b| b.share),
            declared.bounty.as_ref().map(|b| b.share),
            "a share policy flipped crossing the ABI",
        );
        // Windows and periods are compared through their sanitizing accessors,
        // because NaN is not equal to itself: the contract is that the *effective*
        // number crosses unchanged.
        assert_eq!(
            back.bounty.as_ref().and_then(XpBounty::window),
            declared.bounty.as_ref().and_then(XpBounty::window),
            "a credit window changed crossing the ABI",
        );
        assert_eq!(
            back.sources.iter().map(XpSource::period).collect::<Vec<_>>(),
            declared.sources.iter().map(XpSource::period).collect::<Vec<_>>(),
            "an XP source's cadence changed crossing the ABI",
        );
        match (&back.thresholds, &declared.thresholds) {
            (XpCurve::Table(a), XpCurve::Table(b)) => assert_eq!(a, b, "a threshold table changed"),
            (XpCurve::Curve(a), XpCurve::Curve(b)) => assert_eq!(a, b, "a threshold curve changed"),
            _ => panic!("the threshold declaration changed shape crossing the ABI"),
        }
    });
}

#[test]
fn the_remap_walk_rewrites_every_handle_a_declaration_carries() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut carrier = unit(UnitId(u32::from(s.unit)), Some(spec(s)));
        carrier.remap_ids(&ShiftReferenced).expect("an infallible map cannot fail");

        let after = carrier.progression.expect("a remapped unit lost its progression");

        // The threshold curve is a handle into the mod's own curve space.
        if let XpCurve::Curve(id) = after.thresholds {
            assert_eq!(
                id,
                CurveId(s.curve.wrapping_add(1)),
                "the threshold curve handle was not remapped"
            );
        }
        // A grant's tags and granted abilities are handles too.
        let (level, grants) = &after.grants[0];
        assert_eq!(*level, s.grant_level, "a grant's level index was rewritten as if a handle");
        assert_eq!(
            grants.tags,
            vec![TagId(s.tag.wrapping_add(1))],
            "a granted tag was not remapped"
        );
        assert_eq!(
            grants.abilities,
            vec![(Slot(s.slot), AbilityId(s.ability.wrapping_add(1)))],
            "a granted ability was not remapped (or its slot was rewritten)",
        );
        // And so is every handle inside an amount expression, on both directions
        // of the economy — what the unit earns and what it is worth.
        assert_eq!(
            after.sources[0].amount,
            stat_value(s.stat.wrapping_add(1)),
            "an XP source's amount expression was not walked",
        );
        if let Some(bounty) = &after.bounty {
            let Value::Bin(_, _, scaled) = &bounty.amount else {
                panic!("the bounty amount changed shape under the remap walk");
            };
            assert_eq!(
                scaled.as_ref(),
                &stat_value(s.stat.wrapping_add(1)),
                "a bounty amount expression was not walked",
            );
        }
    });
}

#[test]
fn a_unit_that_declares_no_progression_is_given_none() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut carrier = unit(UnitId(u32::from(s.unit)), None);
        carrier.remap_ids(&ShiftReferenced).expect("an infallible map cannot fail");
        assert!(
            carrier.progression.is_none(),
            "a unit that declared no progression was given one by the remap walk",
        );
    });
}

#[test]
fn the_level_ceiling_always_admits_the_level_every_unit_starts_at() {
    check!().with_type::<Scenario>().for_each(|s| {
        let ceiling = spec(s).ceiling();
        assert!(ceiling >= 1, "a declared ceiling excluded the level a unit spawns at");
        if s.max_level >= 1 {
            assert_eq!(ceiling, s.max_level, "an ordinary declared ceiling was altered");
        }
    });
}

#[test]
fn an_effective_credit_window_is_finite_and_non_negative() {
    check!().with_type::<Scenario>().for_each(|s| {
        let Some(bounty) = spec(s).bounty else { return };
        let Some(window) = bounty.window() else { return };
        assert!(window.is_finite(), "a credit window no clock reaches survived sanitizing");
        assert!(window >= 0.0, "a credit window reached backwards in time");
        // A window a mod declared honestly passes through untouched — the
        // normalization is a floor for nonsense, not a policy of its own.
        if let Some((_, _, Some(Amount::Declared(v)))) = &s.bounty {
            assert_eq!(window, f32::from(*v) / 64.0, "an ordinary credit window was altered");
        }
    });
}

#[test]
fn an_effective_timed_period_is_finite_and_strictly_positive() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = spec(s);
        let source = &spec.sources[0];
        match source.period() {
            // A source that fires on a cadence must have a cadence a clock can
            // count: anything else is a source that never fires, never one that
            // fires every tick forever.
            Some(period) => {
                assert!(period.is_finite(), "a period no clock reaches survived sanitizing");
                assert!(period > 0.0, "a non-positive period became a firing cadence");
                assert!(s.timed, "a source with no cadence reported one");
            }
            None => assert!(
                !s.timed || !matches!(s.period, Amount::Declared(v) if v > 0),
                "an ordinary declared period was discarded",
            ),
        }
    });
}

/// Where a unit begins (stormlight/server#131).
///
/// A suggestion the engine has to make sense of whatever a mod writes, so the
/// properties are all about totality:
///
///   - **never below the first level.** Zero means "said nothing", which is the
///     ordinary unit that starts at the bottom and climbs — and it is what every
///     spec written before this field says;
///   - **never above the unit's own ceiling.** A unit declaring a start past its
///     maximum would stand at a level no threshold describes and no climb could
///     produce, so it starts at the top instead, which is what it meant;
///   - **inside its own range**, so the answer is always a level the unit could
///     really be at.
#[test]
fn a_unit_begins_somewhere_it_could_really_be() {
    check!().with_type::<Scenario>().for_each(|s| {
        let spec = spec(s);
        let start = spec.start();
        assert!(start >= 1, "a unit began at level {start}");
        assert!(
            start <= spec.ceiling(),
            "a unit declaring a start of {} began at {start}, past its own ceiling of {}",
            spec.starting_level,
            spec.ceiling(),
        );
        // And it is the declaration wherever the declaration is reachable.
        if spec.starting_level >= 1 && spec.starting_level <= spec.ceiling() {
            assert_eq!(start, spec.starting_level, "a reachable start was not honoured");
        }
        // Saying nothing is saying the bottom.
        let mut silent = spec.clone();
        silent.starting_level = 0;
        assert_eq!(silent.start(), 1, "a spec that declares no start did not begin at the bottom");
    });
}
