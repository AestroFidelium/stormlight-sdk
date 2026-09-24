# stormlight / sdk

[![CI](https://github.com/AestroFidelium/stormlight-sdk/actions/workflows/ci.yml/badge.svg)](https://github.com/AestroFidelium/stormlight-sdk/actions/workflows/ci.yml)
![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)

**The contract between the Stormlight engine and the mods that supply all of
its content, plus the guest SDK mods are written against.**

The engine ships no content. Units, abilities, projectiles, status effects,
talents and interface layouts all arrive as serialisable descriptors that a
WebAssembly mod registers at load. This repository defines those descriptors,
and it depends on nothing in the engine.

> This repository is a read-only mirror of a private upstream, where development
> and planning happen. Bug reports and feedback are welcome as
> [issues](https://github.com/AestroFidelium/stormlight-sdk/issues); pull requests
> are disabled.

| Crate | Path | For |
| --- | --- | --- |
| `stormlight_mod_abi` | `schema/` | Both sides. The descriptor ABI: pure `serde` data, `no_std` + `alloc` by default, `std` behind a feature for the engine. |
| `stormlight_mod_sdk` | `pdk/` | Mod authors. Re-exports the ABI, adds the `register_mod!` macro, the registration context, and the wasm runtime glue. |

## Writing a mod

Start from **[example-mod](https://github.com/AestroFidelium/stormlight-example-mod)**. It is
a complete, tested mod that the real host loads in CI. The whole authoring
surface is one closure:

```rust
register_mod!(|ctx: &mut ModContext| {
    let energy = ctx.resource("energy");
    let spark = ctx.ability("spark", /* AbilityDescriptor */);
    ctx.unit("sentinel", /* UnitDescriptor binding `spark` to a slot */);
});
```

From that closure the macro generates every wasm export the host calls
(`mod_register`, `mod_alloc`, `mod_handle`, `mod_tick`, `mod_trigger`). On the
wasm target the SDK supplies a `dlmalloc` global allocator and a panic handler,
so a mod is a plain `no_std` cdylib that builds on **stable** Rust.

## Design

- **Abilities are data, not code.** An ability is a tree of `Impact`s. There are
  18 of them, including `Damage`, `Heal`, `Spawn` (a missile or area body),
  `ApplyModifiers`, `Dash`, `If` on a `Condition`, `Loop` and `Delay`. Numbers are `Value` expressions over stats, params and curves. The engine
  interprets the tree on the authoritative server, so an ability can be
  serialised, replicated and rewound. A mod that needs real code registers a
  `Custom` handler instead: a pure function from its input to a list of effects.
- **Handles are local, names are global.** A mod names its stats, params,
  resources and tags. `ModContext` interns each name to a local handle, and the
  engine remaps every handle in the descriptor tree to a global id when it
  adopts the mod (`schema/src/remap.rs`). Reserved names such as `move_speed` or
  `cooldown` collapse onto the engine's own ids.
- **Guests are stateless.** Every runtime entry re-runs the builder in a fresh
  store, so no guest state can outlive a call or go stale on rewind.

## Rigor

- `unsafe_code = "forbid"` in the ABI. The guest SDK uses `deny`, with an
  explicit `#[allow]` on its FFI shim: one slice view plus the generated
  `#[unsafe(no_mangle)]` exports.
- 313 [bolero](https://github.com/camshaft/bolero) property, fuzz and
  integration tests. For example, any generated `Impact` tree survives a
  postcard round trip unchanged, and the id-remap walk is total: it returns
  `Err` on a missing id and never panics.
- CI also builds the SDK for `wasm32-unknown-unknown`.

## Limitations

- **Unstable (0.x).** The schema still grows with every engine milestone, and
  the host checks only the ABI major version.
- **Verbose descriptors.** The main descriptors have no `Default`, so a mod
  writes out every field. One upside: adding a field to the schema breaks a
  mod at compile time instead of silently giving it a value.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option.
