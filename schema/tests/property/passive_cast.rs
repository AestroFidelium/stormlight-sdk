//! A passive ability (stormlight/server#189): one a unit carries in a slot and
//! nobody presses. Laws:
//!   - every cast shape survives the wire, the passive one included;
//!   - the passive shape is **appended**: the shapes mods already ship keep their
//!     wire tags, so a mod built before it still decodes unchanged;
//!   - exactly the passive shape says it cannot be pressed.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::abilities::CastSpec;
use stormlight_mod_abi::math::Value;

/// Which cast shape, with the numbers it carries (in hundredths of a second).
#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Shape {
    Instant,
    Cast { centis: u16, movable: bool },
    Channel { centis: u16, movable: bool, tick_centis: Option<u16> },
    Passive,
}

fn secs(centis: u16) -> Value {
    Value::Const(f32::from(centis) / 100.0)
}

impl Shape {
    fn spec(self) -> CastSpec {
        match self {
            Self::Instant => CastSpec::Instant,
            Self::Cast { centis, movable } => CastSpec::Cast { time: secs(centis), movable },
            Self::Channel { centis, movable, tick_centis } => {
                CastSpec::Channel { time: secs(centis), movable, tick: tick_centis.map(secs) }
            }
            Self::Passive => CastSpec::Passive,
        }
    }

    /// The postcard tag each shape has always had; `Passive` is the newest.
    fn wire_tag(self) -> u8 {
        match self {
            Self::Instant => 0,
            Self::Cast { .. } => 1,
            Self::Channel { .. } => 2,
            Self::Passive => 3,
        }
    }
}

#[test]
fn every_cast_shape_round_trips_and_keeps_its_wire_tag() {
    check!().with_type::<Shape>().for_each(|&shape| {
        let spec = shape.spec();
        let bytes = postcard::to_allocvec(&spec).expect("a cast shape serializes");
        assert_eq!(bytes.first().copied(), Some(shape.wire_tag()), "a wire tag moved");
        let back: CastSpec = postcard::from_bytes(&bytes).expect("a cast shape decodes");
        assert_eq!(back, spec);
    });
}

#[test]
fn only_the_passive_shape_cannot_be_pressed() {
    check!().with_type::<Shape>().for_each(|&shape| {
        assert_eq!(shape.spec().is_passive(), matches!(shape, Shape::Passive));
    });
}
