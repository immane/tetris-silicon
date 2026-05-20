use crate::bus::{InputPins, SystemBus};
use super::LogicChip;
pub(crate) use super::DasTimerChip;

impl LogicChip for DasTimerChip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus) {
        let left = pins.key_left;
        let right = pins.key_right;
        let prev_left = bus.prev_key_left;
        let prev_right = bus.prev_key_right;

        let left_just_pressed = left && !prev_left;
        let right_just_pressed = right && !prev_right;
        let any_released = (!left && prev_left) || (!right && prev_right);

        if left_just_pressed && !right {
            bus.wires.das_tick = true;
            bus.wires.dx = -1;
            bus.das_accumulator_ns = 0;
            bus.das_active = true;
            bus.das_direction = -1;
            bus.das_last_repeat_index = 0;
        } else if right_just_pressed && !left {
            bus.wires.das_tick = true;
            bus.wires.dx = 1;
            bus.das_accumulator_ns = 0;
            bus.das_active = true;
            bus.das_direction = 1;
            bus.das_last_repeat_index = 0;
        } else if left_just_pressed && right {
            bus.wires.das_tick = true;
            bus.wires.dx = -1;
            bus.das_accumulator_ns = 0;
            bus.das_active = true;
            bus.das_direction = -1;
            bus.das_last_repeat_index = 0;
        } else if right_just_pressed && left {
            bus.wires.das_tick = true;
            bus.wires.dx = 1;
            bus.das_accumulator_ns = 0;
            bus.das_active = true;
            bus.das_direction = 1;
            bus.das_last_repeat_index = 0;
        } else if any_released {
            bus.das_active = false;
            bus.das_direction = 0;
            bus.wires.dx = 0;
            bus.das_accumulator_ns = 0;
            bus.das_last_repeat_index = 0;
        } else if bus.das_active && bus.das_direction != 0 {
            bus.das_accumulator_ns = bus
                .das_accumulator_ns
                .saturating_add(pins.frame_delta_ns);

            let acc = bus.das_accumulator_ns;
            if acc >= bus.das_delay_ns {
                let elapsed = acc - bus.das_delay_ns;
                let repeat_idx = (elapsed / bus.das_repeat_ns) as u32;
                if repeat_idx > bus.das_last_repeat_index {
                    bus.wires.das_tick = true;
                    bus.wires.dx = bus.das_direction;
                    bus.das_last_repeat_index = repeat_idx;
                }
            }
        }
    }
}
