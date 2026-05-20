use crate::bus::{InputPins, SystemBus};
use super::LogicChip;
pub(crate) use super::InputDecoderChip;

impl LogicChip for InputDecoderChip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus) {
        // Level-triggered movement
        if pins.key_down {
            bus.wires.dy = 1;
        }

        // Edge-triggered action keys
        let up_edge = pins.key_up && !bus.prev_key_up;
        let x_edge = pins.key_x && !bus.prev_key_x;
        let z_edge = pins.key_z && !bus.prev_key_z;
        let space_edge = pins.key_space && !bus.prev_key_space;
        let c_edge = pins.key_c && !bus.prev_key_c;
        let enter_edge = pins.key_enter && !bus.prev_key_enter;

        bus.wires.rotate_cw = up_edge || x_edge;
        bus.wires.rotate_ccw = z_edge;
        bus.wires.hard_drop_requested = space_edge;
        bus.wires.hold_requested = c_edge;
        bus.wires.pause_requested = enter_edge;
    }
}
