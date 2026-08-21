//! Authoring the shape a picture is cut to (stormlight/server#121).
//!
//! [`WidgetExt::mask`] is a one-property write like every other setter on the
//! trait, and that is the whole of what is pinned here — but it is worth pinning,
//! because the two properties nearest it are the ones it would be confused with:
//!
//!   - it is **not** the picture. A widget may declare the shape of its socket and
//!     no picture at all, which is exactly what an ability slot does — the picture
//!     arrives at run time from whatever ability is bound to it, and the socket was
//!     never going to change when the loadout did;
//!   - it is **not** the slice. Both say something about how a picture meets a box
//!     that is not its own shape, and a setter that quietly cleared the other would
//!     be found by eye, on a screen, at the worst possible time.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::Slot;
use stormlight_mod_abi::ui::Style;
use stormlight_mod_sdk::ui::{WidgetExt, ability_slot, icon};

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// Whether the run cuts a picture that was declared, or a socket that is still
    /// waiting for one.
    pictured: bool,
    /// Whether the widget also carries slice insets — the neighbouring property
    /// this must not disturb.
    sliced: bool,
}

const MASK: &str = "mod://pack/socket.png";

#[test]
fn declaring_a_mask_changes_the_mask_and_nothing_else() {
    check!().with_type::<Scenario>().for_each(|s| {
        let build = |cut: bool| {
            let mut w =
                if s.pictured { icon("mod://pack/art.png") } else { ability_slot(Slot(0), "Q") };
            if s.sliced {
                w = w.sliced(4.0, 4.0, 4.0, 4.0);
            }
            if cut { w.mask(MASK) } else { w }
        };
        let plain = build(false);
        let cut = build(true);
        assert_eq!(cut.style.mask.as_deref(), Some(MASK), "the declared shape did not land");
        assert_eq!(plain.style.mask, None, "a widget that declared no shape carries one");
        // Everything else, compared as one value: a setter that touched a second
        // property would show up here whichever property it was.
        let bare = |mut style: Style| {
            style.mask = None;
            style
        };
        assert_eq!(
            bare(cut.style),
            bare(plain.style),
            "declaring the shape of a socket changed something other than the shape",
        );
    });
}
