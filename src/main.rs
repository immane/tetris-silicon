// ============================================================================
// main.rs — Wall-clock sampling, I/O driver, display output
// ============================================================================

use std::io;
use std::time::{Duration, Instant};

use crossterm::cursor;
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use tetris_silicon::bus::{GamePhase, SystemBus, FRAME_NS, MAX_FRAME_DELTA_NS};
use tetris_silicon::motherboard::SiliconMotherboard;
use tetris_silicon::terminal::{poll_input_pins, RawModeGuard};

fn main() -> io::Result<()> {
    // Initialise bus and backend BEFORE entering raw mode so that any CUDA
    // diagnostic messages (device name, fallback warnings) are visible.
    let mut bus = SystemBus::new(1);
    let mut motherboard = SiliconMotherboard::new_with_env_backend();
    eprintln!("[tetris-silicon] backend: {}", motherboard.backend_name());

    let _guard = RawModeGuard::enter()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // bus and motherboard already constructed above

    let mut last_tick = Instant::now();
    let mut last_render = Instant::now();

    loop {
        let now = Instant::now();

        let frame_delta_ns = now.duration_since(last_tick).as_nanos() as u64;
        last_tick = now;
        let delta = frame_delta_ns.min(MAX_FRAME_DELTA_NS);
        let sampled = poll_input_pins(delta);

        motherboard.clock_tick(&sampled.pins, &mut bus);

        if now.duration_since(last_render).as_nanos() as u64 >= FRAME_NS {
            let bn = motherboard.backend_name();
            let gt = motherboard.gpu_tick_count();
            let chip_lines = motherboard.chip_backend_lines();
            terminal.draw(|f| tetris_silicon::tui::render_game(f, &bus, bn, gt, &chip_lines))?;
            last_render = now;
        }

        let work_ns = now.elapsed().as_nanos() as u64;
        if work_ns < FRAME_NS / 2 {
            std::thread::sleep(Duration::from_nanos(500_000));
        }

        if sampled.pins.key_escape || bus.game_phase == GamePhase::GameOver {
            let bn = motherboard.backend_name();
            let gt = motherboard.gpu_tick_count();
            let chip_lines = motherboard.chip_backend_lines();
            terminal.draw(|f| tetris_silicon::tui::render_game(f, &bus, bn, gt, &chip_lines))?;
            std::thread::sleep(Duration::from_secs(2));
            break;
        }
    }

    execute!(io::stdout(), LeaveAlternateScreen, cursor::Show)?;
    Ok(())
}
