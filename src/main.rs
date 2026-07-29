use std::time::{Duration, Instant};
use crossterm::event::{self, Event, KeyCode};
use llamascope::app::AppState;
use llamascope::telemetry::start_polling_thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    // 2. Setup state & polling thread
    let mut state = AppState::new();
    state.log_info("LlamaScope v1.0.0 monitoring initialized.".to_string());
    
    let (tx, rx) = std::sync::mpsc::channel();
    let api_url = std::env::var("LLAMA_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8086".to_string());
    state.log_info(format!("Connecting to llama-server at {}", api_url));
    
    start_polling_thread(api_url, tx);

    // 3. Event Loop
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        // Draw TUI
        terminal.draw(|f| state.draw(f))?;

        // Read channel events
        while let Ok(data) = rx.try_recv() {
            state.update(data);
        }

        // Process keypresses / events
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => {
                            break;
                        }
                        KeyCode::Char('c') => {
                            state.logs.clear();
                            state.log_info("Console logs cleared.".to_string());
                        }
                        KeyCode::Tab => {
                            state.cycle_selected_slot();
                            state.log_info("Cycled selected active slot.".to_string());
                        }
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    // 4. Restore terminal
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
