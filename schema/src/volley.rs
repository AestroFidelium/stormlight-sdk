//! Volley transforms — talents that multiply an ability's **own** projectiles
//! (stormlight/server#210).
//!
//! A talent that adds a projectile by spawning one of its own is welded to the
//! ability it was written for: on another ability it fires the wrong missile, and
//! two such talents only combine if one of them checks for the other. A volley
//! transform instead rewrites whatever the selected ability's cast spawns, so it
//! means the same thing on every ability and every unit, and any number of them
//! compose without knowing about each other:
//!
//! - a [`VolleyMod::Fan`] joins every body by copies turned off its own aim;
//! - a [`VolleyMod::Echo`] fires the whole volley again, later.
//!
//! Each multiplies what the cast already fires, so a fan of `n` and an echo of `m`
//! fire `(n + 1)(m + 1)` times the bodies in either order. The engine applies them
//! after the cast's own riders (a body an `OnCast` rider adds is part of the
//! volley) and before its payload riders (an `OnHit` or `OnSpawn` rider rides
//! every copy).

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::impacts::{Impact, LoopKind, SpawnPattern};
use crate::math::{BinOp, Value};
use crate::missiles::BodyDescriptor;

/// One talent's rewrite of the volley an ability fires.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum VolleyMod {
    /// Every body the cast spawns is joined by `extra` copies, turned off its own
    /// aim by `angle` radians a step on alternating sides — `+angle`, `-angle`,
    /// `+2·angle`, … — so the spread stays centred on what the player aimed at.
    Fan {
        extra: u8,
        angle: Value,
        /// The copies' damage and healing, as a fraction of the original's.
        power: Value,
    },
    /// The cast's volley — every body it spawns, with the delays and loops that
    /// shape it, and nothing else — fires `times` more times, `gap` seconds apart,
    /// the first `gap` after the cast.
    Echo {
        times: u8,
        gap: Value,
        /// The repeats' damage and healing, as a fraction of the original's.
        power: Value,
    },
}

/// Apply `mods` to the cast `program`, in order. Every transform multiplies what
/// the program already fires, so the order changes nothing about how many bodies
/// fly; a transform that adds nothing (`extra` or `times` of zero, or a program
/// that spawns nothing) leaves the program untouched.
pub fn apply_volley(program: &mut Vec<Impact>, mods: &[VolleyMod]) {
    for volley in mods {
        match volley {
            VolleyMod::Fan { extra, angle, power } => fan(program, *extra, angle, power),
            VolleyMod::Echo { times, gap, power } => echo(program, *times, gap, power),
        }
    }
}

/// Join every spawn the program reaches by its fanned copies, in place.
fn fan(program: &mut Vec<Impact>, extra: u8, angle: &Value, power: &Value) {
    if extra == 0 {
        return;
    }
    let mut out = Vec::with_capacity(program.len());
    for mut impact in program.drain(..) {
        let copies = match &mut impact {
            Impact::Spawn { body, at, count, pattern } => (1..=extra)
                .map(|copy| Impact::Spawn {
                    body: scaled(body, power),
                    at: at.clone(),
                    count: count.clone(),
                    pattern: SpawnPattern::Turned {
                        angle: Value::Bin(
                            BinOp::Mul,
                            Box::new(Value::Const(turn_step(copy))),
                            Box::new(angle.clone()),
                        ),
                        inner: Box::new(pattern.clone()),
                    },
                })
                .collect(),
            other => {
                for inner in sub_programs(other) {
                    fan(inner, extra, angle, power);
                }
                Vec::new()
            }
        };
        out.push(impact);
        out.extend(copies);
    }
    *program = out;
}

/// Where copy `copy` (from one) sits, in steps off the aim: `+1, -1, +2, -2, …`.
fn turn_step(copy: u8) -> f32 {
    let rank = f32::from(copy.div_ceil(2));
    if copy % 2 == 1 { rank } else { -rank }
}

/// Append the program's volley, repeated `times` more times `gap` apart.
fn echo(program: &mut Vec<Impact>, times: u8, gap: &Value, power: &Value) {
    if times == 0 {
        return;
    }
    let volley = volley_of(program, power);
    if volley.is_empty() {
        return;
    }
    program.push(Impact::Delay {
        secs: gap.clone(),
        inner: alloc::vec![Impact::Loop {
            kind: LoopKind::Times { count: Value::Const(f32::from(times)), gap: gap.clone() },
            inner: volley,
        }],
    });
}

/// The bodies `program` spawns, with the timing that shapes them and nothing else:
/// every other effect a cast has is the cast's, not its volley's, and an echo that
/// repeated a refund or a direct hit would be a different talent.
fn volley_of(program: &[Impact], power: &Value) -> Vec<Impact> {
    program
        .iter()
        .filter_map(|impact| match impact {
            Impact::Spawn { body, at, count, pattern } => Some(Impact::Spawn {
                body: scaled(body, power),
                at: at.clone(),
                count: count.clone(),
                pattern: pattern.clone(),
            }),
            Impact::Delay { secs, inner } => {
                let inner = volley_of(inner, power);
                (!inner.is_empty()).then(|| Impact::Delay { secs: secs.clone(), inner })
            }
            Impact::Loop { kind, inner } => {
                let inner = volley_of(inner, power);
                (!inner.is_empty()).then(|| Impact::Loop { kind: kind.clone(), inner })
            }
            Impact::If { cond, then, els } => {
                let (then, els) = (volley_of(then, power), volley_of(els, power));
                (!then.is_empty() || !els.is_empty()).then(|| Impact::If {
                    cond: cond.clone(),
                    then,
                    els,
                })
            }
            Impact::Retarget { shape, filter, max_targets, exclude_primary, inner } => {
                let inner = volley_of(inner, power);
                (!inner.is_empty()).then(|| Impact::Retarget {
                    shape: shape.clone(),
                    filter: filter.clone(),
                    max_targets: max_targets.clone(),
                    exclude_primary: *exclude_primary,
                    inner,
                })
            }
            // A dash is the caster's movement, not a volley, and what it spawns on
            // collision belongs to it.
            _ => None,
        })
        .collect()
}

/// The programs a combinator holds, which a fan descends into. Not a body's own
/// payload: what a projectile spawns when it lands is its effect, not the
/// ability's volley.
fn sub_programs(impact: &mut Impact) -> Vec<&mut Vec<Impact>> {
    match impact {
        Impact::Retarget { inner, .. }
        | Impact::Loop { inner, .. }
        | Impact::Delay { inner, .. } => {
            alloc::vec![inner]
        }
        Impact::If { then, els, .. } => alloc::vec![then, els],
        Impact::Dash { motion, .. } => alloc::vec![&mut motion.on_collision, &mut motion.on_end],
        _ => Vec::new(),
    }
}

/// `body` with every damage and heal in its payload scaled by `power`. A power of
/// exactly one is the original, untouched.
fn scaled(body: &BodyDescriptor, power: &Value) -> BodyDescriptor {
    let mut body = body.clone();
    if *power != Value::Const(1.0) {
        for program in [&mut body.on_spawn, &mut body.on_hit, &mut body.on_expire] {
            scale_program(program, power);
        }
    }
    body
}

fn scale_program(program: &mut [Impact], power: &Value) {
    for impact in program {
        match impact {
            Impact::Damage { amount, .. } | Impact::Heal { amount, .. } => {
                let base = core::mem::replace(amount, Value::Const(0.0));
                *amount = Value::Bin(BinOp::Mul, Box::new(base), Box::new(power.clone()));
            }
            Impact::Spawn { body, .. } => {
                for program in [&mut body.on_spawn, &mut body.on_hit, &mut body.on_expire] {
                    scale_program(program, power);
                }
            }
            other => {
                for inner in sub_programs(other) {
                    scale_program(inner, power);
                }
            }
        }
    }
}
