# T02: Trait Contracts & Chip Dispatch (`src/chips/mod.rs`)

**Task ID:** T02
**Title:** LogicChip Trait Definition + Chip Enum Dispatch
**Depends On:** T01 (`src/bus.rs` must exist)
**Produces:** `src/chips/mod.rs`

---

## Paradigm Constraints (Recap)

| Constraint | Meaning for This Task |
|---|---|
| **Stateless Chips** | Every chip struct MUST be a unit struct (`struct FooChip;`). Zero fields. The compiler enforces statelessness — you CANNOT store state in a unit struct. |
| **No Chip-to-Chip Calls** | Chips NEVER call other chips. Data flows exclusively through the bus. The Motherboard iterates the pipeline. |
| **Single Trait Method** | One method: `tick(&self, pins: &InputPins, bus: &mut SystemBus)`. No `init()`, no `on_event()`, no `cleanup()`. |
| **Return Type** | `fn tick()` returns `()`. Chips communicate by writing to `bus.wires` and `bus` registers. No `Result`, no error propagation. Errors are "blown fuse signals" on wires. |
| **No Global State** | Chips read ONLY `pins` and `bus`. No global RNG, no file I/O, no environment variables. |
| **No Async** | `tick()` is a synchronous function. No `async fn`, no `.await`. |

---

## Implementation Goal

Create `src/chips/mod.rs` containing:

### 1. The `LogicChip` Trait

```rust
/// A stateless logic gate that performs one pure deduction per clock tick.
///
/// # Requirements
/// - Implemented by unit structs (zero fields).
/// - `tick()` reads `&InputPins` and reads/writes `&mut SystemBus`.
/// - `tick()` is deterministic: same inputs → same outputs.
/// - Chips NEVER call other chips. Data flow is through the bus.
///
/// # Borrow Checker Safety
/// `&self` (ZST) + `pins: &InputPins` + `bus: &mut SystemBus` are three
/// different allocations. Zero aliasing conflict.
pub trait LogicChip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus);
}
```

### 2. The `Chip` Enum (Dispatch)

A closed-set enum that wraps ALL chip types. This enables zero-heap pipeline storage (`Vec<Chip>`).

```rust
pub enum Chip {
    // Layer 0: Input Decoder
    InputDecoder(InputDecoderChip),
    // Layer 1: Game Rules & Collision
    GravityTimer(GravityTimerChip),
    DasTimer(DasTimerChip),
    LockDelayTimer(LockDelayTimerChip),
    CollisionDetector(CollisionDetectorChip),
    Rotation(RotationChip),
    Movement(MovementChip),
    // Layer 2: State Mutation
    PieceLocker(PieceLockerChip),
    LineClearDetector(LineClearDetectorChip),
    LineClearCommitter(LineClearCommitterChip),
    ScoreKeeper(ScoreKeeperChip),
    LevelCalculator(LevelCalculatorChip),
    HoldController(HoldControllerChip),
    SpawnController(SpawnControllerChip),
    // Layer 3: UI Transformation
    GhostComputer(GhostComputerChip),
}
```

### 3. Dispatch Implementation

```rust
impl LogicChip for Chip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus) {
        match self {
            Chip::InputDecoder(c)         => c.tick(pins, bus),
            Chip::GravityTimer(c)         => c.tick(pins, bus),
            Chip::DasTimer(c)             => c.tick(pins, bus),
            Chip::LockDelayTimer(c)       => c.tick(pins, bus),
            Chip::CollisionDetector(c)    => c.tick(pins, bus),
            Chip::Rotation(c)             => c.tick(pins, bus),
            Chip::Movement(c)             => c.tick(pins, bus),
            Chip::PieceLocker(c)          => c.tick(pins, bus),
            Chip::LineClearDetector(c)    => c.tick(pins, bus),
            Chip::LineClearCommitter(c)   => c.tick(pins, bus),
            Chip::ScoreKeeper(c)          => c.tick(pins, bus),
            Chip::LevelCalculator(c)      => c.tick(pins, bus),
            Chip::HoldController(c)       => c.tick(pins, bus),
            Chip::SpawnController(c)      => c.tick(pins, bus),
            Chip::GhostComputer(c)        => c.tick(pins, bus),
        }
    }
}
```

### 4. Forward-Declare All Chip Structs

Before the enum, declare (or use-import) all chip structs as unit structs. Since the actual implementations live in separate files, use `pub struct FooChip;` declarations here OR in `mod.rs`:

```rust
// Forward declarations of all chip types.
// Actual implementations are in separate files under src/chips/.
pub struct InputDecoderChip;
pub struct GravityTimerChip;
pub struct DasTimerChip;
pub struct LockDelayTimerChip;
pub struct CollisionDetectorChip;
pub struct RotationChip;
pub struct MovementChip;
pub struct PieceLockerChip;
pub struct LineClearDetectorChip;
pub struct LineClearCommitterChip;
pub struct ScoreKeeperChip;
pub struct LevelCalculatorChip;
pub struct HoldControllerChip;
pub struct SpawnControllerChip;
pub struct GhostComputerChip;
```

Alternatively, declare structs in their own files and `pub use` them in `mod.rs`.

### 5. File Organization Choice

Two valid approaches:

**Option A: Everything in `mod.rs`**
- Trait, enum, forward declarations all in `src/chips/mod.rs`
- Each chip's `impl LogicChip for FooChip` in a separate file (`src/chips/gravity_timer.rs`, etc.)
- `mod.rs` contains `mod gravity_timer; mod das_timer; ...` and `pub use` each struct

**Option B: Struct + impl in each file**
- Each file defines its own unit struct + impl
- `mod.rs` re-exports them and defines the `Chip` enum

**Recommendation: Option A for simplicity.** The trait and enum in `mod.rs`. Each chip file only contains its `impl LogicChip for XxxChip` block.

---

## Verification Protocol (Guardrail Agent B)

1. **Statelessness check:** Verify EVERY chip struct is a unit struct (no fields). Run `rg "pub struct \w+Chip" src/chips/` — every result must end with `;` (no braces with fields).
2. **Trait signature check:** The `LogicChip` trait has exactly ONE method: `tick`. Signature matches `fn tick(&self, pins: &InputPins, bus: &mut SystemBus)`.
3. **Return type check:** `tick()` returns `()` — no `Result`, no `Option`, no custom types.
4. **Enum exhaustiveness:** Every variant in `Chip` has a corresponding match arm in `impl LogicChip for Chip`. Run `cargo check` — missing arms produce compiler errors.
5. **No inter-chip calls:** Search for `.tick(` inside chip implementation files. The ONLY occurrence should be in the `impl LogicChip for Chip` dispatch match block in `mod.rs`. Chips never call other chips.
6. **Import check:** Verify `mod.rs` imports `InputPins` and `SystemBus` from `crate::bus`.
7. **Compile check:** After creating the file, run `cargo check`. It should compile even with no chip implementations (the structs exist, they just don't impl `LogicChip` yet — that's the task of T05/T06).
8. **No unsafe:** `grep -rn "unsafe" src/chips/mod.rs` returns nothing.

---

## Acceptance Criteria

- [ ] `src/chips/mod.rs` exists and compiles
- [ ] `LogicChip` trait defined with correct signature
- [ ] `Chip` enum contains all 15 variants (one per chip)
- [ ] `impl LogicChip for Chip` dispatches all variants
- [ ] All chip structs are unit structs (zero fields)
- [ ] No chip calls another chip directly
- [ ] `tick()` returns `()`
- [ ] Zero `unsafe` blocks
