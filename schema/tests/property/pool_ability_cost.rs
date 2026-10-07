//! "The resource this ability costs" as a pool (stormlight/server#210). Laws:
//!   - the new pool is **appended**: every pool mods already name keeps its wire tag;
//!   - its slot reference is rewritten by the slot-binding walk like a cooldown's
//!     (`This` binds to the slot an effect runs from);
//!   - an interface reading it through a relative slot is refused at load, as a
//!     cooldown's is — a widget runs from no slot.

use bolero::check;
use stormlight_mod_abi::ids::{ResourceId, Slot, StackId};
use stormlight_mod_abi::impacts::PoolRef;
use stormlight_mod_abi::slot_ref::{SlotRef, bind_slots};

#[test]
fn the_cost_pool_is_appended_and_binds_like_a_cooldown() {
    let pools = [
        PoolRef::Shield,
        PoolRef::Resource(ResourceId(0)),
        PoolRef::Stacks(StackId(0)),
        PoolRef::Cooldown(SlotRef::This),
        PoolRef::Charges(SlotRef::This),
        PoolRef::Xp,
        PoolRef::AbilityCost(SlotRef::This),
    ];
    for (tag, pool) in pools.iter().enumerate() {
        let bytes = postcard::to_allocvec(pool).expect("a pool serializes");
        assert_eq!(
            bytes.first(),
            Some(&u8::try_from(tag).unwrap()),
            "{pool:?} moved off tag {tag}"
        );
        let back: PoolRef = postcard::from_bytes(&bytes).expect("a pool decodes");
        assert_eq!(back, *pool);
    }
    check!().with_type::<u8>().for_each(|&slot| {
        let mut pool = PoolRef::AbilityCost(SlotRef::This);
        bind_slots(&mut pool, Slot(slot));
        assert_eq!(pool, PoolRef::AbilityCost(SlotRef::At(Slot(slot))));
    });
}
