//! Balance sugar (stormlight/server#195) — authoring a number as a share of a
//! declared baseline curve instead of a literal. Laws, over generated baselines,
//! levels and shares:
//!   - **Proportional at every level**: a share of an axis evaluates to that
//!     fraction of the axis *at the same level*, so "80%" is still 80% when
//!     progression has moved the baseline. A literal would drift off the curve.
//!   - **Identity**: 100% is the baseline curve itself, structurally — not an
//!     expression that happens to evaluate to it.
//!   - **Monotone**: a larger share never yields less, at any level.
//!   - **Additive**: the shares of a split add up to the share of the whole.
//!   - **One health scale**: damage and healing read the health baseline, so a
//!     share of either is a number of hits against an average unit.
//!   - **No numbers of its own**: declaring registers exactly the curves the mod
//!     supplied, under stable names. The SDK never invents a baseline value.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::descriptors::{Curve, Registration};
use stormlight_mod_sdk::abi::ids::{BuffId, CurveId, ResourceId, Slot, StackId, StatId};
use stormlight_mod_sdk::abi::math::{Value, ValueCtx, Var, Who};
use stormlight_mod_sdk::balance::{Baseline, BaselineSpec, Share, flat, pct, share_of};
use stormlight_mod_sdk::context::ModContext;

/// A level curve: up to four points, inputs strictly increasing, outputs
/// non-negative (a baseline is an amount, never a debt).
#[derive(Debug, TypeGenerator)]
struct GenCurve {
    points: Vec<(u8, u16)>,
}

impl GenCurve {
    fn curve(&self) -> Curve {
        let mut points: Vec<[f32; 2]> =
            self.points.iter().take(4).map(|&(x, y)| [f32::from(x % 30), f32::from(y)]).collect();
        points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        points.dedup_by(|a, b| a[0] == b[0]);
        Curve { points }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    health: GenCurve,
    move_speed: GenCurve,
    cooldown: GenCurve,
    /// Tenths of a level, so evaluation also lands between the curve's points.
    level_tenths: u16,
    /// Two shares in hundredths of a percent, 0–500%.
    a: u16,
    b: u16,
}

impl Scenario {
    fn spec(&self) -> BaselineSpec {
        BaselineSpec {
            health: self.health.curve(),
            move_speed: self.move_speed.curve(),
            cooldown: self.cooldown.curve(),
        }
    }
    fn level(&self) -> f32 {
        f32::from(self.level_tenths % 400) / 10.0
    }
    fn share(raw: u16) -> Share {
        pct(f64::from(raw % 50_001) / 100.0)
    }
}

/// Declare the scenario's baseline in a fresh mod and return it with the
/// registration the host would receive.
fn declared(s: &Scenario) -> (Baseline, Registration) {
    let mut ctx = ModContext::new();
    let base = Baseline::declare(&mut ctx, s.spec());
    (base, ctx.finish())
}

/// Evaluates a `Value` at one level against the registration's curves, sampling
/// them the way the engine does (clamp to the ends, linear inside).
struct LevelCtx<'a> {
    level: f32,
    curves: &'a [Curve],
}

fn sample(curve: &Curve, x: f32) -> f32 {
    let pts = &curve.points;
    let Some(&[x0, y0]) = pts.first() else { return 0.0 };
    if x <= x0 {
        return y0;
    }
    let [xn, yn] = pts[pts.len() - 1];
    if x >= xn {
        return yn;
    }
    for w in pts.windows(2) {
        let ([xa, ya], [xb, yb]) = (w[0], w[1]);
        if x >= xa && x <= xb {
            return ya + (x - xa) / (xb - xa) * (yb - ya);
        }
    }
    yn
}

impl ValueCtx for LevelCtx<'_> {
    fn level(&self) -> f32 {
        self.level
    }
    fn curve(&self, curve: CurveId, x: f32) -> f32 {
        self.curves.get(curve.0 as usize).map_or(0.0, |c| sample(c, x))
    }
    fn scale(&self) -> f32 {
        1.0
    }
    // Nothing below is read by a baseline share.
    fn max_hp(&self, _: Who) -> f32 {
        0.0
    }
    fn cur_hp(&self, _: Who) -> f32 {
        0.0
    }
    fn stat(&self, _: StatId, _: Who) -> f32 {
        0.0
    }
    fn resource(&self, _: ResourceId, _: Who) -> f32 {
        0.0
    }
    fn stack_count(&self, _: StackId, _: Who) -> f32 {
        0.0
    }
    fn stack_gain(&self, _: StackId, _: Who) -> f32 {
        0.0
    }
    fn buff_stacks(&self, _: BuffId, _: Who) -> f32 {
        0.0
    }
    fn buff_stacks_from(&self, _: BuffId, _: Who, _: Who) -> f32 {
        0.0
    }
    fn charges_of(&self, _: Slot, _: Who) -> f32 {
        0.0
    }
    fn cooldown_of(&self, _: Slot, _: Who) -> f32 {
        0.0
    }
    fn source_slot(&self) -> Option<Slot> {
        None
    }
    fn ally_count(&self) -> f32 {
        0.0
    }
    fn enemy_count(&self) -> f32 {
        0.0
    }
    fn distance_to_target(&self) -> f32 {
        0.0
    }
    fn channel_progress(&self) -> f32 {
        0.0
    }
    fn rand01(&self) -> f32 {
        0.0
    }
    fn event_magnitude(&self) -> f32 {
        0.0
    }
    fn loop_index(&self) -> f32 {
        0.0
    }
}

/// Equal up to f32 rounding, relative to the magnitudes involved.
fn close(x: f32, y: f32) -> bool {
    (x - y).abs() <= 1e-4 * x.abs().max(y.abs()).max(1.0)
}

/// Every axis a `Baseline` offers, by name.
type Axis = fn(&Baseline, Share) -> Value;
const AXES: [(&str, Axis); 3] = [
    ("health", Baseline::health),
    ("move_speed", Baseline::move_speed),
    ("cooldown", Baseline::cooldown),
];

#[test]
fn a_share_is_the_same_fraction_of_its_baseline_at_every_level() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (base, reg) = declared(s);
        let ctx = LevelCtx { level: s.level(), curves: &reg.curves };
        let share = Scenario::share(s.a);
        for (axis, of) in AXES {
            let whole = of(&base, pct(100)).eval(&ctx);
            let part = of(&base, share).eval(&ctx);
            assert!(
                close(part, share.fraction() * whole),
                "{axis}: {} of {whole} evaluated to {part}",
                share.fraction()
            );
        }
    });
}

#[test]
fn one_hundred_percent_is_the_baseline_curve_itself() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (base, reg) = declared(s);
        for (axis, of) in AXES {
            let index = reg
                .names
                .curves
                .iter()
                .position(|n| *n == format!("baseline.{axis}"))
                .unwrap_or_else(|| panic!("no curve named baseline.{axis}"));
            let curve = CurveId(u16::try_from(index).unwrap());
            assert_eq!(of(&base, pct(100)), Value::Curve(curve, Box::new(Value::Read(Var::Level))));
        }
    });
}

#[test]
fn a_larger_share_never_yields_less() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (base, reg) = declared(s);
        let ctx = LevelCtx { level: s.level(), curves: &reg.curves };
        let (a, b) = (Scenario::share(s.a), Scenario::share(s.b));
        let (lo, hi) = if a.fraction() <= b.fraction() { (a, b) } else { (b, a) };
        for (axis, of) in AXES {
            let (small, large) = (of(&base, lo).eval(&ctx), of(&base, hi).eval(&ctx));
            assert!(small <= large || close(small, large), "{axis}: {small} > {large}");
        }
    });
}

#[test]
fn the_shares_of_a_split_add_up_to_the_whole() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (base, reg) = declared(s);
        let ctx = LevelCtx { level: s.level(), curves: &reg.curves };
        let (a, b) = (Scenario::share(s.a), Scenario::share(s.b));
        let sum = pct(f64::from(a.fraction() + b.fraction()) * 100.0);
        for (axis, of) in AXES {
            let parts = of(&base, a).eval(&ctx) + of(&base, b).eval(&ctx);
            let whole = of(&base, sum).eval(&ctx);
            assert!(close(parts, whole), "{axis}: {parts} != {whole}");
        }
    });
}

#[test]
fn damage_and_healing_are_measured_against_baseline_health() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (base, reg) = declared(s);
        let ctx = LevelCtx { level: s.level(), curves: &reg.curves };
        let share = Scenario::share(s.a);
        let health = base.health(share).eval(&ctx);
        assert!(close(base.damage(share).eval(&ctx), health));
        assert!(close(base.heal(share).eval(&ctx), health));
    });
}

#[test]
fn declaring_registers_exactly_the_mods_own_curves() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (_, reg) = declared(s);
        let spec = s.spec();
        let expected = [
            ("baseline.health", spec.health),
            ("baseline.move_speed", spec.move_speed),
            ("baseline.cooldown", spec.cooldown),
        ];
        assert_eq!(reg.curves.len(), expected.len(), "the SDK registered a curve of its own");
        for (name, curve) in expected {
            let index = reg.names.curves.iter().position(|n| n == name).expect(name);
            assert_eq!(reg.curves[index], curve, "{name} is not the curve the mod declared");
        }
    });
}

#[derive(Debug, TypeGenerator)]
struct OfScenario {
    whole: u16,
    share: u16,
    y: u16,
}

#[test]
fn a_share_of_any_value_is_that_fraction_of_it() {
    check!().with_type::<OfScenario>().for_each(|s| {
        let ctx = LevelCtx { level: 1.0, curves: &[] };
        let whole = Value::Const(f32::from(s.whole));
        let share = Scenario::share(s.share);
        let part = share_of(share, whole.clone()).eval(&ctx);
        assert!(close(part, share.fraction() * whole.eval(&ctx)));
        // A flat curve is the same number at every level.
        let flat_curve = flat(f32::from(s.y));
        assert!(close(sample(&flat_curve, 0.0), sample(&flat_curve, 25.0)));
    });
}

#[test]
fn whole_and_fractional_percentages_mean_the_same_share() {
    check!().with_type::<u16>().for_each(|&n| {
        let n = i32::from(n % 1000);
        assert_eq!(pct(n).fraction(), pct(f64::from(n)).fraction());
        assert!(close(pct(n).fraction() * 100.0, n as f32));
    });
}
