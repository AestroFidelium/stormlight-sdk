//! Invariants of a map's drawn ground height (`HeightField`) — the surface its
//! units are *drawn* standing on. The simulation stays planar; this only lifts the
//! art, so a unit crossing a bridge is drawn on the deck rather than inside it.
//!   - **Exact at samples**: sampling at a grid point returns that sample.
//!   - **Bounded between samples**: a bilinear blend never leaves the range of the
//!     four samples around it — no overshoot that would float a unit.
//!   - **Clamped outside**: a point off the grid reads the nearest edge, never
//!     garbage, so a unit at the rim of the map is still drawn on something.
//!   - **Round-trip**: a registration carrying a field survives postcard intact.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::scenery::HeightField;
use stormlight_mod_abi::visuals::ClientRegistration;

#[derive(Debug, TypeGenerator)]
struct Scenario {
    w: u8,
    h: u8,
    cell: u8,
    origin: (i8, i8),
    samples: Vec<i8>,
    at: (i16, i16),
}

fn field(s: &Scenario) -> HeightField {
    let (w, h) = (u32::from(s.w % 6) + 2, u32::from(s.h % 6) + 2);
    let n = (w * h) as usize;
    let heights = (0..n).map(|i| f32::from(*s.samples.get(i).unwrap_or(&0)) / 8.0).collect();
    HeightField {
        origin: [f32::from(s.origin.0), f32::from(s.origin.1)],
        cell: f32::from(s.cell % 4) + 0.5,
        size: [w, h],
        heights,
    }
}

#[test]
fn sampling_is_exact_at_grid_points_and_bounded_between_them() {
    check!().with_type::<Scenario>().for_each(|s| {
        let f = field(s);
        let [w, h] = f.size;
        for j in 0..h {
            for i in 0..w {
                let p = [f.origin[0] + i as f32 * f.cell, f.origin[1] + j as f32 * f.cell];
                let want = f.heights[(j * w + i) as usize];
                assert!((f.sample(p) - want).abs() < 1e-4, "grid point {i},{j}");
            }
        }
        let p = [f.origin[0] + f32::from(s.at.0) / 64.0, f.origin[1] + f32::from(s.at.1) / 64.0];
        let v = f.sample(p);
        let (lo, hi) =
            f.heights.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
        assert!(v >= lo - 1e-4 && v <= hi + 1e-4, "{v} outside [{lo}, {hi}] at {p:?}");
    });
}

#[test]
fn a_field_round_trips_through_the_registration() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg = ClientRegistration { ground: Some(field(s)), ..ClientRegistration::default() };
        let back: ClientRegistration =
            postcard::from_bytes(&postcard::to_allocvec(&reg).expect("encode")).expect("decode");
        assert_eq!(back, reg);
    });
}
