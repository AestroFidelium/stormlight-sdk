//! Merging per cause only while the reports keep coming fast
//! (stormlight/server#182). Laws:
//!   - the windowed rule is **appended**: the three rules mods already ship keep
//!     their wire tags, so a mod built before it decodes unchanged;
//!   - any window survives the wire exactly.

use bolero::check;
use stormlight_mod_abi::ui_event::Coalesce;

#[test]
fn the_windowed_rule_is_appended_and_round_trips() {
    for (tag, rule) in [Coalesce::Never, Coalesce::PerCause, Coalesce::PerUnit].iter().enumerate() {
        let bytes = postcard::to_allocvec(rule).expect("serialize");
        assert_eq!(bytes, vec![u8::try_from(tag).unwrap()], "{rule:?} moved off wire tag {tag}");
    }
    check!().with_type::<u16>().for_each(|&millis| {
        let rule = Coalesce::PerCauseWithin { millis };
        let bytes = postcard::to_allocvec(&rule).expect("serialize");
        assert_eq!(bytes.first(), Some(&3), "the windowed rule is not the fourth tag");
        let back: Coalesce = postcard::from_bytes(&bytes).expect("decode");
        assert_eq!(back, rule);
    });
}
