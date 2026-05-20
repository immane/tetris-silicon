// ============================================================================
// tui.rs — Pure stateless TUI rendering from SystemBus
// ============================================================================

use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Paragraph},
    Frame,
};

use crate::bus::{GamePhase, SystemBus, BOARD_COLS, BOARD_ROWS};

/// Pure function: derives the complete terminal UI from SystemBus state.
/// Called at ~60 FPS from the main loop. NEVER mutates state. NEVER polls input.
pub fn render_game(frame: &mut Frame, bus: &SystemBus) {
    let chunks = Layout::horizontal([
        Constraint::Length(22),   // 10 cols × 2 chars + 2 borders
        Constraint::Min(20),      // sidebar
    ])
    .split(frame.size());

    render_playfield(frame, chunks[0], bus);

    let side = Layout::vertical([
        Constraint::Length(7),    // Hold
        Constraint::Length(1),    // spacer
        Constraint::Length(7),    // Next
        Constraint::Length(1),    // spacer
        Constraint::Min(3),       // Stats
    ])
    .split(chunks[1]);

    render_mini(frame, side[0], " HOLD ", bus.hold_piece_type.map(|p| p.0));
    render_mini(frame, side[2], " NEXT ", Some(bus.next_piece_type.0));
    render_status(frame, side[4], bus);

    match bus.game_phase {
        GamePhase::Paused => render_overlay(frame, frame.size(), "PAUSED", Color::Yellow),
        GamePhase::GameOver => render_overlay(frame, frame.size(), "GAME OVER", Color::Red),
        _ => {}
    }
}

// ─── Playfield ─────────────────────────────────────────────────────────────

fn render_playfield(frame: &mut Frame, area: ratatui::layout::Rect, bus: &SystemBus) {
    let mut lines: Vec<Line> = Vec::with_capacity(22);
    lines.push(Line::from(Span::raw("┌────────────────────┐")));

    for y in 0..BOARD_ROWS {
        let mut spans: Vec<Span> = Vec::with_capacity(12);
        spans.push(Span::raw("│"));

        for x in 0..BOARD_COLS {
            let cell = bus.board[y][x].0;
            let active = is_cell_active(bus, x as i8, y as i8);
            let ghost = !active && is_cell_ghost(bus, x as i8, y as i8);

            if cell != 0 {
                let color = piece_color(cell);
                spans.push(Span::styled("██", Style::default().fg(color).bg(dim(color))));
            } else if active {
                let color = piece_color(bus.piece_type.0 + 1);
                spans.push(Span::styled("██", Style::default().fg(color).bg(dim(color))));
            } else if ghost {
                let color = piece_color(bus.piece_type.0 + 1);
                spans.push(Span::styled("░░", Style::default().fg(color).reversed()));
            } else {
                spans.push(Span::raw("  "));
            }
        }

        spans.push(Span::raw("│"));
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(Span::raw("└────────────────────┘")));

    let p = Paragraph::new(Text::from(lines)).block(Block::bordered().title(" TETRIS "));
    frame.render_widget(p, area);
}

// ─── Cell Hit Testing ──────────────────────────────────────────────────────

fn is_cell_active(bus: &SystemBus, cx: i8, cy: i8) -> bool {
    let cells =
        &crate::chips::tetrominoes::TETROMINOES[bus.piece_type.0 as usize]
            [bus.piece_rotation as usize];
    cells
        .iter()
        .any(|&(dx, dy)| bus.piece_x + dx == cx && bus.piece_y + dy == cy)
}

fn is_cell_ghost(bus: &SystemBus, cx: i8, cy: i8) -> bool {
    let cells =
        &crate::chips::tetrominoes::TETROMINOES[bus.piece_type.0 as usize]
            [bus.piece_rotation as usize];
    cells
        .iter()
        .any(|&(dx, dy)| bus.ghost_x + dx == cx && bus.ghost_y + dy == cy)
}

// ─── Mini Grid (Hold / Next) ────────────────────────────────────────────────

fn render_mini(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    title: &str,
    piece_id: Option<u8>,
) {
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

// ─── Status Panel ───────────────────────────────────────────────────────────

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

// ─── Overlay ────────────────────────────────────────────────────────────────

fn render_overlay(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    msg: &str,
    color: Color,
) {
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

// ─── Color Mapping ──────────────────────────────────────────────────────────

fn piece_color(piece_id: u8) -> Color {
    match piece_id {
        1 => Color::Cyan,                     // I
        2 => Color::Blue,                     // J
        3 => Color::Rgb(255, 165, 0),         // L (Orange)
        4 => Color::Yellow,                   // O
        5 => Color::Green,                    // S
        6 => Color::Magenta,                  // T
        7 => Color::Red,                      // Z
        _ => Color::White,
    }
}

fn dim(c: Color) -> Color {
    match c {
        Color::Cyan => Color::Rgb(0, 128, 128),
        Color::Blue => Color::Rgb(0, 0, 128),
        Color::Yellow => Color::Rgb(128, 128, 0),
        Color::Green => Color::Rgb(0, 128, 0),
        Color::Magenta => Color::Rgb(128, 0, 128),
        Color::Red => Color::Rgb(128, 0, 0),
        Color::Rgb(r, g, b) => Color::Rgb(r / 2, g / 2, b / 2),
        _ => Color::DarkGray,
    }
}
