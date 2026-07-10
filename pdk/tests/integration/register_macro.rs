//! `register_mod!` wiring — a structural check that the macro generates a
//! builder whose registration, once serialized, decodes host-side to exactly
//! the content the author declared. (The general emit/decode law is fuzzed in
//! `property/context.rs`; this pins the macro expansion itself.)

use stormlight_mod_sdk::abi::behaviors::{BuffSpec, Reapply, StackScope, Stacking};
use stormlight_mod_sdk::abi::descriptors::Registration;
use stormlight_mod_sdk::abi::ids::{AbilityId, BuffId, TagClassId, TagId};
use stormlight_mod_sdk::abi::manifest::ABI_VERSION;
use stormlight_mod_sdk::context::ModContext;
use stormlight_mod_sdk::register_mod;

use stormlight_mod_sdk::abi::abilities::{AbilityDescriptor, CastSpec, Params, Targeting};
use stormlight_mod_sdk::abi::conditions::Condition;

fn sample_ability() -> AbilityDescriptor {
    AbilityDescriptor {
        id: AbilityId(0),
        params: Params(Vec::new()),
        targeting: Targeting::NoTarget,
        cast: CastSpec::Instant,
        cost: Vec::new(),
        cast_gate: Condition::Always,
        on_cast_start: Vec::new(),
        on_cast: Vec::new(),
        tags: Vec::new(),
    }
}

fn sample_buff() -> BuffSpec {
    BuffSpec {
        id: BuffId(0),
        duration: None,
        stacking: Stacking { on_reapply: Reapply::Ignore, scope: StackScope::Global },
        max_stacks: 1,
        modifiers: Vec::new(),
        tags: Vec::new(),
        reactions: Vec::new(),
        on_apply: Vec::new(),
        on_expire: Vec::new(),
        on_remove: Vec::new(),
        drop_on_death: false,
    }
}

// Publish a small mod: one stat, one buff, one ability, one tag→class.
register_mod!(|ctx: &mut ModContext| {
    let _hp = ctx.stat("health");
    let _shield = ctx.buff("shield", sample_buff());
    let _strike = ctx.ability("strike", sample_ability());
    ctx.register_tag_class("rooted", "blocks_move");
});

#[test]
fn macro_registration_decodes_to_the_declared_set() {
    let reg = __stormlight_registration();
    let bytes = stormlight_mod_sdk::bindings::to_bytes(&reg);
    let decoded: Registration = postcard::from_bytes(&bytes).expect("decode");

    assert_eq!(decoded.abi, ABI_VERSION);
    assert_eq!(decoded.names.stats, ["health"]);
    assert_eq!(decoded.names.buffs, ["shield"]);
    assert_eq!(decoded.names.abilities, ["strike"]);
    assert_eq!(decoded.names.tags, ["rooted"]);
    assert_eq!(decoded.names.tag_classes, ["blocks_move"]);

    assert_eq!(decoded.abilities.len(), 1);
    assert_eq!(decoded.abilities[0].id, AbilityId(0));
    assert_eq!(decoded.buffs.len(), 1);
    assert_eq!(decoded.buffs[0].id, BuffId(0));
    assert_eq!(decoded.tag_classes, [(TagId(0), TagClassId(0))]);
}
