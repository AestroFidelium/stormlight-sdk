//! The mod manifest — the load-time contract a mod declares to the engine.
//!
//! Pure, serializable data like the rest of the ABI. A mod's staged folder (or
//! `.zip`) carries a `manifest.toml` whose `id` equals the folder name; assets
//! flatten to the package root and load via `mod://<id>/<path>`. The host reads
//! and validates this before it will instantiate the wasm entry.
//!
//! Parsing TOML is a host-only concern, gated behind the `manifest-parse`
//! feature so guest wasm mods (which only ever *emit*) stay free of `toml`.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The ABI version both the engine and every mod are built against. The host
/// rejects a mod whose declared `abi` major differs from this (see
/// [`Manifest::check_abi`]); minor/patch bumps stay backward compatible.
pub const ABI_VERSION: Version = Version::new(0, 1, 0);

/// A semantic version (`major.minor.patch`).
///
/// Kept as three integers with a string wire form so it is portable to guest
/// wasm without a semver-crate dependency, mirroring the ABI's dependency-free
/// stance elsewhere (e.g. `Point3 = [f32; 3]`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// Why a version string failed to parse (wrong part count or a non-numeric part).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VersionParseError;

impl Version {
    /// A version from explicit components.
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// Parse `"major.minor.patch"`. Total: any malformed string is an `Err`,
    /// never a panic. Exactly three non-negative integer components are required.
    pub fn parse(s: &str) -> Result<Self, VersionParseError> {
        let mut parts = s.split('.');
        let mut next =
            || parts.next().ok_or(VersionParseError)?.parse().map_err(|_| VersionParseError);
        let major = next()?;
        let minor = next()?;
        let patch = next()?;
        if parts.next().is_some() {
            return Err(VersionParseError);
        }
        Ok(Self { major, minor, patch })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

// A version is carried on the wire as its canonical string, so TOML manifests
// read naturally (`version = "1.2.3"`) and the postcard form stays stable.
impl Serialize for Version {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Version::parse(&s).map_err(|_| D::Error::custom("expected a \"major.minor.patch\" version"))
    }
}

/// Which side of the engine a mod runs on.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModKind {
    /// Authoritative gameplay content, loaded by the server.
    Server,
    /// Cosmetic-only content, loaded by the client's wasm runtime.
    Client,
}

/// A declared dependency on another mod, with the minimum version required.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Dependency {
    pub id: String,
    pub version: Version,
}

/// A mod's `manifest.toml`, deserialized.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Manifest {
    /// Stable lowercase id; equals the staged folder name and the `mod://` root.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// The mod's own semantic version.
    pub version: Version,
    /// Server-side gameplay or client-side cosmetic.
    pub kind: ModKind,
    /// Filename of the wasm entry within the package (e.g. `foo.wasm`).
    pub entry: String,
    /// The ABI version the mod was built against; gated on load.
    pub abi: Version,
    /// Other mods this one depends on. Absent in TOML means none.
    #[serde(default)]
    pub deps: Vec<Dependency>,
    /// Declared asset paths relative to the package root. Absent means none.
    #[serde(default)]
    pub assets: Vec<String>,
}

/// Why a manifest was rejected. Distinct variants so callers (and tests) can
/// react to the specific defect rather than an opaque failure.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ManifestError {
    /// `id` was empty.
    EmptyId,
    /// `id` (or a dependency id) had a character outside `[a-z0-9_]` or did not
    /// start with a lowercase letter.
    InvalidId,
    /// `name` was empty.
    EmptyName,
    /// `entry` was empty.
    EmptyEntry,
    /// `entry` was not a bare `*.wasm` filename (missing extension or a path).
    InvalidEntry,
    /// The mod's `abi` major differs from the engine's [`ABI_VERSION`].
    AbiMismatch { found: u32, expected: u32 },
    /// The TOML source could not be deserialized into a [`Manifest`].
    Toml(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => f.write_str("manifest id is empty"),
            Self::InvalidId => f.write_str("manifest id must be `[a-z][a-z0-9_]*`"),
            Self::EmptyName => f.write_str("manifest name is empty"),
            Self::EmptyEntry => f.write_str("manifest entry is empty"),
            Self::InvalidEntry => f.write_str("manifest entry must be a `*.wasm` filename"),
            Self::AbiMismatch { found, expected } => {
                write!(f, "abi major mismatch: mod={found}, engine={expected}")
            }
            Self::Toml(e) => write!(f, "manifest parse error: {e}"),
        }
    }
}

/// A well-formed id: non-empty, first char a lowercase ASCII letter, the rest
/// in `[a-z0-9_]`.
fn is_valid_id(id: &str) -> Result<(), ManifestError> {
    match id.chars().next() {
        None => return Err(ManifestError::EmptyId),
        Some(c) if !c.is_ascii_lowercase() => return Err(ManifestError::InvalidId),
        Some(_) => {}
    }
    if id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        Ok(())
    } else {
        Err(ManifestError::InvalidId)
    }
}

impl Manifest {
    /// Reject a mod whose declared ABI major differs from the engine's. Minor
    /// and patch differences are compatible and accepted.
    pub fn check_abi(&self) -> Result<(), ManifestError> {
        if self.abi.major == ABI_VERSION.major {
            Ok(())
        } else {
            Err(ManifestError::AbiMismatch { found: self.abi.major, expected: ABI_VERSION.major })
        }
    }

    /// Full structural validation: id charset, non-empty name/entry, a bare
    /// `*.wasm` entry, valid dependency ids, and the ABI gate. Semver validity
    /// of `version`/`abi` is already guaranteed by having parsed as [`Version`].
    pub fn validate(&self) -> Result<(), ManifestError> {
        is_valid_id(&self.id)?;
        if self.name.is_empty() {
            return Err(ManifestError::EmptyName);
        }
        if self.entry.is_empty() {
            return Err(ManifestError::EmptyEntry);
        }
        // A bare filename with a `.wasm` stem: no path separators, and something
        // before the extension.
        match self.entry.strip_suffix(".wasm").filter(|s| !s.is_empty()) {
            Some(s) if !s.contains('/') && !s.contains('\\') => {}
            _ => return Err(ManifestError::InvalidEntry),
        }
        for dep in &self.deps {
            is_valid_id(&dep.id)?;
        }
        self.check_abi()
    }
}

/// Parse and validate a `manifest.toml` source string. Host-only.
///
/// Total over arbitrary input: malformed TOML yields [`ManifestError::Toml`],
/// a structurally invalid manifest yields the specific validation error, and
/// nothing panics.
#[cfg(feature = "manifest-parse")]
pub fn parse_manifest(src: &str) -> Result<Manifest, ManifestError> {
    let manifest: Manifest =
        toml::from_str(src).map_err(|e| ManifestError::Toml(e.to_string()))?;
    manifest.validate()?;
    Ok(manifest)
}
