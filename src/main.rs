use crossterm::event::{self, Event};
use llamascope::{
    app::{Action, AppState},
    config::{Config, HELP},
    demo, hardware,
    hardware::HardwareTelemetry,
    telemetry::{Poller, TelemetryData, Worker},
    update,
};
use std::{
    io::{self, IsTerminal},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, TryRecvError},
        Arc,
    },
    time::{Duration, Instant},
};

type Source<T> = (Worker, Receiver<T>);
type Sensors = Source<(HardwareTelemetry, Instant)>;

/// `llamascope update`: the token comes from GH_TOKEN, GITHUB_TOKEN, or `gh auth token`.
fn self_update(args: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = args.collect();
    if args.iter().any(|s| matches!(s.as_str(), "--help" | "-h")) {
        println!("{}", update::USAGE);
        return Ok(());
    }
    let options = update::Options::parse(args)?;
    let target = update::target()
        .ok_or("No release builds exist for this platform; use install.sh --source.")?;
    let env = |name| {
        std::env::var(name)
            .ok()
            .filter(|v: &String| !v.trim().is_empty())
    };
    let token = env("GH_TOKEN").or_else(|| env("GITHUB_TOKEN")).or_else(|| {
        let output = std::process::Command::new("gh")
            .args(["auth", "token"])
            .output()
            .ok()?;
        let token = String::from_utf8(output.stdout).ok()?.trim().to_owned();
        (output.status.success() && !token.is_empty()).then_some(token)
    });
    let source = update::Source {
        api: update::API,
        repository: update::REPOSITORY,
        token,
        target,
        current: env!("CARGO_PKG_VERSION"),
        executable: std::env::current_exe()?,
    };
    let current = source.current;
    match update::run(&options, &source)? {
        update::Outcome::UpToDate(latest) => {
            println!("LlamaScope {current} is up to date (latest {latest}).")
        }
        update::Outcome::Available(latest) => {
            println!("LlamaScope {latest} is available (running {current}). Run: llamascope update")
        }
        update::Outcome::Updated(latest) => println!(
            "Updated {} from {current} to {latest}. Restart running dashboards to use it.",
            source.executable.display()
        ),
    }
    Ok(())
}

fn main() {
    if let Err(error) = start() {
        eprintln!("LlamaScope: {error}");
        std::process::exit(1);
    }
}

fn start() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("update") {
        return self_update(args.into_iter().skip(1));
    }
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
    // SIGTERM, SIGHUP and friends end the loop like `q`, so the terminal is restored.
    let terminate = Arc::new(AtomicBool::new(false));
    for signal in [
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGQUIT,
    ] {
        signal_hook::flag::register(signal, Arc::clone(&terminate))?;
    }
    let mut state = AppState::from_config(&config);
    let (server, sensors) = if config.demo {
        let epoch = demo::seed(&mut state);
        let server = Worker::spawn(demo::TICK, move || {
            demo::telemetry(demo::tick_at(epoch, Instant::now()), epoch)
        });
        let sensors = Worker::spawn(demo::TICK, move || {
            let tick = demo::tick_at(epoch, Instant::now());
            (demo::hardware(tick), epoch + demo::TICK * tick as u32)
        });
        (server, Some(sensors))
    } else {
        let mut poller = Poller::new(config.clone()).map_err(|_| {
            "Could not initialize the HTTP client. Check TLS/network configuration."
        })?;
        let no_gpu = config.no_gpu;
        let sensors = Worker::spawn(Duration::from_secs(2), move || {
            (hardware::sample(no_gpu), Instant::now())
        });
        (
            Worker::spawn(config.interval, move || poller.sample()),
            Some(sensors),
        )
    };
    // dashboard owns terminal cleanup. Workers are joined only after the screen is restored.
    let result = dashboard(state, &server, sensors, &terminate);
    server.0.stop();
    result?;
    Ok(())
}

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

fn dashboard(
    mut state: AppState,
    server: &Source<TelemetryData>,
    mut sensors: Option<Sensors>,
    terminate: &AtomicBool,
) -> io::Result<()> {
    // Create the guard before initialization so partially completed setup is also restored.
    let _guard = TerminalGuard;
    let mut terminal = ratatui::try_init()?;
    state.log(
        false,
        "LlamaScope ready. ? opens the field guide; q / Ctrl-C exits safely.",
    );
    while !terminate.load(Ordering::Relaxed) {
        match server.1.try_recv() {
            Ok(data) => state.receive(data),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Err(io::Error::other(
                    "Telemetry worker stopped unexpectedly. Restart LlamaScope.",
                ))
            }
        }
        // Local sensors are optional: losing them must never end server monitoring.
        if let Some((_, rx)) = &sensors {
            match rx.try_recv() {
                Ok((hardware, at)) => state.receive_hardware(hardware, at),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    state.sensors_stopped("collector ended unexpectedly");
                    sensors = None;
                }
            }
        }
        terminal.draw(|frame| state.draw(frame))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match state.handle_key(key) {
                    Action::Quit => break,
                    Action::Retry => server.0.retry(),
                    Action::None => {}
                }
            }
        }
    }
    Ok(())
}
