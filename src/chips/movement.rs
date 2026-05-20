use crate::bus::{InputPins, SystemBus};
use crate::chips::tetrominoes::{collides, ghost_y};
use super::LogicChip;
pub(crate) use super::MovementChip;

impl LogicChip for MovementChip {
    fn tick(&self, _pins: &InputPins, bus: &mut SystemBus) {
        let pt = bus.piece_type.0;
        let pr = bus.piece_rotation;

        // Hard drop: instantly place at ghost position and lock
        if bus.wires.hard_drop_requested {
            let gy = ghost_y(bus.piece_x, bus.piece_y, pt, pr, &bus.board);
            bus.piece_y = gy;
            bus.wires.piece_locked = true;
            bus.wires.render_dirty = true;
            return;
        }

        // Horizontal movement
        let dx = bus.wires.dx;
        if dx != 0 {
            let new_x = bus.piece_x + dx;
            if !collides(new_x, bus.piece_y, pt, pr, &bus.board) {
                bus.piece_x = new_x;
                bus.wires.collision_any = false;
                bus.wires.render_dirty = true;
            }
        }

        // Vertical movement (soft drop or gravity)
        let should_drop = bus.wires.dy != 0 || bus.wires.gravity_tick;
        if should_drop {
            let new_y = bus.piece_y + 1;
            if !collides(bus.piece_x, new_y, pt, pr, &bus.board) {
                bus.piece_y = new_y;
                bus.wires.collision_any = false;
                bus.wires.render_dirty = true;
            }
        }

        // Re-test collision_down at final position
        bus.wires.collision_down = collides(
            bus.piece_x,
            bus.piece_y + 1,
            pt,
            pr,
            &bus.board,
        );
    }
}
