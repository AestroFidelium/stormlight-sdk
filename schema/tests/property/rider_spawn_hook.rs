//! A rider that answers every body a cast puts on the field — "each bolt fired",
//! as a talent card says it. Laws:
//!   - the new hook is **appended**: the hooks mods already ship keep their wire
//!     tags, so a mod built before it decodes unchanged;
//!   - every hook survives the wire.

use stormlight_mod_abi::talents::AbilityHook;

#[test]
fn the_spawn_hook_is_appended_and_every_hook_round_trips() {
    let hooks =
        [AbilityHook::OnCastStart, AbilityHook::OnCast, AbilityHook::OnHit, AbilityHook::OnSpawn];
    for (tag, hook) in hooks.iter().enumerate() {
        let bytes = postcard::to_allocvec(hook).expect("a hook serializes");
        assert_eq!(bytes, vec![u8::try_from(tag).unwrap()], "{hook:?} moved off wire tag {tag}");
        let back: AbilityHook = postcard::from_bytes(&bytes).expect("a hook decodes");
        assert_eq!(back, *hook);
    }
}
