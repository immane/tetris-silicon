# T05: Physics Engine — Collision, SRS, Timing (`src/chips/tetrominoes.rs` + Layer 1 Chips)

**Task ID:** T05
**Title:** Tetromino Data, Collision Detection, SRS Wall Kicks, Layer 1 Timer Chips
**Depends On:** T01 (`src/bus.rs`), T02 (`src/chips/mod.rs`)
**Produces:** `src/chips/tetrominoes.rs`, `src/chips/gravity_timer.rs`, `src/chips/das_timer.rs`, `src/chips/lock_delay_timer.rs`, `src/chips/collision.rs`, `src/chips/rotation.rs`, `src/chips/movement.rs`

---

## Paradigm Constraints (Recap)

| Constraint | Meaning for This Task |
|---|---|
| **Stateless Chips** | Every chip struct is a UNIT STRUCT (`struct FooChip;`). No fields. All state lives on the bus. |
| **Pure Functions** | Chips read `pins` + `bus`, compute, write to `bus.wires` or `bus` registers. Always deterministic. |
| **No Side Effects** | No file I/O, no network, no RNG calls, no printing. |
| **Bus-Only Communication** | Chips write to bus wires. Downstream chips read those wires. No direct chip-to-chip calls. |
| **Single Responsibility** | Each chip does exactly ONE thing. |

---

## Implementation Goal

### Part A: `src/chips/tetrominoes.rs` — Static Data

This is NOT a chip. It's a module of `const` data and pure functions used by Layer 1 chips.

#### A.1 Tetromino Shapes (All 7 pieces × 4 rotations)

```rust
/// TETROMINOES[piece_type][rotation][block_index] -> (dx, dy)
/// piece_type: 0=I, 1=J, 2=L, 3=O, 4=S, 5=T, 6=Z
/// rotation: 0=spawn, 1=CW, 2=180, 3=CCW
/// dx, dy: offset from anchor (top-left of bounding box). y+ = DOWN.
pub const TETROMINOES: [[[(i8, i8); 4]; 4]; 7] = [
    // 0: I piece (4x4 box)
    [[(0,1),(1,1),(2,1),(3,1)], [(2,0),(2,1),(2,2),(2,3)], [(0,2),(1,2),(2,2),(3,2)], [(1,0),(1,1),(1,2),(1,3)]],
    // 1: J piece (3x3 box)
    [[(0,0),(0,1),(1,1),(2,1)], [(1,0),(2,0),(1,1),(1,2)], [(0,1),(1,1),(2,1),(2,2)], [(1,0),(1,1),(0,2),(1,2)]],
    // 2: L piece (3x3 box)
    [[(2,0),(0,1),(1,1),(2,1)], [(1,0),(1,1),(1,2),(2,2)], [(0,1),(1,1),(2,1),(0,2)], [(0,0),(1,0),(1,1),(1,2)]],
    // 3: O piece (4x4 box, all identical)
    [[(1,0),(2,0),(1,1),(2,1)], [(1,0),(2,0),(1,1),(2,1)], [(1,0),(2,0),(1,1),(2,1)], [(1,0),(2,0),(1,1),(2,1)]],
    // 4: S piece (3x3 box)
    [[(1,0),(2,0),(0,1),(1,1)], [(1,0),(1,1),(2,1),(2,2)], [(1,1),(2,1),(0,2),(1,2)], [(0,0),(0,1),(1,1),(1,2)]],
    // 5: T piece (3x3 box)
    [[(1,0),(0,1),(1,1),(2,1)], [(1,0),(1,1),(2,1),(1,2)], [(0,1),(1,1),(2,1),(1,2)], [(1,0),(0,1),(1,1),(1,2)]],
    // 6: Z piece (3x3 box)
    [[(0,0),(1,0),(1,1),(2,1)], [(2,0),(1,1),(2,1),(1,2)], [(0,1),(1,1),(1,2),(2,2)], [(1,0),(0,1),(1,1),(0,2)]],
];
```

#### A.2 SRS Wall Kick Tables

```rust
/// JLSTZ_KICKS[from_rotation][to_rotation][test_index] -> (dx, dy)
/// Screen coords: y+ = DOWN. Only 8 of 16 transitions are used.
pub const JLSTZ_KICKS: [[[(i8, i8); 5]; 4]; 4] = [
    // from 0
    [[(0,0);5], [(0,0),(-1,0),(-1,1),(0,-2),(-1,-2)], [(0,0);5], [(0,0),(1,0),(1,1),(0,-2),(1,-2)]],
    // from R
    [[(0,0),(1,0),(1,-1),(0,2),(1,2)], [(0,0);5], [(0,0),(1,0),(1,-1),(0,2),(1,2)], [(0,0);5]],
    // from 2
    [[(0,0);5], [(0,0),(-1,0),(-1,1),(0,-2),(-1,-2)], [(0,0);5], [(0,0),(1,0),(1,1),(0,-2),(1,-2)]],
    // from L
    [[(0,0),(-1,0),(-1,-1),(0,2),(-1,2)], [(0,0);5], [(0,0),(-1,0),(-1,-1),(0,2),(-1,2)], [(0,0);5]],
];

/// I_KICKS[from_rotation][to_rotation][test_index] -> (dx, dy)
pub const I_KICKS: [[[(i8, i8); 5]; 4]; 4] = [
    // from 0
    [[(0,0);5], [(1,0),(-1,0),(2,0),(-1,1),(2,-2)], [(0,0);5], [(0,1),(-1,1),(2,1),(-1,-1),(2,2)]],
    // from R
    [[(-1,0),(1,0),(-2,0),(1,-1),(-2,2)], [(0,0);5], [(0,1),(-1,1),(2,1),(-1,-1),(2,2)], [(0,0);5]],
    // from 2
    [[(0,0);5], [(0,-1),(1,-1),(-2,-1),(1,1),(-2,-2)], [(0,0);5], [(-1,0),(1,0),(-2,0),(1,-1),(-2,2)]],
    // from L
    [[(0,-1),(1,-1),(-2,-1),(1,1),(-2,-2)], [(0,0);5], [(1,0),(-1,0),(2,0),(-1,1),(2,-2)], [(0,0);5]],
];
```

#### A.3 Collision Detection Function

```rust
use crate::bus::{Cell, BOARD_COLS, BOARD_ROWS};

/// Returns true if the piece at (test_x, test_y) with given rotation
/// collides with walls, floor, or locked blocks.
pub fn collides(
    test_x: i8, test_y: i8,
    piece_type: u8, rotation: u8,
    board: &[[Cell; BOARD_COLS]; BOARD_ROWS],
) -> bool {
    let cells = &TETROMINOES[piece_type as usize][rotation as usize];
    for &(dx, dy) in cells.iter() {
        let x = test_x + dx;
        let y = test_y + dy;
        if x < 0 || x >= BOARD_COLS as i8 { return true; }
        if y < 0 || y >= BOARD_ROWS as i8 { return true; }
        if board[y as usize][x as usize].0 != 0 { return true; }
    }
    false
}
```

#### A.4 Ghost Computation Function

```rust
/// Compute the ghost piece Y (hard drop preview).
pub fn ghost_y(
    piece_x: i8, piece_y: i8,
    piece_type: u8, rotation: u8,
    board: &[[Cell; BOARD_COLS]; BOARD_ROWS],
) -> i8 {
    let mut gy = piece_y;
    while !collides(piece_x, gy + 1, piece_type, rotation, board) {
        gy += 1;
    }
    gy
}
```

### Part B: Layer 1 Chip Implementations

Each file: `struct XxxChip;` → `impl LogicChip for XxxChip { fn tick(...) { ... } }`.

#### B.1 `src/chips/gravity_timer.rs` — GravityTimerChip

```
Reads:  pins.frame_delta_ns, bus.gravity_accumulator_ns, bus.gravity_interval_ns
Writes: wires.gravity_tick, wires.dy (=1), bus.gravity_accumulator_ns

Algorithm:
  acc += delta_ns
  while acc >= interval:
    set gravity_tick = true
    set dy = 1  (signal downstream to move piece down)
    acc -= interval
  store acc back
```

#### B.2 `src/chips/das_timer.rs` — DasTimerChip

```
Reads:  pins.key_left, pins.key_right, pins.frame_delta_ns,
        bus.prev_key_left, bus.prev_key_right,
        bus.das_accumulator_ns, bus.das_delay_ns, bus.das_repeat_ns,
        bus.das_active, bus.das_direction, bus.das_last_repeat_index

Writes: wires.das_tick, wires.dx, bus.das_* registers

Algorithm:
  - Edge detection: left_just_pressed = key_left && !prev_key_left, etc.
  - On new press: immediate move (das_tick=true, dx=dir), start accumulator
  - On release: reset all DAS state
  - While held past delay: compute repeat_index = (acc - delay) / repeat
    If repeat_index > last_repeat_index: fire das_tick, update dx, update index
  - Last-pressed wins for simultaneous left+right
```

#### B.3 `src/chips/lock_delay_timer.rs` — LockDelayTimerChip

```
Reads:  pins.frame_delta_ns, bus.lock_delay_accumulator_ns, bus.lock_delay_max_ns,
        wires.collision_down, wires.dx, wires.dy, wires.rotate_cw, wires.rotate_ccw,
        wires.piece_locked

Writes: wires.lock_delay_active, wires.lock_delay_expired, bus.lock_delay_accumulator_ns

Algorithm:
  - If piece moved/rotated successfully AND not resting: reset timer
  - If collision_down AND not locked: accumulate time
  - If accumulator >= max: set lock_delay_expired = true
```

#### B.4 `src/chips/collision.rs` — CollisionDetectorChip

```
Reads:  bus.piece_x, bus.piece_y, bus.piece_type.0, bus.piece_rotation, bus.board

Writes: wires.collision_down, wires.collision_any

Algorithm:
  - collision_any = collides(current_x, current_y, type, rot, board)
  - collision_down = collides(current_x, current_y + 1, type, rot, board)
  Note: This tests the PRE-MOVEMENT position. After MovementChip succeeds,
  it updates collision wires to reflect the new state.
```

#### B.5 `src/chips/rotation.rs` — RotationChip

```
Reads:  wires.rotate_cw, wires.rotate_ccw, wires.collision_any,
        bus.piece_type, bus.piece_x, bus.piece_y, bus.piece_rotation,
        bus.board

Writes: bus.piece_rotation, bus.piece_x, bus.piece_y, wires.wall_kick_applied,
        wires.collision_any (clear on success), wires.render_dirty

Algorithm:
  - If no rotation requested: return
  - Determine to_rotation: (current + 1) % 4 for CW, (current + 3) % 4 for CCW
  - Select kick table: I_KICKS for piece 0, skip for piece 3 (O), JLSTZ_KICKS for others
  - For each of 5 tests:
      test_x = piece_x + kick[from][to][i].dx
      test_y = piece_y + kick[from][to][i].dy
      if !collides(test_x, test_y, type, to_rotation, board):
        commit: piece_x=test_x, piece_y=test_y, piece_rotation=to_rotation
        clear collision_any, set wall_kick_applied, set render_dirty
        return
  - If no test passes: rotation fails, leave state unchanged
```

#### B.6 `src/chips/movement.rs` — MovementChip

```
Reads:  wires.dx, wires.dy, wires.gravity_tick, wires.das_tick,
        wires.hard_drop_requested, bus.piece_x, bus.piece_y,
        bus.piece_type.0, bus.piece_rotation, bus.board

Writes: bus.piece_x, bus.piece_y,
        wires.collision_down (re-test after move),
        wires.collision_any (clear on success),
        wires.piece_locked (on hard drop),
        wires.render_dirty

Algorithm:
  - If hard_drop_requested:
      call ghost_y() → set piece_y = ghost_y, set piece_locked = true
      return
  - If dx != 0:
      test new_x = piece_x + dx
      if !collides(new_x, piece_y, type, rot, board):
        piece_x = new_x, clear collision_any
  - If dy != 0 OR gravity_tick:
      test new_y = piece_y + 1  (if dy=1 or gravity_tick)
      if !collides(piece_x, new_y, type, rot, board):
        piece_y = new_y, clear collision_any
  - Re-test collision_down at final position:
      collision_down = collides(piece_x, piece_y + 1, type, rot, board)
```

---

## Verification Protocol (Guardrail Agent B)

1. **Statelessness:** Every chip file must contain ONLY `pub struct XxxChip;` — no fields. Verify with `rg "pub struct \w+Chip" src/chips/` — all results end with `;`.
2. **Tetromino data correctness:** Verify all 28 rotation states (7 pieces × 4 rotations). Verify each state has exactly 4 block offsets. Verify no coordinate exceeds the bounding box dimensions.
3. **Kick table correctness:** Verify `JLSTZ_KICKS` has exactly the values from the SRS spec. Verify `I_KICKS` matches. Verify O-piece uses neither table (rotation always succeeds at current position).
4. **Collision function:** Test with known inputs. Piece at (3, 0) with rotation 0 on empty board must NOT collide. Piece at (3, 18) must collide down. Piece at (-1, 0) must collide left wall.
5. **Gravity timer:** Verify accumulator is ADDED TO (not replaced). Verify remainder is preserved (subtract interval, don't reset to zero). Verify `while` loop allows multiple ticks on lag.
6. **DAS timer:** Verify edge detection uses `prev_key_*` registers. Verify `das_last_repeat_index` prevents missed ticks. Verify release resets all DAS state.
7. **Lock delay timer:** Verify it runs AFTER movement (reads post-move collision_down). Verify reset on successful move/rotate. Verify lock_delay_expired asserts when accumulator exceeds max.
8. **Rotation chip:** Verify it tries all 5 kick tests in order. Verify it stops at first successful test. Verify rotation + position update in a single chip (atomic from downstream perspective).
9. **Movement chip:** Verify horizontal movement is independent of vertical. Verify hard drop uses ghost_y() and sets piece_locked. Verify collision_down is re-tested at final position.
10. **No inter-chip calls:** Verify no chip file contains a `.tick(` call. Only `mod.rs` has the dispatch match.
11. **Compile check:** `cargo check` must pass with all 7 new files.
12. **No unsafe:** `grep -rn "unsafe" src/chips/` returns nothing.

---

## Acceptance Criteria

- [ ] `src/chips/tetrominoes.rs` exists with TETROMINOES, JLSTZ_KICKS, I_KICKS, collides(), ghost_y()
- [ ] `src/chips/gravity_timer.rs` — GravityTimerChip with remainder-preserving accumulator
- [ ] `src/chips/das_timer.rs` — DasTimerChip with edge detection + repeat-index tracking
- [ ] `src/chips/lock_delay_timer.rs` — LockDelayTimerChip with move-reset logic
- [ ] `src/chips/collision.rs` — CollisionDetectorChip testing current + downward positions
- [ ] `src/chips/rotation.rs` — RotationChip with 5-test SRS wall kick loop
- [ ] `src/chips/movement.rs` — MovementChip with horizontal, vertical, hard drop modes
- [ ] All chips are unit structs (zero fields)
- [ ] All chips implement `LogicChip` trait
- [ ] Collision detection correctly handles walls, floor, and locked blocks
- [ ] Rotation with SRS wall kicks works for all 4 non-trivial transitions
- [ ] Hard drop immediately places piece at ghost position and triggers lock
- [ ] Zero direct chip-to-chip calls
- [ ] Zero `unsafe` blocks
