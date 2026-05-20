# Silicon-Based Rust Tetris — Master Research Report

**Project:** `tetris-silicon` — Terminal Tetris in Rust
**Architecture:** [Silicon-Based Software Architecture Paradigm](../architecture/SILICON_PARADIGM_SPEC.md)
**Date:** 2026-05-20

---

## Table of Contents

1. [Sub-Agent 1: Terminal I/O & Driver](#1-terminal-io--driver)
2. [Sub-Agent 2: Tetris Physics & Domain](#2-tetris-physics--domain)
3. [Sub-Agent 3: Rust Memory & Paradigm Mapper](#3-rust-memory--paradigm-mapper)
4. [Sub-Agent 4: Clock & Game Loop Engineer](#4-clock--game-loop-engineer)
5. [Sub-Agent 5: TUI Rendering Engineer](#5-tui-rendering-engineer)
6. [Architecture Synthesis](#6-architecture-synthesis)
7. [Implementation Roadmap](#7-implementation-roadmap)

---

## 1. Terminal I/O & Driver

### 1.1 Dependencies

```toml
[dependencies]
crossterm = "0.28"
ratatui = "0.26"
```

### 1.2 Raw Mode with Panic Safety

A `RawModeGuard` ensures terminal restoration on both normal exit and panic.

```rust
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::{self, Write};
use std::panic;

pub struct RawModeGuard;

impl RawModeGuard {
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let prev_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = crossterm::execute!(
                io::stdout(),
                crossterm::terminal::LeaveAlternateScreen,
                crossterm::cursor::Show,
            );
            prev_hook(info);
        }));
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show,
        );
    }
}
```

### 1.3 InputPins Struct

```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct InputPins {
    pub frame_delta_ns: u64,
    pub key_left: bool,
    pub key_right: bool,
    pub key_down: bool,
    pub key_up: bool,
    pub key_space: bool,
    pub key_z: bool,
    pub key_x: bool,
    pub key_c: bool,
    pub key_escape: bool,
    pub key_enter: bool,
}
```

### 1.4 Non-Blocking Input Polling

```rust
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

pub fn poll_input_pins(frame_delta_ns: u64) -> InputPins {
    let Ok(true) = event::poll(Duration::ZERO) else {
        return InputPins { frame_delta_ns, ..InputPins::default() };
    };

    let mut pins = InputPins { frame_delta_ns, ..InputPins::default() };

    while let Ok(event) = event::read() {
        match event {
            Event::Key(KeyEvent { code, .. }) => match code {
                KeyCode::Up | KeyCode::Char('k') => pins.key_up = true,
                KeyCode::Down | KeyCode::Char('j') => pins.key_down = true,
                KeyCode::Left | KeyCode::Char('h') => pins.key_left = true,
                KeyCode::Right | KeyCode::Char('l') => pins.key_right = true,
                KeyCode::Char(' ') => pins.key_space = true,
                KeyCode::Esc => pins.key_escape = true,
                KeyCode::Enter => pins.key_enter = true,
                KeyCode::Char('z' | 'Z') => pins.key_z = true,
                KeyCode::Char('x' | 'X') => pins.key_x = true,
                KeyCode::Char('c' | 'C') => pins.key_c = true,
                _ => {}
            },
            _ => {}
        }
        if !event::poll(Duration::ZERO).unwrap_or(false) {
            break;
        }
    }
    pins
}
```

**Key design decisions:**
- `poll(Duration::ZERO)` is strictly non-blocking — never waits
- Drain loop captures all simultaneous keypresses between ticks
- Input pins frozen for entire tick duration (Sampling Phase purity)
- Edge detection handled by chips using prev-key latches on the bus

---

## 2. Tetris Physics & Domain

### 2.1 Playfield Matrix

| Property | Value |
|---|---|
| Columns | **10** (universal standard) |
| Visible rows | **20** |
| Total rows (with buffer) | **24** (4 hidden buffer rows above visible) |
| Cell type | `u8`: 0=empty, 1-7=piece color |

```rust
pub type Playfield = [Cell; 200];  // 10 x 20 visible, flat row-major
pub const BOARD_ROWS: usize = 20;
pub const BOARD_COLS: usize = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell(pub u8);

#[inline]
pub const fn pf_index(x: i8, y: i8) -> usize {
    (y as usize) * 10 + (x as usize)
}
```

### 2.2 Tetromino Data (Complete)

All coordinates are `(x_offset, y_offset)` where y+ = DOWN.

```rust
pub const TETROMINOES: [[[(i8, i8); 4]; 4]; 7] = [
    // 0: I piece (4x4 bounding box)
    [
        [(0, 1), (1, 1), (2, 1), (3, 1)],  // State 0 (spawn)
        [(2, 0), (2, 1), (2, 2), (2, 3)],  // State R (CW)
        [(0, 2), (1, 2), (2, 2), (3, 2)],  // State 2 (180)
        [(1, 0), (1, 1), (1, 2), (1, 3)],  // State L (CCW)
    ],
    // 1: J piece (3x3)
    [
        [(0, 0), (0, 1), (1, 1), (2, 1)],  // 0: X..
        [(1, 0), (2, 0), (1, 1), (1, 2)],  // R: .XX
        [(0, 1), (1, 1), (2, 1), (2, 2)],  // 2: ..X
        [(1, 0), (1, 1), (0, 2), (1, 2)],  // L: XX.
    ],
    // 2: L piece (3x3)
    [
        [(2, 0), (0, 1), (1, 1), (2, 1)],  // 0: ..X
        [(1, 0), (1, 1), (1, 2), (2, 2)],  // R: .XX
        [(0, 1), (1, 1), (2, 1), (0, 2)],  // 2: X..
        [(0, 0), (1, 0), (1, 1), (1, 2)],  // L: XX.
    ],
    // 3: O piece (4x4, all states identical)
    [
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
    ],
    // 4: S piece (3x3)
    [
        [(1, 0), (2, 0), (0, 1), (1, 1)],  // 0: .XX
        [(1, 0), (1, 1), (2, 1), (2, 2)],  // R: .XX
        [(1, 1), (2, 1), (0, 2), (1, 2)],  // 2: XX.
        [(0, 0), (0, 1), (1, 1), (1, 2)],  // L: XX.
    ],
    // 5: T piece (3x3)
    [
        [(1, 0), (0, 1), (1, 1), (2, 1)],  // 0: .X.
        [(1, 0), (1, 1), (2, 1), (1, 2)],  // R: .XX
        [(0, 1), (1, 1), (2, 1), (1, 2)],  // 2: XXX
        [(1, 0), (0, 1), (1, 1), (1, 2)],  // L: XX.
    ],
    // 6: Z piece (3x3)
    [
        [(0, 0), (1, 0), (1, 1), (2, 1)],  // 0: XX.
        [(2, 0), (1, 1), (2, 1), (1, 2)],  // R: ..X
        [(0, 1), (1, 1), (1, 2), (2, 2)],  // 2: XX.
        [(1, 0), (0, 1), (1, 1), (0, 2)],  // L: .X.
    ],
];
```

### 2.3 Collision Detection

```rust
pub fn collides(
    test_x: i8, test_y: i8,
    piece_type: usize, rotation: usize,
    board: &[[Cell; BOARD_COLS]; BOARD_ROWS],
) -> bool {
    let cells = TETROMINOES[piece_type][rotation];
    for &(dx, dy) in &cells {
        let x = test_x + dx;
        let y = test_y + dy;
        if x < 0 || x >= BOARD_COLS as i8 { return true; }
        if y < 0 || y >= BOARD_ROWS as i8 { return true; }
        if board[y as usize][x as usize].0 != 0 { return true; }
    }
    false
}
```

### 2.4 SRS Wall Kicks

**JLSTZ Kicks** (screen coords, y+ = down):

```rust
pub const JLSTZ_KICKS: [[[(i8, i8); 5]; 4]; 4] = [
    // from 0 (spawn)
    [
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],           // 0->0 unused
        [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],       // 0->R
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],           // 0->2 unused
        [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],          // 0->L
    ],
    // from R
    [
        [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],           // R->0
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],           // R->R unused
        [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],           // R->2
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],           // R->L unused
    ],
    // from 2
    [
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],           // 2->0 unused
        [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],       // 2->R
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],           // 2->2 unused
        [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],          // 2->L
    ],
    // from L
    [
        [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],        // L->0
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],           // L->R unused
        [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],        // L->2
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],           // L->L unused
    ],
];
```

**I-Piece Kicks:**

```rust
pub const I_KICKS: [[[(i8, i8); 5]; 4]; 4] = [
    // from 0
    [
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
        [(1, 0), (-1, 0), (2, 0), (-1, 1), (2, -2)],         // 0->R
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
        [(0, 1), (-1, 1), (2, 1), (-1, -1), (2, 2)],         // 0->L
    ],
    // from R
    [
        [(-1, 0), (1, 0), (-2, 0), (1, -1), (-2, 2)],        // R->0
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
        [(0, 1), (-1, 1), (2, 1), (-1, -1), (2, 2)],         // R->2
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
    ],
    // from 2
    [
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
        [(0, -1), (1, -1), (-2, -1), (1, 1), (-2, -2)],      // 2->R
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
        [(-1, 0), (1, 0), (-2, 0), (1, -1), (-2, 2)],        // 2->L
    ],
    // from L
    [
        [(0, -1), (1, -1), (-2, -1), (1, 1), (-2, -2)],      // L->0
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
        [(1, 0), (-1, 0), (2, 0), (-1, 1), (2, -2)],         // L->2
        [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)],
    ],
];
```

### 2.5 Rotation Algorithm

```rust
pub fn try_rotate(
    piece_type: usize, from_rot: usize, to_rot: usize,
    piece_x: i8, piece_y: i8, board: &[[Cell; 10]; 20],
) -> Option<(i8, i8)> {
    let kicks = if piece_type == 0 { &I_KICKS[from_rot][to_rot] }
                else if piece_type == 3 { return Some((piece_x, piece_y)); } // O
                else { &JLSTZ_KICKS[from_rot][to_rot] };

    for &(dx, dy) in kicks {
        let test_x = piece_x + dx;
        let test_y = piece_y + dy;
        if !collides(test_x, test_y, piece_type, to_rot, board) {
            return Some((test_x, test_y));
        }
    }
    None
}
```

### 2.6 Scoring & Gravity

```rust
pub const LINE_CLEAR_BASES: [u32; 5] = [0, 40, 100, 300, 1200];

pub fn line_clear_score(lines: usize, level: usize) -> u32 {
    LINE_CLEAR_BASES[lines] * (level as u32 + 1)
}

pub const GRAVITY: &[u64] = &[
    48, 43, 38, 33, 28, 23, 18, 13, 8, 6,  // levels 1-10
    5,  5,  5,  4,  4,  4,  3,  3,  3, 2,   // levels 11-20
    2,  2,  2,  2,  2,  2,  2,  2,  2, 1,   // levels 21-29
];

pub fn gravity_interval_ns(level: u8) -> u64 {
    const FRAME_NS: u64 = 16_666_667;
    let idx = (level.saturating_sub(1)) as usize;
    let frames = GRAVITY.get(idx).copied().unwrap_or(1);
    frames * FRAME_NS
}
```

### 2.7 Ghost Piece

```rust
pub fn compute_ghost(piece_x: i8, piece_y: i8, piece_type: usize, rotation: usize, board: &[[Cell; 10]; 20]) -> (i8, i8) {
    let mut gy = piece_y;
    while !collides(piece_x, gy + 1, piece_type, rotation, board) && gy < 22 {
        gy += 1;
    }
    (piece_x, gy)
}
```

---

## 3. Rust Memory & Paradigm Mapper

### 3.1 SystemBus (Complete Definition)

```rust
use crate::chips::tetrominoes::PieceType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameState { Playing, Paused, GameOver }

#[derive(Clone, Debug)]
pub struct SystemBus {
    // ═══ REGISTERS (persist across ticks) ═══

    // Board
    pub board: [[Cell; BOARD_COLS]; BOARD_ROWS],

    // Active piece
    pub piece_type: u8,
    pub piece_x: i8,
    pub piece_y: i8,
    pub piece_rotation: u8,  // 0-3

    // Queue
    pub next_piece_type: u8,
    pub hold_piece_type: Option<u8>,
    pub hold_used: bool,

    // Scoring
    pub score: u32,
    pub level: u16,
    pub lines_cleared: u32,

    // Game lifecycle
    pub game_state: GameState,
    pub tick_count: u64,

    // Ghost piece (computed each tick, stored as register for TUI)
    pub ghost_x: i8,
    pub ghost_y: i8,

    // Gravity timer
    pub gravity_accumulator_ns: u64,
    pub gravity_interval_ns: u64,
    pub gravity_tick: bool,

    // DAS timer
    pub das_accumulator_ns: u64,
    pub das_delay_ns: u64,
    pub das_repeat_ns: u64,
    pub das_active: bool,
    pub das_direction: i8,
    pub das_tick: bool,
    pub das_last_repeat_index: u32,

    // Lock delay timer
    pub lock_delay_accumulator_ns: u64,
    pub lock_delay_max_ns: u64,
    pub lock_delay_active: bool,
    pub lock_delay_expired: bool,

    // Previous key state (for edge detection)
    pub prev_key_left: bool,
    pub prev_key_right: bool,
    pub prev_key_down: bool,
    pub prev_key_up: bool,
    pub prev_key_z: bool,
    pub prev_key_x: bool,
    pub prev_key_c: bool,
    pub prev_key_space: bool,

    // ═══ WIRES (reset every tick) ═══
    pub dx: i8,
    pub dy: i8,
    pub rotate_cw: bool,
    pub rotate_ccw: bool,
    pub hard_drop: bool,
    pub hold: bool,
    pub collision: bool,
    pub wall_kick_applied: bool,
    pub full_row_mask: u32,
    pub lines_to_clear_count: u8,
    pub piece_locked: bool,
    pub should_spawn_next: bool,
    pub game_over: bool,
}
```

### 3.2 LogicChip Trait

```rust
pub trait LogicChip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus);
}
```

**Borrow checker analysis:**
- `&self` and `&mut bus` are different allocations → no conflict
- `pins: &InputPins` is a third independent allocation → no conflict
- Chips are ZSTs (zero-sized types) → `&self` carries no data pointer
- No re-entrance: chips never call other chips
- Sequential pipeline: one `&mut bus` at a time, released when `tick()` returns
- No `RefCell`, no `Mutex`, no `unsafe` needed

### 3.3 Motherboard Pipeline

```rust
pub struct Motherboard {
    pipeline: Vec<Box<dyn LogicChip>>,
}

impl Motherboard {
    pub fn new() -> Self {
        Self {
            pipeline: vec![
                Box::new(InputDecoderChip),
                Box::new(GravityTimerChip),
                Box::new(DASChip),
                Box::new(CollisionDetectorChip),
                Box::new(RotationChip),
                Box::new(LockDelayChip),
                Box::new(LockChip),
                Box::new(LineClearChip),
                Box::new(ScoreChip),
                Box::new(SpawnChip),
                Box::new(GhostChip),
            ],
        }
    }

    pub fn tick(&mut self, pins: &InputPins, bus: &mut SystemBus) {
        // Phase 0: Reset wires
        Self::reset_wires(bus);

        // Phase 1: Combinational propagation
        for chip in &self.pipeline {
            chip.tick(pins, bus);
        }

        // Phase 2: Latching (falling clock edge)
        Self::latch(bus);

        // Phase 3: Edge detection state capture
        bus.prev_key_left = pins.key_left;
        bus.prev_key_right = pins.key_right;
        bus.prev_key_down = pins.key_down;
        bus.prev_key_up = pins.key_up;
        bus.prev_key_z = pins.key_z;
        bus.prev_key_x = pins.key_x;
        bus.prev_key_c = pins.key_c;
        bus.prev_key_space = pins.key_space;
    }

    fn reset_wires(bus: &mut SystemBus) {
        bus.dx = 0;
        bus.dy = 0;
        bus.rotate_cw = false;
        bus.rotate_ccw = false;
        bus.hard_drop = false;
        bus.hold = false;
        bus.collision = false;
        bus.wall_kick_applied = false;
        bus.full_row_mask = 0;
        bus.lines_to_clear_count = 0;
        bus.piece_locked = false;
        bus.should_spawn_next = false;
        bus.gravity_tick = false;
        bus.das_tick = false;
        bus.lock_delay_expired = false;
    }

    fn latch(bus: &mut SystemBus) {
        if bus.should_spawn_next {
            bus.piece_type = bus.next_piece_type;
            bus.piece_x = 3;
            bus.piece_y = 0;
            bus.piece_rotation = 0;
            bus.should_spawn_next = false;
            bus.hold_used = false;
        }
        if bus.hold && !bus.hold_used {
            let old = bus.piece_type;
            if let Some(held) = bus.hold_piece_type {
                bus.piece_type = held;
            }
            bus.hold_piece_type = Some(old);
            bus.hold_used = true;
            bus.piece_x = 3;
            bus.piece_y = 0;
            bus.piece_rotation = 0;
        }
        if bus.game_over {
            bus.game_state = GameState::GameOver;
        }
    }
}
```

**Enum-based alternative (zero heap, zero indirection):**

```rust
pub enum Chip {
    InputDecoder(InputDecoderChip),
    GravityTimer(GravityTimerChip),
    DAS(DASChip),
    CollisionDetector(CollisionDetectorChip),
    Rotation(RotationChip),
    LockDelay(LockDelayChip),
    Lock(LockChip),
    LineClear(LineClearChip),
    Score(ScoreChip),
    Spawn(SpawnChip),
    Ghost(GhostChip),
}

impl LogicChip for Chip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus) {
        match self {
            Chip::InputDecoder(c) => c.tick(pins, bus),
            Chip::GravityTimer(c) => c.tick(pins, bus),
            // ... each variant delegates
        }
    }
}
```

---

## 4. Clock & Game Loop Engineer

### 4.1 Main Loop Architecture

```rust
use std::time::{Duration, Instant};

const TARGET_FRAME_NS: u64 = 16_666_667;  // ~60 FPS
const MAX_FRAME_DELTA_NS: u64 = 1_000_000_000;  // cap for tab-away
const DAS_DELAY_NS: u64 = 266_666_672;    // ~267ms (16 frames)
const DAS_REPEAT_NS: u64 = 100_000_002;   // ~100ms (6 frames)
const LOCK_DELAY_MAX_NS: u64 = 500_000_010; // ~500ms

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = RawModeGuard::enter()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut bus = SystemBus {
        gravity_interval_ns: gravity_interval_ns(1),
        das_delay_ns: DAS_DELAY_NS,
        das_repeat_ns: DAS_REPEAT_NS,
        lock_delay_max_ns: LOCK_DELAY_MAX_NS,
        ..SystemBus::default()
    };
    let mut motherboard = Motherboard::new();

    let mut last_tick = Instant::now();
    let mut last_render = Instant::now();

    loop {
        let now = Instant::now();
        let frame_delta = now.duration_since(last_tick).as_nanos() as u64;
        let frame_delta = frame_delta.min(MAX_FRAME_DELTA_NS);
        last_tick = now;

        // Phase 1: Sample inputs
        let pins = poll_input_pins(frame_delta);

        // Phase 2+3: Run pipeline + latch
        motherboard.tick(&pins, &mut bus);

        // Phase 4: Render (throttled to ~60 FPS)
        if now.duration_since(last_render).as_nanos() as u64 >= TARGET_FRAME_NS {
            terminal.draw(|f| render_game(f, &bus))?;
            last_render = now;
        }

        // Yield to prevent busy-wait
        let elapsed = now.elapsed().as_nanos() as u64;
        if elapsed < 1_000_000 {
            std::thread::sleep(Duration::from_micros(500));
        }

        if bus.game_state == GameState::GameOver {
            terminal.draw(|f| render_game(f, &bus))?;
            thread::sleep(Duration::from_secs(2));
            break;
        }
    }

    Ok(())
}
```

### 4.2 Gravity Timer Chip

```rust
pub struct GravityTimerChip;
impl LogicChip for GravityTimerChip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus) {
        bus.gravity_accumulator_ns = bus.gravity_accumulator_ns.saturating_add(pins.frame_delta_ns);
        while bus.gravity_accumulator_ns >= bus.gravity_interval_ns {
            bus.gravity_tick = true;
            bus.dy = 1;
            bus.gravity_accumulator_ns -= bus.gravity_interval_ns;
        }
    }
}
```

### 4.3 DAS Chip

```rust
pub struct DASChip;
impl LogicChip for DASChip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus) {
        let left = pins.key_left;
        let right = pins.key_right;
        let prev_left = bus.prev_key_left;
        let prev_right = bus.prev_key_right;
        let left_pressed = left && !prev_left;
        let right_pressed = right && !prev_right;
        let released = (!left && prev_left) || (!right && prev_right);

        if left_pressed && !right {
            bus.das_tick = true; bus.dx = -1;
            bus.das_accumulator_ns = 0; bus.das_active = true;
            bus.das_direction = -1; bus.das_last_repeat_index = 0;
        } else if right_pressed && !left {
            bus.das_tick = true; bus.dx = 1;
            bus.das_accumulator_ns = 0; bus.das_active = true;
            bus.das_direction = 1; bus.das_last_repeat_index = 0;
        } else if left_pressed && right {
            bus.das_tick = true; bus.dx = -1;
            bus.das_accumulator_ns = 0; bus.das_active = true;
            bus.das_direction = -1; bus.das_last_repeat_index = 0;
        } else if right_pressed && left {
            bus.das_tick = true; bus.dx = 1;
            bus.das_accumulator_ns = 0; bus.das_active = true;
            bus.das_direction = 1; bus.das_last_repeat_index = 0;
        } else if released {
            bus.das_active = false; bus.das_direction = 0;
            bus.dx = 0; bus.das_accumulator_ns = 0;
            bus.das_last_repeat_index = 0;
        } else if bus.das_active && bus.das_direction != 0 {
            bus.das_accumulator_ns = bus.das_accumulator_ns.saturating_add(pins.frame_delta_ns);
            let acc = bus.das_accumulator_ns;
            if acc >= bus.das_delay_ns {
                let elapsed = acc - bus.das_delay_ns;
                let repeat_idx = elapsed / bus.das_repeat_ns;
                if repeat_idx > bus.das_last_repeat_index {
                    bus.das_tick = true; bus.dx = bus.das_direction;
                    bus.das_last_repeat_index = repeat_idx;
                }
            }
        }
    }
}
```

### 4.4 Lock Delay Chip

```rust
pub struct LockDelayChip;
impl LogicChip for LockDelayChip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus) {
        let cannot_fall = bus.collision && bus.dy == 0;
        let moved = bus.dx != 0 || bus.dy != 0 || bus.rotate_cw || bus.rotate_ccw;

        if moved && !cannot_fall {
            bus.lock_delay_accumulator_ns = 0;
            bus.lock_delay_active = false;
            bus.lock_delay_expired = false;
        } else if cannot_fall && !bus.piece_locked {
            bus.lock_delay_active = true;
            bus.lock_delay_accumulator_ns = bus.lock_delay_accumulator_ns.saturating_add(pins.frame_delta_ns);
            if bus.lock_delay_accumulator_ns >= bus.lock_delay_max_ns {
                bus.lock_delay_expired = true;
            }
        } else {
            bus.lock_delay_accumulator_ns = 0;
            bus.lock_delay_active = false;
            bus.lock_delay_expired = false;
        }
    }
}
```

### 4.5 Key Design Decisions

| Concern | Decision |
|---|---|
| Gravity accumulator | Uses `while` loop, preserves remainder via subtraction. Multiple drops can fire in one tick on lag. |
| Frame delta cap | Capped at 1 second to prevent gravity avalanche after tab-away |
| DAS repeat tracking | `das_last_repeat_index` prevents missed repeats from frame jitter |
| Timer precision | All u64 nanoseconds — no floating point, no `Duration` on the bus |
| Timer state location | All on `SystemBus` — chips are stateless |

---

## 5. TUI Rendering Engineer

### 5.1 Pure Render Function

```rust
use ratatui::{
    Frame, layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Style, Stylize, Color},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn render_game(frame: &mut Frame, bus: &SystemBus) {
    let chunks = Layout::horizontal([
        Constraint::Length(22),   // playfield
        Constraint::Min(20),      // sidebar
    ]).split(frame.area());

    render_playfield(frame, chunks[0], bus);

    let side = Layout::vertical([
        Constraint::Length(7),    // Hold
        Constraint::Length(1),
        Constraint::Length(7),    // Next
        Constraint::Length(1),
        Constraint::Length(6),    // Score
        Constraint::Min(0),
    ]).split(chunks[1]);

    render_mini(frame, side[0], " HOLD ", bus.hold_piece_type);
    render_mini(frame, side[2], " NEXT ", Some(bus.next_piece_type));
    render_score(frame, side[4], bus);

    match bus.game_state {
        GameState::Paused => render_overlay(frame, frame.area(), "PAUSED", Color::Yellow),
        GameState::GameOver => render_overlay(frame, frame.area(), "GAME OVER", Color::Red),
        _ => {}
    }
}
```

### 5.2 Playfield Rendering

```rust
fn render_playfield(frame: &mut Frame, area: Rect, bus: &SystemBus) {
    let mut lines: Vec<Line> = Vec::with_capacity(22);
    lines.push(Line::from(Span::raw("┌────────────────────┐")));

    for y in 0..20 {
        let mut spans = Vec::with_capacity(10);
        spans.push(Span::raw("│"));
        for x in 0..10 {
            let cell = bus.board[y][x].0;
            let active = is_pixel_filled(bus.piece_type, bus.piece_rotation, x as i8 - bus.piece_x, y as i8 - bus.piece_y);
            let ghost = !active && is_pixel_filled(bus.piece_type, bus.piece_rotation, x as i8 - bus.ghost_x, y as i8 - bus.ghost_y);

            if cell != 0 {
                let color = piece_color(cell);
                spans.push(Span::styled("██", Style::default().fg(color).bg(dim(color))));
            } else if active {
                let color = piece_color(bus.piece_type);
                spans.push(Span::styled("██", Style::default().fg(color).bg(dim(color))));
            } else if ghost {
                let color = piece_color(bus.piece_type);
                spans.push(Span::styled("░░", Style::default().fg(color).reversed()));
            } else {
                spans.push(Span::raw("  "));
            }
        }
        spans.push(Span::raw("│"));
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(Span::raw("└────────────────────┘")));
    let p = Paragraph::new(Text::from(lines)).block(Block::bordered().title(" TETRIS "));
    frame.render_widget(p, area);
}

fn is_pixel_filled(piece_type: u8, rotation: u8, cx: i8, cy: i8) -> bool {
    if piece_type == 0 && piece_type > 6 { return false; }
    let cells = TETROMINOES[piece_type as usize][rotation as usize % 4];
    for &(dx, dy) in &cells {
        if dx == cx && dy == cy { return true; }
    }
    false
}
```

### 5.3 Mini Grid (Hold/Next)

```rust
fn render_mini(frame: &mut Frame, area: Rect, title: &str, piece: Option<u8>) {
    let block = Block::bordered().title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if let Some(p) = piece {
        if p > 0 && p <= 7 {
            let mut lines = Vec::with_capacity(4);
            let cells = TETROMINOES[p as usize - 1][0];
            for row in 0..4 {
                let mut s = String::from(" ");
                for col in 0..4 {
                    let filled = cells.iter().any(|&(dx, dy)| dy == row && dx == col);
                    s.push_str(if filled { "██" } else { "  " });
                }
                s.push(' ');
                lines.push(Line::from(Span::styled(s, Style::default().fg(piece_color(p)))));
            }
            frame.render_widget(Paragraph::new(Text::from(lines)), inner);
        }
    }
}
```

### 5.4 Score & Overlay

```rust
fn render_score(frame: &mut Frame, area: Rect, bus: &SystemBus) {
    let text = Text::from(vec![
        Line::from(format!(" Score: {}", bus.score)),
        Line::from(format!(" Level: {}", bus.level)),
        Line::from(format!(" Lines: {}", bus.lines_cleared)),
    ]);
    frame.render_widget(Paragraph::new(text).block(Block::bordered().title(" STATS ")), area);
}

fn render_overlay(frame: &mut Frame, area: Rect, msg: &str, color: Color) {
    frame.render_widget(Block::default().style(Style::default().bg(Color::Black)), area);
    let para = Paragraph::new(Line::from(Span::styled(
        format!("\n\n  {}  \n\nEsc to quit", msg),
        Style::default().fg(color).bold(),
    ))).alignment(Alignment::Center).block(Block::bordered().style(Style::default().bg(Color::Black)));
    frame.render_widget(para, area);
}
```

### 5.5 Color Mapping

```rust
pub fn piece_color(piece_type: u8) -> Color {
    match piece_type {
        1 => Color::Cyan,
        2 => Color::Blue,
        3 => Color::Rgb(255, 165, 0),   // Orange
        4 => Color::Yellow,
        5 => Color::Green,
        6 => Color::Magenta,
        7 => Color::Red,
        _ => Color::White,
    }
}

fn dim(c: Color) -> Color {
    match c {
        Color::Cyan => Color::Rgb(0, 128, 128),
        Color::Blue => Color::Rgb(0, 0, 128),
        Color::Yellow => Color::Rgb(128, 128, 0),
        Color::Green => Color::Rgb(0, 128, 0),
        Color::Magenta => Color::Rgb(128, 0, 128),
        Color::Red => Color::Rgb(128, 0, 0),
        Color::Rgb(r, g, b) => Color::Rgb(r / 2, g / 2, b / 2),
        _ => Color::DarkGray,
    }
}
```

---

## 6. Architecture Synthesis

### 6.1 File Structure

```
tetris-silicon/
├── Cargo.toml                     # crossterm + ratatui
└── src/
    ├── main.rs                    # Wall-clock, I/O, main loop
    ├── bus.rs                     # InputPins, SystemBus, Cell, GameState
    ├── motherboard.rs             # Motherboard, LogicChip trait, Chip enum
    ├── tui.rs                     # render_game() — pure TUI function
    └── chips/
        ├── mod.rs                 # Re-exports
        ├── tetrominoes.rs         # TETROMINOES, KICKS, gravity, collision
        ├── input_decoder.rs       # Pin flags → movement wires
        ├── gravity_timer.rs       # Gravity accumulator chip
        ├── das_timer.rs           # DAS state machine chip
        ├── lock_delay.rs          # Lock delay accumulator chip
        ├── collision.rs           # Collision detection chip
        ├── rotation.rs            # Rotation + SRS wall kick chip
        ├── piece_lock.rs          # Lock expired → lock piece into board
        ├── line_clear.rs          # Detect + clear full rows
        ├── scoring.rs             # Score + level computation
        ├── spawn.rs               # Spawn next piece
        └── ghost.rs               # Ghost piece position computation
```

### 6.2 Data Flow Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    CLOCK TICK                            │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────┐    ┌───────────┐    ┌─────────────────┐  │
│  │ External │───▶│ InputPins │───▶│  Chip Pipeline   │  │
│  │ Keyboard │    │ (frozen   │    │  (sequential,    │  │
│  │          │    │  per tick)│    │   stateless)     │  │
│  └──────────┘    └───────────┘    └───────┬─────────┘  │
│                                           │            │
│  ┌──────────┐                             ▼            │
│  │ Terminal │◀───────────────────────┐ SystemBus      │
│  │ (ratatui)│    render_game(&bus)   │ (mutated by    │
│  │ pure fn  │◀───────────────────────│ chips)         │
│  └──────────┘                        └────────────────┘  │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 6.3 Paradigm Compliance Checklist

| Constraint | Implementation |
|---|---|
| No OOP | All chips are unit structs, all state on flat `SystemBus` |
| No events/callbacks | Single `tick()` trait, sequential `for chip in pipeline` |
| No async/await | Synchronous `poll(Duration::ZERO)`, blocking-free loop |
| No multi-threading | Single-threaded, no `std::thread`, no `Send`/`Sync` requirement |
| State-logic separation | `SystemBus` = all state, `Chip` impls = all logic |
| Tick-driven | Single `tick()` per iteration, 3-phase lifecycle |
| No unsafe | Entirely safe Rust, no raw pointers, no transmute |
| No RefCell/Mutex | `&mut SystemBus` per chip, sequential access |

---

## 7. Implementation Roadmap

### Phase 1: Architecture Skeleton
1. `src/bus.rs` — All types, `InputPins`, `SystemBus` with `Default`
2. `src/chips/mod.rs` — `LogicChip` trait + `Chip` enum
3. `src/motherboard.rs` — `Motherboard` with empty pipeline
4. `src/main.rs` — Terminal init, raw mode, empty loop
5. `Cargo.toml` — Declare dependencies

### Phase 2: Core Chips
6. `src/chips/tetrominoes.rs` — TETROMINOES const, collision function
7. `src/chips/input_decoder.rs` — Map InputPins to bus wires
8. `src/chips/collision.rs` — Collision detection
9. `src/chips/rotation.rs` — SRS rotation with wall kicks
10. `src/chips/piece_lock.rs` — Lock piece into board

### Phase 3: Game Logic
11. `src/chips/gravity_timer.rs` — Gravity accumulation
12. `src/chips/das_timer.rs` — Delayed Auto Shift
13. `src/chips/lock_delay.rs` — Lock delay timing
14. `src/chips/line_clear.rs` — Full row detection + clearing
15. `src/chips/scoring.rs` — Score + level computation
16. `src/chips/spawn.rs` — Piece spawning + bag randomizer
17. `src/chips/ghost.rs` — Ghost piece computation

### Phase 4: Rendering
18. `src/tui.rs` — Complete playfield, sidebar, overlays

### Phase 5: Polish
19. Hold piece swap feature
20. Next piece preview queue (3-bag randomizer)
21. Pause/resume functionality
22. Game over detection and restart
23. DAS/ARR tuning, lock delay move reset limit

---

*End of Master Research Report — all findings are actionable and aligned with the Silicon-Based Software Architecture Paradigm.*
