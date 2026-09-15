# Patches applied to oxml-layout 0.11.0
`[patch.crates-io] oxml-layout = { path = "patches/oxml-layout" }`
Upstream: https://github.com/tensorbee/rdocx (0.11.0)

## 1. `src/font.rs` — exhaustive `fontdb::Source` match
Upstream matches only `Binary` + `File` (the latter behind
`#[cfg(feature = "system-fonts")]`). Cargo unifies `fontdb` features
workspace-wide, so enabling `office2pdf` (via typst/resvg/usvg default
`fs`/`memmap`/`fontconfig`) adds `File`/`SharedFile` variants even when
`oxml-layout/system-fonts` is off → E0004.

Fix: keep upstream arms, add
- `File(_) => None` under `not(system-fonts) + fs`
- `SharedFile(_, data) => copy bytes` under new `memmap` feature
- `_ => None` fallback.
Deterministic bundled-font rendering is unchanged; file-backed faces are
simply skipped without system-font discovery.

## 2. `Cargo.toml` — `fs` / `memmap` passthrough features
```toml
fs = ["fontdb/fs"]
memmap = ["fontdb/memmap"]
```
Lets downstream opt into `fontdb` unification arms without pulling
`system-fonts` (no fontconfig/system discovery).

## Maintenance / removal
- While this patch is carried locally, crate versions stay pre-release
  (`0.1.0-alpha.x`) and `cargo publish` requires `--no-verify`, because
  verification resolves the *unpatched* crates.io deps (known E0004 in
  `oxml-layout 0.11.0` + unified `fontdb` features).
- Delete this dir + the `[patch.crates-io]` section and promote to a stable
  version once upstream fixes the non-exhaustive match (e.g. wildcard arm
  or decoupling from unified features).
