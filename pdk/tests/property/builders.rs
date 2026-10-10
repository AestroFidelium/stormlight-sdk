//! Descriptors spelled short (stormlight/server#197).
//!
//! A builder is only a spelling: whatever it is told, it must produce exactly the
//! descriptor a mod would have written out by hand. Laws, over any combination of
//! refinements, for every builder:
//!   - **the output equals the fully spelled-out descriptor**, field for field —
//!     what was refined as refined, everything else at the default the builder's
//!     constructor names;
//!   - **refinements that add to a list keep their order**, since a payload runs
//!     in the order it was written.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::abilities::{AbilityDescriptor, CastSpec, Cost, Params, Targeting};
use stormlight_mod_sdk::abi::behaviors::{
    BuffSpec, ModOp, Modifier, Reapply, StackScope, Stacking,
};
use stormlight_mod_sdk::abi::common::{Affiliation, ImpactTarget, TargetFilter};
use stormlight_mod_sdk::abi::conditions::Condition;
use stormlight_mod_sdk::abi::ids::{
    AbilityId, BuffId, DamageTypeId, ParamId, ResourceId, Slot, StatId, TagId, UnitId,
};
use stormlight_mod_sdk::abi::impacts::{DamageFlags, Impact, SpawnAnchor, SpawnPattern};
use stormlight_mod_sdk::abi::math::Value;
use stormlight_mod_sdk::abi::missiles::{BodyDescriptor, BodyFlags, BodyKind, CollisionSpec};
use stormlight_mod_sdk::abi::units::{ResourcePool, UnitDescriptor};
use stormlight_mod_sdk::builders::{
    ability, allies, apply, buff, damage, enemies, everyone, missile, spawn, unit,
};

fn tenth(v: u8) -> f32 {
    f32::from(v) / 10.0
}

fn k(v: u8) -> Value {
    Value::Const(tenth(v))
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Side {
    Enemies,
    Allies,
    Everyone,
}

impl Side {
    fn affiliation(self) -> Affiliation {
        match self {
            Self::Enemies => Affiliation::Enemies,
            Self::Allies => Affiliation::Allies,
            Self::Everyone => Affiliation::All,
        }
    }

    fn filter(self) -> TargetFilter {
        match self {
            Self::Enemies => enemies(),
            Self::Allies => allies(),
            Self::Everyone => everyone(),
        }
    }
}

/// A damage impact and how it was refined.
#[derive(Debug, Clone, Copy, TypeGenerator)]
struct Hit {
    amount: u8,
    #[generator(0u16..=4)]
    dtype: u16,
    to_caster: bool,
}

impl Hit {
    fn built(self) -> Impact {
        let spec = damage(tenth(self.amount), DamageTypeId(self.dtype));
        if self.to_caster { spec.to(ImpactTarget::Caster).into() } else { spec.into() }
    }

    fn spelled(self) -> Impact {
        Impact::Damage {
            amount: k(self.amount),
            dtype: DamageTypeId(self.dtype),
            target: if self.to_caster {
                ImpactTarget::Caster
            } else {
                ImpactTarget::ResolvedTarget
            },
            flags: DamageFlags::default(),
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Missile {
    speed: u8,
    range: u8,
    side: Option<Side>,
    height: Option<u8>,
    pierce: Option<u8>,
    homing: bool,
    through_walls: bool,
    #[generator(bolero::produce::<Vec<Hit>>().with().len(0usize..=3))]
    hits: Vec<Hit>,
}

impl Missile {
    fn built(&self) -> BodyDescriptor {
        let mut b = missile(tenth(self.speed), tenth(self.range));
        if let Some(side) = self.side {
            b = b.hits(side.filter());
        }
        if let Some(h) = self.height {
            b = b.height(tenth(h));
        }
        if let Some(p) = self.pierce {
            b = b.pierce(tenth(p));
        }
        if self.homing {
            b = b.homing();
        }
        if self.through_walls {
            b = b.through_walls();
        }
        for hit in &self.hits {
            b = b.on_hit(hit.built());
        }
        b.into()
    }

    fn spelled(&self) -> BodyDescriptor {
        let pierce = self.pierce.map_or(Value::Const(0.0), k);
        BodyDescriptor {
            kind: BodyKind::Missile {
                speed: k(self.speed),
                range: k(self.range),
                homing: self.homing,
                pierce: pierce.clone(),
            },
            on_spawn: Vec::new(),
            on_hit: self.hits.iter().map(|h| h.spelled()).collect(),
            on_expire: Vec::new(),
            collision: CollisionSpec {
                filter: TargetFilter {
                    affiliation: self.side.map_or(Affiliation::Enemies, Side::affiliation),
                    require_tags: Vec::new(),
                    exclude_tags: Vec::new(),
                    include_dead: false,
                },
                pierce,
                through_walls: self.through_walls,
            },
            flags: BodyFlags::default(),
            height: self.height.map_or(Value::Const(0.0), k),
        }
    }
}

#[test]
fn a_missile_is_the_body_spelled_out() {
    check!().with_type::<Missile>().for_each(|m| {
        assert_eq!(m.built(), m.spelled(), "{m:?}");
    });
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Aim {
    NoTarget,
    Point,
    Vector,
    SelfCast,
    Unit(Side),
}

impl Aim {
    fn targeting(self) -> Targeting {
        match self {
            Self::NoTarget => Targeting::NoTarget,
            Self::Point => Targeting::Point,
            Self::Vector => Targeting::Vector,
            Self::SelfCast => Targeting::SelfCast,
            Self::Unit(side) => Targeting::Unit { filter: side.filter() },
        }
    }
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Window {
    Instant,
    Cast(u8),
    MovableCast(u8),
    Channel(u8, Option<u8>),
    Passive,
}

#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Price {
    Resource(#[generator(0u16..=3)] u16, u8),
    Health(u8),
    Charge,
}

/// What an ability does on cast: shoot a missile, or put a buff on someone.
#[derive(Debug, Clone, Copy, TypeGenerator)]
enum Effect {
    Shoot {
        speed: u8,
        range: u8,
        several: Option<u8>,
    },
    Apply {
        #[generator(0u16..=3)]
        buff: u16,
        stacks: Option<u8>,
        lasting: Option<u8>,
        caster: bool,
    },
}

impl Effect {
    fn built(self) -> Impact {
        match self {
            Self::Shoot { speed, range, several } => {
                let s = spawn(missile(tenth(speed), tenth(range)));
                match several {
                    Some(n) => s.several(tenth(n), SpawnPattern::Single).into(),
                    None => s.into(),
                }
            }
            Self::Apply { buff, stacks, lasting, caster } => {
                let mut a = apply(BuffId(buff));
                if let Some(n) = stacks {
                    a = a.stacks(tenth(n));
                }
                if let Some(s) = lasting {
                    a = a.lasting(tenth(s));
                }
                if caster {
                    a = a.to_caster();
                }
                a.into()
            }
        }
    }

    fn spelled(self) -> Impact {
        match self {
            Self::Shoot { speed, range, several } => Impact::Spawn {
                body: Missile {
                    speed,
                    range,
                    side: None,
                    height: None,
                    pierce: None,
                    homing: false,
                    through_walls: false,
                    hits: Vec::new(),
                }
                .spelled(),
                at: SpawnAnchor::Caster,
                count: several.map_or(Value::Const(1.0), k),
                pattern: SpawnPattern::Single,
            },
            Self::Apply { buff, stacks, lasting, caster } => Impact::ApplyModifiers {
                buff: BuffId(buff),
                stacks: stacks.map_or(Value::Const(1.0), k),
                duration_override: lasting.map(k),
                target: if caster { ImpactTarget::Caster } else { ImpactTarget::ResolvedTarget },
            },
        }
    }
}

#[derive(Debug, TypeGenerator)]
struct Ability {
    aim: Aim,
    window: Window,
    #[generator(bolero::produce::<Vec<(u16, u8)>>().with().len(0usize..=2))]
    params: Vec<(u16, u8)>,
    #[generator(bolero::produce::<Vec<Price>>().with().len(0usize..=2))]
    prices: Vec<Price>,
    #[generator(bolero::produce::<Vec<Effect>>().with().len(0usize..=3))]
    effects: Vec<Effect>,
    #[generator(bolero::produce::<Vec<Effect>>().with().len(0usize..=1))]
    on_start: Vec<Effect>,
    #[generator(bolero::produce::<Vec<u16>>().with().len(0usize..=2))]
    tags: Vec<u16>,
}

impl Ability {
    fn built(&self) -> AbilityDescriptor {
        let mut a = ability(self.aim.targeting());
        for &(p, v) in &self.params {
            a = a.param(ParamId(p), tenth(v));
        }
        a = match self.window {
            Window::Instant => a,
            Window::Cast(t) => a.cast(tenth(t)),
            Window::MovableCast(t) => a.movable_cast(tenth(t)),
            Window::Channel(t, tick) => a.channel(tenth(t), tick.map(tenth)),
            Window::Passive => a.passive(),
        };
        for price in &self.prices {
            a = match *price {
                Price::Resource(r, v) => a.cost(ResourceId(r), tenth(v)),
                Price::Health(v) => a.costs_health(tenth(v)),
                Price::Charge => a.costs_charge(),
            };
        }
        for e in &self.on_start {
            a = a.on_cast_start(e.built());
        }
        for e in &self.effects {
            a = a.on_cast(e.built());
        }
        for &t in &self.tags {
            a = a.tag(TagId(t));
        }
        a.into()
    }

    fn spelled(&self) -> AbilityDescriptor {
        AbilityDescriptor {
            id: AbilityId(0),
            params: Params(self.params.iter().map(|&(p, v)| (ParamId(p), k(v))).collect()),
            targeting: self.aim.targeting(),
            cast: match self.window {
                Window::Instant => CastSpec::Instant,
                Window::Cast(t) => CastSpec::Cast { time: k(t), movable: false },
                Window::MovableCast(t) => CastSpec::Cast { time: k(t), movable: true },
                Window::Channel(t, tick) => {
                    CastSpec::Channel { time: k(t), movable: false, tick: tick.map(k) }
                }
                Window::Passive => CastSpec::Passive,
            },
            cost: self
                .prices
                .iter()
                .map(|p| match *p {
                    Price::Resource(r, v) => Cost::Resource { res: ResourceId(r), amount: k(v) },
                    Price::Health(v) => Cost::Health { amount: k(v) },
                    Price::Charge => Cost::Charge,
                })
                .collect(),
            cast_gate: Condition::Always,
            on_cast_start: self.on_start.iter().map(|e| e.spelled()).collect(),
            on_cast: self.effects.iter().map(|e| e.spelled()).collect(),
            tags: self.tags.iter().map(|&t| TagId(t)).collect(),
        }
    }
}

#[test]
fn an_ability_is_the_descriptor_spelled_out() {
    check!().with_type::<Ability>().for_each(|a| {
        assert_eq!(a.built(), a.spelled(), "{a:?}");
    });
}

#[derive(Debug, TypeGenerator)]
struct Buff {
    lasting: Option<u8>,
    #[generator(bolero::produce::<Vec<(u16, u8)>>().with().len(0usize..=3))]
    adds: Vec<(u16, u8)>,
    stacking: Option<(u8, bool)>,
    survives_death: bool,
    #[generator(bolero::produce::<Vec<u16>>().with().len(0usize..=2))]
    tags: Vec<u16>,
}

impl Buff {
    fn built(&self) -> BuffSpec {
        let mut b = buff();
        if let Some(s) = self.lasting {
            b = b.lasting(tenth(s));
        }
        for &(stat, v) in &self.adds {
            b = b.modifier(StatId(stat), ModOp::AddFlat, tenth(v));
        }
        if let Some((max, independent)) = self.stacking {
            let reapply = if independent { Reapply::Independent } else { Reapply::AddDuration };
            b = b.stacking(u16::from(max), reapply, StackScope::PerSource);
        }
        if self.survives_death {
            b = b.survives_death();
        }
        for &t in &self.tags {
            b = b.tag(TagId(t));
        }
        b.into()
    }

    fn spelled(&self) -> BuffSpec {
        let (max_stacks, stacking) = match self.stacking {
            Some((max, independent)) => (
                u16::from(max),
                Stacking {
                    on_reapply: if independent {
                        Reapply::Independent
                    } else {
                        Reapply::AddDuration
                    },
                    scope: StackScope::PerSource,
                },
            ),
            None => {
                (1, Stacking { on_reapply: Reapply::RefreshDuration, scope: StackScope::Global })
            }
        };
        BuffSpec {
            id: BuffId(0),
            duration: self.lasting.map(k),
            stacking,
            max_stacks,
            modifiers: self
                .adds
                .iter()
                .map(|&(stat, v)| Modifier { stat: StatId(stat), op: ModOp::AddFlat, value: k(v) })
                .collect(),
            tags: self.tags.iter().map(|&t| TagId(t)).collect(),
            reactions: Vec::new(),
            on_apply: Vec::new(),
            on_expire: Vec::new(),
            on_remove: Vec::new(),
            drop_on_death: !self.survives_death,
        }
    }
}

#[test]
fn a_buff_is_the_spec_spelled_out() {
    check!().with_type::<Buff>().for_each(|b| {
        assert_eq!(b.built(), b.spelled(), "{b:?}");
    });
}

#[derive(Debug, TypeGenerator)]
struct Unit {
    health: u8,
    #[generator(bolero::produce::<Vec<(u16, u8)>>().with().len(0usize..=3))]
    stats: Vec<(u16, u8)>,
    #[generator(bolero::produce::<Vec<u16>>().with().len(0usize..=2))]
    tags: Vec<u16>,
    #[generator(bolero::produce::<Vec<(u8, u16)>>().with().len(0usize..=3))]
    abilities: Vec<(u8, u16)>,
    #[generator(bolero::produce::<Vec<(u16, u8, u8)>>().with().len(0usize..=2))]
    pools: Vec<(u16, u8, u8)>,
    turn_rate: Option<u8>,
    #[generator(bolero::produce::<Vec<u8>>().with().len(0usize..=2))]
    grant_slots: Vec<u8>,
}

impl Unit {
    fn built(&self) -> UnitDescriptor {
        let mut u = unit(tenth(self.health));
        for &(s, v) in &self.stats {
            u = u.stat(StatId(s), tenth(v));
        }
        for &t in &self.tags {
            u = u.tag(TagId(t));
        }
        for &(slot, a) in &self.abilities {
            u = u.ability(Slot(slot), AbilityId(u32::from(a)));
        }
        for &(r, max, regen) in &self.pools {
            u = u.pool(ResourceId(r), tenth(max), tenth(regen));
        }
        if let Some(t) = self.turn_rate {
            u = u.turn_rate(tenth(t));
        }
        for &slot in &self.grant_slots {
            u = u.grant_slot(Slot(slot));
        }
        u.into()
    }

    fn spelled(&self) -> UnitDescriptor {
        UnitDescriptor {
            id: UnitId(0),
            health: k(self.health),
            stats: self.stats.iter().map(|&(s, v)| (StatId(s), k(v))).collect(),
            tags: self.tags.iter().map(|&t| TagId(t)).collect(),
            abilities: self
                .abilities
                .iter()
                .map(|&(s, a)| (Slot(s), AbilityId(u32::from(a))))
                .collect(),
            resources: self
                .pools
                .iter()
                .map(|&(r, max, regen)| ResourcePool {
                    id: ResourceId(r),
                    max: k(max),
                    regen: k(regen),
                })
                .collect(),
            talents: Vec::new(),
            talent_tree: None,
            respawn: None,
            progression: None,
            turn_rate: self.turn_rate.map(k),
            tasks: Vec::new(),
            grant_slots: self.grant_slots.iter().map(|&s| Slot(s)).collect(),
            attack: None,
        }
    }
}

#[test]
fn a_unit_is_the_descriptor_spelled_out() {
    check!().with_type::<Unit>().for_each(|u| {
        assert_eq!(u.built(), u.spelled(), "{u:?}");
    });
}

#[test]
fn a_hit_is_the_impact_spelled_out() {
    check!().with_type::<Hit>().for_each(|h| assert_eq!(h.built(), h.spelled(), "{h:?}"));
}
