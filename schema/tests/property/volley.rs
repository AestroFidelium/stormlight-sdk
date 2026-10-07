//! Volley transforms (stormlight/server#210): talents that multiply an ability's
//! *own* projectiles, whatever ability and whatever unit they are given to.
//!
//! Laws, over arbitrary programs and arbitrary stacks of fans and echoes:
//!   - **they multiply**: the bodies a cast fires grow by `extra + 1` per fan and
//!     `times + 1` per echo, in any order — two talents never need to know about
//!     each other to combine;
//!   - **order does not matter**: the same talents in any order fire the same
//!     number of bodies;
//!   - **only the volley repeats**: a cast's other effects (a pool adjust, a
//!     direct hit) run once however many copies fly;
//!   - **copies are the ability's own**: a fan's copy is the original body with its
//!     payload's damage and healing scaled by the fan's power, turned off the
//!     original's aim — never a body the talent brought with it;
//!   - no transform leaves a program byte-identical.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::common::{Affiliation, ImpactTarget, TargetFilter};
use stormlight_mod_abi::ids::DamageTypeId;
use stormlight_mod_abi::impacts::{DamageFlags, Impact, LoopKind, SpawnAnchor, SpawnPattern};
use stormlight_mod_abi::math::{BinOp, Value};
use stormlight_mod_abi::missiles::{BodyDescriptor, BodyFlags, BodyKind, CollisionSpec};
use stormlight_mod_abi::volley::{VolleyMod, apply_volley};

/// One talent's volley transform, in counts small enough to keep programs small.
#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Step {
    Fan {
        #[generator(0u8..=3)]
        extra: u8,
    },
    Echo {
        #[generator(0u8..=3)]
        times: u8,
    },
}

/// Where one of the ability's own spawns sits in its program.
#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Placement {
    /// Straight in the cast.
    Top,
    /// Behind a wind-up delay.
    Delayed,
    /// Inside a burst the ability already loops.
    Looped {
        #[generator(1u8..=3)]
        count: u8,
    },
}

#[derive(Debug, Clone, TypeGenerator)]
struct Scenario {
    #[generator(bolero::produce::<Vec<Placement>>().with().len(0usize..=3))]
    spawns: Vec<Placement>,
    /// Whether the cast also deals a direct hit of its own, beside its bodies.
    direct_hit: bool,
    #[generator(bolero::produce::<Vec<Step>>().with().len(0usize..=4))]
    steps: Vec<Step>,
}

const POWER: f32 = 0.5;

fn hit(amount: f32) -> Impact {
    Impact::Damage {
        amount: Value::Const(amount),
        dtype: DamageTypeId(0),
        target: ImpactTarget::ResolvedTarget,
        flags: DamageFlags::default(),
    }
}

fn body() -> BodyDescriptor {
    BodyDescriptor {
        kind: BodyKind::Missile {
            speed: Value::Const(20.0),
            range: Value::Const(10.0),
            homing: false,
            pierce: Value::Const(0.0),
        },
        on_spawn: Vec::new(),
        on_hit: vec![hit(80.0)],
        on_expire: Vec::new(),
        collision: CollisionSpec {
            filter: TargetFilter::of(Affiliation::Enemies),
            pierce: Value::Const(0.0),
            through_walls: false,
        },
        flags: BodyFlags::default(),
        height: Value::Const(1.0),
    }
}

fn spawn() -> Impact {
    Impact::Spawn {
        body: body(),
        at: SpawnAnchor::Caster,
        count: Value::Const(1.0),
        pattern: SpawnPattern::Single,
    }
}

fn program(s: &Scenario) -> Vec<Impact> {
    let mut program: Vec<Impact> = s
        .spawns
        .iter()
        .map(|placement| match placement {
            Placement::Top => spawn(),
            Placement::Delayed => Impact::Delay { secs: Value::Const(0.2), inner: vec![spawn()] },
            Placement::Looped { count } => Impact::Loop {
                kind: LoopKind::Times {
                    count: Value::Const(f32::from(*count)),
                    gap: Value::Const(0.1),
                },
                inner: vec![spawn()],
            },
        })
        .collect();
    if s.direct_hit {
        program.push(hit(10.0));
    }
    program
}

fn mods(steps: &[Step]) -> Vec<VolleyMod> {
    steps
        .iter()
        .map(|step| match *step {
            Step::Fan { extra } => {
                VolleyMod::Fan { extra, angle: Value::Const(0.1), power: Value::Const(POWER) }
            }
            Step::Echo { times } => {
                VolleyMod::Echo { times, gap: Value::Const(0.25), power: Value::Const(POWER) }
            }
        })
        .collect()
}

fn constant(v: &Value) -> f32 {
    match v {
        Value::Const(c) => *c,
        other => panic!("a test count is a constant, got {other:?}"),
    }
}

/// How many times each kind of leaf fires over one cast: every loop multiplies by
/// its count, and a delay only postpones.
#[derive(Debug, Default, PartialEq)]
struct Fired {
    bodies: u32,
    hits: u32,
}

fn fired(program: &[Impact], times: u32, out: &mut Fired) {
    for impact in program {
        match impact {
            Impact::Spawn { .. } => out.bodies += times,
            Impact::Damage { .. } => out.hits += times,
            Impact::Delay { inner, .. } => fired(inner, times, out),
            Impact::Loop { kind: LoopKind::Times { count, .. }, inner } => {
                fired(inner, times * constant(count) as u32, out);
            }
            other => panic!("the transform produced an unexpected instruction: {other:?}"),
        }
    }
}

fn fire(program: &[Impact]) -> Fired {
    let mut out = Fired::default();
    fired(program, 1, &mut out);
    out
}

fn multiplier(steps: &[Step]) -> u32 {
    steps
        .iter()
        .map(|step| match *step {
            Step::Fan { extra } => u32::from(extra) + 1,
            Step::Echo { times } => u32::from(times) + 1,
        })
        .product()
}

#[test]
fn volley_transforms_multiply_in_any_order_and_repeat_only_the_volley() {
    check!().with_type::<Scenario>().for_each(|s| {
        let base = program(s);
        let before = fire(&base);

        let mut forward = base.clone();
        apply_volley(&mut forward, &mods(&s.steps));
        let after = fire(&forward);
        assert_eq!(after.bodies, before.bodies * multiplier(&s.steps), "bodies did not multiply");
        assert_eq!(after.hits, before.hits, "an effect that is not a body was repeated");

        let reversed: Vec<Step> = s.steps.iter().rev().copied().collect();
        let mut backward = base.clone();
        apply_volley(&mut backward, &mods(&reversed));
        assert_eq!(fire(&backward), after, "the same talents in another order fired differently");

        if s.steps
            .iter()
            .all(|step| matches!(step, Step::Fan { extra: 0 } | Step::Echo { times: 0 }))
        {
            assert_eq!(forward, base, "a transform that adds nothing changed the program");
        }
    });
}

#[test]
fn a_fan_copy_is_the_original_body_turned_and_scaled() {
    check!().with_type::<u8>().for_each(|extra| {
        let extra = extra % 5;
        let mut program = vec![spawn()];
        apply_volley(&mut program, &mods(&[Step::Fan { extra }]));
        assert_eq!(program.first(), Some(&spawn()), "the original body moved or changed");
        let mut turns = Vec::new();
        for copy in program.iter().skip(1) {
            let Impact::Spawn { body: copy_body, at, count, pattern } = copy else {
                panic!("a fan copy is not a spawn: {copy:?}");
            };
            let SpawnPattern::Turned { angle, inner } = pattern else {
                panic!("a fan copy flies the original's aim: {pattern:?}");
            };
            assert_eq!(**inner, SpawnPattern::Single, "the original's own pattern was lost");
            assert_eq!((at, count), (&SpawnAnchor::Caster, &Value::Const(1.0)));
            turns.push(eval_const(angle));
            let mut expected = body();
            expected.on_hit = vec![Impact::Damage {
                amount: Value::Bin(
                    BinOp::Mul,
                    Box::new(Value::Const(80.0)),
                    Box::new(Value::Const(POWER)),
                ),
                dtype: DamageTypeId(0),
                target: ImpactTarget::ResolvedTarget,
                flags: DamageFlags::default(),
            }];
            assert_eq!(copy_body, &expected, "a copy is not the original body at the fan's power");
        }
        assert_eq!(turns.len(), usize::from(extra), "a fan added the wrong number of copies");
        assert!(turns.iter().all(|t| t.abs() > 1e-6), "a copy flies on top of the original");
        for (i, a) in turns.iter().enumerate() {
            for b in turns.iter().skip(i + 1) {
                assert!((a - b).abs() > 1e-6, "two copies fly on top of each other: {turns:?}");
            }
        }
        // Balanced about the aim: an odd count can lean by one copy, never more.
        let lean: f32 = turns.iter().sum();
        let widest = turns.iter().fold(0.0_f32, |w, t| w.max(t.abs()));
        assert!(lean.abs() <= widest + 1e-5, "the copies all lean to one side: {turns:?}");
    });
}

/// A turn built from constants, folded.
fn eval_const(v: &Value) -> f32 {
    match v {
        Value::Const(c) => *c,
        Value::Bin(BinOp::Mul, a, b) => eval_const(a) * eval_const(b),
        other => panic!("a fan's turn is built from its own constants, got {other:?}"),
    }
}
