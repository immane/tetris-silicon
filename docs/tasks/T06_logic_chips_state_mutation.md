# T06: State Mutation Chips — Layer 2 (`src/chips/` — 7 files)

**Task ID:** T06
**Title:** Piece Lock, Line Clear, Scoring, Hold, Spawn — Full State Mutation Pipeline
**Depends On:** T01, T02, T05 (Layer 1 chips write wires that Layer 2 chips read)
**Produces:** `src/chips/piece_locker.rs`, `src/chips/line_clear.rs`, `src/chips/score_keeper.rs`, `src/chips/level_calc.rs`, `src/chips/hold_controller.rs`, `src/chips/spawn_controller.rs`

---

## Paradigm Constraints (Recap)

| Constraint | Meaning for This Task |
|---|---|
| **Stateless Chips** | All unit structs. Zero fields. |
| **Register Mutation** | Layer 2 chips write DIRECTLY to bus registers (board, score, level, piece state). This is the "Sequential Latching" equivalent within the pipeline. |
| **Wire Consumption** | Read wires set by Layer 1 chips (collision_down, piece_locked, lock_delay_expired, etc.). Do NOT modify those wires unless they are Layer 2 outputs read by other Layer 2 chips. |
| **Single Direction** | Data flows Layer 1 → Layer 2 → Layer 3. Layer 2 chips cannot affect Layer 1 chips within the same tick. |
| **No Privilege Escalation** | Each chip ONLY touches its documented registers. PieceLocker never modifies score. ScoreKeeper never modifies board. |

---

## Implementation Goal

### 1. `src/chips/piece_locker.rs` — PieceLockerChip

```
Duty: Lock the active piece into the board when it cannot fall further.

Reads:  wires.piece_locked (from hard drop in MovementChip),
        wires.lock_delay_expired (from LockDelayTimerChip),
        bus.piece_type.0, bus.piece_x, bus.piece_y, bus.piece_rotation,
        bus.board

Writes: bus.board (add piece cells), wires.piece_locked (if lock delay expired),
        wires.should_spawn_next, wires.render_dirty

Algorithm:
  - Determine if piece should lock: hard_drop_already_locked OR lock_delay_expired
  - If not locking: return
  - If locking AND piece_y < 0 (piece above visible area): trigger game_over instead
  - Write piece cells to board: for each offset in TETROMINOES[type][rot]:
      board[piece_y + dy][piece_x + dx] = Cell(piece_type + 1)
  - Set piece_locked = true
  - Set should_spawn_next = true
  - Set render_dirty = true
```

### 2. `src/chips/line_clear.rs` — LineClearDetectorChip + LineClearCommitterChip

Two separate chips in one file (related operations).

#### LineClearDetectorChip

```
Duty: Scan all 20 rows. Set bitmask of full rows.

Reads:  bus.board

Writes: wires.full_row_mask (bit N set = row N is full),
        wires.lines_cleared_this_tick (count of full rows)

Algorithm:
  - full_row_mask = 0
  - count = 0
  - For each row y from 0 to 19:
      if all cells in row are non-zero:
        full_row_mask |= (1 << y)
        count += 1
  - lines_cleared_this_tick = count
```

#### LineClearCommitterChip

```
Duty: Remove full rows. Compact the board downward (rows above shift down).

Reads:  wires.full_row_mask

Writes: bus.board, wires.render_dirty

Algorithm:
  - If full_row_mask == 0: return
  - Compact downward:
      dst = 19 (bottom row)
      for src from 19 down to 0:
        if row src is NOT full:
          copy row[src] to row[dst]
          dst -= 1
      fill rows 0..=dst with empty cells
  - Set render_dirty = true
```

### 3. `src/chips/score_keeper.rs` — ScoreKeeperChip

```
Duty: Compute score from lines cleared. NES formula: base_points[lines] × (level + 1)

Reads:  wires.lines_cleared_this_tick, bus.level, bus.score, bus.lines_cleared

Writes: bus.score, bus.lines_cleared

Algorithm:
  - If lines_cleared_this_tick == 0: return
  - line_clear_bases = [0, 40, 100, 300, 1200]
  - added = line_clear_bases[lines_cleared_this_tick] × (level + 1) as u32
  - score += added
  - lines_cleared += lines_cleared_this_tick as u32
```

### 4. `src/chips/level_calc.rs` — LevelCalculatorChip

```
Duty: Compute level from total lines cleared. Update gravity interval.

Reads:  bus.lines_cleared, bus.level, bus.gravity_interval_ns

Writes: bus.level, bus.gravity_interval_ns

Notes: Level = 1 + (lines_cleared / 10). Only update if changed.
       Need to import gravity_interval_ns from bus module.
```

### 5. `src/chips/hold_controller.rs` — HoldControllerChip

```
Duty: Swap active piece with hold slot. Prevent double-hold per turn.

Reads:  wires.hold_requested, bus.hold_used,
        bus.piece_type, bus.hold_piece_type

Writes: bus.piece_type, bus.piece_x (=3), bus.piece_y (=0),
        bus.piece_rotation (=0), bus.hold_piece_type, bus.hold_used (=true),
        wires.render_dirty

Algorithm:
  - If hold NOT requested OR hold_used: return
  - Save current piece: old_type = piece_type
  - If hold_piece_type is Some(held):
      piece_type = held
  - Else:
      piece_type = next_piece_type (needs next piece access — or handle later)
  - hold_piece_type = Some(old_type)
  - hold_used = true
  - Reset piece position: piece_x = 3, piece_y = 0, piece_rotation = 0
  - Set render_dirty = true
```

### 6. `src/chips/spawn_controller.rs` — SpawnControllerChip

```
Duty: Spawn the next piece. Check game over condition.

Reads:  wires.should_spawn_next,
        bus.next_piece_type, bus.board

Writes: bus.piece_type, bus.piece_x (=3), bus.piece_y (=0),
        bus.piece_rotation (=0), bus.hold_used (=false),
        bus.next_piece_type (random / next from bag),
        wires.game_over_triggered, wires.render_dirty

Algorithm:
  - If NOT should_spawn_next: return
  - Set piece_type = next_piece_type
  - Set piece_x = 3, piece_y = 0, piece_rotation = 0
  - Reset hold_used = false (new turn)
  - Check if spawn position collides:
      if collides(3, 0, piece_type.0, 0, board):
        set game_over_triggered = true
        return
  - Generate next piece (simple: random 0-6 for now; bag randomizer later)
  - Set should_spawn_next = false
  - Set render_dirty = true
```

**Note on RNG:** For the initial implementation, use a simple deterministic sequence or a hardcoded cycle. True randomness must be on the bus (a `prng_state` register). Add skeleton code that leaves `next_piece_type` unchanged if no prng is set up, or use a simple LCG:
```rust
// Simple LCG for deterministic piece generation
// Store prng_state on SystemBus if needed (add a field)
fn next_piece(seed: &mut u32) -> u8 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    ((*seed >> 16) % 7) as u8
}
```

---

## Verification Protocol (Guardrail Agent B)

1. **Statelessness:** All 6 chip structs are unit structs. Verify with `rg "pub struct \w+Chip" src/chips/piece_locker.rs src/chips/line_clear.rs src/chips/score_keeper.rs src/chips/level_calc.rs src/chips/hold_controller.rs src/chips/spawn_controller.rs` — all end with `;`.
2. **Piece locker:** Verify it writes piece cells to the board using the correct offsets from TETROMINOES. Verify it checks `piece_y < 0` for game over. Verify it sets both `piece_locked` and `should_spawn_next` wires.
3. **Line clear detector:** Verify it iterates all 20 rows. Verify `full_row_mask` uses bit N for row N. Verify `lines_cleared_this_tick` equals `full_row_mask.count_ones()` at most (or equals the count from the loop).
4. **Line clear committer:** Verify it ONLY shifts rows when `full_row_mask != 0`. Verify the compaction algorithm correctly fills the top with empty rows. Verify it preserves row order of surviving rows.
5. **Score keeper:** Verify formula: `base[lines] * (level + 1)`. Base array: [0, 40, 100, 300, 1200]. Verify `lines_cleared` register accumulates (not replaced).
6. **Level calculator:** Verify formula: `level = 1 + lines_cleared / 10`. Verify gravity interval updates via `gravity_interval_ns(level)`.
7. **Hold controller:** Verify it prevents double-hold via `hold_used` check. Verify piece position resets to (3, 0, rotation 0). Verify hold swap logic is correct.
8. **Spawn controller:** Verify it only acts when `should_spawn_next` is true. Verify game over when spawn position collides. Verify `hold_used` resets to false. Verify next piece advances.
9. **Ordering dependency:** Verify Layer 2 chips appear in the Motherboard in this order: PieceLocker → HoldController → LineClearDetector → LineClearCommitter → ScoreKeeper → LevelCalculator → SpawnController. The order matters: Lock before clear, clear before score, score before level.
10. **Compile check:** `cargo check` must pass.
11. **No unsafe:** `grep -rn "unsafe" src/chips/` returns nothing.

---

## Acceptance Criteria

- [ ] `src/chips/piece_locker.rs` — locks piece into board, triggers game over if above visible area
- [ ] `src/chips/line_clear.rs` — detects full rows + compacts board
- [ ] `src/chips/score_keeper.rs` — NES formula: base[lines] × (level + 1)
- [ ] `src/chips/level_calc.rs` — level = 1 + lines/10, updates gravity interval
- [ ] `src/chips/hold_controller.rs` — hold swap with double-hold prevention
- [ ] `src/chips/spawn_controller.rs` — spawns next piece, checks game over, resets hold_used
- [ ] All chips are unit structs (zero fields)
- [ ] Layer 2 chips appear in correct order in Motherboard::new()
- [ ] Board mutation only in PieceLocker and LineClearCommitter
- [ ] Scoring uses correct NES formula
- [ ] Game over triggers when spawn position collides
- [ ] Zero inter-chip calls
- [ ] Zero `unsafe` blocks
