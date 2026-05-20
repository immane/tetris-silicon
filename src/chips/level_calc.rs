use crate::bus::{gravity_interval_ns, InputPins, SystemBus};
use super::LogicChip;
pub(crate) use super::LevelCalculatorChip;

impl LogicChip for LevelCalculatorChip {
    fn tick(&self, _pins: &InputPins, bus: &mut SystemBus) {
        let new_level = 1 + (bus.lines_cleared / 10) as u16;
        if new_level != bus.level {
            bus.level = new_level;
            bus.gravity_interval_ns = gravity_interval_ns(new_level as u8);
        }
    }
}
