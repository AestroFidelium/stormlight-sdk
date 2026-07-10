//! `ModContext` invariants — the guest builds content by name, and `finish`
//! must produce a `Registration` whose name tables line up with the descriptors
//! the host will index by handle. Laws:
//!   - **Alignment**: a descriptor defined `i`-th gets handle `i`, and its name
//!     lands at index `i` of the matching `Names` table.
//!   - **Idempotence**: re-interning a name returns the same handle and adds no
//!     new entry (the interning contract, surfaced through the context).
//!   - **Emit/decode**: `finish` → postcard → decode reproduces the exact bundle
//!     (the host will decode precisely these bytes).

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::abilities::{AbilityDescriptor, CastSpec, Params, Targeting};
use stormlight_mod_sdk::abi::conditions::Condition;
use stormlight_mod_sdk::abi::ids::{AbilityId, TagClassId, TagId};
use stormlight_mod_sdk::abi::manifest::ABI_VERSION;
use stormlight_mod_sdk::bindings::to_bytes;
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

#[derive(Debug, TypeGenerator)]
struct Scenario {
    abilities: u8,
    stats: u8,
    tag_classes: u8,
}

#[test]
fn finish_aligns_names_with_descriptor_indices() {
    check!().with_type::<Scenario>().for_each(|s| {
        let na = (s.abilities % 8) as usize;
        let ns = (s.stats % 8) as usize;
        let nt = (s.tag_classes % 6) as usize;

        let mut ctx = ModContext::new();
        for i in 0..na {
            let id = ctx.ability(&format!("ab{i}"), sample_ability());
            assert_eq!(id, AbilityId(i as u32), "define order must mint dense handles");
        }
        for i in 0..ns {
            ctx.stat(&format!("st{i}"));
        }
        for i in 0..nt {
            ctx.register_tag_class(&format!("tg{i}"), &format!("cl{i}"));
        }
        let reg = ctx.finish();

        assert_eq!(reg.abi, ABI_VERSION);

        assert_eq!(reg.abilities.len(), na);
        assert_eq!(reg.names.abilities.len(), na);
        for i in 0..na {
            assert_eq!(reg.abilities[i].id, AbilityId(i as u32));
            assert_eq!(reg.names.abilities[i], format!("ab{i}"));
        }

        assert_eq!(reg.names.stats.len(), ns);
        for i in 0..ns {
            assert_eq!(reg.names.stats[i], format!("st{i}"));
        }

        assert_eq!(reg.tag_classes.len(), nt);
        assert_eq!(reg.names.tags.len(), nt);
        assert_eq!(reg.names.tag_classes.len(), nt);
        for i in 0..nt {
            assert_eq!(reg.tag_classes[i], (TagId(i as u16), TagClassId(i as u16)));
        }
    });
}

#[test]
fn interning_the_same_name_is_idempotent() {
    check!().with_type::<u8>().for_each(|&k| {
        let n = (k % 8) as usize;
        let mut ctx = ModContext::new();
        let first: Vec<_> = (0..n).map(|i| ctx.stat(&format!("s{i}"))).collect();
        // Re-interning adds nothing and returns the original handles.
        for (i, &h) in first.iter().enumerate() {
            assert_eq!(ctx.stat(&format!("s{i}")), h);
        }
        assert_eq!(ctx.finish().names.stats.len(), n);
    });
}

#[test]
fn a_built_registration_decodes_equal() {
    check!().with_type::<Scenario>().for_each(|s| {
        let na = (s.abilities % 8) as usize;
        let ns = (s.stats % 8) as usize;

        let mut ctx = ModContext::new();
        for i in 0..na {
            ctx.ability(&format!("ab{i}"), sample_ability());
        }
        for i in 0..ns {
            ctx.stat(&format!("st{i}"));
        }
        let reg = ctx.finish();

        // The host decodes exactly these bytes; they must reproduce the bundle.
        let decoded = postcard::from_bytes(&to_bytes(&reg)).expect("decode");
        assert_eq!(reg, decoded);
    });
}
