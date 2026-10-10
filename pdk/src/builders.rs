//! Spelling descriptors without spelling out every field (stormlight/server#197).
//!
//! Every descriptor the schema carries is plain data with no defaults, which is
//! what makes adding a field a compile error in every mod rather than a silent
//! default. Writing them out in full is mostly `Vec::new()`, `None` and
//! `Condition::Always`, though, and a reader judges the API by that. So the
//! builders here take what a descriptor cannot do without in their constructor
//! and everything else as a chained refinement, and produce exactly the
//! descriptor a mod would have written by hand — the schema and the host never see
//! a builder.
//!
//! ```ignore
//! use stormlight_mod_sdk::builders::*;
//!
//! let bolt = missile(18.0, 12.0).height(1.0).hits(enemies()).on_hit(damage(40.0, arcane));
//! let spark = ability(Targeting::Vector).movable_cast(0.2).cost(energy, 25.0).on_cast(spawn(bolt));
//! ctx.ability("spark", spark);
//! ```
//!
//! What a builder leaves out is what a hand-written descriptor would most often
//! say anyway — no tags, no reactions, no talents, an ungated, instant, free cast —
//! and each such default is named on the builder that applies it.

pub mod ability;
pub mod body;
pub mod buff;
pub mod impact;
pub mod target;
pub mod unit;
pub mod value;

pub use ability::{AbilitySpec, ability};
pub use body::{BodySpec, missile};
pub use buff::{BuffBuilder, buff};
pub use impact::{ApplySpec, DamageSpec, SpawnSpec, apply, damage, spawn};
pub use target::{allies, enemies, everyone};
pub use unit::{UnitSpec, unit};
pub use value::IntoValue;
