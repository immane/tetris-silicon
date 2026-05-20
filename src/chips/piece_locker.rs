use crate::bus::{InputPins, SystemBus, Cell};
use crate::chips::tetrominoes::TETROMINOES;
use super::LogicChip;
pub(crate) use super::PieceLockerChip;

impl LogicChip for PieceLockerChip {
    fn tick(&self, _pins: &InputPins, bus: &mut SystemBus) {
        let should_lock = bus.wires.piece_locked || bus.wires.lock_delay_expired;
        if !should_lock {
            return;
        }

        let pt = bus.piece_type.0 as usize;
        let pr = bus.piece_rotation as usize;
        let cells = &TETROMINOES[pt][pr];
        let color = (bus.piece_type.0 + 1) as u8;

        for &(dx, dy) in cells.iter() {
            let bx = bus.piece_x + dx;
            let by = bus.piece_y + dy;
            if by < 0 || by >= 20 || bx < 0 || bx >= 10 {
                bus.wires.game_over_triggered = true;
                return;
            }
            bus.board[by as usize][bx as usize] = Cell(color);
        }

        bus.wires.piece_locked = true;
        bus.wires.should_spawn_next = true;
        bus.wires.render_dirty = true;
    }
}
