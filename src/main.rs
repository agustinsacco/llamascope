use crossterm::event::{self, Event};
use llamascope::{
    app::{Action, AppState},
    config::{Config, HELP},
    demo, hardware,
    telemetry::{Poller, TelemetryData, Worker},
};
use std::{
    io::{self, IsTerminal},
    sync::mpsc::{Receiver, TryRecvError},
    time::{Duration, Instant},
};

fn main() {
    if let Err(error) = start() {
        eprintln!("LlamaScope: {error}");
        std::process::exit(1);
    }
}

fn start() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|s| matches!(s.as_str(), "--help" | "-h")) {
        println!("{HELP}");
        return Ok(());
    }
    if args
        .iter()
        .any(|s| matches!(s.as_str(), "--version" | "-V"))
    {
        println!("LlamaScope {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let config = Config::parse(args, |name| std::env::var(name).ok())?;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("An interactive terminal is required. Try llamascope --demo in a terminal, or --help for options.".into());
    }
    let server = if config.demo {
        None
    } else {
        let mut poller = Poller::new(config.clone()).map_err(|_| {
            "Could not initialize the HTTP client. Check TLS/network configuration."
        })?;
        Some(Worker::spawn(config.interval, move || poller.sample()))
    };
    let sensors = if config.demo {
        None
    } else {
        let no_gpu = config.no_gpu;
        Some(Worker::spawn(Duration::from_secs(2), move || {
            hardware::sample(no_gpu)
        }))
    };
    // dashboard owns terminal cleanup. Workers are joined only after the screen is restored.
    let result = dashboard(&config, server.as_ref(), sensors.as_ref());
    if let Some((worker, _)) = &server {
        worker.stop();
    }
    if let Some((worker, _)) = &sensors {
        worker.stop();
    }
    result?;
    Ok(())
}

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

fn receive<T>(source: Option<&(Worker, Receiver<T>)>, pending: &mut Option<T>) -> io::Result<()> {
    if let Some((_, rx)) = source {
        match rx.try_recv() {
            Ok(data) => *pending = Some(data),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Err(io::Error::other(
                    "Telemetry worker stopped unexpectedly. Restart LlamaScope.",
                ))
            }
        }
    }
    Ok(())
}

fn dashboard(
    config: &Config,
    server: Option<&(Worker, Receiver<TelemetryData>)>,
    sensors: Option<&(Worker, Receiver<hardware::HardwareTelemetry>)>,
) -> io::Result<()> {
    // Create the guard before initialization so partially completed setup is also restored.
    let _guard = TerminalGuard;
    let mut terminal = ratatui::try_init()?;
    let mut state = AppState::new();
    state.theme = config.theme;
    state.stale_after = (config.interval * 3).max(Duration::from_secs(3));
    state.endpoint.clone_from(&config.api_url);
    state.demo = config.demo;
    state.log(
        false,
        "LlamaScope ready. ? opens the field guide; q / Ctrl-C exits safely.",
    );
    if config.demo {
        state.started_at = Instant::now() - Duration::from_secs(60);
        for tick in 0..120 {
            let (data, hardware) = demo::sample(tick, state.started_at);
            let at = data.sampled_at;
            state.update(data);
            state.update_hardware(hardware, at);
        }
    }
    let mut pending_server = None;
    let mut pending_hardware = None;
    let mut last_demo_tick = 119;
    loop {
        receive(server, &mut pending_server)?;
        receive(sensors, &mut pending_hardware)?;
        if config.demo {
            let tick = state.started_at.elapsed().as_millis() as u64 / 500;
            if tick != last_demo_tick {
                let (data, hardware) = demo::sample(tick, state.started_at);
                pending_server = Some(data);
                pending_hardware = Some(hardware);
                last_demo_tick = tick;
            }
        }
        if state.paused_at.is_none() {
            if let Some(data) = pending_server.take() {
                state.update(data);
            }
            if let Some(data) = pending_hardware.take() {
                state.update_hardware(data, Instant::now());
            }
        }
        terminal.draw(|frame| state.draw(frame))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match state.handle_key(key) {
                    Action::Quit => return Ok(()),
                    Action::Retry => {
                        if let Some((worker, _)) = server {
                            worker.retry();
                        }
                    }
                    Action::None => {}
                }
            }
        }
    }
}
