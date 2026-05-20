use crate::bus::{InputPins, SystemBus};
use crate::chips::tetrominoes::{collides, I_KICKS, JLSTZ_KICKS};
use super::LogicChip;
pub(crate) use super::RotationChip;

impl LogicChip for RotationChip {
    fn tick(&self, _pins: &InputPins, bus: &mut SystemBus) {
        if !bus.wires.rotate_cw && !bus.wires.rotate_ccw {
            return;
        }

        let from_rot = bus.piece_rotation as usize;
        let to_rot = if bus.wires.rotate_cw {
            (from_rot + 1) & 3
        } else {
            (from_rot + 3) & 3
        };

        let pt = bus.piece_type.0 as usize;

        // O piece (type 3) never needs wall kicks
        if pt == 3 {
            let test_x = bus.piece_x;
            let test_y = bus.piece_y;
            if !collides(test_x, test_y, pt as u8, to_rot as u8, &bus.board) {
                bus.piece_rotation = to_rot as u8;
                bus.wires.collision_any = false;
                bus.wires.render_dirty = true;
            }
            return;
        }

        let kicks = if pt == 0 {
            &I_KICKS[from_rot][to_rot]
        } else {
            &JLSTZ_KICKS[from_rot][to_rot]
        };

        for &(dx, dy) in kicks.iter() {
            let test_x = bus.piece_x + dx;
            let test_y = bus.piece_y + dy;
            if !collides(test_x, test_y, pt as u8, to_rot as u8, &bus.board) {
                bus.piece_x = test_x;
                bus.piece_y = test_y;
                bus.piece_rotation = to_rot as u8;
                bus.wires.collision_any = false;
                bus.wires.wall_kick_applied = dx != 0 || dy != 0;
                bus.wires.render_dirty = true;
                return;
            }
        }
    }
}
