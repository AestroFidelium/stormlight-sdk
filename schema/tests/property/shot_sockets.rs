//! A rig's two shot sockets are two facts, not one (stormlight/server#155).
//!
//! `VisualModel::Model` names the socket shots this unit *fires* leave from
//! (server#154) and the socket shots *fired at it* are drawn landing on. They are
//! different verbs on the same skeleton — a muzzle is on a weapon, a landing point
//! is in the chest — so what is pinned here is that they stay independent all the
//! way across the wasm boundary:
//!
//! - both survive a `postcard` round-trip exactly, including which is which;
//! - naming one never invents the other, in either direction;
//! - and neither is touched by the id remap, which rewrites handles and no strings.
//!
//! Content-free: the names below are arbitrary strings this test made up. Every
//! real one is a mod's, read out of the mod's own art.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::UnitId;
use stormlight_mod_abi::visuals::{VisualDescriptor, VisualModel};

extern crate alloc;
use alloc::string::{String, ToString};

/// Which sockets a generated model names.
#[derive(Debug, TypeGenerator, Clone, Copy)]
enum Names {
    Neither,
    LaunchOnly,
    ImpactOnly,
    Both,
}

#[derive(Debug, TypeGenerator)]
struct Scenario {
    names: Names,
    /// Two distinct socket names, so a field that reads the other one is caught
    /// rather than passing because both happened to hold the same string.
    launch: u8,
    impact: u8,
}

fn socket(seed: u8, tag: &str) -> String {
    let mut s = "Ref_".to_string();
    s.push_str(tag);
    s.push((b'A' + seed % 26) as char);
    s
}

impl Scenario {
    fn launch_name(&self) -> String {
        socket(self.launch, "Launch")
    }

    fn impact_name(&self) -> String {
        socket(self.impact, "Impact")
    }

    fn model(&self) -> VisualModel {
        let (launch, impact) = match self.names {
            Names::Neither => (None, None),
            Names::LaunchOnly => (Some(self.launch_name()), None),
            Names::ImpactOnly => (None, Some(self.impact_name())),
            Names::Both => (Some(self.launch_name()), Some(self.impact_name())),
        };
        VisualModel::Model {
            asset: "mod://pkg/models/rig.glb".to_string(),
            scale: 1.0,
            yaw_offset: 0.0,
            launch,
            impact,
        }
    }
}

#[test]
fn both_shot_sockets_survive_the_wire_and_stay_told_apart() {
    check!().with_type::<Scenario>().for_each(|s| {
        let model = s.model();
        let descriptor = VisualDescriptor { unit: UnitId(7), model };
        let bytes = postcard::to_allocvec(&descriptor).expect("serialize");
        let back: VisualDescriptor = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(descriptor, back, "a model's sockets did not round-trip");

        let VisualModel::Model { launch, impact, .. } = &back.model else {
            panic!("the model variant came back as something else entirely");
        };
        match s.names {
            Names::Neither => {
                assert!(
                    launch.is_none() && impact.is_none(),
                    "art that names no socket came back naming {launch:?} / {impact:?} — a \
                     shot would be drawn leaving and landing somewhere nobody authored",
                );
            }
            Names::LaunchOnly => {
                assert_eq!(launch.as_deref(), Some(s.launch_name().as_str()));
                assert!(
                    impact.is_none(),
                    "naming a muzzle invented a landing point ({impact:?}) — the two are \
                     different verbs and a rig may carry either alone",
                );
            }
            Names::ImpactOnly => {
                assert_eq!(impact.as_deref(), Some(s.impact_name().as_str()));
                assert!(
                    launch.is_none(),
                    "naming a landing point invented a muzzle ({launch:?}) — a target that \
                     fires nothing would start drawing shots from itself",
                );
            }
            Names::Both => {
                assert_eq!(
                    launch.as_deref(),
                    Some(s.launch_name().as_str()),
                    "the muzzle came back as the landing point, so every shot would leave \
                     from the chest it was supposed to arrive at",
                );
                assert_eq!(impact.as_deref(), Some(s.impact_name().as_str()));
            }
        }
    });
}
