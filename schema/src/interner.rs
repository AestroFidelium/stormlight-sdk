//! A generic string interner: stable name -> dense [`Handle`], resolved once at
//! adoption so the running simulation never sees a string.
//!
//! One interner instance backs one id family (a `Interner<StatId>`, a
//! `Interner<TagId>`, …). Interning is idempotent and injective, and mints a
//! contiguous `0..len` index space so handles are array-indexable in hot loops.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::ids::Handle;

/// Maps stable names to compact [`Handle`]s of one family, and back.
pub struct Interner<H: Handle> {
    /// `raw index -> name`; also the source of truth for `len`/density.
    names: Vec<Box<str>>,
    /// `name -> raw index`, for idempotent interning and lookups.
    index: BTreeMap<Box<str>, u32>,
    _family: PhantomData<H>,
}

impl<H: Handle> Default for Interner<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Handle> Interner<H> {
    /// An empty interner.
    #[must_use]
    pub const fn new() -> Self {
        Self { names: Vec::new(), index: BTreeMap::new(), _family: PhantomData }
    }

    /// Intern `name`, returning its stable handle. Idempotent: the same name
    /// always maps to the same handle for the life of the interner.
    pub fn intern(&mut self, name: &str) -> H {
        if let Some(&raw) = self.index.get(name) {
            return H::from_raw(raw);
        }
        let raw = self.names.len() as u32;
        // The family must be able to represent this index; the interner is the
        // component that upholds `raw <= MAX_RAW`. Real adoption will surface an
        // overflow as a validation error (slice 15); here we assert the bound.
        debug_assert!(raw <= H::MAX_RAW, "interner exhausted this handle family");
        let name: Box<str> = name.into();
        self.names.push(name.clone());
        self.index.insert(name, raw);
        H::from_raw(raw)
    }

    /// The handle a name was interned as, if it has been interned.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<H> {
        self.index.get(name).map(|&raw| H::from_raw(raw))
    }

    /// The name behind a handle, if the handle came from this interner.
    #[must_use]
    pub fn resolve(&self, handle: H) -> Option<&str> {
        self.names.get(handle.raw() as usize).map(Box::as_ref)
    }

    /// Number of distinct interned names (also the exclusive upper bound of the
    /// contiguous handle index space).
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether nothing has been interned yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}
