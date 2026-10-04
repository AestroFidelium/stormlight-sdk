//! Invariants of a map's environment (`Environment`) — the light its art is seen
//! in and what is drawn past its edge. Purely cosmetic, declared by a map's client
//! half; the engine knows no map's sun.
//!   - **Round-trip**: a registration carrying an environment survives postcard
//!     unchanged, like every other cosmetic.
//!   - **Validity is finiteness and sense**: a light with no direction, a negative
//!     colour, or a non-positive exposure is refused — each would draw nothing or
//!     poison a shader with NaN — and anything finite and sensible is accepted.
//!   - **Directions are unit on demand**: `toward` normalises whatever length the
//!     mod wrote, and keeps its direction.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::environment::{Environment, Shadow, SunLight};
use stormlight_mod_abi::visuals::ClientRegistration;

#[derive(Debug, TypeGenerator)]
struct Scenario {
    ambient: [u8; 3],
    lights: Vec<([u8; 3], [i8; 3])>,
    shadow: Option<([i8; 3], u8)>,
    exposure: u8,
    backdrop: [u8; 3],
}

fn dir(d: [i8; 3]) -> [f32; 3] {
    let v = d.map(f32::from);
    if v == [0.0; 3] { [0.0, -1.0, 0.0] } else { v }
}

fn build(s: &Scenario) -> Environment {
    Environment {
        ambient: s.ambient.map(|v| f32::from(v) / 128.0),
        lights: s
            .lights
            .iter()
            .take(4)
            .map(|(c, d)| SunLight { color: c.map(|v| f32::from(v) / 128.0), direction: dir(*d) })
            .collect(),
        shadow: s.shadow.map(|(d, k)| Shadow { direction: dir(d), strength: f32::from(k) / 255.0 }),
        exposure: f32::from(s.exposure) / 64.0 + 0.25,
        backdrop: s.backdrop.map(|v| f32::from(v) / 255.0),
    }
}

#[test]
fn an_environment_round_trips_through_the_registration() {
    check!().with_type::<Scenario>().for_each(|s| {
        let reg =
            ClientRegistration { environment: Some(build(s)), ..ClientRegistration::default() };
        let bytes = postcard::to_allocvec(&reg).expect("serialize");
        let back: ClientRegistration = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(back, reg);
        assert_eq!(postcard::to_allocvec(&back).expect("re-serialize"), bytes);
    });
}

#[test]
fn a_sensible_environment_is_valid() {
    check!().with_type::<Scenario>().for_each(|s| {
        assert!(build(s).is_valid(), "{:?}", build(s));
    });
}

#[derive(Debug, TypeGenerator)]
enum Fault {
    NoDirection,
    NegativeColour,
    NonFiniteAmbient,
    ZeroExposure,
    ShadowWithoutDirection,
    ShadowStrengthOutOfRange,
}

#[test]
fn a_broken_environment_is_refused() {
    check!().with_type::<(Scenario, Fault)>().for_each(|(s, fault)| {
        let mut e = build(s);
        if e.lights.is_empty() {
            e.lights.push(SunLight { color: [1.0; 3], direction: [0.0, -1.0, 0.0] });
        }
        match fault {
            Fault::NoDirection => e.lights[0].direction = [0.0; 3],
            Fault::NegativeColour => e.lights[0].color[1] = -0.5,
            Fault::NonFiniteAmbient => e.ambient[2] = f32::NAN,
            Fault::ZeroExposure => e.exposure = 0.0,
            Fault::ShadowWithoutDirection => {
                e.shadow = Some(Shadow { direction: [0.0; 3], strength: 1.0 });
            }
            Fault::ShadowStrengthOutOfRange => {
                e.shadow = Some(Shadow { direction: [0.0, -1.0, 0.0], strength: 1.5 });
            }
        }
        assert!(!e.is_valid(), "{fault:?} accepted: {e:?}");
    });
}

#[test]
fn toward_is_unit_and_keeps_the_direction() {
    check!().with_type::<[i8; 3]>().for_each(|d| {
        let v = dir(*d);
        let light = SunLight { color: [1.0; 3], direction: v };
        let u = light.toward();
        let len = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-5, "{u:?}");
        let dot = u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
        assert!(dot > 0.0, "{u:?} turned away from {v:?}");
    });
}
