pub(crate) use super::LineClearCommitterChip;
pub(crate) use super::LineClearDetectorChip;
use super::LogicChip;
use crate::bus::{Cell, InputPins, SystemBus, BOARD_COLS, BOARD_ROWS};

// ─── LineClearDetectorChip ────────────────────────────────────────────────

impl LogicChip for LineClearDetectorChip {
    fn tick(&self, _pins: &InputPins, bus: &mut SystemBus) {
        let mut mask: u32 = 0;
        let mut count: u8 = 0;

        for y in 0..BOARD_ROWS {
            let full = (0..BOARD_COLS).all(|x| bus.board[y][x].0 != 0);
            if full {
                mask |= 1 << y;
                count += 1;
            }
        }

        bus.wires.full_row_mask = mask;
        bus.wires.lines_cleared_this_tick = count;
    }
}

// ─── LineClearCommitterChip ───────────────────────────────────────────────

impl LogicChip for LineClearCommitterChip {
    fn tick(&self, _pins: &InputPins, bus: &mut SystemBus) {
        let mask = bus.wires.full_row_mask;
        if mask == 0 {
            return;
        }

        // Compact downward: copy non-full rows to the bottom
        let mut dst = (BOARD_ROWS - 1) as i8;
        for src in (0..BOARD_ROWS as i8).rev() {
            if mask & (1 << src) == 0 {
                if dst != src {
                    bus.board[dst as usize] = bus.board[src as usize];
                }
                dst -= 1;
            }
        }

        // Fill remaining top rows with empty cells
        for y in 0..=dst {
            bus.board[y as usize] = [Cell(0); BOARD_COLS];
        }

        bus.wires.render_dirty = true;
    }
}
