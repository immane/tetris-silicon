# Tetris Silicon

A Rust terminal Tetris implementation built with the **Silicon-Based Software Architecture Paradigm**.

Instead of object call chains, this project models software as a synchronous digital circuit:
- **Input Pins** sample external signals
- A flat **System Bus** stores all global state
- Stateless **Logic Chips** perform isolated deductions
- A **Motherboard Clock** drives deterministic ticks

The result is a codebase that is highly deterministic, easy to reason about, and especially friendly to AI-assisted development.

## Terminology Conventions

This document uses the following terms consistently:
- **Input Pins**: external sampled inputs for the current tick only
- **System Bus**: global state carrier, including registers and wires
- **Registers**: persistent state across ticks
- **Wires**: per-tick ephemeral signals, reset at tick start
- **Logic Chips**: stateless transition units
- **Motherboard**: deterministic scheduler of chip layers
- **Tick**: one full sample-propagate-latch cycle

When discussing future portability, **SFL (Silicon Formal Language)** is the semantic source of truth,
while Rust/C/CUDA/HDL implementations are backend realizations.
The formal multi-backend contract is documented in [docs/architecture/SFL_CONTRACT.md](docs/architecture/SFL_CONTRACT.md).

## Why This Project Exists

Traditional game code is often organized around classes, mutable object graphs, and implicit control flow. That style can scale, but it frequently introduces:
- Hidden state transitions
- Hard-to-replay bugs
- Tight coupling across modules
- High context load for human and AI contributors

`tetris-silicon` explores a different approach: software architecture inspired by FPGA/ASIC design principles.

## Silicon Paradigm in One Screen

```text
External I/O -> InputPins (frozen snapshot)
                  |
                  v
        +-----------------------+
        |   SiliconMotherboard  |
        |  layer[0..N] pipeline |
        +-----------------------+
                  |
                  v
       SystemBus Registers + Wires
```

Clock lifecycle per tick:
1. **Sampling**: poll external input into `InputPins`
2. **Combinational Propagation**: chips read Input Pins/System Bus and write wires
3. **Sequential Latching**: motherboard commits edge/state for next tick

## Project Architecture

Core files:
- `src/bus.rs`: board dimensions, timing constants, `InputPins`, `Wires`, `SystemBus`
- `src/chips/`: stateless chip implementations (`GravityTimer`, `Rotation`, `Movement`, etc.)
- `src/motherboard.rs`: chip pipeline topology and `clock_tick` driver
- `src/main.rs`: wall-clock loop, terminal I/O, rendering schedule
- `src/tui.rs`: pure rendering from bus state
- `src/terminal.rs`: non-blocking key polling and raw-mode guard

This repository follows the architecture described in `docs/architecture/SILICON_PARADIGM_SPEC.md`.

![TETRIS-Silicon Architecture Diagram](docs/images/tetris-silicon-arch.png)

## What Makes This Paradigm Powerful

### 1) Determinism by Construction

All game state is centralized in `SystemBus`, and every update happens on explicit clock ticks.
Given the same input sequence and seed, behavior is reproducible.

### 2) Zero Hidden Side Effects

Chips do not call each other. They only communicate through bus fields.
Data flow is explicit in topology and tick order.

### 3) Strong Single Responsibility

Each chip does one thing (decode input, compute gravity pulse, resolve rotation, lock piece, score lines).
This reduces cognitive load and makes local modifications safer.

### 4) Testability and Formal Verification Potential

The architecture naturally fits simulation-style testing, fuzzing, and property checks:
- invariant checks on board bounds
- deterministic replay with recorded pins
- phase-aware assertions per tick

### 5) AI-Friendly Context Shape

The paradigm improves AI coding quality in practical ways:
- **Context locality**: `bus.rs` acts as the global contract
- **Prompt precision**: tasks can be phrased as exact state transforms
- **Safe composition**: new behavior is often adding/reordering chips in motherboard layers

## AI Development in the Silicon Paradigm (Complete Workflow)

This section describes how to build complex features with AI agents using this paradigm.

### A) Contract-First Design

1. Extend `SystemBus` and/or `Wires` with new signals/registers.
2. Define clear invariants (ranges, lifecycle, reset policy).
3. Keep names electrically meaningful (`*_requested`, `*_tick`, `*_expired`, `*_triggered`).

Why this helps AI:
- The bus is a machine-readable source of truth.
- Field names encode semantics, reducing ambiguity.

### B) Stateless Chip Synthesis

1. Ask AI to implement one chip with one responsibility.
2. Input to chip: `&InputPins`, `&mut SystemBus`.
3. Output from chip: System Bus register/wire mutations only.
4. No cross-chip calls; no hidden cache/state.

Prompt template:

```text
Implement <ChipName> in src/chips/<file>.rs.
Rules:
- Read only: InputPins + relevant bus fields
- Write only: listed wire/register outputs
- No calls to other chips
- Preserve determinism
- Respect phase semantics (wires are per-tick)
```

### C) Topology Integration on Motherboard

1. Insert the chip into the correct layer in `src/motherboard.rs`.
2. Verify ordering dependencies (upstream signals available before downstream consumption).
3. Keep comments documenting order contracts.

Why this helps AI:
- Integration is usually a local, explicit edit.
- Minimal blast radius versus deep API plumbing.

### D) Tick-Level Verification

1. Build testbenches/property tests around `clock_tick` behavior.
2. Feed deterministic pin sequences.
3. Assert invariants after each tick.
4. Add regression tests for discovered edge cases.

Suggested properties:
- active piece never escapes board bounds unless game-over transition
- lock/spawn ordering remains valid
- line-clear and score updates are monotonic and phase-correct

### E) Iterative Agentic Loop

For each feature:
1. Update bus contract
2. Generate chip
3. Integrate layer position
4. Run tests/fuzz
5. Record invariant docs

This loop scales to complex systems because each iteration is small, deterministic, and composable.

## Constraints (Physical Purity Rules)

To preserve silicon-style semantics:
- No global mutable state (outside tick-scoped bus mutation)
- No implicit business control flow via panic/exception patterns
- No chip privilege escalation (touch only fields relevant to chip duty)
- Prefer simulation/property-based testing for systemic correctness

## Running the Game

Requirements:
- Rust stable toolchain
- Terminal with ANSI support

```bash
cargo run --release
```

Optional CUDA backend (minimal integration path):

```bash
cargo run --release --features cuda
```

```bash
TETRIS_BACKEND=cuda cargo run --release --features cuda
```

CUDA chip routing policy (contract-aligned mixed execution):

All 15 chips can run via CUDA backend:
- **3 chips** have GPU kernels (CollisionDetector, LineClearDetector, GhostComputer)
- **12 chips** run on CPU via LogicChip trait interface (automatic fallback, no GPU kernel)

```bash
# Default: all chips enabled, GPU kernels execute on GPU, others on CPU
TETRIS_BACKEND=cuda cargo run --release --features cuda

# Force CPU for all chips (still uses CUDA runtime selection path)
TETRIS_BACKEND=cuda TETRIS_CUDA_CHIPS=none cargo run --release --features cuda

# GPU kernels only (3 chips)
TETRIS_BACKEND=cuda TETRIS_CUDA_CHIPS=CollisionDetector,LineClearDetector,GhostComputer cargo run --release --features cuda

# Custom list (comma-separated chip names; unlisted chips run on CPU)
TETRIS_BACKEND=cuda TETRIS_CUDA_CHIPS=CollisionDetector,GhostComputer cargo run --release --features cuda
```

If CUDA is unavailable at runtime, the engine falls back to CPU automatically.

Controls:
- Move: `Left/Right` or `h/l`
- Soft drop: `Down` or `j`
- Rotate CW: `Up` or `x`
- Rotate CCW: `z`
- Hold: `c`
- Hard drop: `Space`
- Quit: `Esc`

## Development Commands

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Repository Layout

```text
src/
  bus.rs
  motherboard.rs
  main.rs
  terminal.rs
  tui.rs
  chips/
    mod.rs
    *.rs
tests/
docs/
```

## Mathematical Verification

### What Is Formally Provable Today

The Silicon paradigm significantly narrows the gap between running code and mathematical proof because
its structure maps directly onto well-studied formalisms.

**Mealy / Moore Finite State Machine (FSM)**

The entire system is a deterministic FSM where:

$$S_{t+1} = F(S_t,\ I_t)$$

- $S_t$ is the complete `SystemBus` register snapshot at tick $t$
- $I_t$ is the frozen `InputPins` sample at tick $t$
- $F$ is the pipeline of chips applied in fixed topological order

Because $F$ is pure and total, the system is fully characterisable as a mathematical function.
Every observable behavior can in principle be proven from the definition of $F$ and $S_0$.

**Chip-Level Denotational Semantics**

Each chip $C_i$ is a state transformer parameterised by the frozen input:

$$C_i(I_t) : \text{SystemBus} \to \text{SystemBus}$$

> Note: in the actual Rust implementation chips mutate `&mut SystemBus` in-place rather than
> returning a new value. The notation above models the observable effect; the implementation
> is equivalent under the assumption that no chip reads a field it has already written in the
> same tick.

The full tick is a sequential fold over the pipeline layers:

$$F(S_t, I_t) = C_n(I_t)\bigl(\cdots C_1(I_t)(S_t)\cdots\bigr)$$

Because chips are stateless and share no hidden references, each can be reasoned about in isolation
given knowledge of which bus fields it reads and writes.

**Invariant Specification as Predicate Logic**

System invariants become first-order predicates over `SystemBus`:

```text
∀ t ≥ 0 :
  0 ≤ piece_x ≤ BOARD_COLS - piece_width
  0 ≤ piece_y ≤ BOARD_ROWS - piece_height
  ¬ collides(piece_x, piece_y, piece_type, piece_rotation, board)
  game_phase = GameOver → ¬ should_spawn_next
```

These predicates can be machine-checked via:
- **Property-based testing** (`proptest`, already a dev-dependency in this project) for statistical coverage
- **Bounded model checking (BMC)** tools such as KLEE or SeaHorn by compiling Rust to LLVM IR
- **TLA⁺ / Alloy** by expressing the tick function and bus schema directly in those specification languages

**What Has Been Done in This Codebase**

- `tests/invariants.rs` already encodes board-level invariants as proptest properties
- `tests/invariants.proptest-regressions` captures confirmed regression seeds

**What Remains Open**

Formal proof of liveness (e.g., "the game always eventually spawns a new piece given valid input") and full
termination proofs require interactive theorem provers such as Coq or Lean 4.
This is theoretically tractable but non-trivial: the reactive system with stochastic input (LCG
piece generator) requires explicit modelling of the random stream, and liveness arguments must
account for all possible input sequences. Substantial proof engineering effort is needed.

---

## Silicon Formal Language Outlook (Post-Rust Core)

### Goal

Evolve the paradigm from a Rust implementation style into a language-independent formal specification layer.
Rust then becomes one backend, not the definition of truth.

### Proposed Core: SFL (Silicon Formal Language)

SFL is a declarative intermediate form that defines:
- bus schema (registers, wires, reset policy)
- chip contracts (read-set, write-set, phase, determinism constraints)
- motherboard topology (layer order, dependency edges, forbidden write conflicts)
- tick semantics (sample, propagate, latch) as formal transition rules

The canonical system meaning is specified in SFL semantics; generated Rust/C/CUDA/HDL code is a backend realization.
For the full contract boundary, supported backend classes, and adapter rules, see [docs/architecture/SFL_CONTRACT.md](docs/architecture/SFL_CONTRACT.md).

### Backend Targets

1. Rust backend: safe reference implementation and fastest iteration path.
2. C backend: portability and access to mature compiler/HLS ecosystems.
3. CUDA backend: parallel chip groups and board-scan kernels on GPU.
4. FPGA/ASIC backend: via HLS/RTL lowering for selected deterministic kernels.
5. Quantum-oriented backend: interface-layer orchestration and reversible-subset experiments.

### Equivalence Requirement

All backends must satisfy semantic equivalence against SFL under the same input stream and initial state:

$$\forall t\ge 0,\; \text{State}^{(backend)}_t = \text{State}^{(SFL)}_t$$

In practice this should be enforced through:
- cross-backend differential simulation
- shared regression seeds and trace replay
- backend conformance suites at chip and full-tick levels

### Engineering Boundaries

- "One-click to any backend" is a long-term objective, not a current guarantee.
- GPU/FPGA lowering is realistic first for narrow, compute-heavy chip subsets.
- Quantum execution is currently an interface-level strategy; full-system quantum lowering is speculative.
- Rust remains the primary proving ground while SFL and conformance tooling mature.

---

### Compiler as Partial Design Rule Checker

For the Rust backend, Rust's type system and borrow checker enforce a meaningful subset of physical laws automatically:

| Physical Rule | Enforcement mechanism |
|---|---|
| No two chips mutate bus simultaneously | `rustc`: exclusive `&mut SystemBus` reference |
| `InputPins` are read-only during tick | `rustc`: `&InputPins` immutable borrow |
| `Wires` are ephemeral (cleared per tick) | **runtime convention only** — `bus.wires = Wires::default()` called by `clock_tick`; the compiler does not enforce this |
| Chips carry no hidden state | `rustc`: zero-field unit structs; `LogicChip` trait has no `&mut self` |

`rustc` is therefore a partial DRC (Design Rule Check) — it eliminates whole categories of
timing violations and aliasing bugs structurally. It does **not** verify business logic correctness.
That requires the formal methods described above.

---

## Theoretical Optimization Horizons

### Automatic Parallelism via Data-Flow Analysis

Because the chip pipeline is a static DAG with explicit read/write bus fields, a compiler or
analysis tool can in principle derive:

1. Which chips have no read-write overlap on the bus → safe to execute in parallel
2. Which chip subsets form independent data-flow islands → packagable as SIMD micro-batches
3. Whether the full pipeline can be fused into a single LLVM function call with no intermediates

This is equivalent to standard **compiler auto-vectorisation** and **loop fusion** analysis, both of
which are solved problems. The barrier is tooling: today this analysis must be done manually or
semi-automatically with profiler feedback. Automation requires a meta-compiler that is aware of
the bus field dependency graph — feasible to build as a proc-macro or code-gen step.

**Engineering status**: Manually achievable now for obvious independent chips. Full automated
dependency extraction is future work.

### LLVM Extreme Optimization

The tick function compiles to a hot loop with:
- no dynamic dispatch (closed enum, fully monomorphised via `match`)
- no heap allocation *during* a tick (the pipeline `Vec<Vec<Chip>>` is allocated once at startup and never resized)
- all mutable state in a single contiguous struct (`SystemBus`)
- arithmetic and branch patterns visible to the optimizer across chip boundaries after inlining

This gives LLVM's optimisation passes maximum visibility. With link-time optimisation (`lto = "fat"`)
and `codegen-units = 1`, the entire tick pipeline can be inlined into one function, enabling:
- global constant propagation across chip boundaries
- dead-code elimination of unused wire paths
- auto-vectorisation of board scans (e.g., `collides`, `ghost_y`)

**Engineering status**: Achievable today by adding to `Cargo.toml`:

```toml
[profile.release]
lto = "fat"
codegen-units = 1
opt-level = 3
```

### High-Level Synthesis (HLS) to Physical Silicon

High-level synthesis tools (Intel HLS Compiler, Xilinx Vitis HLS, Bambu) translate C/C++ or
SystemC into RTL (register-transfer level) descriptions that map to FPGA fabric or ASIC cells.

The Silicon paradigm's structure is **closer to the HLS input model than most software** because:
- State is a flat register file (`SystemBus` ↔ RTL register map)
- Logic is stateless combinational functions (chips ↔ HLS combinational blocks)
- Clock is explicit and uniform (Lamport tick ↔ synchronous clock domain)
- No dynamic memory allocation, no recursion, no runtime polymorphism

Remaining gaps between this codebase and true HLS input:
- Rust is not a supported HLS input language (C/C++ is); a transpiler step is required
- Loop trip counts must be statically known for hardware scheduling (e.g., `BOARD_ROWS` as const — already satisfied here)
- Timing constraints, resource budgets, and clock domain crossings need hardware-specific annotations
- Memory interfaces (`board` array) must map to block RAM or register arrays with declared latency

**Engineering status**: Not achievable directly. The conceptual mapping is sound, but requires
translating Rust to C with fixed-bound loops and feeding it to an HLS toolchain.
A more practical path is writing performance-critical chips (collision, gravity) as annotated C
and synthesising those independently. Full system HLS remains long-term research.

---

## Quantum Computing Prospects

### What Applies and What Does Not

Quantum computers operate on fundamentally different primitives: superposition, entanglement, and
interference. The Silicon paradigm maps well to **classical deterministic computation**, which is
the opposite regime from quantum. This means the paradigm itself does not transfer directly.

However, the structural properties of the paradigm create genuine opportunities at the
**interface layer** between classical and quantum systems.

### Near-Term Applicability: Classical Orchestration of Quantum Circuits

Quantum programs require a classical outer loop to:
- initialise qubit states and circuit parameters
- submit circuits to a quantum processing unit (QPU)
- measure, collect, and decode results
- conditionally update classical state and re-submit

This classical control layer is an excellent fit for the Silicon paradigm:

```text
InputPins   ← measurement results from QPU + wall-clock
SystemBus   ← classical state: circuit parameters, iteration count, result buffer
Chips       ← ParameterUpdateChip, CircuitCompilerChip, MeasurementDecoderChip
Motherboard ← one tick per QPU round-trip
```

This is not hypothetical: variational quantum algorithms (VQE, QAOA) already use exactly this
feedback loop. A Silicon-paradigm classical controller would make that loop deterministic,
testable, and AI-composable in the same way as the game engine.

**Engineering status**: Directly applicable today using QPU cloud APIs (IBM Qiskit, AWS Braket).
The chip-level units are well-defined. Implementation is a design exercise, not a research problem.

### Medium-Term: Reversible Computing Alignment

Quantum gates are **unitary and reversible** by physical law. The most efficient path from classical
software to quantum circuits is through **reversible classical logic**, where every operation can
be undone without information loss.

Chips in the Silicon paradigm, being pure functions, are candidates for reversible implementation
if their outputs uniquely determine their inputs (i.e., the chip function is injective or bijective).
Reversible chips could in principle be compiled to quantum gate sequences (Toffoli, Fredkin) via
tools such as RevKit or Tweedledum.

**Engineering assessment**: Mathematically coherent for pure chips with no irreversible
information destruction (e.g., `CollisionDetectorChip` writes a boolean from position data —
irreversible). Only a subset of chips would be candidates. Full system reversibility would require
redesigning chips to preserve ancilla bits, which conflicts with the simplicity goals of this paradigm.
This is a research direction, not an engineering roadmap.

### Long-Term Speculation: Quantum Advantage for Specific Chips

If a chip implements a function for which a quantum speedup is known — for example, a
chip that performs database search over the board state (Grover's algorithm offers
$O(\sqrt{N})$ vs $O(N)$) or optimisation over piece placement (QUBO formulation) — then that
chip could in principle be replaced by a quantum subroutine while the rest of the classical pipeline
remains unchanged.

The plug-and-play composability of the paradigm makes this **architecturally clean**: the chip
interface (`&InputPins`, `&mut SystemBus`) is independent of the chip's internal implementation.
A quantum-backed chip is structurally indistinguishable from a classical one.

**Engineering assessment**: The interface is compatible. The practical barriers are:
- Current QPUs have high latency (milliseconds per circuit) versus the nanosecond tick budget
- Qubit counts and error rates limit problem sizes to toy scale today
- The functions implemented by game logic chips (collision, gravity) have no known quantum advantage
  — they are simple enough that classical hardware is optimal

This is genuinely long-term (5–15 year) speculation contingent on fault-tolerant quantum hardware.

### Summary Table: Quantum Prospects

| Direction | Feasibility | Timeline | Barrier |
|---|---|---|---|
| Classical orchestration of QPU feedback loops | High | Now | None — design exercise |
| Reversible chip compilation to quantum gates | Medium (subset only) | 2–5 years | Reversibility redesign required |
| Quantum subroutine as drop-in chip replacement | Low (for game domain) | 5–15 years | QPU latency and qubit scale |
| Full system quantum execution | Not applicable | — | Wrong computational model |

---

## Engineering Scorecard

| Claim | Verdict | Evidence |
|---|---|---|
| AI chip generation accuracy is high | **True with conditions** | Stateless, single-responsibility prompts significantly reduce hallucination surface; correctness still requires test verification |
| `rustc` replaces manual code review | **Partially true** | Eliminates aliasing, memory, and type bugs; does not verify business logic |
| Deterministic replay is guaranteed | **True with conditions** | Same `InputPins` sequence + identical initial `SystemBus` (including `prng_state`) → identical evolution; currently `prng_state` is hardcoded at init, no public seed-injection API yet |
| Auto-parallelism is unlocked | **Potential, not automatic** | DAG structure enables analysis; requires tooling to extract and exploit |
| LLVM produces near-optimal machine code | **True with LTO** | Monomorphic, heap-free, inlinable tick loop; standard release profile already benefits |
| HLS to silicon is a near-term path | **False** | Conceptually aligned but requires Rust→C transpilation and hardware-specific annotations |
| Quantum computing integration is viable | **Partially, at interface layer** | Classical orchestration applies now; chip-level quantum substitution is long-term speculation |
| The paradigm eliminates all bugs | **False** | Eliminates structural/concurrency bugs; business logic bugs remain; requires tests + formal methods |

---

## Strategic Visions (Validated)

The following visions are promising, but they require explicit scope boundaries to remain
engineering-realistic.

### 1) Silicon Compiler / Auto-Synthesis Toolchain

**Vision**: Build a toolchain (Rust `syn`/`quote` + dependency-graph analyzer) that can ingest a
bus contract and auto-generate or auto-wire chips from prompts.

**Validated potential**:
- High for code generation, static checks, and motherboard integration scaffolding
- Medium for fully automatic chip correctness without human review

**What is realistic now**:
- Parse chip ASTs and extract read/write sets for each bus field
- Build a chip dependency DAG and detect unsafe write conflicts
- Auto-insert chips into valid motherboard layers based on declared dependencies
- Generate chip skeletons from templates tied to bus contract fields

**What is not realistic yet**:
- "One prompt, fully correct chip at scale" without test or formal-validation gates
- Safe autonomous merge from thousands of agents without conflict arbitration

**Near-term milestones (0-6 months)**:
1. `chip-analyzer`: static read/write graph extractor
2. `chip-linter`: privilege-boundary checker (chip can only touch declared fields)
3. `chip-scaffold`: prompt-to-template code generator
4. `auto-layerer`: deterministic chip placement proposal + conflict report

### 2) Formal Verification and Provable Software Loop

**Vision**: Deliver mathematically justified cores for high-assurance domains (games, trading,
autonomous driving kernels, blockchain VM subsystems).

**Validated potential**:
- High for safety invariants and bounded correctness
- Medium for full liveness/progress proofs
- Low for "provably bug-free" as a blanket claim across whole real-world systems

**Engineering truth**:
- "Provably bug-free" is achievable only relative to a formal spec and proof assumptions
- Most practical programs can reach "proof-backed critical invariants + tested behavior" rather than
  total correctness end-to-end

**Practical loop**:
1. Formalize bus invariants (TLA+, Alloy, or theorem prover model)
2. Enforce property tests and regression seeds in CI
3. Add bounded model checks for critical chip clusters
4. Gate releases on proof obligations for safety-critical fields

### 3) Heterogeneous Silicon System

**Vision**: Keep one silicon-style contract while dispatching chips across heterogeneous backends
(CPU scalar, SIMD, GPU kernels, FPGA/HLS candidates, and optional remote accelerators).

**Validated potential**:
- High for CPU/SIMD/GPU mixed execution
- Medium for FPGA offload of selected kernels
- Low for whole-system transparent heterogenous scheduling without strict runtime contracts

**Key constraints**:
- Backend boundary must preserve deterministic tick semantics
- Cross-device latency must fit the clock budget
- Data marshaling costs can dominate arithmetic speedups

**Near-term milestones (0-12 months)**:
1. Define backend-agnostic chip ABI (inputs, outputs, determinism contract)
2. Implement CPU baseline + SIMD board-scan kernels
3. Prototype GPU offload for embarrassingly parallel analyses
4. Benchmark deterministic equivalence across backends

### 4) AI-Native End-to-End Platform

**Vision**: Platformize the full lifecycle: contract authoring, chip synthesis, dependency analysis,
verification, simulation, and safe merge orchestration.

**Validated potential**:
- High for productivity and consistency
- Medium for autonomous end-to-end operation in safety-critical domains

**Required guardrails**:
- Policy layer: field-level write permissions per chip
- Verification layer: tests/proofs as merge gates
- Traceability layer: every generated chip linked to prompt, model version, and proof/test artifact
- Rollback layer: deterministic replay and bisect on any failed invariant

**Success metric**:
Not "zero human involvement", but measurable reduction in lead time while preserving or improving
defect density and post-release incident rates.

## Roadmap Ideas

- Replace LCG piece generation with configurable 7-bag while keeping deterministic seed injection
- Add headless environment API (`reset/step`) for RL and planning agents
- Introduce bitboard-backed board representation for high-throughput simulation
- Build formal invariant suites for lock-delay, line-clear, and rotation kick correctness
- TLA⁺ specification of the tick state machine for bounded model checking
- Automated bus field dependency graph extractor (proc-macro) for parallelism analysis
- Reversible chip prototype for collision detection (research experiment)
- Build `chip-analyzer` + `auto-layerer` as first Silicon Compiler modules
- Add backend-agnostic chip ABI and deterministic cross-backend conformance tests
- Add provenance tracking for AI-generated chips (prompt/model/test-proof linkage)

## License

MIT — see [LICENSE](LICENSE).
