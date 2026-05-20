// ============================================================================
// terminal.rs — Raw mode guard + non-blocking input polling
// ============================================================================

use crossterm::event::{self, Event, KeyCode, KeyEvent, MouseEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io;
use std::panic;
use std::time::Duration;

use crate::bus::InputPins;

#[derive(Clone, Copy, Debug, Default)]
pub struct SampledInput {
    pub pins: InputPins,
    /// Positive means scroll down the chip routing panel; negative means up.
    pub chip_routing_scroll_delta: i16,
}

// ─── RawModeGuard ──────────────────────────────────────────────────────────

/// Enters raw mode on creation, restores terminal on drop AND on panic.
pub struct RawModeGuard;

impl RawModeGuard {
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let prev_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = crossterm::execute!(
                io::stdout(),
                crossterm::terminal::LeaveAlternateScreen,
                crossterm::cursor::Show,
            );
            prev_hook(info);
        }));
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show,
        );
    }
}

// ─── Non-Blocking Input Polling ────────────────────────────────────────────

/// Poll input events in a strictly non-blocking way.
/// Drains ALL queued events. Returns frozen gameplay pins plus UI deltas.
///
/// This is the SAMPLING PHASE of the clock cycle.
pub fn poll_input_pins(frame_delta_ns: u64) -> SampledInput {
    let Ok(true) = event::poll(Duration::ZERO) else {
        return SampledInput {
            pins: InputPins {
                frame_delta_ns,
                ..InputPins::default()
            },
            chip_routing_scroll_delta: 0,
        };
    };

    let mut pins = InputPins {
        frame_delta_ns,
        ..InputPins::default()
    };
    let mut chip_routing_scroll_delta: i16 = 0;

    while let Ok(event) = event::read() {
        match event {
            Event::Key(KeyEvent { code, .. }) => match code {
                KeyCode::Up | KeyCode::Char('k') => pins.key_up = true,
                KeyCode::Down | KeyCode::Char('j') => pins.key_down = true,
                KeyCode::Left | KeyCode::Char('h') => pins.key_left = true,
                KeyCode::Right | KeyCode::Char('l') => pins.key_right = true,
                KeyCode::Char(' ') => pins.key_space = true,
                KeyCode::Esc => pins.key_escape = true,
                KeyCode::Enter => pins.key_enter = true,
                KeyCode::Char('z') | KeyCode::Char('Z') => pins.key_z = true,
                KeyCode::Char('x') | KeyCode::Char('X') => pins.key_x = true,
                KeyCode::Char('c') | KeyCode::Char('C') => pins.key_c = true,
                _ => {}
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => chip_routing_scroll_delta -= 3,
                MouseEventKind::ScrollDown => chip_routing_scroll_delta += 3,
                _ => {}
            },
            _ => {}
        }
        if !event::poll(Duration::ZERO).unwrap_or(false) {
            break;
        }
    }
    SampledInput {
        pins,
        chip_routing_scroll_delta,
    }
}
