//! Invariants of stat aggregation — the ordered fold that turns a bag of
//! modifiers from many sources into one final stat. The load-bearing law is
//! **order-stability**: the result must not depend on the order sources are
//! collected (a replay/determinism requirement), so a permutation of the same
//! multiset yields a bit-identical stat.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::behaviors::{ModOp, ResolvedModifier, aggregate_stat};

#[derive(Debug, TypeGenerator, Clone)]
struct ModSeed {
    kind: u8,
    value: u16,
    priority: u8,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    base: u16,
    mods: Vec<ModSeed>,
    rotate: u8,
}

fn frac(seed: u16) -> f32 {
    f32::from(seed) / f32::from(u16::MAX)
}

fn to_mod(s: &ModSeed) -> ResolvedModifier {
    let (op, value) = match s.kind % 4 {
        0 => (ModOp::AddFlat, frac(s.value) * 100.0 - 50.0), // [-50, 50]
        1 => (ModOp::AddPct, frac(s.value) * 2.0 - 1.0),     // [-1, 1]
        2 => (ModOp::Mul, frac(s.value) * 2.0),              // [0, 2]
        _ => (ModOp::Override, frac(s.value) * 100.0),       // [0, 100]
    };
    ResolvedModifier { op, value, priority: i32::from(s.priority) }
}

fn build(s: &Scenario) -> (f32, Vec<ResolvedModifier>) {
    let base = frac(s.base) * 100.0; // [0, 100]
    (base, s.mods.iter().map(to_mod).collect())
}

#[test]
fn aggregation_is_order_stable() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (base, mods) = build(s);
        let expected = aggregate_stat(base, &mods);

        // Reversed order -> identical result.
        let mut rev = mods.clone();
        rev.reverse();
        assert_eq!(
            aggregate_stat(base, &rev).to_bits(),
            expected.to_bits(),
            "aggregation changed under reversal"
        );

        // Arbitrary rotation -> identical result.
        if !mods.is_empty() {
            let k = usize::from(s.rotate) % mods.len();
            let mut rot = mods.clone();
            rot.rotate_left(k);
            assert_eq!(
                aggregate_stat(base, &rot).to_bits(),
                expected.to_bits(),
                "aggregation changed under rotation"
            );
        }
    });
}

#[test]
fn empty_is_identity() {
    check!().with_type::<u16>().for_each(|&b| {
        let base = frac(b) * 100.0;
        assert_eq!(aggregate_stat(base, &[]).to_bits(), base.to_bits());
    });
}

#[test]
fn neutral_modifiers_do_not_change_the_stat() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (base, mods) = build(s);
        // Skip Override cases: an Override replaces the whole stat by design, so
        // "append a neutral op" is only meaningful without one.
        if mods.iter().any(|m| m.op == ModOp::Override) {
            return;
        }
        let expected = aggregate_stat(base, &mods);
        let mut padded = mods.clone();
        padded.push(ResolvedModifier { op: ModOp::AddFlat, value: 0.0, priority: 0 });
        padded.push(ResolvedModifier { op: ModOp::AddPct, value: 0.0, priority: 0 });
        padded.push(ResolvedModifier { op: ModOp::Mul, value: 1.0, priority: 0 });
        assert_eq!(
            aggregate_stat(base, &padded).to_bits(),
            expected.to_bits(),
            "neutral modifiers perturbed the stat"
        );
    });
}

#[test]
fn override_dominates_and_takes_highest_priority() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (base, mut mods) = build(s);
        // Force two overrides with priorities above any generated one (seeds are
        // u8, ≤ 255); the higher must win regardless of any flat/pct/mul present.
        mods.push(ResolvedModifier { op: ModOp::Override, value: 7.0, priority: 1000 });
        mods.push(ResolvedModifier { op: ModOp::Override, value: 42.0, priority: 9000 });
        assert_eq!(aggregate_stat(base, &mods), 42.0, "override did not dominate");
    });
}

#[test]
fn positive_flat_only_never_decreases_base() {
    check!().with_type::<(u16, Vec<u16>)>().for_each(|(b, flats)| {
        let base = frac(*b) * 100.0;
        let mods: Vec<ResolvedModifier> = flats
            .iter()
            .map(|&v| ResolvedModifier { op: ModOp::AddFlat, value: frac(v) * 50.0, priority: 0 })
            .collect();
        assert!(aggregate_stat(base, &mods) >= base - 1e-3, "positive flats decreased the stat");
    });
}
