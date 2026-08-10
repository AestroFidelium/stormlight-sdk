//! The wasm runtime ABI is a total, self-describing data model: **any** generated
//! [`GuestEffects`] / [`TickContext`] / [`TriggerContext`] survives a postcard
//! round-trip unchanged. This is the guest→engine contract for the runtime
//! entry points (`mod_handle` / `mod_tick` / `mod_trigger`): a stateless guest's
//! *only* output is a serialized `Vec<Impact>`, dispatched by the engine. If the
//! wrapper were lossy the effect a guest proposes would differ from the effect
//! the engine runs — so identity here is the safety property.

use bolero::check;
use stormlight_mod_abi::common::ImpactTarget;
use stormlight_mod_abi::ids::{DamageTypeId, EventId, HandlerId};
use stormlight_mod_abi::impacts::{DamageFlags, HealFlags, Impact};
use stormlight_mod_abi::math::Value;
use stormlight_mod_abi::runtime::{
    ALLOC_EXPORT, GuestEffects, HANDLE_EXPORT, TICK_EXPORT, TRIGGER_EXPORT, TickContext,
    TriggerContext,
};

/// Build one leaf `Impact` from a small opcode stream — enough variety to
/// exercise the `Vec<Impact>` field (including the `Custom` escape hatch and its
/// opaque `params` blob) without pulling in the full ISA generator.
fn leaf(op: u16, n: u16, params: &[u8]) -> Impact {
    let target = match n % 4 {
        0 => ImpactTarget::Caster,
        1 => ImpactTarget::PrimaryTarget,
        2 => ImpactTarget::ResolvedTarget,
        _ => ImpactTarget::Source,
    };
    match op % 4 {
        0 => Impact::Damage {
            amount: Value::Const(f32::from(n)),
            dtype: DamageTypeId(n),
            target,
            flags: DamageFlags::default(),
        },
        1 => {
            Impact::Heal { amount: Value::Const(f32::from(n)), target, flags: HealFlags::default() }
        }
        2 => Impact::Emit { event: EventId(n), target, payload: Value::Const(f32::from(n)) },
        _ => Impact::Custom { handler: HandlerId(u32::from(n)), params: params.to_vec(), target },
    }
}

#[test]
fn guest_effects_survive_a_round_trip() {
    check!().with_type::<(Vec<(u16, u16)>, Vec<u8>)>().for_each(|(ops, params)| {
        let effects =
            GuestEffects { effects: ops.iter().map(|&(op, n)| leaf(op, n, params)).collect() };
        let bytes = postcard::to_allocvec(&effects).expect("GuestEffects serializes");
        let back: GuestEffects = postcard::from_bytes(&bytes).expect("GuestEffects decodes");
        assert_eq!(effects, back, "round-trip must be identity");
    });
}

#[test]
fn tick_and_trigger_contexts_survive_a_round_trip() {
    check!().with_type::<(u64, u16)>().for_each(|&(tick, ev)| {
        let tc = TickContext { tick };
        let back: TickContext = postcard::from_bytes(&postcard::to_allocvec(&tc).unwrap()).unwrap();
        assert_eq!(tc, back);

        let tg = TriggerContext { event: EventId(ev), payload: Value::Const(f32::from(ev)) };
        let back: TriggerContext =
            postcard::from_bytes(&postcard::to_allocvec(&tg).unwrap()).unwrap();
        assert_eq!(tg, back);
    });
}

#[test]
fn an_empty_result_is_the_default_and_the_export_names_are_distinct() {
    // The zero-effect case is the common one (a handler that only reads); it must
    // be representable and be the `Default`.
    assert!(GuestEffects::default().effects.is_empty());
    // The fixed export set must be four distinct, non-empty symbols — the guest
    // ABI resolves entry points by these names, never by mangled per-handler ones.
    let names = [ALLOC_EXPORT, HANDLE_EXPORT, TICK_EXPORT, TRIGGER_EXPORT];
    assert!(names.iter().all(|n| !n.is_empty()));
    for i in 0..names.len() {
        for j in (i + 1)..names.len() {
            assert_ne!(names[i], names[j], "export names must be distinct");
        }
    }
}
