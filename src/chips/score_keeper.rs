use crate::bus::{InputPins, SystemBus};
use super::LogicChip;
pub(crate) use super::ScoreKeeperChip;

impl LogicChip for ScoreKeeperChip {
    fn tick(&self, _pins: &InputPins, bus: &mut SystemBus) {
        let lines = bus.wires.lines_cleared_this_tick;
        if lines == 0 {
            return;
        }

        const BASES: [u32; 5] = [0, 40, 100, 300, 1200];
        let idx = (lines as usize).min(4);
        let added = BASES[idx] * (bus.level as u32 + 1);
        bus.score += added;
        bus.lines_cleared += lines as u32;
    }
}
