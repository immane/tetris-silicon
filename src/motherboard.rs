// ============================================================================
// motherboard.rs — SiliconMotherboard: pipeline array + clock tick driver
// ============================================================================

use crate::bus::{GamePhase, InputPins, SystemBus, Wires};
use crate::chips::{
    Chip, CollisionDetectorChip, DasTimerChip, GhostComputerChip, GravityTimerChip,
    HoldControllerChip, InputDecoderChip, LevelCalculatorChip, LineClearCommitterChip,
    LineClearDetectorChip, LockDelayTimerChip, LogicChip, MovementChip, PieceLockerChip,
    RotationChip, ScoreKeeperChip, SpawnControllerChip,
};

/// The SiliconMotherboard physically arranges LogicChips in a 4-layer
/// pipeline and provides the clock tick driver.
///
/// # Layers
/// 0: Input Decoder
/// 1: Game Rules & Collision
/// 2: State Mutation
/// 3: UI Transformation
pub struct SiliconMotherboard {
    /// `layers[i]` holds all chips at pipeline stage i.
    /// Chips within a layer execute sequentially.
    /// Layers execute in order: 0 → 1 → 2 → 3.
    pub layers: Vec<Vec<Chip>>,
}

impl SiliconMotherboard {
    /// Create a motherboard with pre-populated layers.
    /// Chip ordering within each layer is critical — see comments.
    pub fn new() -> Self {
        Self {
            layers: vec![
                // Layer 0: Input Decoder
                vec![Chip::InputDecoder(InputDecoderChip)],
                // Layer 1: Game Rules & Collision
                // ORDER: Timers → Collision → Rotation → Movement → LockDelay
                vec![
                    Chip::GravityTimer(GravityTimerChip),
                    Chip::DasTimer(DasTimerChip),
                    Chip::CollisionDetector(CollisionDetectorChip),
                    Chip::Rotation(RotationChip),
                    Chip::Movement(MovementChip),
                    Chip::LockDelayTimer(LockDelayTimerChip),
                ],
                // Layer 2: State Mutation
                // ORDER: Lock → Hold → Clear → Score → Level → Spawn
                vec![
                    Chip::PieceLocker(PieceLockerChip),
                    Chip::HoldController(HoldControllerChip),
                    Chip::LineClearDetector(LineClearDetectorChip),
                    Chip::LineClearCommitter(LineClearCommitterChip),
                    Chip::ScoreKeeper(ScoreKeeperChip),
                    Chip::LevelCalculator(LevelCalculatorChip),
                    Chip::SpawnController(SpawnControllerChip),
                ],
                // Layer 3: UI Transformation
                vec![Chip::GhostComputer(GhostComputerChip)],
            ],
        }
    }

    /// Execute one full clock cycle.
    ///
    /// # Phases
    /// 0. WIRE RESET — Pull all wires to ground (default).
    /// 1. COMBINATIONAL PROPAGATION — Signals flow through the 4-layer pipeline.
    ///    Each chip reads InputPins and writes to SystemBus wires/registers.
    /// 2. SEQUENTIAL LATCHING — Commit edge-detection state and phase transitions
    ///    for the next tick. Increment Lamport clock.
    pub fn clock_tick(&mut self, pins: &InputPins, bus: &mut SystemBus) {
        // ═══ PHASE 0: Wire Reset ═══
        bus.wires = Wires::default();

        // ═══ PHASE 1: Combinational Propagation ═══
        for layer in &self.layers {
            for chip in layer {
                chip.tick(pins, bus);
            }
        }

        // ═══ PHASE 2: Sequential Latching (falling edge) ═══
        bus.prev_key_left = pins.key_left;
        bus.prev_key_right = pins.key_right;
        bus.prev_key_down = pins.key_down;
        bus.prev_key_up = pins.key_up;
        bus.prev_key_z = pins.key_z;
        bus.prev_key_x = pins.key_x;
        bus.prev_key_c = pins.key_c;
        bus.prev_key_space = pins.key_space;
        bus.prev_key_escape = pins.key_escape;
        bus.prev_key_enter = pins.key_enter;

        if bus.wires.game_over_triggered {
            bus.game_phase = GamePhase::GameOver;
        }

        bus.tick_count = bus.tick_count.wrapping_add(1);
    }

    /// Install a chip into a specific layer.
    pub fn install_chip(&mut self, layer: usize, chip: Chip) {
        if layer < self.layers.len() {
            self.layers[layer].push(chip);
        }
    }

    /// Get the total number of chips across all layers.
    pub fn chip_count(&self) -> usize {
        self.layers.iter().map(|l| l.len()).sum()
    }
}
