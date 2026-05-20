use crate::bus::{InputPins, SystemBus};
use super::LogicChip;
pub(crate) use super::GravityTimerChip;

impl LogicChip for GravityTimerChip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus) {
        bus.gravity_accumulator_ns = bus
            .gravity_accumulator_ns
            .saturating_add(pins.frame_delta_ns);

        while bus.gravity_accumulator_ns >= bus.gravity_interval_ns {
            bus.wires.gravity_tick = true;
            bus.wires.dy = 1;
            bus.gravity_accumulator_ns -= bus.gravity_interval_ns;
        }
    }
}
