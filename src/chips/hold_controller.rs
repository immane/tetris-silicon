use crate::bus::{InputPins, SystemBus, PieceType};
use super::LogicChip;
pub(crate) use super::HoldControllerChip;

impl LogicChip for HoldControllerChip {
    fn tick(&self, _pins: &InputPins, bus: &mut SystemBus) {
        if !bus.wires.hold_requested || bus.hold_used {
            return;
        }

        let old_type = bus.piece_type;
        if let Some(held) = bus.hold_piece_type {
            bus.piece_type = held;
        } else {
            bus.piece_type = bus.next_piece_type;
            bus.next_piece_type = PieceType(
                ((bus.prng_state.wrapping_mul(1664525).wrapping_add(1013904223)) >> 16) as u8 % 7,
            );
        }
        bus.hold_piece_type = Some(old_type);
        bus.hold_used = true;
        bus.piece_x = 3;
        bus.piece_y = 0;
        bus.piece_rotation = 0;
        bus.wires.render_dirty = true;
    }
}
