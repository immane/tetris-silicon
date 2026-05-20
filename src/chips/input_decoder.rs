use crate::bus::{InputPins, SystemBus};
use super::LogicChip;
pub(crate) use super::InputDecoderChip;
impl LogicChip for InputDecoderChip {
    fn tick(&self, _pins: &InputPins, _bus: &mut SystemBus) {}
}
