//! Invariants of `Condition` — the predicate layer over `Value` + the same ctx.
//! Boolean-algebra laws must hold structurally so mods can compose gates freely:
//!   - **Totality/determinism**: any tree evaluates to a stable bool, no panic.
//!   - **De Morgan** and **double negation** (proves the algebra is consistent).
//!   - **Comparison swap duality** (`Lt a b == Gt b a`, `Le a b == Ge b a`).

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::conditions::{CmpOp, Condition, ConditionCtx};
use stormlight_mod_abi::ids::{
    BuffId, CurveId, ResourceId, Slot, StackId, StatId, TagId, TalentId,
};
use stormlight_mod_abi::math::{Value, ValueCtx, Who};

/// Minimal ctx: numeric reads are constant (conditions here compare literals),
/// predicates are seeded bitmasks so membership varies across generated cases.
struct Ctx {
    tags: u64,
    buffs: u64,
    talents: u64,
}

fn bit(mask: u64, i: u16) -> bool {
    (mask >> (u32::from(i) % 64)) & 1 == 1
}

impl ValueCtx for Ctx {
    fn level(&self) -> f32 {
        1.0
    }
    fn max_hp(&self, _: Who) -> f32 {
        1.0
    }
    fn cur_hp(&self, _: Who) -> f32 {
        1.0
    }
    fn stat(&self, _: StatId, _: Who) -> f32 {
        1.0
    }
    fn resource(&self, _: ResourceId, _: Who) -> f32 {
        1.0
    }
    fn stack_count(&self, _: StackId, _: Who) -> f32 {
        1.0
    }
    fn stack_gain(&self, _: StackId, _: Who) -> f32 {
        1.0
    }
    fn buff_stacks(&self, _: BuffId, _: Who) -> f32 {
        1.0
    }
    fn buff_stacks_from(&self, _: BuffId, _: Who, _: Who) -> f32 {
        1.0
    }
    fn event_magnitude(&self) -> f32 {
        1.0
    }
    fn loop_index(&self) -> f32 {
        0.0
    }
    fn charges_of(&self, _: Slot, _: Who) -> f32 {
        1.0
    }
    fn cooldown_of(&self, _: Slot, _: Who) -> f32 {
        1.0
    }
    fn source_slot(&self) -> Option<Slot> {
        None
    }
    fn ally_count(&self) -> f32 {
        1.0
    }
    fn enemy_count(&self) -> f32 {
        1.0
    }
    fn distance_to_target(&self) -> f32 {
        1.0
    }
    fn channel_progress(&self) -> f32 {
        0.5
    }
    fn rand01(&self) -> f32 {
        0.5
    }
    fn scale(&self) -> f32 {
        1.0
    }
    fn curve(&self, _: CurveId, x: f32) -> f32 {
        x
    }
}

impl ConditionCtx for Ctx {
    fn has_tag(&self, tag: TagId, _: Who) -> bool {
        bit(self.tags, tag.0)
    }
    fn has_buff(&self, buff: BuffId, _: Who) -> bool {
        bit(self.buffs, buff.0)
    }
    fn has_buff_from(&self, buff: BuffId, _: Who, from: Who) -> bool {
        // Sourced membership varies with the origin, so a gate that dropped its
        // origin would answer the unsourced question here and be spotted.
        bit(self.buffs, buff.0.wrapping_add(from as u16))
    }
    fn has_talent(&self, talent: TalentId) -> bool {
        bit(self.talents, talent.0 as u16)
    }
}

/// Seed-driven builder for bounded `Condition` trees.
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
    fn cmp_op(&mut self) -> CmpOp {
        match self.next() % 5 {
            0 => CmpOp::Lt,
            1 => CmpOp::Le,
            2 => CmpOp::Eq,
            3 => CmpOp::Ge,
            _ => CmpOp::Gt,
        }
    }
    fn konst(&mut self) -> Value {
        Value::Const(f32::from(self.next()) / f32::from(u16::MAX) * 10.0)
    }
    fn cond(&mut self, depth: u8) -> Condition {
        if depth == 0 {
            return self.leaf();
        }
        match self.next() % 8 {
            0 | 1 => self.leaf(),
            2 => {
                let n = (self.next() % 4) as usize;
                Condition::And((0..n).map(|_| self.cond(depth - 1)).collect())
            }
            3 => {
                let n = (self.next() % 4) as usize;
                Condition::Or((0..n).map(|_| self.cond(depth - 1)).collect())
            }
            4 => Condition::Not(Box::new(self.cond(depth - 1))),
            _ => self.leaf(),
        }
    }
    fn leaf(&mut self) -> Condition {
        match self.next() % 5 {
            0 => Condition::Always,
            1 => Condition::Cmp(self.cmp_op(), self.konst(), self.konst()),
            2 => Condition::HasTag(TagId(self.next()), Who::Target),
            3 => Condition::HasBuff(BuffId(self.next()), Who::Caster),
            _ => Condition::HasTalent(TalentId(u32::from(self.next()))),
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    ops: Vec<u16>,
    tags: u64,
    buffs: u64,
    talents: u64,
}

fn build(s: &Scenario) -> (Vec<Condition>, Ctx) {
    let mut b = Builder { ops: &s.ops, pos: 0 };
    // A handful of independent sub-conditions, reused across the algebra laws.
    let conds = (0..4).map(|_| b.cond(3)).collect();
    (conds, Ctx { tags: s.tags, buffs: s.buffs, talents: s.talents })
}

#[test]
fn eval_is_total_and_deterministic() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (conds, ctx) = build(s);
        for c in &conds {
            assert_eq!(c.eval(&ctx), c.eval(&ctx), "condition eval non-deterministic");
        }
    });
}

#[test]
fn de_morgan_holds() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (conds, ctx) = build(s);
        let not_and = Condition::Not(Box::new(Condition::And(conds.clone())));
        let or_nots =
            Condition::Or(conds.iter().cloned().map(|c| Condition::Not(Box::new(c))).collect());
        assert_eq!(not_and.eval(&ctx), or_nots.eval(&ctx), "¬(⋀) ≠ ⋁¬");

        let not_or = Condition::Not(Box::new(Condition::Or(conds.clone())));
        let and_nots =
            Condition::And(conds.iter().cloned().map(|c| Condition::Not(Box::new(c))).collect());
        assert_eq!(not_or.eval(&ctx), and_nots.eval(&ctx), "¬(⋁) ≠ ⋀¬");
    });
}

#[test]
fn double_negation_is_identity() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (conds, ctx) = build(s);
        for c in &conds {
            let dn = Condition::Not(Box::new(Condition::Not(Box::new(c.clone()))));
            assert_eq!(dn.eval(&ctx), c.eval(&ctx), "¬¬c ≠ c");
        }
    });
}

#[test]
fn comparison_swap_is_dual() {
    check!().with_type::<(u16, u16)>().for_each(|&(a, b)| {
        let ctx = Ctx { tags: 0, buffs: 0, talents: 0 };
        let (x, y) = (f32::from(a) * 0.01, f32::from(b) * 0.01);
        let val = |v: f32| Value::Const(v);
        // `x < y` is the same relation as `y > x`; likewise `<=` / `>=`.
        let lt = Condition::Cmp(CmpOp::Lt, val(x), val(y)).eval(&ctx);
        let gt_swapped = Condition::Cmp(CmpOp::Gt, val(y), val(x)).eval(&ctx);
        assert_eq!(lt, gt_swapped);
        let le = Condition::Cmp(CmpOp::Le, val(x), val(y)).eval(&ctx);
        let ge_swapped = Condition::Cmp(CmpOp::Ge, val(y), val(x)).eval(&ctx);
        assert_eq!(le, ge_swapped);
    });
}

#[test]
fn empty_and_is_true_empty_or_is_false() {
    let ctx = Ctx { tags: 0, buffs: 0, talents: 0 };
    assert!(Condition::Always.eval(&ctx));
    assert!(!Condition::Not(Box::new(Condition::Always)).eval(&ctx));
    assert!(Condition::And(Vec::new()).eval(&ctx), "vacuous ⋀ is true");
    assert!(!Condition::Or(Vec::new()).eval(&ctx), "vacuous ⋁ is false");
}
