//! Who a body or an aimed cast may affect.

use stormlight_mod_abi::common::{Affiliation, TargetFilter};

/// Living units on the other side.
#[must_use]
pub fn enemies() -> TargetFilter {
    TargetFilter::of(Affiliation::Enemies)
}

/// Living units on the caster's side.
#[must_use]
pub fn allies() -> TargetFilter {
    TargetFilter::of(Affiliation::Allies)
}

/// Every living unit, whichever side.
#[must_use]
pub fn everyone() -> TargetFilter {
    TargetFilter::of(Affiliation::All)
}
