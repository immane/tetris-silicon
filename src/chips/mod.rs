// ============================================================================
// chips/mod.rs — LogicChip trait, Chip enum dispatch, chip struct declarations
// ============================================================================

use crate::bus::{InputPins, SystemBus};

// ─── Module declarations for chip implementations ─────────────────────────
mod gravity_timer;
mod das_timer;
mod lock_delay_timer;
mod collision;
mod rotation;
mod movement;
mod piece_locker;
mod line_clear;
mod score_keeper;
mod level_calc;
mod hold_controller;
mod spawn_controller;
mod ghost;
mod input_decoder;
pub mod tetrominoes;

// ─── Unit Struct Declarations ─────────────────────────────────────────────
pub struct InputDecoderChip;
pub struct GravityTimerChip;
pub struct DasTimerChip;
pub struct LockDelayTimerChip;
pub struct CollisionDetectorChip;
pub struct RotationChip;
pub struct MovementChip;
pub struct PieceLockerChip;
pub struct LineClearDetectorChip;
pub struct LineClearCommitterChip;
pub struct ScoreKeeperChip;
pub struct LevelCalculatorChip;
pub struct HoldControllerChip;
pub struct SpawnControllerChip;
pub struct GhostComputerChip;

// ─── LogicChip Trait ──────────────────────────────────────────────────────
/// A stateless logic gate that performs one pure deduction per clock tick.
///
/// # Requirements
/// - Implemented by unit structs (zero fields).
/// - `tick()` reads `&InputPins` and reads/writes `&mut SystemBus`.
/// - `tick()` is deterministic: same inputs → same outputs.
/// - Chips NEVER call other chips. Data flow is through the bus.
///
/// # Borrow Checker Safety
/// `&self` (ZST) + `pins: &InputPins` + `bus: &mut SystemBus` are three
/// different allocations. Zero aliasing conflict.
pub trait LogicChip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus);
}

// ─── Chip Enum (Dispatch) ─────────────────────────────────────────────────
/// Closed-set enum wrapping all 15 chip types.
/// Enables zero-heap pipeline storage (`Vec<Chip>`).
pub enum Chip {
    // Layer 0: Input Decoder
    InputDecoder(InputDecoderChip),
    // Layer 1: Game Rules & Collision
    GravityTimer(GravityTimerChip),
    DasTimer(DasTimerChip),
    LockDelayTimer(LockDelayTimerChip),
    CollisionDetector(CollisionDetectorChip),
    Rotation(RotationChip),
    Movement(MovementChip),
    // Layer 2: State Mutation
    PieceLocker(PieceLockerChip),
    LineClearDetector(LineClearDetectorChip),
    LineClearCommitter(LineClearCommitterChip),
    ScoreKeeper(ScoreKeeperChip),
    LevelCalculator(LevelCalculatorChip),
    HoldController(HoldControllerChip),
    SpawnController(SpawnControllerChip),
    // Layer 3: UI Transformation
    GhostComputer(GhostComputerChip),
}

impl LogicChip for Chip {
    fn tick(&self, pins: &InputPins, bus: &mut SystemBus) {
        match self {
            Chip::InputDecoder(c)       => c.tick(pins, bus),
            Chip::GravityTimer(c)       => c.tick(pins, bus),
            Chip::DasTimer(c)           => c.tick(pins, bus),
            Chip::LockDelayTimer(c)     => c.tick(pins, bus),
            Chip::CollisionDetector(c)  => c.tick(pins, bus),
            Chip::Rotation(c)           => c.tick(pins, bus),
            Chip::Movement(c)           => c.tick(pins, bus),
            Chip::PieceLocker(c)        => c.tick(pins, bus),
            Chip::LineClearDetector(c)  => c.tick(pins, bus),
            Chip::LineClearCommitter(c) => c.tick(pins, bus),
            Chip::ScoreKeeper(c)        => c.tick(pins, bus),
            Chip::LevelCalculator(c)    => c.tick(pins, bus),
            Chip::HoldController(c)     => c.tick(pins, bus),
            Chip::SpawnController(c)    => c.tick(pins, bus),
            Chip::GhostComputer(c)      => c.tick(pins, bus),
        }
    }
}
