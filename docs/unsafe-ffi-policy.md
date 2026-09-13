# Unsafe and FFI review policy

**Status:** binding for SoftGPU host code. Phase 0 must not introduce a ROCr/HSA FFI surface.

## Principles

1. `#[repr(C)]` and `extern "C"` are **necessary but not sufficient** for ABI correctness.
2. Every `unsafe` block needs a local safety comment naming: preconditions; ownership/lifetime/aliasing; thread-safety; why safe alternatives are insufficient.
3. Never form Rust references to unvalidated foreign memory.
4. Never unwind across an FFI boundary. Exports/callbacks need an explicit panic containment policy (Phase 1).
5. `Send`/`Sync` impls are exceptional and reviewed.
6. Prefer safe parsers and owned buffers; copy untrusted bytes when feasible.
7. Independent C probes (Phase 1) check layouts/signatures against pinned headers—not Rust declarations alone.

## Review checklist (for future ABI work)

- [ ] Symbol name, version, visibility, SONAME verified against pinned ROCm
- [ ] Size/align/offset/enum checks vs official headers
- [ ] Nullability, length, alignment validated before dereference
- [ ] Panic/unwind policy documented and tested
- [ ] Support matrix cell updated with verification level

## Phase 0 expectation

The skeleton crate should contain **no** `unsafe` blocks. If one appears, it requires this checklist and an ADR note.
