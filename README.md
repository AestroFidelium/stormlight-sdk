# stormlight / sdk

The mod-author-facing SDK. **No engine code, no assets.**

- `schema/` → `stormlight_mod_abi`: the shared descriptor + manifest ABI — pure
  serializable data agreed on by both the engine and every mod. `no_std` by
  default; the engine enables `std`.
- `pdk/` → `stormlight_mod_sdk`: the guest PDK mods are written against —
  re-exports the schema, adds host bindings and registration helpers.

A Cargo workspace; the leaf of the dependency graph (depends on nothing in the
engine). See the umbrella `../CLAUDE.md`.
