# Silicon-Based Rust Tetris — Architectural Blueprint

**Phase:** Design
**Paradigm:** Silicon-Based Software Architecture
**References:** `docs/architecture/SILICON_PARADIGM_SPEC.md`, `docs/research/MASTER_RESEARCH_REPORT.md`
**Date:** 2026-05-20

---

## 1. SystemBus Specification (`src/bus.rs`)

### 1.1 Primitive Types

```rust
// ============================================================================
// bus.rs — PCB Traces: InputPins, SystemBus (Registers + Wires)
// ============================================================================

use std::time::Duration;

// ─── Dimensions ───────────────────────────────────────────────────────────
pub const BOARD_COLS: usize = 10;
pub const BOARD_ROWS: usize = 20;
pub const BOARD_SIZE: usize = BOARD_COLS * BOARD_ROWS;  // 200

// ─── Cell Value Encoding ──────────────────────────────────────────────────
// 0  = empty
// 1  = I piece (cyan)
// 2  = J piece (blue)
// 3  = L piece (orange)
// 4  = O piece (yellow)
// 5  = S piece (green)
// 6  = T piece (purple)
// 7  = Z piece (red)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell(pub u8);

// ─── Piece Type (0-based: 0=I, 1=O, 2=T, 3=S, 4=Z, 5=J, 6=L) ─────────
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PieceType(pub u8);

impl PieceType {
    pub const I: PieceType = PieceType(0);
    pub const J: PieceType = PieceType(1);
    pub const L: PieceType = PieceType(2);
    pub const O: PieceType = PieceType(3);
    pub const S: PieceType = PieceType(4);
    pub const T: PieceType = PieceType(5);
    pub const Z: PieceType = PieceType(6);
}

// ─── Rotation State ──────────────────────────────────────────────────────
// 0 = spawn, 1 = CW (R), 2 = 180 (2), 3 = CCW (L)
pub type Rotation = u8;

// ─── Game State ──────────────────────────────────────────────────────────
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamePhase {
    Playing,
    Paused,
    GameOver,
}
```

### 1.2 InputPins (External Environment Snapshot)

Frozen once per tick during the Sampling Phase. Strictly read-only for all chips.

```rust
/// Sampled once per clock tick. Frozen immutable for the tick duration.
/// Chips treat these pins as absolute physical truth.
#[derive(Clone, Copy, Debug)]
pub struct InputPins {
    /// Wall-clock nanoseconds elapsed since the previous tick.
    /// This is the ONLY time input. All timer chips derive their
    /// behavior from this single value.
    pub frame_delta_ns: u64,

    // ─── Key states (latched during Sampling Phase) ───
    pub key_left:   bool,
    pub key_right:  bool,
    pub key_down:   bool,    // soft drop
    pub key_up:     bool,    // rotate CW (alternate mapping)
    pub key_z:      bool,    // rotate CCW
    pub key_x:      bool,    // rotate CW
    pub key_space:  bool,    // hard drop
    pub key_c:      bool,    // hold
    pub key_escape: bool,
    pub key_enter:  bool,    // pause / confirm
}
```

### 1.3 Wires (Temporary Per-Tick Signals)

Wires exist ONLY for the duration of one clock tick. They connect upstream chips to downstream chips within the same tick. Reset to default at the start of every tick.

```rust
/// Ephemeral signal lines. Valid only within a single clock tick.
/// Reset to `Default` at tick start. Chips read and write these
/// to communicate without calling each other.
#[derive(Clone, Debug, Default)]
pub struct Wires {
    // ─── Movement Requests (Layer 0 → Layer 1) ───
    /// Horizontal movement request: -1=left, 0=none, +1=right
    pub dx: i8,
    /// Vertical movement request:  0=none, +1=soft drop
    pub dy: i8,
    /// Rotation requests
    pub rotate_cw: bool,
    pub rotate_ccw: bool,
    /// Hard drop request (instant floor landing)
    pub hard_drop_requested: bool,
    /// Hold piece swap request
    pub hold_requested: bool,
    /// Pause toggle request
    pub pause_requested: bool,

    // ─── Timer Triggers (Layer 1: timer chips → movement chips) ───
    pub gravity_tick: bool,
    pub das_tick: bool,
    pub lock_delay_expired: bool,
    pub lock_delay_active: bool,

    // ─── Collision Signals (Layer 1: collision → rotation/lock chips) ───
    /// True if the piece cannot move downward (resting on surface or floor)
    pub collision_down: bool,
    /// True if the move/rotate attempt hit a wall/block
    pub collision_any: bool,
    /// True if SRS wall kick offsets resolved the rotation collision
    pub wall_kick_applied: bool,

    // ─── Lock / Spawn Signals (Layer 1 → Layer 2) ───
    /// Asserted when the active piece locks into the board
    pub piece_locked: bool,
    /// Asserted when a new piece should be spawned
    pub should_spawn_next: bool,

    // ─── Line Clear Signals (Layer 2) ───
    /// Bitmask of rows that are full: bit N set = row N is complete
    pub full_row_mask: u32,
    /// Count of rows that will be cleared this tick
    pub lines_cleared_this_tick: u8,

    // ─── Game State Events (Layer 2) ───
    pub game_over_triggered: bool,

    // ─── Render Optimization ───
    /// Set by any chip that changes visible state; TUI skips redraw if false
    pub render_dirty: bool,
}
```

### 1.4 SystemBus (The Complete Register File)

All persistent game state in a single flat struct. Passed as `&mut SystemBus` through the pipeline.

```rust
/// The single source of truth. Contains all Registers (persist across ticks)
/// and an embedded Wires block (reset every tick).
#[derive(Clone, Debug)]
pub struct SystemBus {
    // ═══════════════════════════════════════════════════════════════════
    // REGISTERS — persist across clock ticks. Only modified during the
    // Combinational Propagation Phase, then frozen at the Latching Phase.
    // ═══════════════════════════════════════════════════════════════════

    // ─── Board ────────────────────────────────────────────────────────
    /// Row-major playfield. board[y][x]. 0=empty, 1-7=locked piece color.
    pub board: [[Cell; BOARD_COLS]; BOARD_ROWS],

    // ─── Active Piece ─────────────────────────────────────────────────
    /// 0=I, 1=J, 2=L, 3=O, 4=S, 5=T, 6=Z
    pub piece_type: PieceType,
    /// Anchor column of the active piece (top-left of bounding box)
    pub piece_x: i8,
    /// Anchor row of the active piece (top-left of bounding box)
    pub piece_y: i8,
    /// Rotation state: 0=spawn, 1=CW, 2=180, 3=CCW
    pub piece_rotation: Rotation,

    // ─── Piece Queue ──────────────────────────────────────────────────
    pub next_piece_type: PieceType,
    /// None = no piece held yet
    pub hold_piece_type: Option<PieceType>,
    /// True if hold was used this turn (prevents double-hold)
    pub hold_used: bool,

    // ─── Scoring ──────────────────────────────────────────────────────
    pub score: u32,
    pub level: u16,
    pub lines_cleared: u32,

    // ─── Game Lifecycle ───────────────────────────────────────────────
    pub game_phase: GamePhase,
    /// Lamport clock: increments every tick. Useful for debugging/determinism.
    pub tick_count: u64,

    // ─── Ghost Piece (computed each tick, read by TUI) ────────────────
    pub ghost_x: i8,
    pub ghost_y: i8,

    // ─── Gravity Timer ────────────────────────────────────────────────
    /// Accumulated nanoseconds toward the next gravity drop
    pub gravity_accumulator_ns: u64,
    /// Target interval for one gravity tick (derived from level)
    pub gravity_interval_ns: u64,

    // ─── DAS (Delayed Auto Shift) Timer ───────────────────────────────
    /// Accumulated nanoseconds since DAS key press
    pub das_accumulator_ns: u64,
    /// Initial delay before auto-repeat begins (~267ms)
    pub das_delay_ns: u64,
    /// Interval between auto-repeats (~100ms)
    pub das_repeat_ns: u64,
    /// Is the DAS state machine currently active?
    pub das_active: bool,
    /// Direction being auto-repeated: -1=left, +1=right, 0=none
    pub das_direction: i8,
    /// Tracks the last repeat index to prevent missed ticks from frame jitter
    pub das_last_repeat_index: u32,

    // ─── Lock Delay Timer ─────────────────────────────────────────────
    /// Accumulated nanoseconds while piece rests on a surface
    pub lock_delay_accumulator_ns: u64,
    /// Maximum lock delay before auto-lock (~500ms)
    pub lock_delay_max_ns: u64,

    // ─── Previous Key State (edge detection latches) ──────────────────
    pub prev_key_left:  bool,
    pub prev_key_right: bool,
    pub prev_key_down:  bool,
    pub prev_key_up:    bool,
    pub prev_key_z:     bool,
    pub prev_key_x:     bool,
    pub prev_key_c:     bool,
    pub prev_key_space: bool,
    pub prev_key_escape:bool,
    pub prev_key_enter: bool,

    // ═══════════════════════════════════════════════════════════════════
    // WIRES — reset to Default at the start of every clock tick.
    // Embedded struct enables single-statement reset.
    // ═══════════════════════════════════════════════════════════════════
    pub wires: Wires,
}

// ─── Constructor ──────────────────────────────────────────────────────

impl SystemBus {
    pub fn new(level: u16) -> Self {
        Self {
            board: [[Cell(0); BOARD_COLS]; BOARD_ROWS],
            piece_type: PieceType::default(),
            piece_x: 3,
            piece_y: 0,
            piece_rotation: 0,
            next_piece_type: PieceType::default(),
            hold_piece_type: None,
            hold_used: false,
            score: 0,
            level,
            lines_cleared: 0,
            game_phase: GamePhase::Playing,
            tick_count: 0,
            ghost_x: 0,
            ghost_y: 0,
            gravity_accumulator_ns: 0,
            gravity_interval_ns: gravity_interval_ns(level as u8),
            das_accumulator_ns: 0,
            das_delay_ns: DAS_DELAY_NS,
            das_repeat_ns: DAS_REPEAT_NS,
            das_active: false,
            das_direction: 0,
            das_last_repeat_index: 0,
            lock_delay_accumulator_ns: 0,
            lock_delay_max_ns: LOCK_DELAY_MAX_NS,
            prev_key_left: false,
            prev_key_right: false,
            prev_key_down: false,
            prev_key_up: false,
            prev_key_z: false,
            prev_key_x: false,
            prev_key_c: false,
            prev_key_space: false,
            prev_key_escape: false,
            prev_key_enter: false,
            wires: Wires::default(),
        }
    }
}

// ─── Timing Constants ─────────────────────────────────────────────────

pub const FRAME_NS: u64 = 16_666_667;            // ~60 Hz (1/60s)
pub const DAS_DELAY_NS: u64 = 266_666_672;        // ~267ms (16 frames)
pub const DAS_REPEAT_NS: u64 = 100_000_002;       // ~100ms (6 frames)
pub const LOCK_DELAY_MAX_NS: u64 = 500_000_010;   // ~500ms (30 frames)

pub fn gravity_interval_ns(level: u8) -> u64 {
    // NES-style gravity table: frames per gridcell at 60 Hz
    const GRAVITY: &[u64] = &[
        48, 43, 38, 33, 28, 23, 18, 13, 8, 6,   // levels 1-10
        5,  5,  5,  4,  4,  4,  3,  3,  3, 2,    // levels 11-20
    ];
    let idx = level.saturating_sub(1) as usize;
    let frames = GRAVITY.get(idx).copied().unwrap_or(1);
    frames * FRAME_NS
}
```

### 1.5 Wire Reset Protocol

The `Wires` struct's `Default` implementation guarantees all fields reset with a single assignment:

```rust
bus.wires = Wires::default();
```

This is called at the **start** of every clock tick, BEFORE chips execute. This matches hardware: wires are pulled to ground (0/false) between clock cycles by pull-down resistors.

### 1.6 Field Justification Table

| Field | Category | Justification |
|---|---|---|
| `board` | Register | Persists locked pieces across ticks. Flat `[[Cell; 10]; 20]` = 200 bytes, no heap. |
| `piece_type/x/y/rotation` | Register | Active piece state. Must survive between ticks for continuous movement. |
| `next_piece_type` | Register | Preview display. Set at spawn time, read by TUI. |
| `hold_piece_type` | Register | Hold queue. `Option` avoids sentinel value for "empty". |
| `hold_used` | Register | Prevents double-hold in a single turn. Reset on new piece spawn. |
| `score/level/lines_cleared` | Register | Accumulating counters. Must persist. |
| `game_phase` | Register | Enum: Playing/Paused/GameOver. Controls pipeline behavior and TUI overlays. |
| `tick_count` | Register | Lamport clock. Enables deterministic replay. |
| `ghost_x/ghost_y` | Register | Computed each tick by GhostChip. Persists so TUI can read it post-pipeline. |
| `gravity_accumulator_ns` | Register | Running total of elapsed time. Must survive across ticks. |
| `gravity_interval_ns` | Register | Derived from level. Cache to avoid recomputation. |
| `das_accumulator_ns/delay/repeat` | Register | DAS state machine accumulator + config. |
| `das_active/direction/last_repeat_index` | Register | DAS state machine control. |
| `lock_delay_accumulator_ns/max` | Register | Lock delay accumulator + threshold. |
| `prev_key_*` | Register | Edge-detection latches. Capture previous tick's key state for rising-edge detection. |
| `wires.*` | Wire | All ephemeral. Reset every tick. Flows between chips within one tick. |

---

## 2. Interface Contract (`LogicChip` Trait)

### 2.1 Trait Definition (`src/motherboard.rs` or `src/chips/mod.rs`)

```rust
use crate::bus::{InputPins, SystemBus};

/// A stateless logic gate. Implements one pure deduction per clock tick.
///
/// # Invariants
///
/// - The implementing struct MUST be a unit struct (`struct FooChip;`)
///   containing zero fields. The compiler enforces statelessness.
/// - `tick()` reads `&InputPins` (immutable) and reads/writes
///   `&mut SystemBus`. It MUST NOT access any external state.
/// - `tick()` MUST be deterministic given the same `(pins, bus)` inputs.
/// - Chips MUST NOT call other chips. Data flow is exclusively through
///   the bus wires. The Motherboard orchestrates the pipeline ordering.
/// - Each chip MUST only read/write the bus fields relevant to its
///   specific duty (no privilege escalation).
///
/// # Borrow Checker Safety
///
/// `&self` + `&mut bus` = different allocations. No aliasing conflict.
/// `&pins` + `&mut bus` = different allocations. No aliasing conflict.
/// The motherboard iterates chips sequentially — only one `&mut bus`
/// exists at a time. No RefCell, no Mutex, no unsafe.
pub trait LogicChip {
    /// Execute one clock cycle of logic for this chip.
    ///
    /// Reads the frozen `InputPins` snapshot and the current `SystemBus`
    /// state (both Registers and Wires set by upstream chips). Computes
    /// new values and writes them to the `SystemBus` Wires (or directly
    /// to Registers for persistent state changes).
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus);
}
```

### 2.2 Signature Rationale

- **`&self`**: Zero-sized type receiver. Carries no data, zero runtime cost. The chip is a compile-time entity.
- **`pins: &InputPins`**: Immutable snapshot of the external world for this tick. All chips see the same pins.
- **`bus: &mut SystemBus`**: Exclusive mutable access. The chip can read any register/wire written by upstream chips and write to any wire for downstream chips. The compiler guarantees no concurrent modification.

### 2.3 No Return Value

The trait returns `()`. Chips communicate exclusively by writing to the bus. There are no return values, no `Result`, no error propagation. Errors are modeled as "blown fuse signals" — boolean wires on the bus (e.g., `game_over_triggered`).

---

## 3. Motherboard Design (`src/motherboard.rs`)

### 3.1 SiliconMotherboard Struct

```rust
use crate::bus::{InputPins, SystemBus, Wires};
use crate::chips::LogicChip;

/// The SiliconMotherboard physically arranges LogicChips in a layered
/// pipeline and provides the clock tick driver.
///
/// # Layer Architecture
///
/// Chips are organized in 4 layers. Within each layer, chips execute
/// sequentially. Signals propagate forward: Layer N chips may read
/// wires written by Layer N-1 chips within the same tick.
///
/// ```
/// Layer 0 (Input Decoder)  →  Layer 1 (Game Rules)  →  Layer 2 (State)  →  Layer 3 (UI)
/// ```
pub struct SiliconMotherboard {
    /// `layers[i]` is a homogeneous Vec of chips at pipeline stage i.
    /// Each chip is a stateless unit struct behind a trait object.
    ///
    /// Using `Vec<Vec<Box<dyn LogicChip>>>` enables:
    /// - Heterogeneous chip types (all impl LogicChip)
    /// - Layer-based grouping (ordering between layers is enforced)
    /// - Runtime extensibility (plug new chips into any layer)
    ///
    /// Alternative (zero-heap): Use a `Vec<Chip>` with an enum dispatch
    /// (see §3.4). Recommended for final implementation.
    pub layers: Vec<Vec<Box<dyn LogicChip>>>,
}
```

### 3.2 clock_tick: The 3-Phase Orchestrator

```rust
impl SiliconMotherboard {
    /// Execute one full clock cycle.
    ///
    /// # Phases
    ///
    /// 1. **Wire Reset**: Pull all wires to default (ground).
    /// 2. **Combinational Propagation**: Iterate through all 4 layers,
    ///    running every chip sequentially. Each chip reads InputPins
    ///    and SystemBus, writes to SystemBus Wires and Registers.
    /// 3. **Sequential Latching**: Commit edge-detection state (prev_key_*)
    ///    for the next tick. Increment Lamport clock.
    ///
    /// # Note on Sampling Phase
    ///
    /// The Sampling Phase (polling external I/O into InputPins) occurs
    /// OUTSIDE this method, in `main.rs`. `clock_tick` receives the
    /// already-frozen InputPins snapshot.
    pub fn clock_tick(&mut self, pins: &InputPins, bus: &mut SystemBus) {
        // ═══ PHASE 0: Wire Reset ═══
        // Pull all wires to ground (default) before the signal propagates.
        // Wires from the previous tick are invalid — only what chips
        // write this tick matters.
        bus.wires = Wires::default();

        // ═══ PHASE 1: Combinational Propagation ═══
        // Signals flow through the 4-layer pipeline. Each chip reads
        // InputPins and the current SystemBus state (including wires
        // set by upstream chips earlier in this same tick).
        for layer in &self.layers {
            for chip in layer {
                chip.tick(pins, bus);
            }
        }

        // ═══ PHASE 2: Sequential Latching (falling edge) ═══
        // Capture current key state for edge detection in the next tick.
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

        // Enforce game phase transitions
        if bus.wires.game_over_triggered {
            bus.game_phase = GamePhase::GameOver;
        }

        // Increment Lamport clock
        bus.tick_count = bus.tick_count.wrapping_add(1);
    }
}
```

### 3.3 Constructor with Empty Layers

```rust
impl SiliconMotherboard {
    /// Create a new motherboard with empty layers.
    /// Chips are injected after construction via `install_chip()`.
    pub fn new() -> Self {
        Self {
            layers: vec![
                Vec::new(),  // Layer 0: Input Decoder
                Vec::new(),  // Layer 1: Game Rules & Collision
                Vec::new(),  // Layer 2: State Mutation
                Vec::new(),  // Layer 3: UI Transformation
            ],
        }
    }

    /// Install a chip into a specific layer.
    /// Layer index: 0=Input, 1=Rules, 2=State, 3=UI
    pub fn install_chip(&mut self, layer: usize, chip: Box<dyn LogicChip>) {
        self.layers[layer].push(chip);
    }
}
```

### 3.4 Zero-Heap Alternative: Enum Dispatch

The `Box<dyn LogicChip>` approach requires heap allocation for trait objects. For truly zero-allocation, use an enum:

```rust
/// A closed-set enum of all chip types.
/// Used with `Vec<Chip>` instead of `Vec<Box<dyn LogicChip>>`.
pub enum Chip {
    // Layer 0
    InputDecoder(InputDecoderChip),
    // Layer 1
    GravityTimer(GravityTimerChip),
    DasTimer(DasTimerChip),
    LockDelayTimer(LockDelayTimerChip),
    CollisionDetector(CollisionDetectorChip),
    Rotation(RotationChip),
    // Layer 2
    Movement(MovementChip),
    PieceLocker(PieceLockerChip),
    LineClearDetector(LineClearDetectorChip),
    LineClearCommitter(LineClearCommitterChip),
    ScoreKeeper(ScoreKeeperChip),
    LevelCalculator(LevelCalculatorChip),
    SpawnController(SpawnControllerChip),
    // Layer 3
    GhostComputer(GhostComputerChip),
}

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
            Chip::SpawnController(c)      => c.tick(pins, bus),
            Chip::GhostComputer(c)        => c.tick(pins, bus),
        }
    }
}
```

With the enum approach, the motherboard stores `layers: Vec<Vec<Chip>>` — zero heap, zero indirection. The trade-off: adding a new chip requires adding a variant + match arm. Recommended for the implementation phase.

---

## 4. Layer Partitioning Plan

### 4.1 Layer Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                     CLOCK TICK DATA FLOW                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  InputPins ─────────────────────────────────────────────────┐   │
│  (frozen)                                                    │   │
│      │                                                       │   │
│      ▼                                                       │   │
│  ┌──────────────────────────────────────────────────────┐    │   │
│  │ LAYER 0: INPUT DECODER                               │    │   │
│  │ "Keyboard Interfacing"                             │    │   │
│  │                                                       │    │   │
│  │  Reads:  InputPins, bus.prev_key_*                    │    │   │
│  │  Writes: wires.dx, .dy, .rotate_cw, .rotate_ccw,     │    │   │
│  │          .hard_drop_requested, .hold_requested,       │    │   │
│  │          .pause_requested                              │    │   │
│  │                                                       │    │   │
│  │  [InputDecoderChip]                                    │    │   │
│  └──────────────────────┬───────────────────────────────┘    │   │
│                         │                                     │   │
│                         ▼                                     │   │
│  ┌──────────────────────────────────────────────────────┐    │   │
│  │ LAYER 1: GAME RULES & COLLISION                      │    │   │
│  │ "Physics Engine"                                     │    │   │
│  │                                                       │    │   │
│  │  Reads:  wires.dx, .dy, .rotate_cw, .rotate_ccw,     │    │   │
│  │          .hard_drop_requested, bus.piece_*, bus.board  │    │   │
│  │          timer registers                              │    │   │
│  │  Writes: wires.gravity_tick, .das_tick,               │    │   │
│  │          .lock_delay_expired, .lock_delay_active,     │    │   │
│  │          .collision_down, .collision_any,              │    │   │
│  │          .wall_kick_applied,                           │    │   │
│  │          timer registers                              │    │   │
│  │                                                       │    │   │
│  │  [GravityTimerChip] [DasTimerChip]                     │    │   │
│  │  [LockDelayTimerChip] [CollisionDetectorChip]          │    │   │
│  │  [RotationChip] [MovementChip]                         │    │   │
│  └──────────────────────┬───────────────────────────────┘    │   │
│                         │                                     │   │
│                         ▼                                     │   │
│  ┌──────────────────────────────────────────────────────┐    │   │
│  │ LAYER 2: STATE MUTATION                              │    │   │
│  │ "Board / Score Update"                               │    │   │
│  │                                                       │    │   │
│  │  Reads:  wires.piece_locked, .should_spawn_next,      │    │   │
│  │          .full_row_mask, bus.board, bus.score/level    │    │   │
│  │  Writes: bus.board, bus.piece_*, bus.score,            │    │   │
│  │          bus.level, bus.lines_cleared,                 │    │   │
│  │          bus.hold_piece_type, bus.next_piece_type,     │    │   │
│  │          wires.game_over_triggered                     │    │   │
│  │                                                       │    │   │
│  │  [PieceLockerChip] [LineClearDetectorChip]             │    │   │
│  │  [LineClearCommitterChip] [ScoreKeeperChip]            │    │   │
│  │  [LevelCalculatorChip] [SpawnControllerChip]           │    │   │
│  │  [HoldControllerChip]                                  │    │   │
│  └──────────────────────┬───────────────────────────────┘    │   │
│                         │                                     │   │
│                         ▼                                     │   │
│  ┌──────────────────────────────────────────────────────┐    │   │
│  │ LAYER 3: UI TRANSFORMATION                           │    │   │
│  │ "Rendering Logic"                                    │    │   │
│  │                                                       │    │   │
│  │  Reads:  bus.piece_*, bus.board                       │    │   │
│  │  Writes: bus.ghost_x, .ghost_y                         │    │   │
│  │                                                       │    │   │
│  │  [GhostComputerChip]                                   │    │   │
│  └──────────────────────────────────────────────────────┘    │   │
│                         │                                     │   │
│                         ▼                                     │   │
│              ┌──────────────────────┐                        │   │
│              │ SystemBus (frozen)   │── readonly ──▶ TUI     │   │
│              │ ready for rendering  │                        │   │
│              └──────────────────────┘                        │   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 Chip Assignment Table

| Layer | Chip | Duty | Reads | Writes |
|-------|------|------|-------|--------|
| **0: Input Decoder** | `InputDecoderChip` | Map raw key flags to movement/action wires. Edge-detection for one-shot inputs (rotate, hold, pause, hard drop). Continuous inputs (left/right/down) set dx/dy directly. | `pins.key_*`, `bus.prev_key_*` | `wires.dx`, `.dy`, `.rotate_cw`, `.rotate_ccw`, `.hard_drop_requested`, `.hold_requested`, `.pause_requested` |
| **1: Game Rules** | `GravityTimerChip` | Accumulate frame delta, fire gravity tick when interval exceeded. Preserve remainder. | `pins.frame_delta_ns`, `bus.gravity_accumulator_ns`, `.gravity_interval_ns` | `wires.gravity_tick`, `bus.gravity_accumulator_ns` |
| | `DasTimerChip` | DAS state machine: initial delay + auto-repeat with repeat-index tracking. | `pins.key_left/right`, `pins.frame_delta_ns`, `bus.prev_key_left/right`, `bus.das_*` registers | `wires.das_tick`, `wires.dx`, `bus.das_*` registers |
| | `LockDelayTimerChip` | Lock delay accumulation. Reset on successful move/rotate. Fire lock_delay_expired when threshold reached. | `wires.collision_down`, `.dx`, `.dy`, `.rotate_cw`, `.rotate_ccw`, `pins.frame_delta_ns` | `wires.lock_delay_active`, `.lock_delay_expired`, `bus.lock_delay_accumulator_ns` |
| | `CollisionDetectorChip` | Test if current piece position collides with walls/floor/blocks. Run BEFORE movement/rotation. | `bus.piece_*`, `bus.board` | `wires.collision_down`, `.collision_any` |
| | `RotationChip` | Apply rotation + SRS wall kicks. Test 5 kick offsets sequentially. | `wires.rotate_cw`, `.rotate_ccw`, `wires.collision_any`, `bus.piece_*`, `bus.board` | `bus.piece_rotation`, `.piece_x`, `.piece_y`, `wires.wall_kick_applied` |
| | `MovementChip` | Apply dx/dy movement. Handle hard drop (instant floor). Move ghost along with piece conceptually. | `wires.dx`, `.dy`, `.gravity_tick`, `.das_tick`, `.hard_drop_requested`, `bus.piece_*`, `bus.board` | `bus.piece_x`, `.piece_y`, `wires.piece_locked` (if hard drop) |
| **2: State Mutation** | `PieceLockerChip` | Lock active piece into board when lock_delay_expired OR hard drop completed. | `wires.lock_delay_expired`, `.piece_locked`, `bus.piece_*`, `bus.board` | `bus.board`, `wires.piece_locked`, `.should_spawn_next`, `.render_dirty` |
| | `LineClearDetectorChip` | Scan all 20 rows. Set bitmask of full rows. | `bus.board` | `wires.full_row_mask`, `.lines_cleared_this_tick` |
| | `LineClearCommitterChip` | Remove full rows from board. Shift rows above down. | `wires.full_row_mask`, `bus.board` | `bus.board`, `wires.render_dirty` |
| | `ScoreKeeperChip` | Compute score from lines cleared × level. NES formula: base[lines] × (level+1). | `wires.lines_cleared_this_tick`, `bus.level`, `bus.score` | `bus.score`, `bus.lines_cleared` |
| | `LevelCalculatorChip` | Compute level from total lines cleared. Level = 1 + lines / 10. Update gravity interval. | `bus.lines_cleared`, `bus.level` | `bus.level`, `bus.gravity_interval_ns` |
| | `HoldControllerChip` | Swap active piece with hold slot. Prevent double-hold. | `wires.hold_requested`, `bus.hold_used`, `bus.piece_*`, `bus.hold_piece_type` | `bus.piece_*`, `bus.hold_piece_type`, `bus.hold_used` |
| | `SpawnControllerChip` | Spawn next piece. Check if spawn position collides → game over. Set hold_used=false for new turn. | `wires.should_spawn_next`, `bus.next_piece_type`, `bus.board` | `bus.piece_*`, `bus.next_piece_type`, `bus.hold_used`, `wires.game_over_triggered` |
| **3: UI Transformation** | `GhostComputerChip` | Raycast downward from piece position to find ghost Y. | `bus.piece_*`, `bus.board` | `bus.ghost_x`, `bus.ghost_y` |

### 4.3 Intra-Layer Ordering (Critical)

Within Layer 1, ordering is critical:

```
GravityTimerChip → DasTimerChip → LockDelayTimerChip → CollisionDetectorChip → RotationChip → MovementChip
```

**Rationale:**
1. **Timers first**: `GravityTimerChip`, `DasTimerChip`, `LockDelayTimerChip` accumulate time and set trigger wires (`gravity_tick`, `das_tick`, `lock_delay_expired`). They must run before collision/rotation/movement because those chips read these triggers.
2. **Collision before movement**: `CollisionDetectorChip` tests the CURRENT position. It must run before `RotationChip` and `MovementChip` because those chips need to know if the current position is valid before attempting changes. The collision wires are then cleared on success by movement/rotation.
3. **Rotation before movement**: `RotationChip` processes rotation first (with SRS kicks). If rotation succeeds, the new position is written. `MovementChip` then processes dx/dy/hard-drop on the (possibly rotated) piece position.
4. **LockDelayTimerChip reads collision from CollisionDetectorChip**: Lock delay only accumulates when `collision_down` is true (piece resting on surface). Position-dependent, so it runs after collision detection but before movement (which would change the position). Wait, actually: LockDelayTimerChip needs to know if the piece can't move down. But CollisionDetectorChip runs BEFORE MovementChip. However, the lock delay should check if the piece is resting AFTER movement. Let me reconsider...

**Revised Layer 1 ordering:**

```
GravityTimerChip → DasTimerChip → CollisionDetectorChip → RotationChip → MovementChip → LockDelayTimerChip
```

LockDelayTimerChip runs AFTER MovementChip because:
- It needs to know if the piece can fall (collision_down after movement attempt)
- It resets on successful move/rotate (checks if dx/dy/rotate wires were acted on)
- It only accumulates when the piece is truly resting after this tick's movement

Actually, there's a subtlety. The CollisionDetectorChip tests the position; MovementChip and RotationChip change the position. LockDelayTimerChip needs to know the post-movement state. So LockDelayTimerChip should run AFTER MovementChip and RotationChip.

But wait: LockDelayTimerChip currently reads `wires.collision_down`, which CollisionDetectorChip sets. If MovementChip/RotationChip succeed, they clear `collision_any`. We need CollisionDetectorChip to run AGAIN after movement, or we need MovementChip to re-set the collision wires. 

The cleanest approach: have a second CollisionDetectorChip after MovementChip, or have MovementChip itself set `collision_down` after attempting movement. Since MovementChip already knows whether the movement succeeded (by checking collisions internally), it can set the wire.

**Final Layer 1 ordering (revised):**

```
GravityTimerChip → DasTimerChip → CollisionDetectorChip (pre) → RotationChip → MovementChip → LockDelayTimerChip
```

- `CollisionDetectorChip (pre)`: Tests current piece position. Sets `collision_any`, `collision_down`.
- `RotationChip`: Reads `collision_any` from pre-test. Attempts rotation + SRS kicks. On success, clears `collision_any`, sets `wall_kick_applied`, updates piece position/rotation.
- `MovementChip`: Reads `collision_any` from pre-test. Attempts dx/dy/hard-drop moves. On success, clears `collision_any`, updates piece position. On HARD DROP: sets `piece_locked = true`. After movement, re-tests collision_down and sets `wires.collision_down`.
- `LockDelayTimerChip`: Reads `collision_down` (set by MovementChip post-move). Manages lock delay accumulator.

This is cleaner. `MovementChip` is the final authority on collision state after all movement.

### 4.4 Chips That Mutate Registers Directly

Most chips only write to wires. Few chips directly modify registers:

| Register | Modified by | When |
|---|---|---|
| `board` | `PieceLockerChip`, `LineClearCommitterChip` | Layer 2 |
| `piece_x`, `piece_y`, `piece_rotation` | `RotationChip`, `MovementChip`, `SpawnControllerChip`, `HoldControllerChip` | Layers 1, 2 |
| `piece_type` | `SpawnControllerChip`, `HoldControllerChip` | Layer 2 |
| `score`, `level`, `lines_cleared` | `ScoreKeeperChip`, `LevelCalculatorChip` | Layer 2 |
| `gravity_interval_ns` | `LevelCalculatorChip` | Layer 2 |
| `ghost_x`, `ghost_y` | `GhostComputerChip` | Layer 3 |
| `hold_piece_type`, `hold_used` | `HoldControllerChip`, `SpawnControllerChip` | Layer 2 |
| `game_phase` | `SiliconMotherboard.clock_tick()` (latching phase) | Post-pipeline |
| Timer registers | `GravityTimerChip`, `DasTimerChip`, `LockDelayTimerChip` | Layer 1 |
| `prev_key_*` | `SiliconMotherboard.clock_tick()` (latching phase) | Post-pipeline |

---

## 5. Clock Cycle Lifecycle Validation

### 5.1 Sampling Phase (in `main.rs`)

```rust
let now = Instant::now();
let frame_delta_ns = now.duration_since(last_tick).as_nanos() as u64;
last_tick = now;

// Poll keyboard (non-blocking drain loop)
let pins = poll_input_pins(frame_delta_ns);
// pins is now frozen — readonly for all chips
```

### 5.2 Combinational Propagation Phase (in `SiliconMotherboard::clock_tick`)

```rust
bus.wires = Wires::default();  // Phase 0: Wire reset

for layer in &self.layers {     // Phase 1: Layer-by-layer propagation
    for chip in layer {         // Sequential chip execution within layer
        chip.tick(pins, bus);
    }
}
```

During this phase:
- Layer 0 chips write wires (dx, dy, rotate_cw, etc.)
- Layer 1 chips read those wires + write new wires (gravity_tick, collision_down, etc.)
- Layer 2 chips read wires from Layer 1 + write directly to registers (board, score)
- Layer 3 chips read updated registers + write ghost position

Data propagates forward through the layers exactly as in a digital circuit. No chip calls another chip. No backward data flow (Layer 3 cannot affect Layer 0).

### 5.3 Sequential Latching Phase (in `SiliconMotherboard::clock_tick`)

```rust
// Latch key state for next tick's edge detection
bus.prev_key_* = pins.key_*;

// Commit any pending phase transitions
if bus.wires.game_over_triggered {
    bus.game_phase = GamePhase::GameOver;
}

bus.tick_count += 1;
```

### 5.4 Render Phase (in `main.rs`, after `clock_tick` returns)

```rust
// Throttle to ~60 FPS independently of game logic
if render_delta >= TARGET_FRAME_NS {
    terminal.draw(|f| render_game(f, &bus))?;
    last_render = now;
}
```

The render function receives `&SystemBus` (immutable). It reads registers and wires to derive the visual layout. It never mutates anything.

---

## 6. Full Main Loop Contract

```rust
// src/main.rs

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = RawModeGuard::enter()?;
    // ... terminal init ...

    let mut bus = SystemBus::new(1);  // start at level 1
    let mut motherboard = SiliconMotherboard::new();
    motherboard.install_layer_0();
    motherboard.install_layer_1();
    motherboard.install_layer_2();
    motherboard.install_layer_3();

    let mut last_tick = Instant::now();
    let mut last_render = Instant::now();

    loop {
        let now = Instant::now();

        // ═══ SAMPLING PHASE ═══
        let frame_delta_ns = now.duration_since(last_tick).as_nanos() as u64;
        last_tick = now;
        let pins = poll_input_pins(frame_delta_ns.min(MAX_FRAME_DELTA_NS));

        // ═══ PROPAGATION + LATCHING ═══
        motherboard.clock_tick(&pins, &mut bus);

        // ═══ RENDER (throttled) ═══
        if now.duration_since(last_render).as_nanos() as u64 >= FRAME_NS {
            terminal.draw(|f| render_game(f, &bus))?;
            last_render = now;
        }

        // ═══ YIELD ═══
        if now.elapsed().as_nanos() < 1_000_000 {
            std::thread::sleep(Duration::from_micros(500));
        }

        // ═══ EXIT CHECK ═══
        if pins.key_escape || bus.game_phase == GamePhase::GameOver {
            break;
        }
    }

    Ok(())
}
```

---

## 7. Paradigm Compliance Checklist

| Constraint | Compliance |
|---|---|
| **No OOP** | All chips are unit structs. All state in flat `SystemBus`. No methods on state. |
| **No events/callbacks** | Single `tick()` per chip. Sequential iteration. No registration. |
| **No async/await** | Synchronous `poll(Duration::ZERO)`. Non-blocking. |
| **No multi-threading** | Single thread. One `&mut bus` at a time. Compiler enforced. |
| **State-Logic Separation** | `SystemBus` holds ALL state (Registers + Wires). `LogicChip` impls have ZERO fields. |
| **Tick-Driven** | Everything happens inside `clock_tick()`. 3-phase lifecycle. |
| **Zero API Call Chains** | Chips cannot call other chips. Data flow exclusively via bus wires. |
| **Flat State** | `SystemBus` is a single struct. All fields are primitives or fixed-size arrays. |
| **No Unsafe** | No `unsafe` blocks. Entirely safe Rust. |
| **No RefCell/Mutex** | `&mut SystemBus` provides exclusive access. Sequential pipelining. |
| **Wires vs Registers** | `Wires` struct explicitly separated. Reset enforced at tick start. |
| **Lamport Clock** | `tick_count` increments every tick. Enables deterministic replay. |
| **Blown Fuse Signals** | `game_over_triggered` wire models error state. No `panic!` for control flow. |
| **Privilege Escalation** | Each chip only touches its documented bus fields. Convention enforced by design. |

---

## 8. File Structure (Final)

```
src/
├── main.rs                 # Wall-clock, I/O sampling, main loop, terminal init
├── bus.rs                  # Cell, PieceType, GamePhase, InputPins, Wires, SystemBus
├── motherboard.rs          # SiliconMotherboard, LogicChip trait, Chip enum
├── tui.rs                  # render_game(&Frame, &SystemBus) — pure render function
├── chips/
│   ├── mod.rs              # Re-exports all chips
│   ├── input_decoder.rs    # [Layer 0] InputDecoderChip
│   ├── gravity_timer.rs    # [Layer 1] GravityTimerChip
│   ├── das_timer.rs        # [Layer 1] DasTimerChip
│   ├── lock_delay_timer.rs # [Layer 1] LockDelayTimerChip
│   ├── collision.rs        # [Layer 1] CollisionDetectorChip
│   ├── rotation.rs         # [Layer 1] RotationChip
│   ├── movement.rs         # [Layer 1] MovementChip
│   ├── piece_locker.rs     # [Layer 2] PieceLockerChip
│   ├── line_clear.rs       # [Layer 2] LineClearDetectorChip + LineClearCommitterChip
│   ├── score_keeper.rs     # [Layer 2] ScoreKeeperChip
│   ├── level_calc.rs       # [Layer 2] LevelCalculatorChip
│   ├── hold_controller.rs  # [Layer 2] HoldControllerChip
│   ├── spawn_controller.rs # [Layer 2] SpawnControllerChip
│   └── ghost.rs            # [Layer 3] GhostComputerChip
└── constants/
    └── tetrominoes.rs       # TETROMINOES, KICKS, LINE_CLEAR_BASES, GRAVITY table
```

---

*End of Architectural Blueprint — ready for the Implementation Phase.*
