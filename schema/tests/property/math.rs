//! Invariants of the `Value` expression language — the anti-growth lever that
//! keeps "% of max hp", "scaling per stack", "% chance" as *formulas* instead
//! of new ISA primitives. Directional/structural only:
//!   - **Totality**: evaluating any generated tree never panics.
//!   - **Determinism**: same tree + same ctx -> bit-identical result (a hard
//!     requirement for replay).
//!   - Leaf laws: `Const` is identity, `ScaleCtx` reads the ambient scale,
//!     `Div`-by-zero is defined (0), `Clamp`/`Min`/`Max` are ordered.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{BuffId, CurveId, ResourceId, Slot, StackId, StatId};
use stormlight_mod_abi::math::{BinOp, Origin, Value, ValueCtx, Var, Who};

fn frac(seed: u16) -> f32 {
    f32::from(seed) / f32::from(u16::MAX) // [0, 1]
}

/// A finite mock context: every read returns a bounded finite number derived
/// from seeds, so any non-finite result is the evaluator's fault, not the ctx.
struct MockCtx {
    scale: f32,
    base: f32,
    rand: f32,
}

impl MockCtx {
    fn from_seeds(scale: u16, base: u16, rand: u16) -> Self {
        Self {
            scale: frac(scale) * 4.0,       // [0, 4]
            base: frac(base) * 100.0 + 1.0, // [1, 101], nonzero for ratios
            rand: frac(rand),               // [0, 1]
        }
    }
    /// A deterministic finite value that varies a little per id/who so distinct
    /// reads are distinguishable but everything stays bounded.
    fn read(&self, salt: u32) -> f32 {
        self.base + f32::from((salt % 37) as u16) * 0.5
    }
}

impl ValueCtx for MockCtx {
    fn level(&self) -> f32 {
        self.read(1)
    }
    fn max_hp(&self, who: Who) -> f32 {
        self.read(10 + who as u32)
    }
    fn cur_hp(&self, who: Who) -> f32 {
        // Strictly <= max_hp so ratios land in [0, 1].
        0.5 * self.read(10 + who as u32)
    }
    fn stat(&self, s: StatId, who: Who) -> f32 {
        self.read(20 + u32::from(s.0) + who as u32)
    }
    fn resource(&self, r: ResourceId, who: Who) -> f32 {
        self.read(30 + u32::from(r.0) + who as u32)
    }
    fn stack_count(&self, s: StackId, who: Who) -> f32 {
        self.read(40 + u32::from(s.0) + who as u32)
    }
    fn stack_gain(&self, s: StackId, who: Who) -> f32 {
        self.read(45 + u32::from(s.0) + who as u32)
    }
    fn buff_stacks(&self, b: BuffId, who: Who) -> f32 {
        self.read(50 + u32::from(b.0) + who as u32)
    }
    fn buff_stacks_from(&self, b: BuffId, who: Who, from: Who) -> f32 {
        self.read(55 + u32::from(b.0) + who as u32 + from as u32)
    }
    fn event_magnitude(&self) -> f32 {
        self.read(83)
    }
    fn loop_index(&self) -> f32 {
        self.read(84)
    }
    fn charges_of(&self, sl: Slot, who: Who) -> f32 {
        self.read(60 + u32::from(sl.0) + who as u32)
    }
    fn cooldown_of(&self, sl: Slot, who: Who) -> f32 {
        self.read(70 + u32::from(sl.0) + who as u32)
    }
    fn ally_count(&self) -> f32 {
        self.read(80)
    }
    fn enemy_count(&self) -> f32 {
        self.read(81)
    }
    fn distance_to_target(&self) -> f32 {
        self.read(82)
    }
    fn channel_progress(&self) -> f32 {
        self.rand // already [0, 1]
    }
    fn rand01(&self) -> f32 {
        self.rand
    }
    fn scale(&self) -> f32 {
        self.scale
    }
    fn curve(&self, _c: CurveId, x: f32) -> f32 {
        x * 0.5 // deterministic, monotone
    }
}

/// Interprets a flat seed stream into a bounded `Value` tree (the recursive
/// analog of `build()` in the other property tests).
struct Builder<'a> {
    ops: &'a [u16],
    pos: usize,
}

impl Builder<'_> {
    fn next(&mut self) -> u16 {
        let v = self.ops.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        v
    }
    fn leaf_const(&mut self) -> f32 {
        frac(self.next()) * 200.0 - 100.0 // [-100, 100], always finite
    }
    fn binop(&mut self) -> BinOp {
        match self.next() % 6 {
            0 => BinOp::Add,
            1 => BinOp::Sub,
            2 => BinOp::Mul,
            3 => BinOp::Div,
            4 => BinOp::Min,
            _ => BinOp::Max,
        }
    }
    fn who(&mut self) -> Who {
        match self.next() % 3 {
            0 => Who::Caster,
            1 => Who::Target,
            _ => Who::Source,
        }
    }
    fn origin(&mut self) -> Origin {
        match self.next() % 4 {
            0 => Origin::Anyone,
            _ => Origin::By(self.who()),
        }
    }
    fn var(&mut self) -> Var {
        let w = self.who();
        match self.next() % 20 {
            0 => Var::Level,
            1 => Var::MaxHp(w),
            2 => Var::CurHp(w),
            3 => Var::MissingHp(w),
            4 => Var::HpRatio(w),
            5 => Var::MissingHpRatio(w),
            6 => Var::Stat(StatId(self.next()), w),
            7 => Var::Resource(ResourceId(self.next()), w),
            8 => Var::StackCount(StackId(self.next()), w),
            9 => Var::BuffStacks(BuffId(self.next()), w),
            10 => Var::ChargesOf(Slot(self.next() as u8), w),
            11 => Var::CooldownOf(Slot(self.next() as u8), w),
            12 => Var::AllyCount,
            13 => Var::EnemyCount,
            14 => Var::DistanceToTarget,
            15 => Var::ChannelProgress,
            16 => Var::StackGain(StackId(self.next()), w),
            17 => Var::BuffStacksFrom(BuffId(self.next()), w, self.origin()),
            18 => Var::EventMagnitude,
            19 => Var::LoopIndex,
            _ => Var::Rand01,
        }
    }
    fn value(&mut self, depth: u8) -> Value {
        if depth == 0 {
            return Value::Const(self.leaf_const());
        }
        match self.next() % 6 {
            0 => Value::Const(self.leaf_const()),
            1 => Value::Read(self.var()),
            2 => Value::Bin(
                self.binop(),
                Box::new(self.value(depth - 1)),
                Box::new(self.value(depth - 1)),
            ),
            3 => Value::Clamp {
                v: Box::new(self.value(depth - 1)),
                lo: Box::new(self.value(depth - 1)),
                hi: Box::new(self.value(depth - 1)),
            },
            4 => Value::Curve(CurveId(self.next()), Box::new(self.value(depth - 1))),
            _ => Value::ScaleCtx,
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    ops: Vec<u16>,
    scale: u16,
    base: u16,
    rand: u16,
}

fn build(s: &Scenario) -> (Value, MockCtx) {
    let mut b = Builder { ops: &s.ops, pos: 0 };
    let value = b.value(6); // depth cap keeps trees bounded
    (value, MockCtx::from_seeds(s.scale, s.base, s.rand))
}

#[test]
fn eval_is_total_and_deterministic() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (value, ctx) = build(s);
        // Totality: this line panicking is the failure. Determinism: two evals
        // of the same tree against the same ctx must be bit-identical.
        let a = value.eval(&ctx);
        let b = value.eval(&ctx);
        assert_eq!(a.to_bits(), b.to_bits(), "eval is non-deterministic");
    });
}

#[test]
fn const_is_identity() {
    check!().with_type::<u16>().for_each(|&seed| {
        let ctx = MockCtx::from_seeds(0, 0, 0);
        let x = frac(seed) * 200.0 - 100.0;
        assert_eq!(Value::Const(x).eval(&ctx).to_bits(), x.to_bits());
    });
}

#[test]
fn scalectx_reads_ambient_scale() {
    check!().with_type::<u16>().for_each(|&seed| {
        let ctx = MockCtx::from_seeds(seed, 0, 0);
        assert_eq!(Value::ScaleCtx.eval(&ctx).to_bits(), ctx.scale().to_bits());
    });
}

#[test]
fn div_by_zero_is_defined() {
    let ctx = MockCtx::from_seeds(0, 0, 0);
    check!().with_type::<u16>().for_each(|&seed| {
        let num = frac(seed) * 100.0;
        let v = Value::Bin(BinOp::Div, Box::new(Value::Const(num)), Box::new(Value::Const(0.0)));
        // Defined, not NaN/inf: division by zero yields 0.
        assert_eq!(v.eval(&ctx), 0.0);
    });
}

#[test]
fn clamp_result_lands_between_its_bounds() {
    let ctx = MockCtx::from_seeds(0, 0, 0);
    check!().with_type::<(u16, u16, u16)>().for_each(|&(vs, los, his)| {
        let (v, lo, hi) =
            (frac(vs) * 200.0 - 100.0, frac(los) * 200.0 - 100.0, frac(his) * 200.0 - 100.0);
        let out = Value::Clamp {
            v: Box::new(Value::Const(v)),
            lo: Box::new(Value::Const(lo)),
            hi: Box::new(Value::Const(hi)),
        }
        .eval(&ctx);
        // Total even when lo > hi: the result never leaves [min(lo,hi), max(lo,hi)].
        let (lo_b, hi_b) = (lo.min(hi), lo.max(hi));
        assert!(out >= lo_b && out <= hi_b, "clamp {out} escaped [{lo_b}, {hi_b}]");
    });
}

#[test]
fn min_max_are_ordered() {
    let ctx = MockCtx::from_seeds(0, 0, 0);
    check!().with_type::<(u16, u16)>().for_each(|&(a, b)| {
        let (x, y) = (frac(a) * 200.0 - 100.0, frac(b) * 200.0 - 100.0);
        let mk =
            |op| Value::Bin(op, Box::new(Value::Const(x)), Box::new(Value::Const(y))).eval(&ctx);
        let lo = mk(BinOp::Min);
        let hi = mk(BinOp::Max);
        assert!(lo <= x && lo <= y, "min {lo} exceeded an operand");
        assert!(hi >= x && hi >= y, "max {hi} below an operand");
        assert!(lo <= hi, "min {lo} > max {hi}");
    });
}
