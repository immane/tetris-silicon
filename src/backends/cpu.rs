use crate::bus::{InputPins, SystemBus};
use crate::chips::{Chip, LogicChip};

pub(super) fn execute_layers_cpu(layers: &[Vec<Chip>], pins: &InputPins, bus: &mut SystemBus) {
    for layer in layers {
        for chip in layer {
            chip.tick(pins, bus);
        }
    }
}
