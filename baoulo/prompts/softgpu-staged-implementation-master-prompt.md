# SoftGPU: Staged Implementation and Article-Series Master Prompt

> **Purpose:** Hand this document directly to a coding LLM responsible for designing, implementing, testing, documenting, and teaching the SoftGPU project. Treat it as the project execution charter. Do not treat examples, guessed constants, remembered APIs, product names, or ABI sketches in this document as authoritative specifications. Verify all version-sensitive facts against current primary sources and pinned source revisions before implementation.

## 1. Mission

Build **SoftGPU**, a Rust-first, developer-oriented GPU emulation, testing, debugging, sanitization, and CI runtime. Rust is the implementation language for the host-side runtime, parsers, execution engines, and tools. Retain AMD's real HIP compiler/runtime as the initial application-facing stack; Rust-authored GPU kernels are a separate optional research track, not a prerequisite.

The initial compatibility target is the **AMD Radeon AI PRO R9700**, **RDNA 4**, and LLVM/ROCm target **`gfx1201`**. The first integration goal is not to clone or mock HIP. Use AMD's real, supported HIP compiler/toolchain and real HIP userspace wherever practical, then intercept or substitute the lowest practical user-space hardware-facing boundary: **ROCr/HSA**.

The long-term execution stack is:

```text
HIP application and kernels
        |
        v
AMD compiler + real HIP runtime
        |
        v
SoftGPU ROCr/HSA compatibility adapter
        |
        +--> agent, region/pool, queue, signal, executable, and loader behavior
        +--> AQL packet observation and validation
        |
        v
Vendor-neutral SoftGPU core
        |
        +--> functional execution engine
        +--> deterministic scheduler
        +--> memory/race/barrier sanitizers
        +--> debugger and structured traces
        +--> optional analytical performance model
        |
        v
AMD code-object frontend and, eventually, gfx1201 ISA interpreter
```

The same application should eventually be able to run against SoftGPU or a physical R9700 through environment/runtime selection, not source changes to the application. The Rust choice also serves an explicit educational/portfolio goal: show rigorous work on GPU runtimes, compilers' output, emulation, concurrency, and diagnostics. Do not let that goal displace fidelity or scope discipline. Keep any existing Zig inference project, including Zynfer, as a separate reference client/baseline rather than rewriting it as part of SoftGPU.

SoftGPU is not initially:

- a replacement HIP SDK;
- a kernel-mode AMDGPU driver;
- a PCIe/MMIO/firmware device model;
- a cycle-accurate R9700 simulator;
- a graphics API implementation;
- a performance oracle;
- permission to invent undocumented AMD behavior;
- permission to return success for unimplemented behavior.

## 2. Non-negotiable engineering rules

1. **Prove the boundary before building the machine.** First demonstrate that a real HIP userspace process loads the SoftGPU ROCr/HSA library and observes a controlled virtual agent. Do not begin broad ISA work before this is repeatable in x86-64 Linux CI.
2. **Use test-driven development.** Add or identify a failing test before each behavioral change. Implement the smallest behavior that makes it pass, then refactor with all gates green.
3. **Work in gated phases.** Do not declare a phase complete or start dependent production work until its acceptance criteria are met and recorded. Spikes may explore later risks, but remain isolated, disposable, and clearly labeled.
4. **Verify, do not remember.** Inspect current official documentation, installed headers, exported symbols, upstream source, compiler output, and real traces. Record versions and commit hashes. Never assume ABI layouts, enum values, symbol versions, packet fields, metadata keys, ISA encodings, device capabilities, or product specifications.
5. **Preserve C ABI compatibility.** The externally visible HSA/ROCr layer must use verified calling conventions, integer widths, alignment, packing, ownership, callbacks, symbol names, symbol versions, and error behavior. ABI conformance is a release gate.
6. **Keep the core vendor-neutral.** AMD/HSA/AQL/code-object/gfx logic belongs in adapters and frontends. Scheduling, abstract memory, execution, diagnostics, and sanitizers must not depend on AMD names or encodings.
7. **Never silently fake unsupported behavior.** Return a specific error, reject the artifact, or stop execution with a structured diagnostic. A compatibility stub may report a capability only when its advertised contract is implemented. Any deliberate approximation must be opt-in, labeled, traced, documented, and excluded from conformance claims.
8. **Separate correctness from timing.** Functional execution determines architectural results under a declared semantics model. Performance estimation is a separate, explicitly non-conformant analytical subsystem with confidence and provenance. Wall-clock emulator speed must never be presented as emulated GPU speed.
9. **Prefer deterministic behavior.** Default tests and sanitizer runs use reproducible scheduling, stable IDs, seeded nondeterministic explorations, and replayable traces.
10. **Keep documentation synchronized.** Every completed stage updates architecture decisions, support matrices, limitations, testing instructions, and its paired article(s). Documentation drift fails the stage.
11. **No speculative framework building.** Introduce abstractions only after at least one real use case demonstrates the seam. Prefer small explicit components and data structures.
12. **Do not copy restricted material.** Use compatible upstream source and specifications lawfully. Track license and provenance for borrowed tables, generated decoders, fixtures, and headers.
13. **Constrain unsafety.** The ROCr/HSA ABI, foreign callbacks, raw pointers, shared-memory rings, and any hardware-facing layouts require reviewed `unsafe`; keep it in small modules with documented invariants and safe, validating internal APIs. Rust's type system does not prove external ABI compatibility or GPU semantic correctness.
14. **Keep portfolio claims evidence-based.** This project may demonstrate Rust GPU-systems competence, but it must not claim that SoftGPU substitutes for a real R9700 benchmark, certifies AMD compatibility, or guarantees employment. Publish reproducible engineering evidence rather than aspirational claims.

## 3. Operating protocol for the implementation LLM

At the start of every work session:

1. Read `README.md`, `docs/status.md`, `docs/support-matrix.md`, `docs/architecture.md`, relevant ADRs, and the current phase plan.
2. Inspect the repository and working tree. Preserve user changes. Do not overwrite unrelated work.
3. Identify the active phase, its unfulfilled acceptance criteria, and the smallest vertical slice that advances one criterion.
4. Consult the source-of-truth ledger described below. If facts may have changed or are uncertain, retrieve current primary sources before coding.
5. Write a short execution note: objective, evidence, assumptions, test to fail first, risks, and files expected to change.

For every change:

- Start with an executable failing test or a reproducible probe.
- Keep ABI-facing and binary-parsing code defensive: checked arithmetic, explicit endian handling, bounded reads, no alignment assumptions, and precise errors.
- Run the narrow test, then affected suites, format/lint checks, ABI checks where relevant, and at least one negative test.
- For every new or changed `unsafe` block, state its preconditions, ownership/lifetime and aliasing invariants, thread-safety assumptions, and why a safe alternative is insufficient. Review unwind behavior at foreign callbacks and exported functions.
- Update observability so failures expose enough context to diagnose without attaching a debugger.
- Update documentation and the phase evidence log.
- Report what is implemented, what is verified, what remains unsupported, and what was not tested in the current environment.

At the end of every work session, leave the repository buildable. If that is impossible during an explicitly approved migration, put incomplete work behind an off-by-default experimental flag and document it.

Do not claim compatibility from unit tests alone. Use the conformance ladder in this prompt.

## 4. Sources of truth and evidence discipline

Create `docs/sources.md` as a versioned source ledger. For every ABI, packet format, metadata schema, target feature, and instruction family, record:

- primary-source URL or repository path;
- document/version, release tag, and commit hash where available;
- access date;
- relevant installed package and header version;
- what SoftGPU derives from it;
- whether the fact is normative, observed, inferred, or provisional;
- licensing/provenance notes;
- a test or generated artifact tied to the source.

Prioritize current primary sources:

- AMD ROCm and HIP official documentation;
- HSA Foundation specifications;
- AMD ROCr runtime source and its public headers;
- AMD HIP runtime source and public headers;
- LLVM AMDGPU backend documentation and source;
- AMD GPU ISA documentation applicable to RDNA 4/gfx12 when officially available;
- ELF specification plus AMDGPU code-object and metadata documentation;
- official Rust Reference, Rustonomicon, Cargo and rustc target documentation for the pinned toolchain; verify unstable/nightly features before proposing them;
- Apple's official `container` repository and documentation;
- observed output from pinned official tools such as `hipcc`, Clang, `llvm-readelf`, and `llvm-objdump`;
- differential traces from real R9700 hardware when available.

Secondary articles and existing emulators may explain concepts but must not establish ABI or ISA facts. Study relevant prior art—including gem5 AMD GPU models, GPGPU-Sim/Accel-Sim, GPU Ocelot, PoCL, Mesa software paths, sanitizers, and interpreters—without inheriting their scope or licensing accidentally.

When sources disagree:

1. Preserve the conflicting evidence.
2. Prefer the behavior required by the pinned supported toolchain/runtime combination.
3. Add an executable probe or differential test.
4. Isolate version-dependent behavior behind a compatibility descriptor.
5. Document the resolution and do not generalize it beyond the evidence.

Never bake scraped/generated encoding data into the repository without a reproducible generator, provenance header, license review, and golden tests.

## 5. Product contracts and fidelity vocabulary

Every run, diagnostic, test result, and public report must name its fidelity level:

- **ABI:** the host process can call a declared subset of HSA/ROCr with verified C ABI behavior.
- **Protocol:** queues, signals, AQL packets, code-object registration, and dispatch flow follow a declared subset.
- **Functional:** kernels execute on a CPU-backed semantic engine using a declared input representation. Results are not evidence of gfx1201 instruction compatibility.
- **Architectural ISA:** supported gfx1201 instructions and state transitions execute according to verified architectural semantics. Unsupported instructions fail closed.
- **Sanitized:** additional checking changes scheduling, storage, or timing and is intended for defect discovery, not timing fidelity.
- **Analytical performance:** parameterized estimates or counters only; not cycle accuracy unless separately validated and explicitly named.
- **Hardware-conformant:** a named test, toolchain, device profile revision, driver/runtime version, and real hardware sample produced equivalent results under stated tolerances.

Rust host-code memory safety is not a fidelity level: it neither proves ROCr/HSA ABI correctness nor makes a kernel's GPU behavior well-defined. Likewise, a Rust-authored GPU kernel is not required for any AMD/HIP integration gate below.

Do not use “R9700 emulator,” “gfx1201 compatible,” or “conformant” without attaching the applicable scope and evidence.

## 6. Proposed repository layout

Start smaller if the repository is new; create directories only as their phase begins.

```text
softgpu/
├── Cargo.toml                       # workspace; start as one crate if simpler
├── Cargo.lock                       # commit for reproducible application builds
├── rust-toolchain.toml               # pin the supported compiler/channel
├── README.md
├── LICENSE
├── SECURITY.md
├── CONTRIBUTING.md
├── src/                              # minimal single-crate Phase 0 start
├── crates/                           # split only when a real boundary demands it
│   ├── softgpu-core/src/
│   │   ├── device.rs                # vendor-neutral device/session contract
│   │   ├── dispatch.rs              # grid/workgroup/invocation model
│   │   ├── execution.rs             # engine interface and traps
│   │   ├── scheduler.rs             # deterministic scheduling policies
│   │   ├── memory.rs                # virtual allocations/address spaces
│   │   ├── sync.rs                  # barriers, atomics, ordering events
│   │   └── diagnostics.rs           # structured diagnostics
│   ├── softgpu-hsa/src/              # Linux cdylib exporting verified C ABI
│   │   ├── ffi/                     # reviewed bindings and layout checks
│   │   ├── api/                     # implemented HSA/ROCr API families
│   │   ├── agent.rs
│   │   ├── memory.rs
│   │   ├── queue.rs
│   │   ├── signal.rs
│   │   └── executable.rs
│   ├── softgpu-aql/src/              # packet validation and doorbells
│   ├── softgpu-amd-code-object/src/  # bounded ELF and metadata parsing
│   ├── softgpu-functional/src/       # verified functional input/execution
│   ├── softgpu-gfx/src/              # common -> gfx12 -> gfx1201 layering
│   ├── softgpu-sanitizer/src/        # memory, race, barrier checking
│   ├── softgpu-debugger/src/         # later phase
│   ├── softgpu-cli/src/              # run, inspect, trace, replay, profile
│   └── softgpu-ptx/src/              # future; do not create prematurely
├── include/                         # only SoftGPU-owned public headers
├── profiles/                        # versioned data + evidence, not marketing guesses
├── tests/
│   ├── unit/
│   ├── properties/
│   ├── fuzz/
│   ├── abi/
│   ├── protocol/
│   ├── integration/
│   ├── negative/
│   ├── differential/
│   ├── conformance/
│   └── fixtures/
├── examples/
├── tools/                            # probes, generators, corpus reducers
├── fuzz/                             # optional dedicated Rust fuzz targets
├── environments/
│   ├── apple-container/              # Linux ARM64 development recipe
│   └── rocm-x86_64/                  # pinned official ROCm integration image
├── ci/
├── docs/
│   ├── architecture.md
│   ├── status.md
│   ├── support-matrix.md
│   ├── sources.md
│   ├── conformance.md
│   ├── diagnostics.md
│   ├── threat-model.md
│   ├── adr/
│   └── articles/
└── third_party/                      # notices/patches only when necessary
```

### Rust implementation contract

- Start with a small Cargo crate, not an elaborate workspace. Split out a `cdylib` HSA adapter and vendor-neutral libraries when the boundary is proven. Do not let crate boundaries become speculative architecture.
- Pin a supported stable Rust toolchain and minimum supported Rust version (MSRV). Nightly-only features are prohibited in the host runtime unless a narrow spike proves necessity and the project explicitly accepts the maintenance cost.
- Build the HSA adapter as a Linux `cdylib` with deliberately controlled exports and dependencies. Verify SONAME, symbol names/versions, visibility, calling conventions, struct/union layouts, enum representations, callback signatures, and actual loader resolution against pinned ROCm headers and binaries. `#[repr(C)]` and `extern "C"` are necessary but not sufficient evidence.
- Generate C bindings from pinned official headers when practical, but review generated output, lock the generator and header versions, and test layouts against an independent C probe. Do not hand-transcribe large ABI surfaces from memory. Keep target-specific bindings separate when ABI facts differ.
- Keep raw FFI pointers, `UnsafeCell`, shared queue memory, atomics over foreign storage, and conversions from C handles inside small audited modules. Each `unsafe` operation requires a local safety comment naming the invariant it relies on. Expose validated Rust types and explicit ownership transitions to the rest of the program.
- Do not form Rust references to unvalidated foreign memory or assume C-owned buffers outlive a call. Validate nullability, alignment, length, aliasing, and lifetime before reading/writing. Copy untrusted packet/metadata bytes into bounded owned storage when feasible.
- Never unwind across an FFI boundary. Use an explicitly verified panic strategy and containment policy for exports/callbacks; convert recoverable faults into HSA status plus structured diagnostics, and define what happens after an internal invariant failure. Do not disguise a panic as successful GPU execution.
- Make `Send`/`Sync` implementations exceptional, documented, and reviewed. Audit lock order, callback reentrancy, queue wait/wakeup behavior, atomics and memory ordering. Safe Rust alone does not validate a foreign producer/consumer protocol.
- Prefer safe parsers over raw pointer walking. Fuzz parsers and API sequences; run Clippy, formatting, dependency/license/security checks, and host sanitizers or Miri where supported and applicable. Record unsupported instrumentation environments rather than silently skipping them.
- Use explicit newtypes for handles, virtual addresses, allocation IDs, profile revisions, and instruction encodings. Keep `unsafe` counts and their invariants visible in review; do not impose a zero-`unsafe` slogan that would merely hide ABI work in dependencies.
- Keep Rust GPU-code experiments isolated. At the time of this revision, the official [rustc AMD GPU compute-target page](https://doc.rust-lang.org/rustc/platform-support/amdgcn-amd-amdhsa.html) labels the target Tier 3 and documents special build requirements; recheck current rustc/LLVM/ROCm documentation before any attempt. Such experiments may produce test fixtures or a future frontend, but do not replace the real HIP compiler path that establishes initial compatibility. The [Rust-on-every-GPU demonstration](https://rust-gpu.github.io/blog/2025/07/25/rust-on-every-gpu/) concerns portable Rust-authored kernels, not ROCr/HSA substitution; do not conflate the two.

## 7. Architectural boundaries and interfaces

### 7.1 Runtime adapter

The runtime adapter translates a vendor API/protocol into vendor-neutral operations. It owns handles, lifecycle, ABI error mapping, callbacks, queues, signals, executable loading, and dispatch submission. It must not contain instruction execution semantics.

Define internal typed identifiers rather than exposing raw ABI handles throughout the core. Validate handle type, generation, ownership, and lifetime. Make stale, cross-context, double-destroyed, and forged handles deterministic errors.

### 7.2 Dispatch contract

A vendor adapter should lower a launch into a checked descriptor conceptually containing:

```text
Dispatch {
  device/profile revision
  executable + kernel identity
  grid and workgroup dimensions
  kernarg bytes and schema/provenance
  address-space bindings
  group/private segment requirements
  completion signal/event
  execution mode and sanitizer policy
}
```

Do not freeze this exact structure before observing real AQL/code-object inputs. Keep vendor packet bytes available for diagnostics but do not leak them into the core scheduler.

### 7.3 Execution engine

Define a narrow engine boundary only after the first functional vertical slice. It should support prepare, execute/step, trap, completion, cancellation, and state inspection. Both functional and ISA engines should emit the same normalized memory and synchronization events so sanitizers do not depend on the instruction source.

### 7.4 Memory service

Model allocations explicitly: identity, address space, extent, alignment, permissions, owner, liveness, initialization shadow, profile visibility, and optional host backing. Avoid treating host pointers as universal device pointers. Translation and pointer classification must be checked operations.

Represent at least global, group/shared/LDS, private/scratch, kernarg, and host-visible/coherent categories once required by a verified workload. Add spaces incrementally; do not pretend they alias the way host memory does.

### 7.5 Device profile

A profile is a versioned compatibility contract, not a bag of display strings. Split it into:

- identity advertised through a runtime adapter;
- architectural capabilities relevant to legality and execution;
- resource limits used to reject impossible launches;
- memory topology and visibility;
- optional analytical performance parameters;
- provenance and confidence per field;
- known quirks and version constraints.

Fields may be `verified`, `observed`, `inferred`, or `unknown`. Unknown is valid. Never invent a value merely because an API requests it. If compatibility demands a value before verification, isolate it as provisional, make the reason visible, and prohibit conformance claims.

Provide a generic test profile and an evidence-backed `amd-radeon-ai-pro-r9700/gfx1201` profile. Multiple card profiles must share schemas and validation but may not inherit unsupported semantics through optimistic defaults.

### 7.6 AMD architecture layering

Separate:

```text
AMDGPU-wide concepts
  -> GFX-family concepts
    -> gfx12/RDNA 4 concepts
      -> gfx1201 target features
        -> R9700 device-profile limits/identity
```

Code-object parsing is not ISA decoding. ISA decoding is not execution semantics. Execution semantics are not resource/performance modeling. Keep each independently testable.

Features must be capability-driven. Do not scatter checks such as `if gfx1201` throughout the engine. Use verified feature descriptors while preserving target-specific exceptions close to their source evidence.

### 7.7 Future NVIDIA path

Do not implement CUDA/PTX during the AMD foundation phases. Preserve these seams:

```text
Real CUDA application/runtime (future feasibility must be researched)
        |
NVIDIA runtime adapter or explicit SoftGPU launch adapter
        |
PTX module/parser/verifier frontend
        |
PTX semantic execution or lowering to a proven SoftGPU IR
        |
same core scheduler, memory-event model, diagnostics, and sanitizers
```

Do not assume ROCr-style substitution maps cleanly onto proprietary CUDA boundaries. The NVIDIA phase begins with a feasibility and legal/technical boundary study. PTX address spaces, warps, masks, barriers, memory ordering, texture/surface features, and target versions retain their own semantics; do not force them into AMD terminology.

Introduce a shared IR only when both a working AMD path and a concrete second frontend show stable common operations. The IR must preserve source semantics and permit explicit “cannot represent” errors.

## 8. Supported and unsupported behavior policy

Maintain a machine-readable support matrix and render it into documentation. Index support by:

- host OS and architecture;
- Rust toolchain version, target triple, Cargo lockfile, and feature set;
- ROCm/HIP version;
- ABI/API symbol family;
- AQL packet type and field behavior;
- code-object version and metadata feature;
- execution mode;
- gfx target and instruction/opcode variant;
- memory/atomic/barrier semantics;
- device profile revision;
- sanitizer capability;
- validation environment.

Allowed states are: `implemented-unverified`, `verified-unit`, `verified-integration`, `hardware-differential`, `experimental`, and `unsupported`. Avoid a vague checkmark.

Unsupported or malformed input must produce:

- a stable error code/category;
- human-readable context;
- structured machine-readable output;
- relevant API, packet, kernel, PC/opcode, workgroup/wave/lane, address, and profile information when available;
- a remediation or link to the support matrix;
- no partial success unless the API contract explicitly permits it.

An optional permissive mode may help discovery, but it must print a prominent approximation warning, set a non-conformance marker in traces, and never be CI's default.

## 9. Testing and conformance strategy

Use a ladder; each level depends on the earlier levels:

1. **Layout tests:** compile-time and runtime checks for sizes, alignment, offsets, enum values, calling conventions, symbol names, and exports against pinned official headers/tooling.
2. **Unit tests:** state machines, handle tables, signals, queues, schedulers, memory translation, parsers, decoders, and instruction semantics.
3. **Property tests:** checked binary parsing, address translation, scheduler invariants, wave mask operations, encode/decode round trips where an authoritative encoder exists.
4. **Fuzz tests:** ELF/code objects, metadata, AQL packets, API call sequences, kernarg descriptors, instruction bytes, trace readers, and CLI inputs. Crashes, hangs, unbounded allocation, and silent acceptance are failures.
5. **Protocol tests:** producer/consumer ordering, doorbell behavior, packet ownership transitions, signal waits, wraparound, malformed packets, and cancellation.
6. **Integration tests:** real compiled HIP host programs and kernels against the pinned official userspace plus SoftGPU substitution on Linux x86-64.
7. **Golden compiler artifacts:** tiny kernels compiled by pinned `hipcc`/Clang; inspect and version metadata/code objects. Store minimal redistributable fixtures or regenerate them with hashes.
8. **Metamorphic tests:** scheduling order changes, harmless padding/alignment changes, equivalent launch decompositions, and sanitizer modes must preserve applicable outcomes.
9. **Differential tests:** compare functional and ISA modes where their supported overlap is nonempty.
10. **Hardware conformance:** run identical generated inputs on SoftGPU and a real R9700, compare outputs, error behavior where observable, atomics, floating-point tolerances, and trace-derived invariants.

Every conformance test records:

- source and generated artifact hashes;
- compiler, ROCm, driver, firmware where available, and SoftGPU revisions;
- exact target/features;
- device identity/profile revision;
- environment and flags;
- seed and scheduler policy;
- expected comparison rule, including NaN/signed-zero/rounding tolerance;
- result plus captured diagnostics.

Do not make tests depend accidentally on unspecified scheduling. Tests for races must identify that the program is ill-defined rather than canonizing one result.

## 10. Observability and debugger contract

From the first runtime prototype, support structured logging with stable event categories and optional JSON Lines output. Logs must be bounded and redact host secrets. Use monotonic sequence numbers and virtual timestamps; distinguish them from wall time.

Normalized events should eventually cover:

- API enter/exit and status;
- handle create/destroy;
- allocation/map/unmap/free;
- executable/module/kernel discovery;
- queue creation and packet consumption;
- signal reads/writes/waits;
- dispatch begin/end/trap;
- workgroup, wave, and lane scheduling;
- instruction/IR steps when enabled;
- memory accesses and atomic/synchronization events;
- sanitizer findings;
- unsupported and approximate behavior.

Tracing is off or low-cost by default, uses filters and budgets, and must not alter correctness. When observation necessarily perturbs scheduling, label the run.

Build deterministic record/replay around normalized inputs and scheduling decisions, not raw host pointers. Trace formats are versioned, self-describing, bounded, and treated as untrusted on read.

Debugger staging: launch and packet inspection first; kernel/workgroup/wave/lane state next; breakpoints and single-step after an execution engine exists; source correlation only when debug metadata is verified. Never fabricate source lines.

## 11. Sanitizer semantics

Sanitizers consume normalized execution events and maintain shadow state. They must report possible false-positive/negative boundaries.

### Memory sanitizer

Incrementally detect:

- out-of-bounds and overflowed address calculations;
- use-after-free and stale handles/pointers;
- invalid address-space access;
- misalignment where architecturally illegal or contractually required;
- reads of uninitialized bytes/registers when tracked;
- access-permission and visibility violations;
- bad kernarg layout/extent;
- illegal host/device pointer use.

Diagnostics identify allocation, offset, valid range, access width/type, kernel, PC or IR location, and workgroup/wave/lane. Shadow-memory scale and granularity must be configurable and documented.

### Race detector

Begin with a precise declared subset, such as conflicting accesses within a workgroup under a happens-before model using barriers and supported atomics. Track access epochs, scope, ordering, address range, and participants. Account for SIMD lanes and masks; a vector instruction may yield multiple logical accesses.

Do not claim whole-device race completeness until cross-workgroup synchronization, atomics, scopes, and memory ordering are modeled. Classify definite versus potential races.

### Barrier sanitizer

Detect divergent participation, missing participants, mismatched barrier generations, illegal scope, premature workgroup completion, and deadlock cycles within the supported model. Report which lanes/waves arrived, which did not, active masks, and the control locations leading to the barrier.

Default sanitizer scheduling is deterministic. Add seeded schedule exploration later to expose ordering defects; preserve a replay seed and trace for every finding.

## 12. Performance-modeling boundary

Functional/ISA correctness and performance modeling are separate modules, APIs, tests, and documentation.

Permitted early outputs include instruction counts, memory-access counts, divergence metrics, occupancy/resource-limit calculations based on sourced values, and synchronization counts. These are architectural or analytical metrics, not time predictions.

Any latency, bandwidth, throughput, cache, occupancy, or runtime estimate must state:

- parameter source and profile revision;
- formula/model version;
- calibration hardware/software;
- confidence interval or known error where feasible;
- omitted effects;
- whether the result is static, trace-driven, analytical, or simulated.

Never tune functional semantics to match timing. Never infer correctness from a performance match. Cycle-accurate simulation is a separate future project unless explicitly chartered with new acceptance criteria.

## 13. Security and robustness

Treat applications, code objects, ELF sections, metadata, AQL packets, traces, profiles, and environment variables as untrusted.

- Use checked addition/multiplication and bounded slices for all sizes and offsets.
- Impose configurable limits on allocations, virtual VRAM, queues, signals, packets, workgroups, instructions, recursion, trace size, and execution time.
- Prevent host pointer forgery and arbitrary host memory access.
- Do not execute embedded host code or shell commands from artifacts.
- Avoid loading arbitrary plugins in the runtime process.
- Define cancellation and watchdog behavior for infinite kernels and waits.
- Ensure signal/callback handling cannot cause use-after-free or reentrant corruption.
- Make temp files private and avoid leaking source, kernargs, environment data, or memory contents in logs by default.
- Run parsers/fuzzers under host sanitizers where available and use process isolation for hostile corpora.
- Pin CI images and dependencies; generate an SBOM and perform license/provenance checks for releases.
- Sign or checksum release artifacts and publish reproducible build information where practical.
- Maintain `SECURITY.md`, a threat model, vulnerability reporting instructions, and safe disclosure practice.

No security boundary is implied merely because execution is emulated. Until proven otherwise, document SoftGPU as a developer tool that must not run hostile kernels in a sensitive host process.

## 14. Development and CI environments

### Native Apple Silicon macOS

This is the primary fast loop for platform-neutral Rust work: parsers, scheduler, memory, profiles, functional engine, decoder tables, diagnostics, sanitizers, property tests, and CLI. Pin the Rust toolchain and make `cargo test --locked` the low-friction command. Add `cargo fmt --check` and strict Clippy checks without allowing lint churn to replace correctness work.

Do not require ROCm or a container for most core changes. Keep endian, pointer-width, and host-architecture assumptions out of formats and runtime-neutral code.

### Apple `container` on macOS

Use Apple's official `container` project as a convenient OCI-based Linux ARM64 laboratory for Linux builds, dynamic-loader/export checks that are architecture-neutral, packaging, fuzzing, and reproducible tools. Verify current installation, OS/hardware requirements, CLI syntax, networking, mounts, and virtualization features from the official repository at implementation time.

Apple `container` is development infrastructure, not a SoftGPU runtime dependency. Do not assume Linux ARM64 can run official x86-64 ROCm packages. If architecture emulation is explored, mark it optional and do not make it the canonical integration gate.

### Linux x86-64 CI

This is the canonical official ROCm/HIP integration environment. Pin supported ROCm versions and image digests. Compile tiny real HIP applications/kernels for `gfx1201`, inject or select the SoftGPU HSA/ROCr library through a documented, reversible loader setup, and verify the actual loaded libraries and exported symbol surface.

The CI matrix should include:

- macOS Apple Silicon core tests;
- Linux ARM64 core and ABI-build tests where supported;
- Linux x86-64 without ROCm for portability;
- Linux x86-64 with each supported pinned ROCm version;
- debug/release builds; formatting, Clippy, audit/license checks, and host sanitizers/Miri where practical;
- fuzz smoke tests on every change and longer scheduled fuzzing;
- deterministic replay tests;
- article link/snippet checks and support-matrix consistency;
- scheduled real-R9700 conformance once hardware is available.

CI must not report a skipped official integration test as success. Make “not run,” “unsupported runner,” and “passed” distinct.

## 15. Gated implementation roadmap

Each phase produces code, tests, evidence, documentation, and the paired article installment. Phase numbers express dependency, not calendar promises.

### Phase 0 — Charter, evidence baseline, and skeleton

**Objective:** Establish a minimal Rust project and truthful scope before compatibility code.

**Implement:** pinned Rust toolchain/MSRV policy; small Cargo build/test skeleton and committed lockfile; architecture/status/support/source documents; ADR template; structured error taxonomy; profile schema draft with provenance; CI skeleton for native macOS and Linux; minimal contribution/security policies. Record the FFI/`unsafe` review policy without implementing the HSA library yet.

**Tests:** one unit test, one negative CLI/config test, reproducible build smoke test, documentation link check, profile-schema validation.

**Acceptance criteria:**

- One documented command builds and tests on Apple Silicon macOS and Linux x86-64.
- Scope, fidelity vocabulary, and unsupported-behavior policy are visible in the README.
- Sources ledger identifies pinned HSA, ROCr, HIP, LLVM/AMDGPU, Rust, and Apple-container primary sources.
- No unverified R9700 numeric specification is advertised as fact.
- The supported Rust toolchain, lockfile policy, minimum supported version, and `unsafe` review rules are documented; stable Rust builds the skeleton without a nightly dependency.
- CI distinguishes skipped from passed jobs.
- Article 1 is published with tested commands and source links; Article 19 is drafted as an early Rust-language/FFI rationale and marked for revision when real ABI evidence arrives.

### Phase 1 — ROCr/HSA ABI reconnaissance and load proof

**Objective:** Prove real AMD HIP userspace can load a SoftGPU-produced shared library at the intended boundary.

**Implement:** automated header/symbol/layout probes; minimal exported shared library; explicit version script/export control as required; loader-verification tool; only the initialization/shutdown and discovery behavior necessary for the probe, with honest unsupported errors elsewhere.

**Tests:** compile against pinned official headers; compare size/alignment/offsets/enums; inspect dynamic exports/dependencies; test load/unload/repeated init/error paths; verify with real loader traces in x86-64 ROCm CI.

**Acceptance criteria:**

- A tiny real HIP-linked host program demonstrably loads SoftGPU at the intended ROCr/HSA boundary in x86-64 Linux CI.
- Evidence shows which exact library and symbols were resolved; accidental loading of system ROCr fails the test.
- All exported implemented functions match verified C ABI declarations.
- Every exported function and callback has a reviewed panic/unwind policy; FFI `unsafe` blocks have local invariants, and an independent C probe checks layouts/signatures rather than relying only on Rust declarations.
- Unknown calls do not succeed silently.
- The supported ROCm version and exact limitations are recorded.
- No kernel execution claim is made.
- Article 2 explains the boundary and the load proof.

### Phase 2 — Virtual agent discovery and device profile v1

**Objective:** Let real userspace discover one controlled virtual GPU agent without claiming dispatch capability.

**Implement:** generation-safe handles; one CPU and/or GPU agent only as required by observed behavior; agent iteration/info; profile-backed identity/capability responses; lifecycle and thread-safety rules; stable structured traces.

**Tests:** official-header API probes, repeated enumeration, callback behavior, invalid handles/attributes, concurrent discovery, differential observation against ROCr where comparison is meaningful.

**Acceptance criteria:**

- A real HIP userspace probe reaches device discovery through SoftGPU and sees the intended virtual agent or a documented controlled subset.
- Every advertised field has source/provenance and a test.
- Unsupported attributes return verified errors rather than guessed values.
- Handle misuse tests pass with no leak or crash.
- Trace output identifies profile and fidelity level.
- Article 3 teaches agents, profiles, and why identity is a contract.

### Phase 3 — Memory regions/pools, signals, and queue mechanics

**Objective:** Implement enough verified runtime primitives to create a queue and observe producer activity safely.

**Implement:** required region/pool discovery, bounded CPU-backed allocations, permissions/lifetime metadata, signals with verified wait semantics, queue allocation/state, doorbell observation, packet ring validation, shutdown/cancellation. Prefer the API family actually used by the pinned HIP runtime; document legacy/extension boundaries.

**Tests:** wraparound, ordering, malformed sizes, misalignment, stale signals, wait timeouts, allocation exhaustion, queue destruction during waits, concurrent producer/consumer stress, resource caps.

**Acceptance criteria:**

- The real runtime creates required memory/signal/queue resources against SoftGPU.
- A submitted packet is observed exactly once under tested producer/consumer orderings.
- No packet is executed yet unless explicitly part of Phase 4.
- Bounds and lifetime failures are deterministic and diagnostic.
- Thread/race analysis for the runtime implementation is documented and tested.
- Any `unsafe impl Send`/`Sync` or raw foreign-memory synchronization has a specific invariant and stress test; safe Rust is not treated as proof of AQL memory ordering.
- Article 4 teaches HSA queues, signals, and ownership.

### Phase 4 — AQL dispatch interception

**Objective:** Decode, validate, trace, and complete the minimal dispatch protocol without executing kernel semantics.

**Implement:** packet header/type parser; kernel-dispatch packet validation; barriers only if required; completion-signal behavior; normalized dispatch descriptor; packet capture/replay; explicit unsupported packet handling.

**Tests:** golden packets obtained lawfully from pinned sources/tools, field mutations, dimensions/overflow, kernarg pointer classification, completion transitions, packet reuse, malformed/unsupported packet types, replay determinism.

**Acceptance criteria:**

- A tiny real HIP launch reaches a validated AQL kernel-dispatch packet in SoftGPU.
- Kernel identity, grid/workgroup dimensions, segment sizes, kernarg location, and completion signal are traced when available.
- A no-execution diagnostic mode completes or rejects only according to a documented experimental contract; it is never represented as kernel success.
- Captured packets can be replayed through the parser without the original process and without raw host-pointer dependence.
- Article 5 teaches HSA/AQL dispatch end to end.

### Phase 5 — AMD code-object and kernel metadata handling

**Objective:** Safely identify and describe kernels in real compiler-produced AMDGPU code objects.

**Implement:** bounded ELF reader or carefully evaluated dependency; AMD code-object discovery; supported metadata-version parser; target/feature validation; kernel symbol/descriptor and kernarg metadata extraction; inspection CLI; fixture generator and provenance.

**Tests:** tiny kernels covering arguments, alignment, group/private segments, multiple kernels, globals, malformed ELF/metadata, unsupported versions/targets, fuzz/property corpus, comparison with official LLVM inspection tools.

**Acceptance criteria:**

- SoftGPU parses a pinned `gfx1201` code object produced by the official compiler and identifies the dispatched kernel and verified launch metadata.
- Unsupported code-object versions or target features fail closed.
- Parser fuzzing meets a stated duration/corpus threshold with no crash, hang, or unbounded allocation.
- Inspection output cross-checks against pinned official tools.
- Article 6 teaches fat binaries, ELF, code objects, metadata, and gfx targets.

### Phase 6 — Minimal vendor-neutral functional execution

**Objective:** Execute one tiny kernel functionally on the CPU without claiming gfx1201 ISA execution.

**Research gate:** Select a lawful, reproducible functional input path based on current toolchain evidence—such as a deliberately emitted LLVM-based artifact, a small explicit SoftGPU test IR, or another verifiable representation. Do not assert that arbitrary final AMD code objects contain executable high-level IR.

**Implement:** minimal dispatch scheduler; workgroup/invocation identifiers; explicit address spaces; kernarg binding; load/store and a tiny arithmetic/control subset; deterministic completion; functional-mode marker in all output.

**Tests:** scalar add/copy/index kernels; zero/one/max supported dimensions; bounds failures; multiple workgroups; deterministic reruns; differential host reference.

**Acceptance criteria:**

- At least one real-source kernel follows a documented reproducible path from source/toolchain to SoftGPU functional execution and produces the reference result.
- The representation and translation path are fully disclosed.
- Unsupported operations stop with precise diagnostics.
- No output calls this gfx1201 ISA emulation.
- Article 7 teaches emulation versus simulation and the chosen functional path.

### Phase 7 — GPU execution semantics v1

**Objective:** Model the minimum useful GPU semantic machine: workgroups, waves/subgroups, lanes, masks, group memory, barriers, and selected atomics.

**Implement:** deterministic wave/lane execution; divergence/reconvergence appropriate to the chosen input semantics; group/LDS storage; barrier generations; selected atomic operations with explicit scope/order; resource-limit validation; step budgets.

**Tests:** indexing, predication/divergence, partial waves, group-memory exchange, reductions, barrier convergence, atomics, infinite-loop watchdog, schedule metamorphism.

**Acceptance criteria:**

- A documented kernel suite exercises every supported semantic concept.
- Results match host references across multiple deterministic scheduler policies where the kernels are race-free.
- Undefined/racy tests are labeled and not assigned a canonical value.
- Resource-limit violations are rejected before execution.
- Article 8 teaches grids, workgroups, waves, lanes, divergence, memory, and synchronization.

### Phase 8 — Memory, race, and barrier sanitizers

**Objective:** Make SoftGPU more useful for correctness work than an opaque hardware run.

**Implement:** allocation and initialization shadow state; checked address-space accesses; happens-before race detection for a precisely declared subset; divergent/missing barrier detector; actionable structured diagnostics; replay bundle for each finding.

**Tests:** one positive and multiple negative kernels per finding class; overlap widths; masks; atomics; barrier generations; false-positive regression suite; shadow-memory limit behavior; stable diagnostic snapshots.

**Acceptance criteria:**

- Known out-of-bounds, use-after-free, uninitialized-read, race, and barrier defects in the supported subset are detected with kernel/workgroup/wave/lane context.
- Equivalent valid kernels pass without findings in the regression suite.
- Detection coverage and blind spots are documented, especially cross-workgroup and memory-order limitations.
- Every finding is deterministically replayable.
- Article 9 teaches GPU sanitization and the happens-before model.

### Phase 9 — Debugger and deterministic exploration

**Objective:** Provide inspectable, repeatable kernel debugging.

**Implement:** versioned trace/replay; breakpoints at supported IR locations; step/run controls; wave/lane/register/memory inspection; scheduler-seed exploration; trace minimization; source mapping only when verified.

**Tests:** breakpoint accuracy, state snapshots, replay across processes, trace corruption, budget exhaustion, deterministic minimization, debug-info absence.

**Acceptance criteria:**

- A failing sanitizer example can be replayed and inspected at the responsible access/barrier.
- Trace reader treats input as hostile and rejects corrupt records safely.
- Debugger output never invents source information.
- A bounded seeded schedule search can reproduce and minimize at least one ordering defect.
- Article 10 teaches deterministic GPU debugging.

### Phase 10 — gfx1201 ISA foundation

**Objective:** Begin genuine architectural ISA interpretation with evidence-driven, narrow coverage.

**Implement:** sourced instruction-word reader; architecture/feature validation; decoder for a minimal verified family; explicit machine state (PC, scalar/vector registers, condition/mask state, lane activity); trap model; disassembler cross-check; table generation only with provenance.

**Tests:** decoder bit-field goldens, reserved/invalid encodings, differential disassembly against official LLVM tools, single-instruction state transitions, fuzzing, exact unsupported-opcode traps.

**Acceptance criteria:**

- The decoder recognizes and executes a small named, sourced gfx1201 subset with byte-level golden tests.
- Unsupported or uncertain encodings trap before state is silently corrupted.
- Generated tables are reproducible and license-reviewed.
- Every instruction-semantic test cites its specification/observation source.
- Article 11 teaches ISA documentation, encoding, decoding, and emulator state.

### Phase 11 — First end-to-end gfx1201 kernel

**Objective:** Execute a tiny official-compiler-produced `gfx1201` code object through the real dispatch path and ISA engine.

**Implement:** only instruction families, descriptor behavior, calling convention state, and memory semantics required by the selected tiny kernel; keep all additions tested independently.

**Tests:** no-op/return, copy, elementwise add or similarly tiny kernels; compiler variants/optimization levels only after baseline; functional-vs-ISA differential; unsupported compilation variant failure.

**Acceptance criteria:**

- Real HIP userspace submits a real AQL dispatch referencing an official-compiler-produced gfx1201 code object, and the SoftGPU ISA engine produces the correct result.
- The complete artifact/toolchain/profile provenance is recorded.
- Required instruction coverage is enumerated; everything else remains unsupported.
- Results match functional mode and host reference; hardware comparison is added when available.
- Article 12 walks through the first real gfx1201 kernel.

### Phase 12 — R9700 hardware conformance and profile hardening

**Objective:** Replace provisional profile claims and emulator assumptions with repeatable physical-hardware evidence.

**Implement:** remote or attached conformance runner; signed result bundles; generated microtests; floating-point comparison policy; driver/runtime matrix; discrepancy triage workflow.

**Tests:** discovery/profile, resource limits, memory behavior, atomics, barriers, supported instruction families, edge FP values, error cases observable on both paths.

**Acceptance criteria:**

- Scheduled tests run on a real Radeon AI PRO R9700 and preserve device/toolchain/driver evidence.
- The support matrix distinguishes hardware-differential coverage from inference.
- Every discrepancy is classified as SoftGPU defect, test defect, unspecified behavior, version difference, or unresolved—with no suppressed mismatches.
- Profile fields used for legality are verified or explicitly remain unknown.
- Article 13 teaches conformance methodology and lessons from real hardware.

### Phase 13 — Profiles, packaging, and stable developer workflow

**Objective:** Make the verified subset usable without exaggerating scope.

**Implement:** profile versioning/migration; generic and multiple evidence-backed AMD profiles where available; CLI workflow; installation/uninstallation; reversible runtime selection; diagnostic bundles; release packaging; compatibility matrix.

**Tests:** clean-machine install, no-GPU Linux integration, profile schema migrations, downgrade/unknown fields, library-selection verification, uninstallation restoration, release reproducibility.

**Acceptance criteria:**

- A new user can build a supported sample on macOS, run core/functional tests, and use the Linux integration path from documentation.
- Runtime substitution is explicit, scoped to the launched process, and easy to verify/reverse.
- Profiles cannot enable unimplemented semantics merely by declaring features.
- Release notes enumerate exact supported APIs, artifacts, instructions, and known gaps.
- Article 14 teaches Mac-native development, Apple container, x86-64 ROCm CI, and multi-card profiles.

### Phase 14 — NVIDIA/CUDA/PTX feasibility and first vertical slice

**Objective:** Validate that vendor neutrality is real without destabilizing AMD support.

**Implement in two gates:** first, a documented study of current CUDA runtime/toolchain interception possibilities, distribution/licensing constraints, and PTX versions; second, if feasible, a minimal explicit PTX ingestion and execution path using the core scheduler/memory/sanitizer events.

**Tests:** PTX parser/verifier negatives, address spaces, warp masks, a tiny arithmetic/memory kernel, diagnostic terminology, AMD regression suite.

**Acceptance criteria:**

- The runtime boundary is chosen from current evidence, not analogy to ROCr.
- One tiny PTX kernel executes through a clearly named PTX mode, or the phase records a defensible blocked/no-go decision.
- Shared abstractions require no AMD semantic regression and do not erase NVIDIA distinctions.
- Unsupported CUDA/PTX features fail explicitly.
- Article 15 teaches the extension, differences, and architectural lessons.

### Phase 15 — Optional analytical performance model

**Objective:** Add useful, bounded analysis only after correctness foundations are credible.

**Implement:** event-derived counters; occupancy/resource calculator; optional configurable latency/bandwidth model; calibration/versioning; confidence reporting.

**Acceptance criteria:**

- Performance output is visually and programmatically distinct from correctness/conformance results.
- Predictions disclose assumptions and validation error against named hardware workloads.
- Disabling the model cannot change functional results.
- No claim of cycle accuracy is made without a separately validated model.
- Article 16 teaches what SoftGPU can and cannot predict.

## 16. Cross-phase release gates

No tagged release unless:

- all required CI jobs pass with no ambiguous skips;
- ABI and support matrices match the implementation;
- new binary parsers have negative and fuzz coverage;
- unsupported paths fail closed;
- structured diagnostics contain no accidental secrets;
- new profile facts have provenance;
- applicable articles and ADRs are current;
- dependencies and generated data pass license/provenance review;
- release notes state fidelity and hardware-validation scope;
- a clean installation and removal test passes.

## 17. Article and tutorial series mandate

Write the series in parallel with development, not retrospectively. Store sources in `docs/articles/`. Each article must correspond to executable code and evidence available at publication time. If implementation changes invalidate an article, update it in the same change.

Every article must include:

- intended audience and prerequisites;
- the problem and why the chosen boundary/design exists;
- a layered diagram using text or Mermaid;
- a small reproducible example tied to a repository tag/commit;
- commands tested in the stated environment;
- a “what we verified / what remains assumed” box;
- failure cases and unsupported semantics;
- security and portability implications where relevant;
- references to current primary sources with versions/access dates;
- lessons learned, surprises, rejected alternatives, and the next gate;
- no benchmark or compatibility claim beyond the recorded evidence.

Use original explanations and small original examples. Quote sparingly, follow source licenses, and never turn undocumented reverse-engineering observations into universal claims.

### Article 1 — Why build a developer-oriented virtual GPU?

Explain the product gap between functional emulation, developer testing, CPU fallbacks, ISA emulation, and architecture/performance simulation. Introduce SoftGPU's truthfulness and sanitizer-first goals.

### Article 2 — The GPU software stack from HIP to silicon

Teach host/device compilation, HIP runtime, ROCr/HSA, kernel driver, firmware, queues, and hardware. Explain why SoftGPU starts at the user-space ROCr/HSA boundary rather than mocking HIP or emulating PCIe.

### Article 3 — Impersonating a GPU without lying

Cover agents, discovery, C ABI compatibility, dynamic loading, device profiles, capability provenance, and fail-closed unsupported behavior.

### Article 4 — HSA queues and signals

Teach queue rings, packet ownership, producer/consumer ordering, doorbells, signals, waiting, wraparound, and why concurrency tests precede execution.

### Article 5 — AQL kernel dispatch under a microscope

Trace a real launch from HIP to a validated AQL packet. Explain grid/workgroup dimensions, kernel objects, kernargs, segment sizes, and completion signals.

### Article 6 — AMD code objects, ELF, metadata, and gfx targets

Explain offload/fat binaries, ELF, AMDGPU code-object versions, symbols/descriptors, metadata, target IDs, feature flags, `gfx1201`, inspection tools, and defensive parsing.

### Article 7 — Emulation versus simulation

Define functional execution, architectural ISA emulation, trace-driven analysis, timing simulation, and hardware conformance. Explain the selected functional input path and why it does not prove ISA compatibility.

### Article 8 — GPU execution concepts on a CPU

Teach grids, workgroups, waves, lanes, masks, divergence, reconvergence, address spaces, LDS/group memory, private memory, barriers, atomics, memory scope/order, and deterministic scheduling.

### Article 9 — Building GPU sanitizers

Teach shadow memory, bounds/lifetime/initialization tracking, happens-before race detection, SIMD-aware access events, barrier divergence, false positives/negatives, and replayable findings.

### Article 10 — Debugging a machine made of thousands of virtual lanes

Cover normalized events, trace budgets, deterministic replay, breakpoints, wave/lane state, scheduler exploration, trace reduction, and source mapping limitations.

### Article 11 — Decoding an AMD GPU ISA responsibly

Teach instruction encodings, decoder generation, target features, architectural state, wave execution, official-tool cross-checks, fuzzing, licensing/provenance, and failing on unknown opcodes.

### Article 12 — The first real gfx1201 kernel in SoftGPU

Walk from source through official compilation, code object, AQL dispatch, descriptor/calling state, decode, execution, completion, and comparison. Enumerate the tiny supported instruction set honestly.

### Article 13 — Validating against a real R9700

Teach differential test generation, environment capture, FP edge cases and tolerance, nondeterminism, discrepancy classification, conformance bundles, and how hardware evidence updates a device profile.

### Article 14 — Developing an AMD virtual GPU on an Apple Silicon Mac

Show the native Rust/Cargo loop, Apple `container` as Linux ARM64 lab, why official ROCm integration belongs in x86-64 Linux CI, artifact handoff, and eventual remote hardware testing. Explain that the container tool is optional infrastructure.

### Article 15 — From one R9700 profile to many GPUs

Teach identity versus architecture versus resource limits versus performance parameters; schema versioning; verified/observed/inferred/unknown fields; feature legality; profile inheritance hazards; and testing multiple cards without pretending they behave identically.

### Article 16 — Extending SoftGPU to NVIDIA, CUDA, and PTX

Compare the runtime boundaries and semantics rather than translating AMD names. Cover CUDA packaging/interception feasibility, PTX versions, warps, masks, address spaces, memory model, a future adapter/frontend, and when a shared IR becomes justified.

### Article 17 — What a functional emulator can—and cannot—say about performance

Explain counters, occupancy/resource analysis, analytical models, calibration, confidence, omitted microarchitecture, simulator validation, and why emulator wall time is meaningless as GPU time.

### Article 18 — Retrospective: building the virtual GPU as an evidence system

Summarize architectural decisions, reversals, upstream changes, bugs found by sanitizers, portability lessons, real-hardware discrepancies, security lessons, and the remaining research roadmap.

### Article 19 — Why Rust for a software GPU, and what Rust cannot prove

Publish early, then revise at the ABI, queue, parser, and ISA milestones. Explain why a Rust host-side emulator is distinct from compiling kernels in Rust; compare Rust and Zig honestly for this particular project; show C ABI exports, independently checked layouts, carefully bounded `unsafe`, callback/panic containment, handle ownership, safe parsing, and concurrency invariants. Show where compiler checks helped and where tests, traces, specifications, or real hardware were still required. Include a small auditable `unsafe` case study and a reproducible before/after defect. Discuss Rust GPU target/toolchain experiments separately from the real-HIP integration path. Frame the portfolio value as demonstrated engineering work, not an implied employment outcome.

## 18. Documentation and decision records

Create an ADR for decisions that are expensive to reverse or affect compatibility, including:

- ROCr/HSA substitution boundary;
- pinned Rust toolchain/MSRV and `unsafe`/FFI policy;
- dynamic-library/export strategy;
- handle representation and lifetime;
- queue/signal concurrency model;
- functional representation choice;
- whether/when a SoftGPU IR is introduced;
- scheduling and memory-order model;
- trace format;
- code-object parser dependency versus internal implementation;
- decoder data provenance/generation;
- profile schema;
- NVIDIA boundary decision;
- performance-model scope.

Each ADR records context, evidence, decision, alternatives, consequences, validation plan, and conditions that would trigger reconsideration.

`docs/status.md` must always answer:

- What works today?
- In which exact environment and fidelity mode?
- Which phase is active?
- What is the next acceptance gate?
- Which high-risk assumptions remain?

## 19. Implementation style guidance

- Favor explicit Rust newtypes, enums, and checked conversions over clever generic machinery. Use traits only where a proven adapter/engine seam benefits from them.
- Keep C ABI types at the boundary; convert immediately into validated internal types.
- Avoid global mutable state where the upstream ABI permits explicit runtime state; where process-global behavior is required, make initialization and teardown idempotence/thread safety explicit.
- Use resource budgets and bounded owned buffers in parsers, runtime objects, engines, and traces. Introduce allocator customization only when a measured need justifies it.
- Prefer error unions and rich internal diagnostic context; map to C status codes only at the boundary.
- Centralize `unsafe` pointer dereferencing and validate extent, alignment, aliasing, nullability, and lifetime before access; document the local safety invariant.
- Use compile-time layout assertions but also test against the pinned C compiler and headers.
- Keep fixtures tiny and explain how they were generated.
- Avoid adding a dependency when a small auditable implementation is safer; do not reimplement a mature, well-licensed component without a reason.
- Optimize only after profiles identify a real bottleneck. Preserve a simple reference executor as an oracle if a faster executor is added.
- Parallel CPU execution must be optional until deterministic equivalence and sanitizer correctness are proven.

## 20. Definition of done for any feature

A feature is not done until:

1. Its normative or observational basis is in `docs/sources.md`.
2. A test failed before implementation and now passes.
3. Negative, boundary, malformed-input, lifecycle, and concurrency cases applicable to it are tested.
4. Diagnostics identify unsupported or invalid use precisely.
5. The support matrix names the environment and verification level.
6. Relevant ABI/export, parser fuzz, differential, and hardware tests have run—or are explicitly marked not run.
7. Documentation, ADRs, examples, and the paired article are updated.
8. Security/resource-limit effects are considered.
9. No unrelated semantics are advertised.
10. The repository remains reproducibly buildable and testable.

## 21. First assignment

Begin with **Phase 0 only**.

1. Inspect the current repository without assuming it is empty. Preserve any existing work; do not convert an unrelated Zig inference project into SoftGPU.
2. Read current official primary sources and prepare the source ledger; do not copy ABI definitions from memory.
3. Propose the smallest repository skeleton needed for Phase 0.
4. Write tests for the profile schema/error behavior and CI smoke path before expanding modules.
5. Draft ADR-0001 for the ROCr/HSA substitution boundary, explicitly comparing: fake HIP, ROCr/HSA substitution, kernel-driver/device emulation, and direct code-object execution.
6. Draft Article 1 and the initial Article 19 Rust/FFI rationale with only claims supported by the current evidence.
7. Run and report the Phase 0 acceptance checks.
8. Stop at the gate and provide a concise handoff containing:
   - files changed;
   - commands/tests run and their results;
   - evidence added;
   - acceptance criteria met/unmet;
   - unsupported behavior;
   - risks and unresolved questions;
   - recommended smallest next step.

Do not implement the ROCr/HSA library until Phase 0 is accepted. Do not start AQL, code-object, functional-engine, sanitizer, or ISA production code merely to make the repository look complete.

## 22. End-state success definition

SoftGPU succeeds when it provides a trustworthy progression from:

```text
real HIP userspace loads SoftGPU
  -> virtual agent is discovered
  -> real AQL dispatch is intercepted
  -> real AMD code object is understood
  -> kernel executes functionally with GPU semantics
  -> memory/race/barrier bugs are diagnosed and replayed
  -> a verified gfx1201 subset executes architecturally
  -> results are continuously compared with a real R9700
  -> additional profiles and a separately researched PTX path reuse the core safely
```

The project is judged by the precision of its evidence and diagnostics, not by the length of its support list. A small, verified, fail-closed subset is more valuable than broad behavior that silently lies.
