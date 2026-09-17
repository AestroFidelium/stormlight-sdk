//! Binding a relative slot reference to the slot it turned out to be
//! (stormlight/server#187).
//!
//! `bind_slots` is what makes "each of your basic abilities refunds *its own*
//! cooldown" one card instead of one card per slot: the fold loops over the
//! matched slots it already knows and rewrites each copy of the rider with its own.
//! It rides the id-remap traversal, so the invariants here are about that choice as
//! much as about the rewrite:
//!
//!   - **binding reaches everywhere** — after it, nothing anywhere in the tree is
//!     still relative, however deeply a `CastAbility` or a `CooldownOf` was buried.
//!     A traversal that missed one would leave a reference that silently reads zero
//!     at runtime, which is the failure this whole issue is about;
//!   - **binding is idempotent, and the first binding wins** — a rider bound at
//!     fold time and frozen again at spawn must not be re-pointed by the second
//!     pass. This is what lets the three resolution sites layer without an order
//!     rule between them;
//!   - **an absolute reference is never touched** — binding a tree that has no
//!     relative reference in it leaves it byte-identical;
//!   - **an ordinary remap leaves a relative reference alone.** That one is pinned
//!     next door, by `remap::identity_remap_is_a_noop`, whose generated trees now
//!     carry `This`: adoption's local→global walk visits slot references too, and if
//!     it *bound* them every mod's `This` would freeze at load with no diagnostic;
//!   - **`require_bound` agrees with `bind_slots` exactly**: it errors on precisely
//!     the trees binding would change. Adoption's refusal and the fold's rewrite are
//!     then two readings of one predicate rather than two opinions.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::common::{ImpactTarget, NumOp};
use stormlight_mod_abi::conditions::{CmpOp, Condition};
use stormlight_mod_abi::ids::{DamageTypeId, EventId, Slot};
use stormlight_mod_abi::impacts::{AbilityTarget, CostMode, DamageFlags, Impact, PoolRef};
use stormlight_mod_abi::math::{Value, Var, Who};
use stormlight_mod_abi::slot_ref::{SlotRef, bind_slots, require_bound};
use stormlight_mod_abi::triggers::{EventFilter, EventKind, Reaction};

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Interprets a seed stream into a reaction — the one declaration that carries a
/// slot reference in **all four** of its positions at once: the filter narrows on
/// one, its condition and payload read one, and its effects act on one.
struct Gen<'a> {
    ops: &'a [u8],
    pos: usize,
    /// How many relative references the stream asked for, counted as it builds.
    relative: u32,
}

impl Gen<'_> {
    fn next(&mut self) -> u8 {
        let v = self.ops.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        v
    }
    /// A slot reference, relative one time in three, tallying as it goes so the
    /// test knows whether the tree it built has anything to bind.
    fn slot_ref(&mut self) -> SlotRef {
        if self.next().is_multiple_of(3) {
            self.relative += 1;
            SlotRef::This
        } else {
            SlotRef::At(Slot(self.next()))
        }
    }
    fn who(&mut self) -> Who {
        match self.next() % 3 {
            0 => Who::Caster,
            1 => Who::Target,
            _ => Who::Source,
        }
    }
    fn value(&mut self, depth: u8) -> Value {
        if depth == 0 {
            return Value::Const(f32::from(self.next()));
        }
        match self.next() % 5 {
            0 => Value::Const(f32::from(self.next())),
            1 => Value::Read(Var::CooldownOf(self.slot_ref(), self.who())),
            2 => Value::Read(Var::ChargesOf(self.slot_ref(), self.who())),
            3 => Value::Read(Var::Level),
            _ => Value::Clamp {
                v: Box::new(self.value(depth - 1)),
                lo: Box::new(self.value(depth - 1)),
                hi: Box::new(self.value(depth - 1)),
            },
        }
    }
    fn cond(&mut self, depth: u8) -> Condition {
        if depth == 0 {
            return Condition::Always;
        }
        match self.next() % 3 {
            0 => Condition::Always,
            1 => Condition::Cmp(CmpOp::Lt, self.value(1), self.value(1)),
            _ => Condition::Not(Box::new(self.cond(depth - 1))),
        }
    }
    fn pool(&mut self) -> PoolRef {
        match self.next() % 3 {
            0 => PoolRef::Cooldown(self.slot_ref()),
            1 => PoolRef::Charges(self.slot_ref()),
            _ => PoolRef::Shield,
        }
    }
    fn impacts(&mut self, depth: u8) -> Vec<Impact> {
        (0..self.next() % 3).map(|_| self.impact(depth)).collect()
    }
    fn impact(&mut self, depth: u8) -> Impact {
        if depth == 0 {
            return Impact::Interrupt { target: ImpactTarget::PrimaryTarget };
        }
        match self.next() % 7 {
            0 => Impact::Damage {
                amount: self.value(2),
                dtype: DamageTypeId(u16::from(self.next())),
                target: ImpactTarget::PrimaryTarget,
                flags: DamageFlags::default(),
            },
            1 => Impact::AdjustPool {
                pool: self.pool(),
                op: NumOp::Sub,
                amount: self.value(2),
                target: ImpactTarget::Caster,
            },
            2 => Impact::CastAbility {
                slot: self.slot_ref(),
                target: AbilityTarget::Inherit,
                value_scale: self.value(1),
                cost: CostMode::Free,
                params: Vec::new(),
            },
            // Buried under the combinators, and under a spawned body's own hooks,
            // which is where a traversal that missed a case would show it.
            3 => Impact::If {
                cond: self.cond(2),
                then: self.impacts(depth - 1),
                els: self.impacts(depth - 1),
            },
            4 => Impact::Delay { secs: self.value(1), inner: self.impacts(depth - 1) },
            5 => Impact::Emit {
                event: EventId(u16::from(self.next())),
                target: ImpactTarget::Caster,
                payload: self.value(2),
            },
            _ => Impact::Interrupt { target: ImpactTarget::Caster },
        }
    }
    fn reaction(&mut self) -> Reaction {
        Reaction {
            on: EventKind::OnCast,
            filter: EventFilter {
                source_slot: self.next().is_multiple_of(2).then(|| self.slot_ref()),
                every_nth: None,
                require_tag_on_target: None,
            },
            cond: self.cond(2),
            effects: self.impacts(3),
            target: ImpactTarget::PrimaryTarget,
            internal_cd: self.next().is_multiple_of(2).then(|| self.value(2)),
            charges: None,
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    ops: Vec<u8>,
    slot: u8,
    other: u8,
}

/// The generated tree, and how many relative references it carries.
fn build(s: &Scenario) -> (Reaction, u32) {
    let mut stream = Gen { ops: &s.ops, pos: 0, relative: 0 };
    let reaction = stream.reaction();
    (reaction, stream.relative)
}

fn bytes(r: &Reaction) -> Vec<u8> {
    postcard::to_allocvec(r).expect("serialize")
}

#[test]
fn binding_leaves_nothing_relative() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (mut reaction, relative) = build(s);
        assert_eq!(
            require_bound(&mut reaction.clone()).is_err(),
            relative > 0,
            "`require_bound` disagrees with what the tree actually carries",
        );
        bind_slots(&mut reaction, Slot(s.slot));
        require_bound(&mut reaction).expect("binding left a relative reference behind");
    });
}

#[test]
fn binding_is_idempotent_and_the_first_one_wins() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (mut once, _) = build(s);
        bind_slots(&mut once, Slot(s.slot));
        let mut twice = once.clone();
        bind_slots(&mut twice, Slot(s.slot));
        assert_eq!(bytes(&once), bytes(&twice), "binding twice is not binding once");

        // And a *later* pass with a different slot changes nothing: a rider bound
        // at fold time and frozen again at spawn keeps the slot the fold gave it.
        let mut relabelled = once.clone();
        bind_slots(&mut relabelled, Slot(s.other));
        assert_eq!(bytes(&once), bytes(&relabelled), "a second binding re-pointed the first");
    });
}

#[test]
fn an_already_absolute_tree_is_untouched() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (mut reaction, _) = build(s);
        bind_slots(&mut reaction, Slot(s.slot));
        let before = bytes(&reaction);
        // Binding to a different slot is the sharpest version of "untouched":
        // anything it could still rewrite would move.
        bind_slots(&mut reaction, Slot(s.other));
        assert_eq!(before, bytes(&reaction), "binding rewrote an absolute reference");
    });
}

#[test]
fn binding_changes_exactly_the_relative_trees() {
    check!().with_type::<Scenario>().for_each(|s| {
        let (reaction, relative) = build(s);
        // A slot the generator can never have written down, so "bound to it" and
        // "was already that" cannot be confused.
        let fresh = Slot(u8::MAX);
        let mut bound = reaction.clone();
        bind_slots(&mut bound, fresh);
        let changed = bytes(&reaction) != bytes(&bound);
        assert_eq!(
            changed,
            relative > 0,
            "binding changed a tree with nothing to bind, or missed one that had something",
        );
    });
}
