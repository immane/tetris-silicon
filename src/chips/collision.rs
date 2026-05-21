pub(crate) use super::CollisionDetectorChip;
use super::LogicChip;
use crate::bus::{InputPins, SystemBus};
use crate::chips::tetrominoes::collides;

impl LogicChip for CollisionDetectorChip {
    fn tick(&self, _pins: &InputPins, bus: &mut SystemBus) {
        let px = bus.piece_x;
        let py = bus.piece_y;
        let pt = bus.piece_type.0;
        let pr = bus.piece_rotation;

        bus.wires.collision_any = collides(px, py, pt, pr, &bus.board);
        bus.wires.collision_down = collides(px, py + 1, pt, pr, &bus.board);
    }
}
