//! Interner invariants: a stable name always maps to one dense handle, and a
//! handle always resolves back to the name that minted it.
//!
//! These pin the adoption-time contract (strings -> compact handles) the whole
//! ISA relies on: interning is idempotent, injective, and produces a contiguous
//! index space so handles are array-indexable in hot loops.

use bolero::check;
use stormlight_mod_abi::ids::{Handle, StatId};
use stormlight_mod_abi::interner::Interner;

#[test]
fn interner_round_trips_stable_injective_dense() {
    check!().with_type::<Vec<String>>().for_each(|names| {
        let mut interner = Interner::<StatId>::new();

        // Intern every name, remembering the handle each call returned.
        let minted: Vec<(&str, StatId)> =
            names.iter().map(|n| (n.as_str(), interner.intern(n))).collect();

        for &(name, id) in &minted {
            // Round-trip: the handle resolves back to the exact name.
            assert_eq!(interner.resolve(id), Some(name));
            // Idempotence: re-interning the same name yields the same handle.
            assert_eq!(interner.intern(name), id);
            // Lookup agrees with interning.
            assert_eq!(interner.get(name), Some(id));
        }

        // Injectivity: two handles are equal iff their source names are equal.
        for &(na, ia) in &minted {
            for &(nb, ib) in &minted {
                assert_eq!(ia == ib, na == nb);
            }
        }

        // Density: exactly one handle per distinct name, indices contiguous 0..len.
        let mut distinct: Vec<&str> = names.iter().map(String::as_str).collect();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(interner.len(), distinct.len());
        for raw in 0..interner.len() {
            // Every index below len resolves (no gaps).
            assert!(interner.resolve(StatId::from_raw(raw as u32)).is_some());
        }
        // One past the end never resolves.
        assert_eq!(interner.resolve(StatId::from_raw(interner.len() as u32)), None);
    });
}
