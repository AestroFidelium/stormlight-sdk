//! A **cosmetic key** is a name in the ability family with nothing hanging off it
//! (stormlight/server#152).
//!
//! The effect-visual table is keyed by ability handle, because until now
//! everything with a look was cast from a slot. A basic attack is not — it
//! occupies no slot, appears in no loadout and has no descriptor — so it had no
//! way to be dressed at all, and the thing a player fires most often wore the
//! engine's placeholder while every ability in the game could be given a look.
//!
//! What has to hold for a bare key to be safe:
//!
//! - **The same name is the same handle.** That is the whole mechanism: the
//!   gameplay half names its attack's look, the cosmetic half declares visuals
//!   under the same name, and adoption interns both into one global family.
//! - **A key takes no descriptor's place.** An ability's handle used to be its
//!   index among the descriptors; a key breaks that, and everything downstream
//!   reads the handle. So minting one must shift nothing, and the name table — the
//!   thing adoption actually remaps by position — must still line up with it.
//! - **A key and an ability may share a name on purpose**, so content can dress an
//!   attack exactly like the ability it echoes.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::abilities::{AbilityDescriptor, CastSpec, Params, Targeting};
use stormlight_mod_sdk::abi::conditions::Condition;
use stormlight_mod_sdk::abi::ids::AbilityId;
use stormlight_mod_sdk::context::ModContext;

/// A minimal ability whose `id` the context overwrites at definition.
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

/// A run of declarations: `true` mints a bare key, `false` declares an ability.
#[derive(Debug, TypeGenerator)]
struct Scenario {
    steps: Vec<bool>,
}

#[test]
fn a_key_takes_no_abilitys_place() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ModContext::new();
        let mut declared: Vec<AbilityId> = Vec::new();
        let mut names: Vec<String> = Vec::new();

        for (n, is_key) in s.steps.iter().take(16).enumerate() {
            let name = format!("item{n}");
            names.push(name.clone());
            if *is_key {
                ctx.effect_key(&name);
            } else {
                declared.push(ctx.ability(&name, sample_ability()));
            }
        }

        let reg = ctx.finish();
        assert_eq!(
            reg.abilities.len(),
            declared.len(),
            "minting a cosmetic key added or removed an ability descriptor",
        );
        let ids: Vec<AbilityId> = reg.abilities.iter().map(|a| a.id).collect();
        assert_eq!(ids, declared, "a key shifted the handle of an ability around it");

        // The name table is what adoption remaps by position, so a key has to be
        // *in* it — a handle whose name never crossed would point at whatever
        // content occupied that index globally.
        assert_eq!(
            reg.names.abilities, names,
            "the ability name table lost a key, or kept them out of declaration order",
        );
    });
}

#[test]
fn the_same_name_is_the_same_key() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ModContext::new();
        for (n, _) in s.steps.iter().take(16).enumerate() {
            ctx.effect_key(&format!("filler{n}"));
        }
        let first = ctx.effect_key("shot");
        let again = ctx.effect_key("shot");
        assert_eq!(first, again, "the same cosmetic name minted two different keys");

        // …and an ability declared under that name gets it too, so content can
        // dress an attack exactly like the ability it echoes.
        let shared = ctx.ability("shot", sample_ability());
        assert_eq!(shared, first, "an ability and a key with one name took two handles");
    });
}
