# T04: Input Driver & Main Loop (`src/main.rs` + `src/terminal.rs`)

**Task ID:** T04
**Title:** Terminal I/O Driver, Raw Mode, Main Loop
**Depends On:** T01 (`src/bus.rs`)
**Produces:** `src/main.rs`, `src/terminal.rs`

---

## Paradigm Constraints (Recap)

| Constraint | Meaning for This Task |
|---|---|
| **No Async** | `poll(Duration::ZERO)` is non-blocking. No `async fn`, no `tokio`, no event loop. |
| **Single-Threaded** | Main loop runs on one thread. No `std::thread::spawn()`. |
| **Sampling Phase Isolation** | Input polling happens at the START of each loop iteration, BEFORE the pipeline runs. InputPins is frozen for the tick. |
| **No Callbacks** | No key handler registration. No event callbacks. Just a function call. |
| **Panic Safety** | Terminal MUST be restored even on panic. Use `RawModeGuard` with panic hook. |
| **Rendering is Pure** | `render_game(f, &bus)` reads `&SystemBus` immutably. Never mutates game state. |

---

## Implementation Goal

Create two files: `src/terminal.rs` and `src/main.rs`.

### Part A: `src/terminal.rs` — Terminal I/O

#### A.1 RawModeGuard

```rust
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::{self, Write};
use std::panic;

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
```

#### A.2 poll_input_pins (Pure Function)

```rust
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::time::Duration;
use crate::bus::InputPins;

/// Poll keyboard in a strictly non-blocking way.
/// Drains ALL queued events. Returns a frozen InputPins snapshot.
///
/// This is the SAMPLING PHASE of the clock cycle.
pub fn poll_input_pins(frame_delta_ns: u64) -> InputPins {
    let Ok(true) = event::poll(Duration::ZERO) else {
        return InputPins { frame_delta_ns, ..InputPins::default() };
    };

    let mut pins = InputPins { frame_delta_ns, ..InputPins::default() };

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
            _ => {}
        }
        if !event::poll(Duration::ZERO).unwrap_or(false) {
            break;
        }
    }

    pins
}
```

### Part B: `src/main.rs` — Main Loop

```rust
use std::io;
use std::time::{Duration, Instant};

use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::cursor;

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

mod bus;
mod chips;
mod motherboard;
mod terminal;
mod tui;

use bus::{SystemBus, GamePhase, FRAME_NS, MAX_FRAME_DELTA_NS};
use motherboard::SiliconMotherboard;
use terminal::{RawModeGuard, poll_input_pins};

fn main() -> io::Result<()> {
    // ── Terminal Init ─────────────────────────────────────────
    let _guard = RawModeGuard::enter()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // ── Silicon Init ─────────────────────────────────────────
    let mut bus = SystemBus::new(1);  // start at level 1
    let mut motherboard = SiliconMotherboard::new();

    let mut last_tick = Instant::now();
    let mut last_render = Instant::now();

    // ── Main Loop ────────────────────────────────────────────
    loop {
        let now = Instant::now();

        // ═══ SAMPLING PHASE ═══
        let frame_delta_ns = now.duration_since(last_tick).as_nanos() as u64;
        last_tick = now;
        let delta = frame_delta_ns.min(MAX_FRAME_DELTA_NS);
        let pins = poll_input_pins(delta);

        // ═══ PROPAGATION + LATCHING ═══
        motherboard.clock_tick(&pins, &mut bus);

        // ═══ RENDER (throttled to ~60 FPS) ═══
        if now.duration_since(last_render).as_nanos() as u64 >= FRAME_NS {
            terminal.draw(|f| tui::render_game(f, &bus))?;
            last_render = now;
        }

        // ═══ YIELD (prevent busy-wait) ═══
        let work_ns = now.elapsed().as_nanos() as u64;
        if work_ns < FRAME_NS / 2 {
            std::thread::sleep(Duration::from_nanos(500_000)); // 0.5ms yield
        }

        // ═══ EXIT CHECK ═══
        if pins.key_escape || bus.game_phase == GamePhase::GameOver {
            // Show final frame
            terminal.draw(|f| tui::render_game(f, &bus))?;
            std::thread::sleep(Duration::from_secs(2));
            break;
        }
    }

    // ── Cleanup ──────────────────────────────────────────────
    execute!(io::stdout(), LeaveAlternateScreen, cursor::Show)?;
    Ok(())
}
```

---

## Verification Protocol (Guardrail Agent B)

1. **No async:** `grep -rn "async\|\.await\|tokio" src/main.rs src/terminal.rs` returns nothing.
2. **No threading:** `grep -rn "thread::spawn\|JoinHandle\|std::thread" src/main.rs src/terminal.rs` — only `std::thread::sleep` is allowed (yielding).
3. **Non-blocking I/O:** Verify `event::poll(Duration::ZERO)` is used. No `Duration::from_millis(n)` with n > 0 in polling.
4. **Panic safety:** Verify `RawModeGuard` calls `disable_raw_mode()` in both `Drop` and the panic hook. Verify the panic hook calls the original hook after restoring the terminal.
5. **Input drain loop:** Verify `poll_input_pins()` uses a `while` loop to drain ALL queued events. Not just `event::read()` once.
6. **frame_delta_ns capping:** Verify `frame_delta_ns.min(MAX_FRAME_DELTA_NS)` prevents gravity avalanche after tab-away.
7. **Throttled rendering:** Verify the render call is guarded by `if now.duration_since(last_render)... >= FRAME_NS`. Rendering does NOT block the game loop.
8. **Clock tick ordering:** Verify the main loop calls in this EXACT order: (1) sample inputs → (2) clock_tick → (3) render. Not: sample → render → tick.
9. **Yield pattern:** Verify `std::thread::sleep` is used (not busy-wait loop). Verify the sleep is unconditional (prevents 100% CPU).
10. **Compile check:** `cargo check` must pass. Note: this task depends on T01 being complete. T02 and T03 are also needed for `SiliconMotherboard` and `render_game`.
11. **Import check:** Verify `use` statements import from `crate::bus`, `crate::chips`, `crate::motherboard`, `crate::terminal`, `crate::tui`. No external dependencies beyond crossterm and ratatui.
12. **No unsafe:** `grep -rn "unsafe" src/main.rs src/terminal.rs` returns nothing.

---

## Acceptance Criteria

- [ ] `src/terminal.rs` exists with `RawModeGuard` and `poll_input_pins`
- [ ] `src/main.rs` exists with the complete main loop
- [ ] Raw mode entered/restored on both normal exit and panic
- [ ] Input polling is strictly non-blocking (`Duration::ZERO`)
- [ ] Drain loop captures all simultaneous keypresses
- [ ] Frame delta capped at `MAX_FRAME_DELTA_NS` (prevents gravity avalanche)
- [ ] Rendering throttled to ~60 FPS independently of game logic
- [ ] Main loop ordering: Sample → Propagate → Latch → Render
- [ ] Escape key exits; `GameOver` phase exits after 2-second delay
- [ ] Terminal restored on exit (`LeaveAlternateScreen`, `cursor::Show`)
- [ ] Zero async, zero threading (except `thread::sleep` for yield)
- [ ] Zero `unsafe` blocks
