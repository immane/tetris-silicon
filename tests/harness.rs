// ============================================================================
// tests/harness.rs — TestBench and proptest strategies for the Silicon paradigm
// ============================================================================

use proptest::prelude::*;
use tetris_silicon::bus::{
    Cell, GamePhase, InputPins, PieceType, SystemBus, Wires, BOARD_COLS, BOARD_ROWS,
};
use tetris_silicon::chips::tetrominoes::TETROMINOES;
use tetris_silicon::motherboard::SiliconMotherboard;

/// A snapshot of SystemBus state for test assertions.
#[derive(Debug, Clone)]
pub struct TestBench {
    pub bus_before: SystemBus,
    pub bus_after: SystemBus,
}

impl TestBench {
    /// Run one clock tick with the given input pins, capturing before/after state.
    pub fn tick(pins: &InputPins, bus: &mut SystemBus) -> Self {
        let bus_before = bus.clone();
        let mut motherboard = SiliconMotherboard::new();
        motherboard.clock_tick(pins, bus);
        Self {
            bus_before,
            bus_after: bus.clone(),
        }
    }

    /// Run the motherboard against empty (no-input) pins for one tick.
    #[allow(dead_code)]
    pub fn tick_idle(bus: &mut SystemBus) -> Self {
        let pins = InputPins::default();
        Self::tick(&pins, bus)
    }
}

// ─── Cell Strategies ────────────────────────────────────────────────────────

pub fn arb_cell() -> impl Strategy<Value = Cell> {
    (0u8..=7).prop_map(Cell)
}

pub fn arb_locked_cell() -> impl Strategy<Value = Cell> {
    (1u8..=7).prop_map(Cell)
}

pub fn arb_empty_cell() -> impl Strategy<Value = Cell> {
    Just(Cell(0))
}

// ─── Board Strategies ───────────────────────────────────────────────────────

pub fn arb_board() -> impl Strategy<Value = [[Cell; BOARD_COLS]; BOARD_ROWS]> {
    let cell_strat = prop_oneof![3 => arb_empty_cell(), 1 => arb_locked_cell()];

    proptest::collection::vec(cell_strat, BOARD_ROWS * BOARD_COLS).prop_map(|cells| {
        let mut board = [[Cell(0); BOARD_COLS]; BOARD_ROWS];
        for (i, cell) in cells.into_iter().enumerate() {
            board[i / BOARD_COLS][i % BOARD_COLS] = cell;
        }
        board
    })
}

pub fn arb_sparse_board() -> impl Strategy<Value = [[Cell; BOARD_COLS]; BOARD_ROWS]> {
    let cell_strat = prop_oneof![9 => arb_empty_cell(), 1 => arb_locked_cell()];

    proptest::collection::vec(cell_strat, BOARD_ROWS * BOARD_COLS).prop_map(|cells| {
        let mut board = [[Cell(0); BOARD_COLS]; BOARD_ROWS];
        for (i, cell) in cells.into_iter().enumerate() {
            board[i / BOARD_COLS][i % BOARD_COLS] = cell;
        }
        board
    })
}

// ─── Piece Strategies ───────────────────────────────────────────────────────

pub fn arb_piece_type() -> impl Strategy<Value = PieceType> {
    (0u8..=6).prop_map(PieceType)
}

pub fn arb_piece_position() -> impl Strategy<Value = (i8, i8)> {
    (-3i8..10, -3i8..20)
}

pub fn arb_rotation() -> impl Strategy<Value = u8> {
    (0u8..=3u8)
}

// ─── SystemBus Strategy (using prop_compose! for large struct) ──────────────

prop_compose! {
    /// Generate a fully valid (mostly empty) SystemBus state for property testing.
    pub fn arb_system_bus()
        (board in arb_sparse_board())
        (piece_type in arb_piece_type(),
         pos in arb_piece_position(),
         rotation in arb_rotation(),
         next_type in arb_piece_type(),
         hold_type in proptest::option::of(arb_piece_type()),
         hold_used in proptest::bool::ANY,
         score in 0u32..1_000_000,
         level in 1u16..20,
         lines in 0u32..500,
         das_active in proptest::bool::ANY,
         das_dir in -1i8..2i8,
         grav_acc in 0u64..1_000_000_000,
         board in Just(board))
        -> SystemBus
    {
        let (px, py) = pos;
        SystemBus {
            board,
            piece_type,
            piece_x: px,
            piece_y: py,
            piece_rotation: rotation,
            next_piece_type: next_type,
            hold_piece_type: hold_type,
            hold_used,
            score,
            level,
            lines_cleared: lines,
            game_phase: GamePhase::Playing,
            tick_count: 0,
            ghost_x: px,
            ghost_y: py,
            gravity_accumulator_ns: grav_acc,
            gravity_interval_ns: tetris_silicon::bus::gravity_interval_ns(level as u8),
            das_accumulator_ns: 0,
            das_delay_ns: tetris_silicon::bus::DAS_DELAY_NS,
            das_repeat_ns: tetris_silicon::bus::DAS_REPEAT_NS,
            das_active,
            das_direction: das_dir,
            das_last_repeat_index: 0,
            prng_state: 0xDEADBEEF,
            lock_delay_accumulator_ns: 0,
            lock_delay_max_ns: tetris_silicon::bus::LOCK_DELAY_MAX_NS,
            prev_key_left: false,
            prev_key_right: false,
            prev_key_down: false,
            prev_key_up: false,
            prev_key_z: false,
            prev_key_x: false,
            prev_key_c: false,
            prev_key_space: false,
            prev_key_escape: false,
            prev_key_enter: false,
            wires: Wires::default(),
        }
    }
}

// ─── InputPins Strategy ─────────────────────────────────────────────────────

pub fn arb_input_pins(delta_ns: u64) -> impl Strategy<Value = InputPins> {
    proptest::collection::vec(proptest::bool::ANY, 10).prop_map(move |v| InputPins {
        frame_delta_ns: delta_ns,
        key_left: v[0],
        key_right: v[1],
        key_down: v[2],
        key_up: v[3],
        key_z: v[4],
        key_x: v[5],
        key_space: v[6],
        key_c: v[7],
        key_escape: v[8],
        key_enter: v[9],
    })
}

// ─── Board Analysis Helpers ─────────────────────────────────────────────────

/// Count the number of occupied cells on the board.
pub fn count_occupied(board: &[[Cell; BOARD_COLS]; BOARD_ROWS]) -> u32 {
    let mut count = 0u32;
    for row in board.iter() {
        for cell in row.iter() {
            if cell.0 != 0 {
                count += 1;
            }
        }
    }
    count
}

/// Count how many cells the active piece occupies within board bounds.
pub fn count_active_cells(bus: &SystemBus) -> u32 {
    let pt = bus.piece_type.0 as usize;
    let pr = bus.piece_rotation as usize;
    let cells = &TETROMINOES[pt][pr];
    let mut count = 0u32;
    for &(dx, dy) in cells.iter() {
        let x = bus.piece_x + dx;
        let y = bus.piece_y + dy;
        if x >= 0 && x < BOARD_COLS as i8 && y >= 0 && y < BOARD_ROWS as i8 {
            count += 1;
        }
    }
    count
}

// ─── Determinism Helper ─────────────────────────────────────────────────────

/// Run the motherboard twice with identical inputs, asserting bitwise equality.
pub fn assert_deterministic(bus_before: &SystemBus, pins: &InputPins) {
    let mut bus1 = bus_before.clone();
    let mut bus2 = bus_before.clone();

    let mut mb1 = SiliconMotherboard::new();
    let mut mb2 = SiliconMotherboard::new();

    mb1.clock_tick(pins, &mut bus1);
    mb2.clock_tick(pins, &mut bus2);

    assert_eq!(bus1.board, bus2.board, "Board diverged");
    assert_eq!(bus1.piece_type, bus2.piece_type, "piece_type diverged");
    assert_eq!(bus1.piece_x, bus2.piece_x, "piece_x diverged");
    assert_eq!(bus1.piece_y, bus2.piece_y, "piece_y diverged");
    assert_eq!(
        bus1.piece_rotation, bus2.piece_rotation,
        "rotation diverged"
    );
    assert_eq!(bus1.score, bus2.score, "score diverged");
    assert_eq!(bus1.level, bus2.level, "level diverged");
    assert_eq!(
        bus1.lines_cleared, bus2.lines_cleared,
        "lines diverged"
    );
    assert_eq!(bus1.game_phase, bus2.game_phase, "game_phase diverged");
    assert_eq!(
        bus1.gravity_accumulator_ns, bus2.gravity_accumulator_ns,
        "gravity_accumulator_ns diverged"
    );
    assert_eq!(
        bus1.wires.piece_locked, bus2.wires.piece_locked,
        "piece_locked diverged"
    );
    assert_eq!(
        bus1.wires.should_spawn_next, bus2.wires.should_spawn_next,
        "should_spawn_next diverged"
    );
    assert_eq!(
        bus1.wires.game_over_triggered, bus2.wires.game_over_triggered,
        "game_over_triggered diverged"
    );
}

// ─── Constraint Invariant Helper ────────────────────────────────────────────

/// Verify the piece position is within maximum permitted bounds.
pub fn assert_piece_in_bounds(bus: &SystemBus) {
    assert!(
        bus.piece_x >= -3 && bus.piece_x <= 10,
        "piece_x {} out of bounds [-3, 10]",
        bus.piece_x
    );
    assert!(
        bus.piece_y >= -4 && bus.piece_y <= 20,
        "piece_y {} out of bounds [-4, 20]",
        bus.piece_y
    );
}
