// ============================================================================
// bus.rs — PCB Traces: InputPins, SystemBus (Registers + Wires)
// ============================================================================

// ─── Dimensions ───────────────────────────────────────────────────────────
pub const BOARD_COLS: usize = 10;
pub const BOARD_ROWS: usize = 20;
pub const BOARD_SIZE: usize = BOARD_COLS * BOARD_ROWS; // 200

// ─── Timing Constants ─────────────────────────────────────────────────────
pub const FRAME_NS: u64 = 16_666_667;            // ~60 Hz
pub const DAS_DELAY_NS: u64 = 266_666_672;        // ~267ms (16 frames)
pub const DAS_REPEAT_NS: u64 = 100_000_002;       // ~100ms (6 frames)
pub const LOCK_DELAY_MAX_NS: u64 = 500_000_010;   // ~500ms (30 frames)
pub const MAX_FRAME_DELTA_NS: u64 = 1_000_000_000; // 1s cap for tab-away

/// NES-style gravity table: frames per gridcell at 60 Hz.
/// Index = level - 1. Beyond index 19, uses last entry (1 frame).
pub const GRAVITY_FRAMES: [u64; 20] = [
    48, 43, 38, 33, 28, 23, 18, 13, 8, 6,   // levels 1-10
    5,  5,  5,  4,  4,  4,  3,  3,  3, 2,    // levels 11-20
];

/// Convert a 1-based level number to the gravity interval in nanoseconds.
pub fn gravity_interval_ns(level: u8) -> u64 {
    let idx = level.saturating_sub(1) as usize;
    let frames = GRAVITY_FRAMES.get(idx).copied().unwrap_or(1);
    frames * FRAME_NS
}

// ─── Cell Value Encoding ──────────────────────────────────────────────────
/// 0=empty, 1=I, 2=J, 3=L, 4=O, 5=S, 6=T, 7=Z
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell(pub u8);

// ─── Piece Type ───────────────────────────────────────────────────────────
/// 0=I, 1=J, 2=L, 3=O, 4=S, 5=T, 6=Z
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PieceType(pub u8);

// ─── Game Phase ───────────────────────────────────────────────────────────
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamePhase {
    Playing,
    Paused,
    GameOver,
}

// ─── External Input Pins ──────────────────────────────────────────────────
/// Sampled once per clock tick. Frozen immutable for the tick duration.
/// Chips treat these pins as absolute physical truth.
#[derive(Clone, Copy, Debug)]
pub struct InputPins {
    /// Wall-clock nanoseconds elapsed since previous tick.
    pub frame_delta_ns: u64,

    pub key_left:   bool,
    pub key_right:  bool,
    pub key_down:   bool,    // soft drop
    pub key_up:     bool,    // rotate CW (alternate mapping)
    pub key_z:      bool,    // rotate CCW
    pub key_x:      bool,    // rotate CW
    pub key_space:  bool,    // hard drop
    pub key_c:      bool,    // hold
    pub key_escape: bool,
    pub key_enter:  bool,
}

impl Default for InputPins {
    fn default() -> Self {
        Self {
            frame_delta_ns: 0,
            key_left:   false,
            key_right:  false,
            key_down:   false,
            key_up:     false,
            key_z:      false,
            key_x:      false,
            key_space:  false,
            key_c:      false,
            key_escape: false,
            key_enter:  false,
        }
    }
}

// ─── Wires (Ephemeral Per-Tick Signals) ───────────────────────────────────
/// Temporary signal lines. Valid only within a single clock tick.
/// Reset to Default at the start of every tick.
#[derive(Clone, Debug)]
pub struct Wires {
    // Movement requests
    pub dx: i8,                           // -1 left, 0 none, +1 right
    pub dy: i8,                           // 0 none, +1 soft drop
    pub rotate_cw: bool,
    pub rotate_ccw: bool,
    pub hard_drop_requested: bool,
    pub hold_requested: bool,
    pub pause_requested: bool,

    // Timer triggers
    pub gravity_tick: bool,
    pub das_tick: bool,
    pub lock_delay_expired: bool,
    pub lock_delay_active: bool,

    // Collision signals
    pub collision_down: bool,
    pub collision_any: bool,
    pub wall_kick_applied: bool,

    // Lock / spawn signals
    pub piece_locked: bool,
    pub should_spawn_next: bool,

    // Line clear signals
    pub full_row_mask: u32,
    pub lines_cleared_this_tick: u8,

    // Game state events
    pub game_over_triggered: bool,

    // Render optimization
    pub render_dirty: bool,
}

impl Default for Wires {
    fn default() -> Self {
        Self {
            dx: 0,
            dy: 0,
            rotate_cw: false,
            rotate_ccw: false,
            hard_drop_requested: false,
            hold_requested: false,
            pause_requested: false,
            gravity_tick: false,
            das_tick: false,
            lock_delay_expired: false,
            lock_delay_active: false,
            collision_down: false,
            collision_any: false,
            wall_kick_applied: false,
            piece_locked: false,
            should_spawn_next: false,
            full_row_mask: 0,
            lines_cleared_this_tick: 0,
            game_over_triggered: false,
            render_dirty: false,
        }
    }
}

// ─── System Bus (Register File) ───────────────────────────────────────────
/// The single source of truth. Contains all Registers (persist across ticks)
/// and an embedded Wires block (reset every tick).
#[derive(Clone, Debug)]
pub struct SystemBus {
    // ═══ REGISTERS (persist across ticks) ═══

    // Board
    pub board: [[Cell; BOARD_COLS]; BOARD_ROWS],

    // Active piece
    pub piece_type: PieceType,
    pub piece_x: i8,
    pub piece_y: i8,
    pub piece_rotation: u8,          // 0=spawn, 1=CW, 2=180, 3=CCW

    // Piece queue
    pub next_piece_type: PieceType,
    pub hold_piece_type: Option<PieceType>,
    pub hold_used: bool,

    // Scoring
    pub score: u32,
    pub level: u16,
    pub lines_cleared: u32,

    // Game lifecycle
    pub game_phase: GamePhase,
    pub tick_count: u64,

    // Ghost piece (computed each tick, read by TUI)
    pub ghost_x: i8,
    pub ghost_y: i8,

    // Gravity timer
    pub gravity_accumulator_ns: u64,
    pub gravity_interval_ns: u64,

    // DAS (Delayed Auto Shift) timer
    pub das_accumulator_ns: u64,
    pub das_delay_ns: u64,
    pub das_repeat_ns: u64,
    pub das_active: bool,
    pub das_direction: i8,
    pub das_last_repeat_index: u32,

    // PRNG state (deterministic piece generation)
    pub prng_state: u32,

    // Lock delay timer
    pub lock_delay_accumulator_ns: u64,
    pub lock_delay_max_ns: u64,

    // Previous key state (edge detection latches)
    pub prev_key_left:   bool,
    pub prev_key_right:  bool,
    pub prev_key_down:   bool,
    pub prev_key_up:     bool,
    pub prev_key_z:      bool,
    pub prev_key_x:      bool,
    pub prev_key_c:      bool,
    pub prev_key_space:  bool,
    pub prev_key_escape: bool,
    pub prev_key_enter:  bool,

    // ═══ WIRES (reset every tick) ═══
    pub wires: Wires,
}

impl SystemBus {
    pub fn new(level: u16) -> Self {
        Self {
            board: [[Cell(0); BOARD_COLS]; BOARD_ROWS],
            piece_type: PieceType(0),
            piece_x: 3,
            piece_y: 0,
            piece_rotation: 0,
            next_piece_type: PieceType(1),
            hold_piece_type: None,
            hold_used: false,
            score: 0,
            level,
            lines_cleared: 0,
            game_phase: GamePhase::Playing,
            tick_count: 0,
            ghost_x: 0,
            ghost_y: 0,
            gravity_accumulator_ns: 0,
            gravity_interval_ns: gravity_interval_ns(level as u8),
            das_accumulator_ns: 0,
            das_delay_ns: DAS_DELAY_NS,
            das_repeat_ns: DAS_REPEAT_NS,
            das_active: false,
            das_direction: 0,
            das_last_repeat_index: 0,
            prng_state: 0xDEADBEEF,
            lock_delay_accumulator_ns: 0,
            lock_delay_max_ns: LOCK_DELAY_MAX_NS,
            prev_key_left:   false,
            prev_key_right:  false,
            prev_key_down:   false,
            prev_key_up:     false,
            prev_key_z:      false,
            prev_key_x:      false,
            prev_key_c:      false,
            prev_key_space:  false,
            prev_key_escape: false,
            prev_key_enter:  false,
            wires: Wires::default(),
        }
    }
}
