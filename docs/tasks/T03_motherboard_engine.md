# T03: Motherboard Engine (`src/motherboard.rs`)

**Task ID:** T03
**Title:** SiliconMotherboard — Pipeline Orchestration & Clock Tick Driver
**Depends On:** T01 (`src/bus.rs`), T02 (`src/chips/mod.rs`)
**Produces:** `src/motherboard.rs`

---

## Paradigm Constraints (Recap)

| Constraint | Meaning for This Task |
|---|---|
| **3-Phase Lifecycle** | `clock_tick()` MUST execute: (0) Wire Reset → (1) Combinational Propagation → (2) Sequential Latching. Exactly this order, every tick. |
| **No Chip Calls Chips** | The Motherboard, and ONLY the Motherboard, iterates the pipeline and invokes `chip.tick()`. Chips never invoke each other. |
| **Layered Propagation** | Chips execute in layer order. All chips in Layer N complete before any chip in Layer N+1 begins. |
| **Wire Reset** | `bus.wires = Wires::default()` MUST happen at the START of every tick, before any chip runs. |
| **Latching** | Edge-detection state (`prev_key_*`) and phase transitions MUST be latched at the END of the tick, after all chips have run. |
| **No Heap in Hot Path** | The motherboard itself may use `Vec<Chip>` for layer storage (allocated once at startup). No per-tick allocation. |

---

## Implementation Goal

Create `src/motherboard.rs` containing:

### 1. SiliconMotherboard Struct

```rust
use crate::bus::{InputPins, SystemBus, Wires, GamePhase};
use crate::chips::{Chip, LogicChip};

/// The SiliconMotherboard physically arranges LogicChips in a 4-layer
/// pipeline and provides the clock tick driver.
///
/// # Layers
/// 0: Input Decoder
/// 1: Game Rules & Collision
/// 2: State Mutation
/// 3: UI Transformation
pub struct SiliconMotherboard {
    /// `layers[i]` holds all chips at pipeline stage i.
    /// Chips within a layer execute sequentially.
    /// Layers execute in order: 0 → 1 → 2 → 3.
    pub layers: Vec<Vec<Chip>>,
}
```

### 2. Constructor

```rust
impl SiliconMotherboard {
    /// Create a motherboard with pre-populated layers.
    /// Chip ordering within each layer is critical — see comments.
    pub fn new() -> Self {
        Self {
            layers: vec![
                // Layer 0: Input Decoder
                vec![
                    Chip::InputDecoder(InputDecoderChip),
                ],
                // Layer 1: Game Rules & Collision
                // ORDER IS CRITICAL: Timers → Collision → Rotation → Movement → LockDelay
                vec![
                    Chip::GravityTimer(GravityTimerChip),
                    Chip::DasTimer(DasTimerChip),
                    Chip::CollisionDetector(CollisionDetectorChip),
                    Chip::Rotation(RotationChip),
                    Chip::Movement(MovementChip),
                    Chip::LockDelayTimer(LockDelayTimerChip),
                ],
                // Layer 2: State Mutation
                // ORDER: Lock → Hold → Clear → Score → Level → Spawn
                vec![
                    Chip::PieceLocker(PieceLockerChip),
                    Chip::HoldController(HoldControllerChip),
                    Chip::LineClearDetector(LineClearDetectorChip),
                    Chip::LineClearCommitter(LineClearCommitterChip),
                    Chip::ScoreKeeper(ScoreKeeperChip),
                    Chip::LevelCalculator(LevelCalculatorChip),
                    Chip::SpawnController(SpawnControllerChip),
                ],
                // Layer 3: UI Transformation
                vec![
                    Chip::GhostComputer(GhostComputerChip),
                ],
            ],
        }
    }
}
```

### 3. clock_tick: The 3-Phase Orchestrator

```rust
    /// Execute one full clock cycle.
    ///
    /// # Phases
    /// 0. WIRE RESET — Pull all wires to ground (default).
    /// 1. COMBINATIONAL PROPAGATION — Signals flow through the 4-layer pipeline.
    ///    Each chip reads InputPins and writes to SystemBus wires/registers.
    /// 2. SEQUENTIAL LATCHING — Commit edge-detection state and phase transitions
    ///    for the next tick. Increment Lamport clock.
    ///
    /// # Parameters
    /// - `pins`: Frozen InputPins snapshot from the Sampling Phase (done in main.rs)
    /// - `bus`: Mutable reference to the single SystemBus
    pub fn clock_tick(&mut self, pins: &InputPins, bus: &mut SystemBus) {
        // ═══ PHASE 0: Wire Reset ═══
        // All wires pulled to ground before signal propagation begins.
        bus.wires = Wires::default();

        // ═══ PHASE 1: Combinational Propagation ═══
        // Signals flow through layers sequentially. Within each layer,
        // chips execute in insertion order. Upstream chips write wires
        // that downstream chips read — all within the same tick.
        for layer in &self.layers {
            for chip in layer {
                chip.tick(pins, bus);
            }
        }

        // ═══ PHASE 2: Sequential Latching (falling edge) ═══
        // Latch key state for next tick's edge detection.
        bus.prev_key_left   = pins.key_left;
        bus.prev_key_right  = pins.key_right;
        bus.prev_key_down   = pins.key_down;
        bus.prev_key_up     = pins.key_up;
        bus.prev_key_z      = pins.key_z;
        bus.prev_key_x      = pins.key_x;
        bus.prev_key_c      = pins.key_c;
        bus.prev_key_space  = pins.key_space;
        bus.prev_key_escape = pins.key_escape;
        bus.prev_key_enter  = pins.key_enter;

        // Latch game phase transitions.
        // GameOver is triggered by a wire, committed here.
        if bus.wires.game_over_triggered {
            bus.game_phase = GamePhase::GameOver;
        }
        if bus.wires.pause_requested {
            // TODO: implement pause toggle once pause logic exists
        }

        // Increment Lamport clock (useful for deterministic replay/debugging).
        bus.tick_count = bus.tick_count.wrapping_add(1);
    }
```

### 4. Helper Methods (Optional)

```rust
    /// Install a chip into a specific layer (for dynamic pipeline construction).
    pub fn install_chip(&mut self, layer: usize, chip: Chip) {
        if layer < self.layers.len() {
            self.layers[layer].push(chip);
        }
    }

    /// Get the total number of chips across all layers.
    pub fn chip_count(&self) -> usize {
        self.layers.iter().map(|l| l.len()).sum()
    }
```

---

## Verification Protocol (Guardrail Agent B)

1. **Phase ordering check:** Verify `bus.wires = Wires::default()` executes BEFORE `for layer in &self.layers`. Verify prev_key latching executes AFTER the propagation loop.
2. **Double reset check:** The ONLY `Wires::default()` call in the codebase must be in `clock_tick()`. Search `rg "Wires::default" src/` — exactly one result, in `src/motherboard.rs`.
3. **Layer ordering check:** Verify layer 0 has `InputDecoder`. Layer 1 has timers BEFORE collision BEFORE rotation BEFORE movement BEFORE lock delay (exactly the order in the spec). Layer 2 has lock BEFORE hold BEFORE clear BEFORE score BEFORE level BEFORE spawn. Layer 3 has ghost.
4. **No mutation of pins:** The `clock_tick` receives `&InputPins` (immutable). Grep for any `pins.` field assignment inside `clock_tick` — must return nothing.
5. **Lamport clock:** Verify `bus.tick_count` increments by 1 every tick. Uses `wrapping_add` (correct for u64).
6. **Game phase latching:** Verify `game_over_triggered` wire is read AFTER propagation and BEFORE returning. Only the motherboard mutates `game_phase`.
7. **All chips present:** Count chip variants in the `Chip` enum (15). Count instantiated chips in `SiliconMotherboard::new()` — must also be 15.
8. **Compile check:** `cargo check` must pass. The motherboard references all 15 chip structs, which must exist (even if their impls are empty placeholders).
9. **No per-tick allocation:** Verify no `vec![]`, `Box::new()`, or `String::new()` inside `clock_tick()`. The `layers` Vecs are allocated once at construction.

---

## Acceptance Criteria

- [ ] `src/motherboard.rs` exists and compiles
- [ ] `SiliconMotherboard` struct with `layers: Vec<Vec<Chip>>`
- [ ] `clock_tick(&mut self, pins: &InputPins, bus: &mut SystemBus)` method
- [ ] Phase 0 (wire reset) executes before Phase 1 (propagation)
- [ ] Phase 2 (latching) executes after Phase 1
- [ ] All 15 chips instantiated in `new()`, in correct layers and order
- [ ] `prev_key_*` latches capture current pin state for next tick
- [ ] `tick_count` increments every tick
- [ ] `game_over_triggered` → `game_phase = GameOver`
- [ ] Zero per-tick heap allocation
- [ ] Zero `unsafe` blocks
