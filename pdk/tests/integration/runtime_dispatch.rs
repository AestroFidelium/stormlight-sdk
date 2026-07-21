//! The runtime dispatch tables a mod builds are reconstructed by the macro's
//! `__stormlight_build()` and route each entry point to the author's closure.
//!
//! This is the guest half of the runtime ABI: `mod_handle`/`mod_tick`/
//! `mod_trigger` (wasm-only exports) re-run the builder and dispatch through
//! exactly these `ModContext::run_*` methods, so proving the host-callable
//! dispatch here proves the guest wiring without a wasm target. Invariants are
//! structural/directional: the right closure runs, its input flows through, and
//! an *un*registered selector is total (yields no effects, never panics).

use bolero::check;
use stormlight_mod_sdk::abi::common::ImpactTarget;
use stormlight_mod_sdk::abi::ids::{DamageTypeId, EventId, HandlerId};
use stormlight_mod_sdk::abi::impacts::{DamageFlags, HealFlags, Impact};
use stormlight_mod_sdk::abi::math::Value;
use stormlight_mod_sdk::abi::runtime::{GuestEffects, TickContext, TriggerContext};
use stormlight_mod_sdk::context::ModContext;
use stormlight_mod_sdk::register_mod;
use stormlight_mod_sdk::runtime::HandlerCall;

const TRIGGER_EVENT: u16 = 7;

// A mod with all three runtime entry points wired. Each closure is *pure*: its
// output is a function of its input only, matching the stateless-guest contract.
register_mod!(|ctx: &mut ModContext| {
    // A `Custom` handler that turns its opaque param blob's length into damage,
    // so the test can prove the blob reaches the closure intact.
    ctx.on_handler("blob_to_damage", |call: &HandlerCall| {
        GuestEffects::new(vec![Impact::Damage {
            amount: Value::Const(call.params.len() as f32),
            dtype: DamageTypeId(0),
            target: ImpactTarget::ResolvedTarget,
            flags: DamageFlags::default(),
        }])
    });
    // A second handler so dispatch must select by id, not just "the only one".
    ctx.on_handler("always_heal", |_| {
        GuestEffects::new(vec![Impact::Heal {
            amount: Value::Const(1.0),
            target: ImpactTarget::Caster,
            flags: HealFlags::default(),
        }])
    });
    // Per-tick entry: echoes the tick number so we can prove the context flows.
    ctx.on_tick(|t: &TickContext| {
        GuestEffects::new(vec![Impact::Emit {
            event: EventId(0),
            target: ImpactTarget::Caster,
            payload: Value::Const(t.tick as f32),
        }])
    });
    // Trigger entry keyed by event id.
    ctx.on_trigger(EventId(TRIGGER_EVENT), |g: &TriggerContext| {
        GuestEffects::new(vec![Impact::Emit {
            event: g.event,
            target: ImpactTarget::Source,
            payload: g.payload.clone(),
        }])
    });
});

/// The `HandlerId`s the builder interns, in declaration order.
fn ids() -> (HandlerId, HandlerId) {
    let reg = __stormlight_registration();
    assert_eq!(reg.names.handlers, ["blob_to_damage", "always_heal"]);
    (HandlerId(0), HandlerId(1))
}

#[test]
fn a_handler_receives_its_param_blob_and_the_right_one_is_selected() {
    let (blob_id, heal_id) = ids();
    check!().with_type::<Vec<u8>>().for_each(|params| {
        let ctx = __stormlight_build();

        let out = ctx.run_handler(blob_id, &HandlerCall { params });
        match out.effects.as_slice() {
            [Impact::Damage { amount: Value::Const(a), .. }] => {
                assert_eq!(*a, params.len() as f32, "param blob length must reach the closure");
            }
            other => panic!("expected one Damage leaf, got {other:?}"),
        }

        // Same input, different id → the *other* handler runs (a heal, not damage).
        let heal = ctx.run_handler(heal_id, &HandlerCall { params });
        assert!(matches!(heal.effects.as_slice(), [Impact::Heal { .. }]));
    });
}

#[test]
fn an_unregistered_selector_yields_no_effects() {
    check!().with_type::<u32>().for_each(|&raw| {
        let ctx = __stormlight_build();
        // Only ids 0 and 1 are registered; anything else is a total no-op.
        if raw > 1 {
            assert!(
                ctx.run_handler(HandlerId(raw), &HandlerCall { params: &[] }).effects.is_empty(),
                "unknown handler id must produce no effects, never panic"
            );
        }
        // An event with no trigger registered is likewise empty.
        if raw as u16 != TRIGGER_EVENT {
            let tc = TriggerContext { event: EventId(raw as u16), payload: Value::Const(0.0) };
            assert!(ctx.run_trigger(&tc).effects.is_empty());
        }
    });
}

#[test]
fn tick_and_trigger_route_their_context_through() {
    check!().with_type::<(u64, f32)>().for_each(|&(tick, pay)| {
        let ctx = __stormlight_build();

        match ctx.run_tick(&TickContext { tick }).effects.as_slice() {
            [Impact::Emit { payload: Value::Const(p), .. }] => assert_eq!(*p, tick as f32),
            other => panic!("expected one Emit from tick, got {other:?}"),
        }

        let tc = TriggerContext { event: EventId(TRIGGER_EVENT), payload: Value::Const(pay) };
        match ctx.run_trigger(&tc).effects.as_slice() {
            [Impact::Emit { event, payload: Value::Const(p), .. }] => {
                assert_eq!(*event, EventId(TRIGGER_EVENT));
                assert!((*p == pay) || (p.is_nan() && pay.is_nan()));
            }
            other => panic!("expected one Emit from trigger, got {other:?}"),
        }
    });
}
