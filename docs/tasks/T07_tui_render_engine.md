# T07: TUI Render Engine (`src/tui.rs` + `src/chips/ghost.rs`)

**Task ID:** T07
**Title:** Pure Stateless TUI Rendering + Ghost Piece Computation
**Depends On:** T01 (`src/bus.rs`)
**Produces:** `src/tui.rs`, `src/chips/ghost.rs`

---

## Paradigm Constraints (Recap)

| Constraint | Meaning for This Task |
|---|---|
| **TUI is Pure** | `render_game(f, &bus)` takes `&SystemBus` (IMMUTABLE reference). It derives the visual layout from the bus. It NEVER mutates game state. |
| **No State in TUI** | The render function does not cache anything. Every frame is a fresh derivation from the bus. No `self`, no stored `previous_frame_state`. |
| **Read-Only** | The render function reads bus registers (board, piece state, score, etc.). It also reads bus wires (e.g., ghost_x/y, render_dirty — though render_dirty is optional optimization). |
| **No Input Handling** | The render function does NOT poll keyboard. Input is handled in `main.rs` before the pipeline runs. Terminal draw happens between pipeline ticks. |
| **ratatui Integration** | Use `ratatui::Frame`, `Paragraph`, `Block`, `Layout`, `Span`, `Text`, `Line`. No `Canvas` widget (overkill for a grid). |

---

## Implementation Goal

### Part A: `src/chips/ghost.rs` — GhostComputerChip (Layer 3)

```
Duty: Compute the ghost piece Y position (hard drop preview) and write
      it to bus registers so the TUI can read them.

Reads:  bus.piece_type.0, bus.piece_x, bus.piece_y, bus.piece_rotation, bus.board

Writes: bus.ghost_x (=piece_x), bus.ghost_y (= computed ghost Y)

Algorithm:
  ghost_x = piece_x  (ghost is directly below the active piece)
  ghost_y = ghost_y(piece_x, piece_y, piece_type, rotation, board)
  (uses ghost_y() function from tetrominoes module)
```

Note: `ghost_x` is always the same as `piece_x` since ghost drops straight down. Writing it explicitly keeps the bus self-describing.

### Part B: `src/tui.rs` — Pure Render Function

#### B.1 Main Entry Point

```rust
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Style, Stylize, Color},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use crate::bus::{SystemBus, GamePhase, BOARD_COLS, BOARD_ROWS};

/// Pure function: derives the complete terminal UI from SystemBus state.
/// Called at ~60 FPS from the main loop.
/// NEVER mutates state. NEVER polls input.
pub fn render_game(frame: &mut Frame, bus: &SystemBus) {
    // Horizontal split: playfield | sidebar
    let chunks = Layout::horizontal([
        Constraint::Length(22),   // 10 cols × 2 chars + 2 borders
        Constraint::Min(20),      // sidebar
    ]).split(frame.area());

    render_playfield(frame, chunks[0], bus);

    // Sidebar: Hold | Next | Stats
    let side = Layout::vertical([
        Constraint::Length(7),    // Hold
        Constraint::Length(1),    // spacer
        Constraint::Length(7),    // Next
        Constraint::Length(1),    // spacer
        Constraint::Min(3),       // Stats
    ]).split(chunks[1]);

    render_mini(frame, side[0], " HOLD ", bus.hold_piece_type.map(|p| p.0));
    render_mini(frame, side[2], " NEXT ", Some(bus.next_piece_type.0));
    render_status(frame, side[4], bus);

    // Overlays (drawn last, on top)
    match bus.game_phase {
        GamePhase::Paused   => render_overlay(frame, frame.area(), "PAUSED", Color::Yellow),
        GamePhase::GameOver => render_overlay(frame, frame.area(), "GAME OVER", Color::Red),
        _ => {}
    }
}
```

#### B.2 Playfield Rendering

```rust
fn render_playfield(frame: &mut Frame, area: ratatui::layout::Rect, bus: &SystemBus) {
    let mut lines: Vec<Line> = Vec::with_capacity(22);

    // Top border
    lines.push(Line::from(Span::raw("┌────────────────────┐")));

    for y in 0..BOARD_ROWS {
        let mut spans: Vec<Span> = Vec::with_capacity(12);
        spans.push(Span::raw("│"));

        for x in 0..BOARD_COLS {
            let cell = bus.board[y][x].0;
            let active = is_cell_active(bus, x as i8, y as i8);
            let ghost  = !active && is_cell_ghost(bus, x as i8, y as i8);

            if cell != 0 {
                // Locked piece cell
                let color = piece_color(cell);
                spans.push(Span::styled("██", Style::default().fg(color).bg(dim(color))));
            } else if active {
                // Active piece cell
                let color = piece_color(bus.piece_type.0 + 1);
                spans.push(Span::styled("██", Style::default().fg(color).bg(dim(color))));
            } else if ghost {
                // Ghost piece cell (dimmed preview)
                let color = piece_color(bus.piece_type.0 + 1);
                spans.push(Span::styled("░░", Style::default().fg(color).reversed()));
            } else {
                // Empty cell
                spans.push(Span::raw("  "));
            }
        }

        spans.push(Span::raw("│"));
        lines.push(Line::from(spans));
    }

    // Bottom border
    lines.push(Line::from(Span::raw("└────────────────────┘")));

    let p = Paragraph::new(Text::from(lines))
        .block(Block::bordered().title(" TETRIS "));
    frame.render_widget(p, area);
}
```

#### B.3 Cell Hit Testing

```rust
/// Test if the active piece occupies cell (cx, cy) in playfield coordinates.
fn is_cell_active(bus: &SystemBus, cx: i8, cy: i8) -> bool {
    let cells = &crate::chips::tetrominoes::TETROMINOES
        [bus.piece_type.0 as usize][bus.piece_rotation as usize];
    cells.iter().any(|&(dx, dy)| {
        bus.piece_x + dx == cx && bus.piece_y + dy == cy
    })
}

/// Test if the ghost piece occupies cell (cx, cy) in playfield coordinates.
fn is_cell_ghost(bus: &SystemBus, cx: i8, cy: i8) -> bool {
    let cells = &crate::chips::tetrominoes::TETROMINOES
        [bus.piece_type.0 as usize][bus.piece_rotation as usize];
    cells.iter().any(|&(dx, dy)| {
        bus.ghost_x + dx == cx && bus.ghost_y + dy == cy
    })
}
```

#### B.4 Mini Grid (Hold / Next)

```rust
/// Render a 4×4 mini grid for hold/next piece preview.
fn render_mini(frame: &mut Frame, area: ratatui::layout::Rect, title: &str, piece_id: Option<u8>) {
    let block = Block::bordered().title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(pid) = piece_id {
        if pid < 7 {
            let cells = &crate::chips::tetrominoes::TETROMINOES[pid as usize][0];
            let color = piece_color(pid + 1);
            let mut lines = Vec::with_capacity(4);
            for row in 0..4 {
                let mut line_spans = Vec::new();
                for col in 0..4 {
                    let filled = cells.iter().any(|&(dx, dy)| dy == row && dx == col);
                    line_spans.push(Span::styled(
                        if filled { "██" } else { "  " },
                        Style::default().fg(if filled { color } else { Color::Reset }),
                    ));
                }
                lines.push(Line::from(line_spans));
            }
            frame.render_widget(Paragraph::new(Text::from(lines)), inner);
        }
    }
}
```

#### B.5 Status Panel

```rust
fn render_status(frame: &mut Frame, area: ratatui::layout::Rect, bus: &SystemBus) {
    let lines = vec![
        Line::from(vec![
            Span::raw(" Score: "),
            Span::styled(format!("{}", bus.score), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw(" Level: "),
            Span::styled(format!("{}", bus.level), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw(" Lines: "),
            Span::styled(format!("{}", bus.lines_cleared), Style::default().fg(Color::Green)),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(Block::bordered().title(" STATS ")),
        area,
    );
}
```

#### B.6 Overlay

```rust
fn render_overlay(frame: &mut Frame, area: ratatui::layout::Rect, msg: &str, color: Color) {
    let overlay = Block::default().style(Style::default().bg(Color::Black));
    frame.render_widget(overlay, area);

    let text = vec![
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}  ", msg),
            Style::default().fg(color).bold(),
        )),
        Line::from(""),
        Line::from(Span::raw("  Esc to quit  ")),
    ];
    let para = Paragraph::new(Text::from(text))
        .alignment(Alignment::Center)
        .block(Block::bordered().style(Style::default().bg(Color::Black)));
    frame.render_widget(para, area);
}
```

#### B.7 Color Mapping

```rust
/// Map piece type ID (1-7) to ANSI color.
pub fn piece_color(piece_id: u8) -> Color {
    match piece_id {
        1 => Color::Cyan,                      // I
        2 => Color::Blue,                      // J
        3 => Color::Rgb(255, 165, 0),          // L (Orange)
        4 => Color::Yellow,                    // O
        5 => Color::Green,                     // S
        6 => Color::Magenta,                   // T
        7 => Color::Red,                       // Z
        _ => Color::White,
    }
}

/// Dimmed background variant for filled cells.
fn dim(c: Color) -> Color {
    match c {
        Color::Cyan    => Color::Rgb(0, 128, 128),
        Color::Blue    => Color::Rgb(0, 0, 128),
        Color::Yellow  => Color::Rgb(128, 128, 0),
        Color::Green   => Color::Rgb(0, 128, 0),
        Color::Magenta => Color::Rgb(128, 0, 128),
        Color::Red     => Color::Rgb(128, 0, 0),
        Color::Rgb(r, g, b) => Color::Rgb(r / 2, g / 2, b / 2),
        _ => Color::DarkGray,
    }
}
```

### Part C: `src/chips/ghost.rs` — GhostComputerChip

```rust
use crate::bus::{InputPins, SystemBus};
use crate::chips::LogicChip;
use crate::chips::tetrominoes::ghost_y;

pub struct GhostComputerChip;

impl LogicChip for GhostComputerChip {
    fn tick(&self, _pins: &InputPins, bus: &mut SystemBus) {
        bus.ghost_x = bus.piece_x;
        bus.ghost_y = ghost_y(
            bus.piece_x,
            bus.piece_y,
            bus.piece_type.0,
            bus.piece_rotation,
            &bus.board,
        );
    }
}
```

---

## Verification Protocol (Guardrail Agent B)

1. **Pure render function:** Verify `render_game` takes `&SystemBus` (immutable). Verify it NEVER mutates bus fields. Grep for `bus.` in the render function — must only be reads, no assignments.
2. **No state caching:** Verify no `static`, `lazy_static`, or `thread_local` in `tui.rs`. Every frame is derived fresh from the bus.
3. **Playfield dimensions:** Verify the playfield renders EXACTLY 20 rows and 10 columns. Verify borders use box-drawing characters.
4. **Cell rendering:** Verify locked cells render as `██` with correct color + dimmed background. Verify active piece cells render the same. Verify ghost cells render as `░░` with reversed style. Verify empty cells render as `  ` (two spaces).
5. **Mini grids:** Verify hold/next render 4×4 grids with correct piece shape (rotation state 0). Verify empty hold slot renders empty space.
6. **Status panel:** Verify score, level, and lines are displayed with correct values from the bus.
7. **Overlays:** Verify GameOver and Paused overlays render with correct text. Verify black background covers the playfield.
8. **Ghost chip:** Verify GhostComputerChip computes ghost_y using the shared `ghost_y()` function from tetrominoes. Verify it writes both `ghost_x` and `ghost_y` to bus registers.
9. **Layer placement:** Verify GhostComputerChip is in Layer 3 in `SiliconMotherboard::new()`.
10. **Compile check:** `cargo check` must pass. The render function references `SystemBus` and `GamePhase` from `crate::bus`.
11. **No input handling:** Verify `tui.rs` does NOT import or use `crossterm::event`. No keyboard polling in the render path.
12. **No unsafe:** `grep -rn "unsafe" src/tui.rs src/chips/ghost.rs` returns nothing.

---

## Acceptance Criteria

- [ ] `src/chips/ghost.rs` — GhostComputerChip computes and writes ghost_x/ghost_y
- [ ] `src/tui.rs` — `render_game(frame, bus)` pure render function
- [ ] Playfield renders 10×20 grid with box-drawing borders
- [ ] Locked cells: `██` with piece color + dimmed background
- [ ] Active piece: `██` with piece color + dimmed background
- [ ] Ghost piece: `░░` with piece color + reversed style
- [ ] Hold/Next mini grids render correctly
- [ ] Score/Level/Lines status panel renders correctly
- [ ] Game Over and Pause overlays render correctly
- [ ] Render function NEVER mutates game state
- [ ] No input handling in render path
- [ ] GhostComputerChip in Layer 3 of Motherboard
- [ ] Zero state caching
- [ ] Zero `unsafe` blocks
