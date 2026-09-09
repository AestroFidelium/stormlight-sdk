//! The client cosmetic ABI — how a `*_client` mod tells the content-free client
//! to *draw* a unit.
//!
//! The gameplay ABI (`Registration`, `descriptors.rs`) is content the **server**
//! interprets; this is the parallel bundle the **client** interprets. A cosmetic
//! mod emits a [`ClientRegistration`]: a set of [`VisualDescriptor`]s, each keyed
//! by a unit's interned handle, saying how an entity spawned from that unit looks.
//! The engine still ships nothing — it renders whatever asset the descriptor
//! names via a `mod://<id>/<path>` URL, and a [`VisualModel::Primitive`] is the
//! asset-free fallback an unknown or missing model degrades to.
//!
//! Pure, serializable data like the rest of the ABI. Handles are authored in the
//! mod's **local** id space and remapped to global at adoption (see
//! [`crate::remap`]); the [`crate::descriptors::Names`] `units` table gives each
//! local unit handle the stable name the host maps to the gameplay unit.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::animation::AnimationDescriptor;
use crate::descriptors::Names;
use crate::ids::{AbilityId, TalentId, UnitId};
use crate::manifest::Version;
use crate::ui::UiRoot;

/// A procedural primitive the client can draw with no asset. The graceful
/// fallback a cosmetic mod always has, and what a missing/unknown model degrades
/// to so the field is still *seen* rather than blank.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum PrimitiveShape {
    Cube,
    Sphere,
    Capsule,
}

/// How a unit is drawn. Generic: the engine knows "a tinted primitive", "a 3D
/// model", or "a flat sprite"; what the asset *is* is mod content, addressed by a
/// `mod://<id>/<path>` URL the host resolves within the package root.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum VisualModel {
    /// A procedural primitive tinted `color` (linear RGBA, `0.0..=1.0`). No asset.
    Primitive { shape: PrimitiveShape, color: [f32; 4] },
    /// A 3D model/scene loaded from a `mod://` asset, uniformly scaled, and turned
    /// about `+Y` by `yaw_offset` radians within the unit it dresses.
    ///
    /// The engine faces a unit along its travel direction with **`-Z` forward**
    /// (Bevy's and glTF's own convention), so art authored that way needs no offset
    /// at all — `0.0` is the ordinary case. `yaw_offset` is the escape hatch for art
    /// that faces some other axis: rather than the engine guessing, or the author
    /// re-exporting the asset, the mod states which way its own model looks. A
    /// quarter turn is `FRAC_PI_2`.
    ///
    /// `launch` names the **attachment point** shots this unit fires are drawn
    /// leaving from — an authored socket in the model's own skeleton, such as a
    /// weapon or a hand (stormlight/server#154). It sits on `Model` and on no other
    /// variant because it is the only one with a skeleton to have sockets in.
    ///
    /// It is a *drawing*, and only a drawing: the shot's authoritative origin is
    /// the server's, and no socket ever moves a hitbox or a range. `None`, or a
    /// name the art does not carry, falls back to that origin — which is what every
    /// shot did before sockets, so the fallback is the old behaviour rather than a
    /// broken one.
    ///
    /// `impact` is the other end of the same flight (stormlight/server#155): the
    /// attachment point a shot **aimed at this unit** is drawn arriving on — the
    /// place on the body the art authored for being hit, rather than the ground
    /// point the simulation tracks the unit by. Without it a drawn shot keeps the
    /// authoritative velocity and so flies *parallel* to the real one, passing over
    /// or under the model by however far the muzzle sits from the shooter's own
    /// anchor.
    ///
    /// Two fields because they are two verbs: a rig may name where its shots leave
    /// and never be shot at, or be a target that fires nothing at all. Both are
    /// drawings and neither is ever asked about a hitbox; a missing one leaves the
    /// server's flight exactly as it was.
    ///
    /// `clips` is what the art plays on its own (stormlight/server#158): effect
    /// containers ship a birth / live / death cycle rather than a state machine,
    /// and a container mounted without one sits at its **bind pose** — which for a
    /// missile whose first frame scales it up from half size means a shot half the
    /// size it was drawn, with none of its emitters moving. Empty (the default) is
    /// "this art plays nothing", which is what a unit's model wants: a unit is
    /// animated by its [`AnimationDescriptor`] instead.
    Model {
        asset: String,
        scale: f32,
        yaw_offset: f32,
        launch: Option<String>,
        impact: Option<String>,
        #[serde(default)]
        clips: ModelClips,
    },
    /// A flat, billboarded sprite from a `mod://` asset, sized in world units.
    Sprite { asset: String, size: [f32; 2] },
}

/// What a mounted model plays **on its own** — the lifecycle a piece of art has
/// when nothing else is driving it.
///
/// A *unit* is animated by an [`AnimationDescriptor`]: layers, masks, and states
/// chosen every frame from what the unit is doing. This is the other case, and it is
/// most of the art in a cosmetic package — an effect container with no state machine
/// and exactly one thing to do: appear, then keep going for as long as it lasts.
///
/// Both fields name a clip **inside the same container** the model does, exactly
/// like a [`ClipRef`](crate::animation::ClipRef)'s `clip`. Empty means "nothing to
/// play", which is the ordinary case for a unit's model and for a piece of art whose
/// first frame is already the whole of it.
///
/// There is deliberately no "and then it goes away" clip yet: nothing in the engine
/// defers a despawn to wait for one, and a field that reads well and never fires is
/// worse than an absent one.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct ModelClips {
    /// Played **once** as the model appears, then handing over to `live`. Empty:
    /// the model starts live.
    pub birth: String,
    /// Looped for as long as the model exists. Empty: whatever `birth` left behind
    /// is held, or the bind pose if there was no birth either.
    pub live: String,
}

/// The cosmetic descriptor a client mod attaches to a unit: how the client draws
/// an entity spawned from that unit. Keyed by the unit's interned handle (local
/// at authoring, global after adoption).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct VisualDescriptor {
    /// The unit this visual is for.
    pub unit: UnitId,
    /// How to draw it.
    pub model: VisualModel,
}

/// Which piece of ability feedback an [`EffectVisualDescriptor`] dresses. Generic:
/// the engine knows "a flying missile", "a one-shot burst where a shot lands", or
/// "an in-progress cast/channel indicator" — never what ability or hero it is for.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum EffectRole {
    /// The body drawn for a locally-simulated projectile the ability launches.
    Projectile,
    /// The transient burst played where an ability's shot authoritatively lands.
    Impact,
    /// The indicator shown on the caster while a timed cast / channel is running.
    CastIndicator,
}

/// The cosmetic descriptor a client mod attaches to an *ability's feedback*: how
/// the client draws that ability's projectile, impact, or cast indicator. The
/// parallel to [`VisualDescriptor`] (which dresses a *unit*), keyed by the
/// ability's interned handle (local at authoring, global after adoption) plus the
/// [`EffectRole`] it fills, so one ability can declare a visual per role.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct EffectVisualDescriptor {
    /// The ability whose feedback this visual is for.
    pub ability: AbilityId,
    /// Which piece of that ability's feedback it dresses.
    pub role: EffectRole,
    /// How to draw it.
    pub model: VisualModel,
}

/// The picture an ability wears in an interface — the icon a HUD draws in
/// whichever slot that ability occupies (stormlight/server#94).
///
/// Keyed by the ability's interned handle for the same reason
/// [`EffectVisualDescriptor`] is: the picture is a fact about the *ability*, and
/// the mod that draws the ability bar is not the mod that authored the ability.
/// An interface mod ships widget trees and dresses no unit — it cannot know which
/// ability a unit binds in slot 2, so the only thing it can supply for that slot
/// is the socket it draws round every one of them. The engine resolves the icon
/// per slot from whatever the bound ability declared, leaving
/// [`Style::image`](crate::ui::Style::image) on the widget as what a slot wears
/// when its ability supplies no picture — or when nothing is bound to it at all.
///
/// A bare `mod://<id>/<path>` URL rather than a [`VisualModel`]: an icon is a flat
/// picture in a widget, so a mesh, a scale and a yaw offset would all be fields
/// with nothing to mean. How it is *drawn* — its tint, its nine-slice, whether it
/// flips — stays the interface's, declared once on the socket and applied to
/// whatever picture lands in it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AbilityIcon {
    /// The ability this picture is for.
    pub ability: AbilityId,
    /// The `mod://<id>/<path>` URL of the picture.
    pub image: String,
}

/// The flat picture a unit wears in an interface (stormlight/server#145) — the
/// portrait a roster row, a nameplate or a hero panel draws.
///
/// Keyed by the unit's interned handle for the same reason [`AbilityIcon`] is keyed
/// by the ability's, and it is the same separation: the mod that lays out a top bar
/// is not the mod that authored the heroes. A roster row is instanced per *player*,
/// so the interface cannot name the picture — it does not know who picked what, and
/// the whole point of the row is that it is drawn for a player whose unit this
/// client may never receive. The picture travels with the unit and is resolved from
/// the roster's own unit id.
///
/// Distinct from [`VisualDescriptor`], and not derivable from it: that says how the
/// unit is *built in the world* — a mesh, a scale, a yaw — and there is no way to
/// turn a model into a portrait without rendering it. A portrait is authored art.
///
/// A bare `mod://<id>/<path>` URL, exactly like [`AbilityIcon`]'s, for exactly the
/// same reason: it is a flat picture in a widget, and how it is *drawn* — tint,
/// nine-slice, mask, whether it flips — stays the interface's, declared once on the
/// socket and applied to whatever picture lands in it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UnitIcon {
    /// The unit this picture is for.
    pub unit: UnitId,
    /// The `mod://<id>/<path>` URL of the picture.
    pub image: String,
}

/// What a talent is called, what it does in words, and what it looks like
/// (stormlight/server#95) — everything a panel offering a pending tier needs in
/// order to say what the choice *means*.
///
/// Keyed by the talent's interned handle for the same reason [`AbilityIcon`] is
/// keyed by the ability's, and it is the same separation one level along: the mod
/// that lays out the talent panel is not the mod that authored the talents. A
/// panel addresses a cell by **tier and option index** ([`UiAction::PickTalent`](crate::ui::UiAction::PickTalent)),
/// so it can offer a whole tree while naming no content — but that also means it
/// has nothing to print in the cell. The card travels with the talent, and the
/// panel asks for it by coordinate.
///
/// Cosmetic rather than a field of
/// [`TalentDescriptor`](crate::talents::TalentDescriptor): these are words on a
/// screen and a picture in a widget, and the server neither reads nor replicates
/// any of them. The gameplay side keeps naming its talents with the stable
/// identifier it interns them by, which stays what a card-less talent prints.
///
/// **One language.** The strings are authored literals, so a mod ships the words of
/// whichever language it was written in — see the localization work this is the
/// forcing case for (stormlight/server#101).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TalentCard {
    /// The talent this card describes.
    pub talent: TalentId,
    /// What to show for it.
    pub info: TalentInfo,
}

/// The presentation half of a [`TalentCard`] — what a talent is called, what it
/// does in words, and what it looks like, with no handle attached.
///
/// Carried on its own so that every table downstream is keyed by the id the *host*
/// resolved the declaration to, and holds no copy of the local handle the mod
/// authored it with. A stale handle sitting inside an adopted value is how a card
/// ends up describing the neighbouring package's talent.
///
/// Every field may be empty, and each means the same thing on its own: nothing to
/// show. A talent with a name and no description is a cell with a heading; a talent
/// with no picture leaves the socket wearing whatever the interface declared for an
/// empty one.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct TalentInfo {
    /// Its display name — what a cell's heading prints. Empty falls back to the
    /// identifier the gameplay mod interned it under.
    pub name: String,
    /// What it does, in the mod's own words. Empty prints nothing.
    pub description: String,
    /// The `mod://<id>/<path>` URL of its picture. Empty wears no picture.
    pub image: String,
}

/// A cosmetic effect declared under a name of the mod's own choosing, for
/// anything that spawns a visual without an ability behind it — today, an
/// animation notify (server#76): a footstep puff, a weapon trail, a landing
/// cloud.
///
/// Name-keyed rather than handle-keyed because the key never crosses the wire and
/// never leaves the mod that declared it: it is resolved against that mod's own
/// declarations, exactly like a [`ClipRef`](crate::animation::ClipRef) naming a
/// clip inside a container. The host qualifies the name with the declaring
/// package at adoption, so two mods may each have a `footstep`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct NamedEffect {
    /// The mod's own name for it, as a notify's
    /// [`key`](crate::notify::NotifyAction::Effect) spells it.
    pub name: String,
    /// How to draw it.
    pub model: VisualModel,
}

/// Everything a *client* (cosmetic) mod registers — the client-side parallel to
/// [`crate::descriptors::Registration`]. Emitted once at `mod_register` on the
/// client's wasm runtime and decoded by the client host.
///
/// `visuals` and `animations` are indexed by the mod's *local* unit handle;
/// `effects` key on a local ability handle plus a role. The [`Names`]
/// `units`/`abilities` tables give each referenced handle a stable name the host
/// maps to the gameplay mod's global id, and `anim_states` names the mod-defined
/// animation states its animations declare. Embeds the
/// [`crate::manifest::ABI_VERSION`] it was built against so the host can reject a
/// major mismatch at decode.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct ClientRegistration {
    /// The ABI the mod was built against; the host rejects a major mismatch.
    pub abi: Version,
    /// Stable names for every interned handle the visuals reference.
    pub names: Names,
    /// The visuals this cosmetic mod declares, one per unit it dresses.
    pub visuals: Vec<VisualDescriptor>,
    /// The ability-feedback visuals this cosmetic mod declares (projectile /
    /// impact / cast indicator), keyed by ability + role.
    pub effects: Vec<EffectVisualDescriptor>,
    /// The icons this cosmetic mod declares, one per ability it gives a picture
    /// to (server#94). Read by an interface it knows nothing about: the icon
    /// travels with the ability so a HUD can ask for it *by slot*.
    pub icons: Vec<AbilityIcon>,
    /// The animations this cosmetic mod declares, one per unit it animates
    /// (stormlight/server#72). Independent of `visuals`: a mod may dress a unit
    /// without animating it, or animate a unit whose model another mod supplied.
    pub animations: Vec<AnimationDescriptor>,
    /// The name-keyed cosmetic effects this mod declares (server#76), which its
    /// animation notifies spawn by name. Keyed within this mod alone — the host
    /// qualifies each name with the declaring package at adoption.
    pub named_effects: Vec<NamedEffect>,
    /// The widget trees this mod declares (server#66) — the interface the
    /// content-free client presents. Independent of everything above: a mod may
    /// ship a HUD and dress nothing, or dress a unit and ship no HUD.
    pub ui: Vec<UiRoot>,
    /// What each talent is called, does and looks like (server#95), one per talent
    /// this mod cards. Read by a panel that knows no talent's name: the card
    /// travels with the talent so the panel can ask for it *by tier and option*.
    #[serde(default)]
    pub cards: Vec<TalentCard>,
    /// The portraits this mod declares, one per unit it gives a flat picture to
    /// (server#145). Read by an interface that knows no hero's name: the picture
    /// travels with the unit so a roster row can ask for it *by seat*.
    #[serde(default)]
    pub unit_icons: Vec<UnitIcon>,
}
