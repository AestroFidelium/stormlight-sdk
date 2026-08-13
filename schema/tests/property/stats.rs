//! The engine-reserved stat names are an **id table** (stormlight/server#67).
//!
//! Both ends of the wire seed their stat interner from this list before any mod
//! interns above it, so its order *is* the id order: the server mitigates with
//! `StatId(1)` because `RESERVED[1]` is `armor`, and the client resolves a HUD's
//! `ValueBinding::Stat` against an id space it rebuilds from the same seed. A
//! duplicate name or a disagreeing index shifts every mod-defined stat by the
//! difference, which reads as a bar showing the wrong number rather than as a
//! failure — hence a test on the table itself.

use bolero::check;
use stormlight_mod_abi::stats;

#[test]
fn reserved_stat_names_are_a_dense_unique_table() {
    check!().with_type::<()>().for_each(|()| {
        for (i, name) in stats::RESERVED.iter().enumerate() {
            assert_eq!(
                stats::RESERVED.iter().filter(|n| *n == name).count(),
                1,
                "reserved stat `{name}` listed twice — every id after it shifts",
            );
            assert_eq!(
                stats::reserved_index(name),
                Some(i),
                "reserved stat `{name}` does not report the id it is interned at",
            );
        }
        assert_eq!(stats::reserved_index("a name no engine reserves"), None);
    });
}
