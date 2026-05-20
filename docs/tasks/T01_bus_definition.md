# T01: SystemBus Definition (`src/bus.rs`)

**Task ID:** T01
**Title:** SystemBus, Wires, and InputPins — Complete Type Definitions
**Depends On:** None (foundational)
**Produces:** `src/bus.rs`

---

## Paradigm Constraints (Recap)

| Constraint | Meaning for This Task |
|---|---|
| **Flat State** | `SystemBus` must be a single struct. All fields must be primitives or fixed-size arrays (`[T; N]`). NO `Vec`, `HashMap`, `Box`, `Rc`, `Arc`, `Mutex`. |
| **Register/Wire Separation** | `Wires` must be a separate embedded struct so it can be reset in ONE assignment per tick (`bus.wires = Wires::default()`). |
| **No Heap** | Zero heap allocations. `[[Cell; 10]; 20]` is stack-allocated. |
| **No Hidden State** | No `static mut`, no `lazy_static`, no globals. All state lives in `SystemBus`. |
| **Primitive Types** | Use `u8`, `i8`, `u16`, `u32`, `u64`, `bool`. Use `Option` only for `hold_piece_type` (OCaml-style discriminant, no heap). |
| **No OOP** | No methods that mutate `self`. Only free functions and trait impls on ZSTs belong here. Constructors (`new()`) are acceptable. |

---

## Implementation Goal

Create the complete `src/bus.rs` file containing:

### 1. Constants
- `BOARD_COLS: usize = 10`
- `BOARD_ROWS: usize = 20`
- `BOARD_SIZE: usize = 200`
- `FRAME_NS: u64 = 16_666_667` (~60 Hz)
- `DAS_DELAY_NS: u64 = 266_666_672`
- `DAS_REPEAT_NS: u64 = 100_000_002`
- `LOCK_DELAY_MAX_NS: u64 = 500_000_010`
- NES gravity table as `const GRAVITY_FRAMES: [u64; 20]`
- `gravity_interval_ns(level: u8) -> u64` function

### 2. Newtypes
- `Cell(pub u8)` — playfield cell (0=empty, 1-7=piece color)
- `PieceType(pub u8)` — 0=I, 1=J, 2=L, 3=O, 4=S, 5=T, 6=Z

### 3. Enums
- `GamePhase` — `Playing`, `Paused`, `GameOver`

### 4. InputPins (Frozen external snapshot)
```rust
pub struct InputPins {
    pub frame_delta_ns: u64,
    pub key_left: bool,
    pub key_right: bool,
    pub key_down: bool,
    pub key_up: bool,
    pub key_z: bool,       // rotate CCW
    pub key_x: bool,       // rotate CW
    pub key_space: bool,   // hard drop
    pub key_c: bool,       // hold
    pub key_escape: bool,
    pub key_enter: bool,
}
```

### 5. Wires (Temporary per-tick signals, reset EVERY tick)
```rust
pub struct Wires {
    // Movement requests
    pub dx: i8,                           // -1 left, 0 none, +1 right
    pub dy: i8,                           // 0 none, +1 soft drop
    pub rotate_cw: bool,
    pub rotate_ccw: bool,
    pub hard_drop_requested: bool,
    pub hold_requested: bool,
    pub pause_requested: bool,
    // Timer triggers
    pub gravity_tick: bool,
    pub das_tick: bool,
    pub lock_delay_expired: bool,
    pub lock_delay_active: bool,
    // Collision signals
    pub collision_down: bool,
    pub collision_any: bool,
    pub wall_kick_applied: bool,
    // Lock / spawn signals
    pub piece_locked: bool,
    pub should_spawn_next: bool,
    // Line clear signals
    pub full_row_mask: u32,
    pub lines_cleared_this_tick: u8,
    // Game state events
    pub game_over_triggered: bool,
    // Render optimization
    pub render_dirty: bool,
}
```

### 6. SystemBus (Registers + embedded Wires)
```rust
pub struct SystemBus {
    // REGISTERS (persist across ticks)
    pub board: [[Cell; BOARD_COLS]; BOARD_ROWS],
    pub piece_type: PieceType,
    pub piece_x: i8,
    pub piece_y: i8,
    pub piece_rotation: u8,         // 0-3
    pub next_piece_type: PieceType,
    pub hold_piece_type: Option<PieceType>,
    pub hold_used: bool,
    pub score: u32,
    pub level: u16,
    pub lines_cleared: u32,
    pub game_phase: GamePhase,
    pub tick_count: u64,
    pub ghost_x: i8,
    pub ghost_y: i8,
    // Timer registers
    pub gravity_accumulator_ns: u64,
    pub gravity_interval_ns: u64,
    pub das_accumulator_ns: u64,
    pub das_delay_ns: u64,
    pub das_repeat_ns: u64,
    pub das_active: bool,
    pub das_direction: i8,
    pub das_last_repeat_index: u32,
    pub lock_delay_accumulator_ns: u64,
    pub lock_delay_max_ns: u64,
    // Previous key state (edge detection)
    pub prev_key_left: bool,
    pub prev_key_right: bool,
    pub prev_key_down: bool,
    pub prev_key_up: bool,
    pub prev_key_z: bool,
    pub prev_key_x: bool,
    pub prev_key_c: bool,
    pub prev_key_space: bool,
    pub prev_key_escape: bool,
    pub prev_key_enter: bool,
    // WIRES (reset every tick)
    pub wires: Wires,
}
```

### 7. Implementations
- `Default` for `Cell` (returns `Cell(0)`)
- `Default` for `PieceType` (returns `PieceType(0)`)
- `Default` for `Wires` (all false/0)
- `Default` for `InputPins` (all false/0, `frame_delta_ns: 0`)
- `SystemBus::new(level: u16)` — initializes all fields to starting values
- `Derive` macros: `Clone, Copy, Debug, Default, PartialEq, Eq` where appropriate

---

## Verification Protocol (Guardrail Agent B)

1. **Flatness check:** Run `grep -n "Vec<" src/bus.rs` — must return ZERO results. Run `grep -n "Box<" src/bus.rs` — must return ZERO results. Run `grep -n "HashMap" src/bus.rs` — must return ZERO results.
2. **Wire reset check:** Verify that `Wires::default()` sets ALL fields to `false` or `0` (no `true` or non-zero defaults).
3. **Size check:** Compute `size_of::<SystemBus>()`. Confirm it's a small constant (should be under 1KB). `[[Cell; 10]; 20]` = 200 bytes. All other fields are primitives. Total should be under 400 bytes.
4. **Derive check:** Verify `Cell` and `PieceType` are `Copy` (required for array indexing). Verify `Wires` is NOT `Copy` (must be reset explicitly, not silently copied).
5. **Constant correctness:** Verify `FRAME_NS` = 1_000_000_000 / 60. Verify `GRAVITY_FRAMES` values match NES spec: [48, 43, 38, 33, 28, 23, 18, 13, 8, 6, 5, 5, 5, 4, 4, 4, 3, 3, 3, 2].
6. **Compile check:** The file must compile with `cargo check` (no logic, just type definitions — but must pass the compiler).
7. **`Option` check:** Verify `hold_piece_type: Option<PieceType>` is the ONLY `Option` on the bus. No other `Option` fields.
8. **No `unsafe`:** `grep -n "unsafe" src/bus.rs` must return nothing.

---

## Acceptance Criteria

- [ ] `src/bus.rs` exists and compiles
- [ ] All types defined: `Cell`, `PieceType`, `GamePhase`, `InputPins`, `Wires`, `SystemBus`
- [ ] `Wires` is a separate struct embedded in `SystemBus`
- [ ] `SystemBus::new(level)` constructor exists
- [ ] `Default` impls exist for `Wires`, `Cell`, `PieceType`, `InputPins`
- [ ] Zero heap types (`Vec`, `Box`, `HashMap`) anywhere in the file
- [ ] Zero `unsafe` blocks
- [ ] All constants match spec values
- [ ] Total struct size under 1024 bytes
