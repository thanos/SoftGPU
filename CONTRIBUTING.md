# Contributing to SoftGPU

## Before you write code

1. Read `README.md`, `docs/status.md`, `docs/architecture.md`, and the active phase notes.
2. Prefer the smallest change that advances one acceptance criterion.
3. Use test-driven development: fail first, then implement, then refactor.
4. Do not invent ABI layouts, ISA encodings, or R9700 numeric specs from memory. Cite `docs/sources.md`.

## Rust policy

- Pin builds with `cargo test --locked` / `cargo build --locked`.
- MSRV is **1.85** (`rust-version` in `Cargo.toml`); toolchain pin is **1.85.0**.
- No nightly-only features in the host runtime.
- Review every `unsafe` block per `docs/unsafe-ffi-policy.md`. Phase 0 should add essentially none.

## Documentation

Every completed stage updates architecture, status, support matrix, sources (as needed), and paired articles. Documentation drift fails the stage.

## Pull requests

- Keep PRs scoped to one phase criterion when practical.
- Include failing-then-passing test evidence in the description.
- State what remains unsupported and what was not tested in your environment.

## License

Contributions are dual-licensed Apache-2.0 OR MIT unless otherwise stated.
