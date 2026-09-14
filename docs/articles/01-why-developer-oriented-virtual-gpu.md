# Why we’re building SoftGPU

*A developer-oriented virtual GPU, built around a simple requirement: explain what happened, and be honest about what you know.*

Consider a kernel that passes every test until you change the size of the input. Now it occasionally returns the wrong answer. Add some logging and the problem disappears. Change the driver and it comes back differently.

The output tells you something went wrong. It does very little to explain why.

Perhaps a thread read shared memory before another thread finished writing it. Perhaps some threads skipped a barrier. Perhaps a pointer was valid when the work was submitted and invalid by the time it was used. Each possibility sends you into a different part of the stack, and a successful rerun does not settle any of them.

That is the problem I want SoftGPU to help with.

We are building a software GPU runtime for developers: somewhere to inspect execution, test assumptions, and eventually catch memory and synchronization mistakes with diagnostics we can reproduce. The first integration target is AMD HIP, with a Rust implementation underneath.

It is early. SoftGPU does not execute kernels yet. But the reason to build it, and the decisions that constrain it, are already concrete.

I want a tool that can earn my trust when it says a test passed.

The obvious starting point is real hardware. It remains essential. If you want to know how fast a kernel runs on a particular GPU, run it there. If you want to establish that your implementation agrees with that GPU, you need hardware evidence.

There are also useful debugging tools already. NVIDIA’s Compute Sanitizer, for example, checks CUDA applications for out-of-bounds and misaligned accesses, shared-memory hazards, uninitialized global-memory reads, and invalid synchronization. It would be unfair to justify SoftGPU by pretending these tools do not exist. [NVIDIA’s documentation](https://docs.nvidia.com/compute-sanitizer/ComputeSanitizer/index.html) describes their scope in detail.

What interests me is control over the execution environment. I want to be able to retain the events leading to a failure, inspect the state behind them, and eventually explore different permitted execution orders. I also want useful correctness checks to run without allocating a physical GPU to every development session or CI job.

A virtual runtime could give us that control. It would still need comparison against hardware. Passing inside our model would establish something about the behavior we modeled, with the checks we implemented. We would have to say exactly what that was.

Another obvious alternative is a CPU reference implementation. I like reference implementations. A straightforward version of an algorithm is often the best way to establish what the answer should be.

But matching the answer is only part of the problem. A sequential loop can compute the right result while telling us nothing about whether the GPU version uses its barriers correctly. A different backend can also change the memory behavior and runtime path that made the original failure possible.

There are more substantial options than writing a second loop by hand. [HIP-CPU](https://github.com/ROCm/HIP-CPU/blob/master/docs/overview.md) provides a CPU implementation of HIP using C++ parallel algorithms, and documents that its HIP implementation is incomplete. It is a relevant alternative when the objective is to run supported HIP source on a CPU.

Our objective includes keeping the official AMD compiler and HIP runtime in the compatibility path. We want to investigate what happens when that runtime discovers a device and submits work. Substituting a CPU implementation at the HIP level answers a different set of questions.

Functional emulators get closer to the execution problem. They can model instructions or operations and produce results without reproducing the timing of a physical device. That is useful territory, and SoftGPU’s roadmap includes a CPU-backed semantic engine.

The trap is treating every successful execution as the same kind of evidence. A model that computes the expected result has demonstrated that result under its own semantics. It has not automatically demonstrated the instruction behavior, memory ordering, or scheduling of a specific GPU. And an emulator that runs an extracted kernel directly may bypass the runtime interactions we also need to test.

SoftGPU has to account for those limits too. Calling it a virtual GPU does not exempt us from them.

At the other end are architecture and full-system simulators. If you are studying the interaction between a GPU, its memory system, and the rest of a machine, that broader model can be exactly what you need. [gem5’s full-system AMD GPU model](https://www.gem5.org/documentation/general_docs/gpu_models/gpufs), for example, involves a guest kernel and disk image, with the software environment forming part of the simulation setup.

For our first useful diagnostic, that is more machinery than I want to take on. Modeling a kernel driver, firmware, and device registers would bring a large body of work before we reached the developer experience that motivated the project. Performance research and everyday correctness debugging can share techniques without needing the same starting point.

Then there is the tempting shortcut: mock HIP. Implement enough familiar calls to make an application believe it has a GPU, return success, and get a demo running.

Mocks are useful when a test deliberately isolates application behavior. But as SoftGPU’s main compatibility strategy, that approach would make us responsible for our own interpretation of an evolving application-facing runtime. We could end up testing agreement between our mock and our examples while missing the behavior of the stack people actually use.

This is why the boundary matters so much.

For the AMD path we are targeting, the application uses HIP, and the HIP runtime reaches down into ROCr/HSA userspace before the driver and hardware. Our [architecture decision](../adr/0001-rocr-hsa-substitution-boundary.md) places SoftGPU at that ROCr/HSA boundary.

The intended arrangement keeps the real AMD HIP compiler and runtime. The launched process resolves a SoftGPU compatibility library in place of system ROCr. That library translates the supported runtime operations into our core.

This gives us concrete things to model: devices exposed as agents, queues that receive work, dispatch packets, completion signals, and compiled code objects. It also keeps runtime integration inside the problem we are testing.

There is a cost. We have to match the relevant C interface, structure layouts, exported symbols, and behavior expected by a pinned ROCm version. A library loading successfully is only the first step. Device discovery is another. Submitting work and executing it correctly are further steps, each requiring its own evidence.

The first integration test therefore has a deliberately modest job: prove that a HIP-linked process actually loaded SoftGPU’s library. If it accidentally loaded the system ROCr library, a green test would tell us nothing about our implementation.

That concern runs through the whole design. Unsupported behavior must fail explicitly. A function that returns success without doing the work can allow a test to pass for the wrong reason. Worse, it can move the failure somewhere else and leave the developer debugging the consequences of our shortcut.

I would rather see a precise unsupported-operation error than spend an afternoon trusting an operation that never happened.

Device profiles follow the same rule. Recording a device name and compiler target does not establish its numeric limits or instruction semantics. Our R9700 profile leaves numeric limits unknown where we have not established them. An empty field is useful information when the alternative is a guess disguised as a fact.

We also distinguish kinds of fidelity. Matching a runtime interface is different from implementing dispatch behavior. Functional execution is different from verified instruction semantics. Additional sanitizer checks can change scheduling and timing. Hardware agreement needs a named test, toolchain, and device.

Those distinctions should be visible in reports. Otherwise, “it works” can quietly grow from “this discovery call passed” into “this emulates the GPU,” without any new evidence in between.

Sanitizers are central to what we want to build next. Imagine a diagnostic that identifies an invalid access, the allocation it exceeded, and the work item responsible. Or a barrier report that explains which participants arrived and which took a different path. Those are examples of the experience we are aiming for, not features available today.

A useful implementation will need to retain enough execution history to explain those failures. Repeatability alone will not be enough: a fixed schedule can repeatedly miss a race. We will need to think carefully about which execution orders the model permits and which ones our tests explore. A CPU engine that casually serializes everything could hide the very mistakes we built it to find.

Rust is our host implementation language because ownership, explicit types, and structured errors fit this work well. They help us manage the runtime we are writing. They do not prove that a foreign interface is correct or that a GPU memory model has been implemented faithfully. Those questions still need specifications, independent probes, and tests. Application kernels remain on the real AMD HIP compiler path for compatibility evidence.

As of the repository’s September 13, 2026 status snapshot, the foundation includes profile validation, a minimal HSA compatibility library with explicit failure stubs, and virtual-agent discovery tested at unit level. The HIP-linked load-proof harness is implemented, but the project requires a green pinned Linux x86-64 integration job before treating that path as verified. Queues, dispatch packets, kernel execution, and kernel sanitizers remain future work. The [status document](../status.md) tracks those boundaries.

That leaves a lot to build. It also gives us a sensible sequence: establish that the application reaches us, establish that we understand the work it submits, then execute a declared subset and explain its behavior. Each step should leave behind evidence that the next step can rely on.

Real GPUs, CPU references, sanitizers, and simulators will remain part of the workflow. SoftGPU will have to demonstrate where it adds value alongside them.

The result I want is practical: a small failing test, an explanation of what went wrong, and enough recorded context for another developer to reproduce it. If we can make that a dependable part of GPU development, this will have been worth coding.
