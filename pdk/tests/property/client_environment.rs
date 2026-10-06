//! `ClientContext::environment` invariants — how a map's cosmetic half declares
//! the light it is seen in. Laws:
//!   - **Last declaration wins**: a map has one environment; declaring again
//!     replaces it, so a layered mod can override a base one.
//!   - **Nothing declared, nothing sent**: a context that never declares one
//!     finishes with `None`, leaving the engine its neutral default.
//!   - **Nothing is interned**: lighting names nothing.

use bolero::check;
use stormlight_mod_sdk::abi::environment::{Environment, SunLight};
use stormlight_mod_sdk::client::ClientContext;

fn env(k: u8) -> Environment {
    Environment {
        ambient: [f32::from(k) / 255.0; 3],
        lights: vec![SunLight { color: [1.0, 0.9, 0.8], direction: [0.5, -1.0, f32::from(k)] }],
        shadow: None,
        exposure: 1.0 + f32::from(k) / 100.0,
        backdrop: [0.0; 3],
    }
}

#[test]
fn the_last_declaration_wins() {
    check!().with_type::<Vec<u8>>().for_each(|ks| {
        let mut ctx = ClientContext::new();
        for k in ks.iter().take(6) {
            ctx.environment(env(*k));
        }
        let reg = ctx.finish();
        assert_eq!(reg.environment, ks.iter().take(6).next_back().map(|k| env(*k)));
        assert!(reg.names.units.is_empty() && reg.names.abilities.is_empty());
    });
}
