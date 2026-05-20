use crate::bus::{InputPins, SystemBus, PieceType};
use crate::chips::tetrominoes::collides;
use super::LogicChip;
pub(crate) use super::SpawnControllerChip;

/// Simple LCG for deterministic piece generation.
fn next_rand(state: &mut u32) -> u8 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    ((*state >> 16) % 7) as u8
}

impl LogicChip for SpawnControllerChip {
    fn tick(&self, _pins: &InputPins, bus: &mut SystemBus) {
        if !bus.wires.should_spawn_next {
            return;
        }

        bus.piece_type = bus.next_piece_type;
        bus.piece_x = 3;
        bus.piece_y = 0;
        bus.piece_rotation = 0;
        bus.hold_used = false;

        if collides(3, 0, bus.piece_type.0, 0, &bus.board) {
            bus.wires.game_over_triggered = true;
            return;
        }

        // Generate next piece using LCG stored on bus
        bus.next_piece_type = PieceType(next_rand(&mut bus.prng_state));

        bus.wires.should_spawn_next = false;
        bus.wires.render_dirty = true;
    }
}
