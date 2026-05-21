pub(crate) use super::GhostComputerChip;
use super::LogicChip;
use crate::bus::{InputPins, SystemBus};
use crate::chips::tetrominoes::ghost_y;

impl LogicChip for GhostComputerChip {
    fn tick(&self, _pins: &InputPins, bus: &mut SystemBus) {
        bus.ghost_x = bus.piece_x;
        bus.ghost_y = ghost_y(
            bus.piece_x,
            bus.piece_y,
            bus.piece_type.0,
            bus.piece_rotation,
            &bus.board,
        );
    }
}
