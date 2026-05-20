// ============================================================================
// chips/tetrominoes.rs — Static tetromino data and pure collision functions
// ============================================================================

use crate::bus::{Cell, BOARD_COLS, BOARD_ROWS};

/// TETROMINOES[piece_type][rotation][block_index] -> (dx, dy)
/// piece_type: 0=I, 1=J, 2=L, 3=O, 4=S, 5=T, 6=Z
/// rotation: 0=spawn, 1=CW, 2=180, 3=CCW
/// dx, dy: offset from anchor (top-left of bounding box). y+ = DOWN.
pub const TETROMINOES: [[[(i8, i8); 4]; 4]; 7] = [
    // 0: I piece (4x4 box)
    [
        [(0, 1), (1, 1), (2, 1), (3, 1)],
        [(2, 0), (2, 1), (2, 2), (2, 3)],
        [(0, 2), (1, 2), (2, 2), (3, 2)],
        [(1, 0), (1, 1), (1, 2), (1, 3)],
    ],
    // 1: J piece (3x3 box)
    [
        [(0, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (2, 2)],
        [(1, 0), (1, 1), (0, 2), (1, 2)],
    ],
    // 2: L piece (3x3 box)
    [
        [(2, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (1, 2), (2, 2)],
        [(0, 1), (1, 1), (2, 1), (0, 2)],
        [(0, 0), (1, 0), (1, 1), (1, 2)],
    ],
    // 3: O piece (4x4 box, all rotations identical)
    [
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
    ],
    // 4: S piece (3x3 box)
    [
        [(1, 0), (2, 0), (0, 1), (1, 1)],
        [(1, 0), (1, 1), (2, 1), (2, 2)],
        [(1, 1), (2, 1), (0, 2), (1, 2)],
        [(0, 0), (0, 1), (1, 1), (1, 2)],
    ],
    // 5: T piece (3x3 box)
    [
        [(1, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (1, 2)],
        [(1, 0), (0, 1), (1, 1), (1, 2)],
    ],
    // 6: Z piece (3x3 box)
    [
        [(0, 0), (1, 0), (1, 1), (2, 1)],
        [(2, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (1, 2), (2, 2)],
        [(1, 0), (0, 1), (1, 1), (0, 2)],
    ],
];

/// JLSTZ_KICKS[from_rotation][to_rotation][test_index] -> (dx, dy)
/// Screen coords: y+ = DOWN. Unused transitions are all (0,0).
pub const JLSTZ_KICKS: [[[(i8, i8); 5]; 4]; 4] = [
    // from 0 (spawn)
    [
        [(0, 0); 5],
        [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],
        [(0, 0); 5],
        [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],
    ],
    // from R
    [
        [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
        [(0, 0); 5],
        [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
        [(0, 0); 5],
    ],
    // from 2
    [
        [(0, 0); 5],
        [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],
        [(0, 0); 5],
        [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],
    ],
    // from L
    [
        [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
        [(0, 0); 5],
        [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
        [(0, 0); 5],
    ],
];

/// I_KICKS[from_rotation][to_rotation][test_index] -> (dx, dy)
pub const I_KICKS: [[[(i8, i8); 5]; 4]; 4] = [
    // from 0
    [
        [(0, 0); 5],
        [(1, 0), (-1, 0), (2, 0), (-1, 1), (2, -2)],
        [(0, 0); 5],
        [(0, 1), (-1, 1), (2, 1), (-1, -1), (2, 2)],
    ],
    // from R
    [
        [(-1, 0), (1, 0), (-2, 0), (1, -1), (-2, 2)],
        [(0, 0); 5],
        [(0, 1), (-1, 1), (2, 1), (-1, -1), (2, 2)],
        [(0, 0); 5],
    ],
    // from 2
    [
        [(0, 0); 5],
        [(0, -1), (1, -1), (-2, -1), (1, 1), (-2, -2)],
        [(0, 0); 5],
        [(-1, 0), (1, 0), (-2, 0), (1, -1), (-2, 2)],
    ],
    // from L
    [
        [(0, -1), (1, -1), (-2, -1), (1, 1), (-2, -2)],
        [(0, 0); 5],
        [(1, 0), (-1, 0), (2, 0), (-1, 1), (2, -2)],
        [(0, 0); 5],
    ],
];

/// Returns true if the piece at (test_x, test_y) with given rotation
/// collides with walls, floor, or locked blocks.
pub fn collides(
    test_x: i8,
    test_y: i8,
    piece_type: u8,
    rotation: u8,
    board: &[[Cell; BOARD_COLS]; BOARD_ROWS],
) -> bool {
    let cells = &TETROMINOES[piece_type as usize][rotation as usize];
    for &(dx, dy) in cells.iter() {
        let x = test_x + dx;
        let y = test_y + dy;
        if x < 0 || x >= BOARD_COLS as i8 {
            return true;
        }
        if y < 0 || y >= BOARD_ROWS as i8 {
            return true;
        }
        if board[y as usize][x as usize].0 != 0 {
            return true;
        }
    }
    false
}

/// Compute the ghost piece Y (hard drop preview).
/// Raycasts downward from piece_y until collision.
pub fn ghost_y(
    piece_x: i8,
    piece_y: i8,
    piece_type: u8,
    rotation: u8,
    board: &[[Cell; BOARD_COLS]; BOARD_ROWS],
) -> i8 {
    let mut gy = piece_y;
    while !collides(piece_x, gy + 1, piece_type, rotation, board) {
        gy += 1;
    }
    gy
}
