//! Authoring a cooldown sweep (stormlight/server#99).
//!
//! The pdk's job here is to make the *good* default free and the override cheap,
//! so two things are pinned:
//!
//!   - **A slot built by the constructor already sweeps.** An author who never
//!     heard of [`WidgetExt::sweep`] still ships a HUD whose keys show what they
//!     are waiting for — the whole reason the declaration has a default rather
//!     than an `Option`.
//!   - **The override touches the sweep and nothing else.** Every other setter on
//!     this trait is a one-property write, and one that quietly reset a colour or
//!     dropped a picture would be found by eye, on a screen, at the worst time.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::Slot;
use stormlight_mod_abi::ui::{Style, Sweep, SweepDirection};
use stormlight_mod_sdk::ui::{WidgetExt, ability_slot, icon};

#[derive(Debug, TypeGenerator)]
struct Scenario {
    rgba: [u8; 4],
    counter: bool,
    /// Whether the run overrides a slot or a kind with no cooldown at all — the
    /// setter is inert on the second, and must still be harmless.
    on_a_slot: bool,
}

impl Scenario {
    fn color(&self) -> [f32; 4] {
        self.rgba.map(|v| f32::from(v) / 255.0)
    }

    fn direction(&self) -> SweepDirection {
        if self.counter { SweepDirection::CounterClockwise } else { SweepDirection::Clockwise }
    }
}

#[test]
fn a_slot_sweeps_without_being_asked_to() {
    let slot = ability_slot(Slot(0), "Q");
    assert_eq!(
        slot.style.sweep,
        Sweep::default(),
        "the constructor no longer hands out the blessed default, so a HUD that \
         says nothing about cooldowns stopped drawing them",
    );
    assert!(slot.style.sweep.is_drawn(), "a slot built by the pdk draws no cooldown at all");
}

#[test]
fn declaring_a_sweep_changes_the_sweep_and_nothing_else() {
    check!().with_type::<Scenario>().for_each(|s| {
        let base = if s.on_a_slot {
            ability_slot(Slot(1), "W").color([1.0, 0.5, 0.25, 1.0]).image("mod://pack/bolt.png")
        } else {
            icon("mod://pack/bolt.png").color([1.0, 0.5, 0.25, 1.0])
        };
        let after = base.clone().sweep(s.color(), s.direction());

        assert_eq!(
            after.style.sweep,
            Sweep { color: s.color(), direction: s.direction() },
            "the sweep the author declared is not the one on the widget",
        );
        assert_eq!(
            Style { sweep: base.style.sweep, ..after.style.clone() },
            base.style,
            "declaring a sweep disturbed another property of the style",
        );
        assert_eq!(after.layout, base.layout, "declaring a sweep moved the widget");
        assert_eq!(after.kind, base.kind, "declaring a sweep changed what the widget is");
    });
}
