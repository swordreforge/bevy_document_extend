# Taste

- Prefers truly independent Cargo features: any `--no-default-features --features <single-backend>` combo must build/test, via shared `common` module, feature-gated mods/examples/tests, and no `compile_error!` guard blocking single-format builds. Confidence: 0.9
- Prefers explore-then-scaffold workflow per document format: survey native libs first, then scaffold `types.rs` + `backend` trait + backend impl(s) + `mod.rs` expose API plus viewer example and integration tests. Confidence: 0.85
- Prefers full-matrix verification as done criteria: `cargo check`/`cargo test` on default/all/single/empty combos plus `cargo clippy --all-targets` and `cargo fmt --check` clean. Confidence: 0.85
- Prefers terse action-oriented collaboration: short directives like `let's continue` / `of course` mean proceed immediately without extra questions or proposals. Confidence: 0.8
