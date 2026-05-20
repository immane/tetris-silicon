use crate::bus::{InputPins, SystemBus};
use super::LogicChip;
pub(crate) use super::LockDelayTimerChip;

impl LogicChip for LockDelayTimerChip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus) {
        let cannot_fall = bus.wires.collision_down && bus.wires.dy == 0;
        let moved = bus.wires.dx != 0
            || bus.wires.dy != 0
            || bus.wires.rotate_cw
            || bus.wires.rotate_ccw;

        if moved && !cannot_fall {
            bus.lock_delay_accumulator_ns = 0;
            bus.wires.lock_delay_active = false;
            bus.wires.lock_delay_expired = false;
        } else if cannot_fall && !bus.wires.piece_locked {
            bus.wires.lock_delay_active = true;
            bus.lock_delay_accumulator_ns = bus
                .lock_delay_accumulator_ns
                .saturating_add(pins.frame_delta_ns);
            if bus.lock_delay_accumulator_ns >= bus.lock_delay_max_ns {
                bus.wires.lock_delay_expired = true;
            }
        } else {
            bus.lock_delay_accumulator_ns = 0;
            bus.wires.lock_delay_active = false;
            bus.wires.lock_delay_expired = false;
        }
    }
}
