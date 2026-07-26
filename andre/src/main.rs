use std::fs;
use std::path::PathBuf;

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use andre::app::App;
use andre::discovery::ConfigSearch;
use andre::ui;
use andre_core::Result;

/// Parsed CLI flags. Hand-rolled from `std::env::args` (M06 DIY-5) replacing clap;
/// surface is intentionally small and stable.
#[derive(Debug)]
struct Args {
    config: Option<PathBuf>,
    yolo: bool,
    onboard: bool,
    debug: bool,
    log_file: Option<PathBuf>,
}

const HELP_TEXT: &str = "andre — A TUI wrapper for GNU Stow

USAGE:
    andre [OPTIONS]

OPTIONS:
    -c, --config <PATH>      Path to YAML config file
        --yolo               Headless mode: auto-stow all packages without interaction
        --onboard            Setup wizard: open config screen to create a new config file
                             (requires --config)
        --debug              Enable debug logging to file
        --log-file <PATH>    Path for debug log file (default: {config_dir}/andre.log)
    -h, --help               Print this help

PATH RESOLUTION
    Source and target paths in andre.yml always resolve relative
    to the config file's directory, not the executable or CWD.
    ~       expands to $HOME
    $VAR    resolves from environment
    rel/    resolved relative to andre.yml's directory";

fn print_help() {
    println!("{HELP_TEXT}");
}

fn usage_error(msg: &str) -> ! {
    eprintln!("error: {msg}");
    eprintln!();
    eprintln!("Usage: andre [OPTIONS]");
    eprintln!();
    eprintln!("For more information, try '--help'.");
    std::process::exit(2);
}

/// Outcome of parsing. `Help` is a control-flow signal (not a real error):
/// the caller prints help and exits 0.
#[derive(Debug, PartialEq, Eq)]
enum ParseError {
    Help,
    Usage(String),
}

fn take_value(
    iter: &mut std::vec::IntoIter<String>,
    flag: &str,
) -> std::result::Result<PathBuf, ParseError> {
    match iter.next() {
        Some(v) if !v.is_empty() => Ok(PathBuf::from(v)),
        _ => Err(ParseError::Usage(format!(
            "a value is required for '{flag}' but none was supplied"
        ))),
    }
}

fn path_value(flag: &str, raw: &str) -> std::result::Result<PathBuf, ParseError> {
    if raw.is_empty() {
        Err(ParseError::Usage(format!(
            "a value is required for '{flag}' but none was supplied"
        )))
    } else {
        Ok(PathBuf::from(raw))
    }
}

/// Pure argument parser (no `env`/`process::exit`) so it is unit-testable.
/// Accepts the argv vector (program name already stripped) and returns the
/// parsed flags, or `Help`/`Usage` for the caller to render + exit.
fn parse(argv: Vec<String>) -> std::result::Result<Args, ParseError> {
    let mut iter = argv.into_iter();
    let mut args = Args {
        config: None,
        yolo: false,
        onboard: false,
        debug: false,
        log_file: None,
    };

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(ParseError::Help),
            "--yolo" => args.yolo = true,
            "--onboard" => args.onboard = true,
            "--debug" => args.debug = true,
            "--config" => args.config = Some(take_value(&mut iter, "--config")?),
            "-c" => args.config = Some(take_value(&mut iter, "-c")?),
            "--log-file" => args.log_file = Some(take_value(&mut iter, "--log-file")?),
            s if s.starts_with("--config=") => {
                args.config = Some(path_value("--config", &s["--config=".len()..])?);
            }
            s if s.starts_with("--log-file=") => {
                args.log_file = Some(path_value("--log-file", &s["--log-file=".len()..])?);
            }
            s if s.starts_with("-c") && s.len() > 2 => {
                // Short-attached form: `-cVALUE` or `-c=VALUE` (strip one leading '=').
                let rest = &s[2..];
                let rest = rest.strip_prefix('=').unwrap_or(rest);
                args.config = Some(path_value("-c", rest)?);
            }
            other => {
                return Err(ParseError::Usage(format!(
                    "unexpected argument '{other}' found"
                )));
            }
        }
    }
    Ok(args)
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match parse(argv) {
        Ok(args) => args,
        Err(ParseError::Help) => {
            print_help();
            std::process::exit(0);
        }
        Err(ParseError::Usage(msg)) => usage_error(&msg),
    }
}

fn run_yolo(app: &App) -> Result<()> {
    use andre_core::{engine_for, yolo};

    let jobs = yolo::build_plan(&app.ctx.core.config, &app.ctx.config_dir, &app.ctx.home_dir);

    // Ensure target dirs exist
    for job in &jobs {
        if !job.target.exists() {
            fs::create_dir_all(&job.target)?;
        }
    }

    // Report skipped groups
    let config_group_count = app.ctx.core.config.groups.len();
    let planned_count = jobs.len();
    if planned_count < config_group_count {
        eprintln!(
            "[yolo] {} group(s) skipped (missing source or no packages)",
            config_group_count - planned_count
        );
    }

    let engine = engine_for(&app.ctx.core.config.effective_engine());
    let action = app.ctx.core.action;
    let verbosity = app.ctx.core.verbosity;
    let dry_run = app.ctx.core.dry_run;
    let no_folding = app.ctx.core.no_folding;
    let adopt = app.ctx.core.adopt;
    let dotfiles = app.ctx.core.dotfiles;

    let mut total = 0usize;
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for job in &jobs {
        let count = job.packages.len();
        total += count;
        let (ok, fail) = yolo::run_job(
            job,
            engine.as_ref(),
            action,
            verbosity,
            dry_run,
            no_folding,
            adopt,
            dotfiles,
        );
        if fail == 0 {
            println!("[{}] {} package(s): OK", job.group, ok);
        } else {
            eprintln!(
                "[{}] {}/{} package(s): FAILED (engine={})",
                job.group,
                fail,
                count,
                engine.name()
            );
        }
        succeeded += ok;
        failed += fail;
    }

    println!();
    println!(
        "{} succeeded, {} failed (total: {})",
        succeeded, failed, total
    );
    Ok(())
}

fn find_config(args: &Args) -> Option<PathBuf> {
    let mut search = ConfigSearch::new();
    search.explicit = args.config.clone();
    search.discover()
}

fn main() {
    // Print Display (not Debug) on failure — std Termination uses {err:?}.
    if let Err(e) = try_main() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let args = parse_args();

    if args.onboard && args.config.is_none() {
        eprintln!("Error: --onboard requires --config to specify where to create the config file.");
        std::process::exit(1);
    }

    let config_path = match find_config(&args) {
        Some(p) => p,
        None if args.onboard => {
            // Create empty config file at the specified path
            let path = args.config.unwrap();
            let content = "global: {}\ngroups: {}\n";
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, content)?;
            eprintln!("Created empty config at {}", path.display());
            path
        }
        None => {
            let search = ConfigSearch::new();
            eprintln!("Error: config file not found. Searched:");
            for loc in search.search_locations() {
                eprintln!("  - {}", loc.display());
            }
            eprintln!("Use --config to specify a custom path.");
            std::process::exit(1);
        }
    };

    let log_file = args.log_file.clone().or_else(|| {
        if args.debug {
            Some(
                config_path
                    .parent()
                    .unwrap_or(&config_path)
                    .join("andre.log"),
            )
        } else {
            None
        }
    });
    let app = match App::with_debug(config_path.clone(), args.yolo, args.debug, log_file, None) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Error loading config {}: {}", config_path.display(), e);
            std::process::exit(1);
        }
    };

    // Print config validation warnings
    for warning in app.ctx.core.config.validate() {
        eprintln!("config warning: {}", warning);
    }

    if args.yolo {
        return run_yolo(&app);
    }

    let rt = tokio::runtime::Runtime::new()?;
    let _guard = rt.enter();

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(std::io::stderr(), LeaveAlternateScreen, Show);
        default_hook(info);
    }));

    enable_raw_mode()?;
    crossterm::execute!(std::io::stderr(), EnterAlternateScreen)?;
    crossterm::execute!(std::io::stderr(), Hide)?;

    let result = (|| -> Result<()> {
        let mut app = app;

        if args.onboard {
            use andre::components::settings::SettingsComponent;
            app.component_stack
                .push(Box::new(SettingsComponent::default()));
            app.onboard_mode = true;
        }

        let backend = CrosstermBackend::new(std::io::stderr());
        let mut terminal = Terminal::new(backend)?;

        let size = terminal.size()?;
        app.set_terminal_size(size.width, size.height);

        terminal.clear()?;

        loop {
            app.update_components();

            terminal.draw(|f| {
                ui::render(&mut app, f);
            })?;

            if event::poll(std::time::Duration::from_millis(50))? {
                match event::read() {
                    Ok(event::Event::Key(key)) => {
                        if key.kind != event::KeyEventKind::Press {
                            continue;
                        }

                        // Ctrl+C always quits (terminal signal)
                        if key.code == crossterm::event::KeyCode::Char('c')
                            && key.modifiers == crossterm::event::KeyModifiers::CONTROL
                        {
                            app.quitting = true;
                            break;
                        }
                        app.component_dispatch(key);
                        app.flush_debug_log();
                        if app.quitting {
                            break;
                        }
                    }
                    Ok(event::Event::Resize(cols, rows)) => {
                        app.set_terminal_size(cols, rows);
                        terminal.draw(|f| ui::render(&mut app, f))?;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }

        terminal.clear()?;
        Ok(())
    })();

    crossterm::execute!(std::io::stderr(), LeaveAlternateScreen)?;
    crossterm::execute!(std::io::stderr(), Show)?;
    disable_raw_mode()?;

    result
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn parse_empty_yields_defaults() {
        let a = parse(s(&[])).unwrap();
        assert!(a.config.is_none());
        assert!(a.log_file.is_none());
        assert!(!a.yolo && !a.onboard && !a.debug);
    }

    #[test]
    fn parse_config_long_space() {
        let a = parse(s(&["--config", "x.yml"])).unwrap();
        assert_eq!(a.config.unwrap(), PathBuf::from("x.yml"));
    }

    #[test]
    fn parse_config_long_equals() {
        let a = parse(s(&["--config=x.yml"])).unwrap();
        assert_eq!(a.config.unwrap(), PathBuf::from("x.yml"));
    }

    #[test]
    fn parse_config_short_space() {
        let a = parse(s(&["-c", "x.yml"])).unwrap();
        assert_eq!(a.config.unwrap(), PathBuf::from("x.yml"));
    }

    #[test]
    fn parse_config_short_attached() {
        let a = parse(s(&["-cx.yml"])).unwrap();
        assert_eq!(a.config.unwrap(), PathBuf::from("x.yml"));
    }

    #[test]
    fn parse_config_short_equals_strips_equals() {
        // clap-consistent: `-c=x.yml` yields x.yml (one leading '=' stripped).
        let a = parse(s(&["-c=x.yml"])).unwrap();
        assert_eq!(a.config.unwrap(), PathBuf::from("x.yml"));
    }

    #[test]
    fn parse_log_file_space_and_equals() {
        let a = parse(s(&["--log-file", "a.log"])).unwrap();
        assert_eq!(a.log_file.unwrap(), PathBuf::from("a.log"));
        let a = parse(s(&["--log-file=b.log"])).unwrap();
        assert_eq!(a.log_file.unwrap(), PathBuf::from("b.log"));
    }

    #[test]
    fn parse_bool_flags() {
        let a = parse(s(&["--yolo", "--onboard", "--debug"])).unwrap();
        assert!(a.yolo && a.onboard && a.debug);
    }

    #[test]
    fn parse_help_long_and_short() {
        assert!(matches!(
            parse(s(&["--help"])).unwrap_err(),
            ParseError::Help
        ));
        assert!(matches!(parse(s(&["-h"])).unwrap_err(), ParseError::Help));
    }

    #[test]
    fn parse_unknown_arg_is_usage_error() {
        match parse(s(&["--bogus"])) {
            Err(ParseError::Usage(m)) => assert!(m.contains("--bogus"), "msg: {m}"),
            other => panic!("expected Usage error, got {other:?}"),
        }
    }

    #[test]
    fn parse_missing_value_is_usage_error() {
        match parse(s(&["--config"])) {
            Err(ParseError::Usage(m)) => assert!(m.contains("--config"), "msg: {m}"),
            other => panic!("expected Usage error, got {other:?}"),
        }
    }

    #[test]
    fn parse_empty_equals_value_is_usage_error() {
        for flag in ["--config=", "--log-file=", "-c="] {
            match parse(s(&[flag])) {
                Err(ParseError::Usage(m)) => {
                    assert!(m.contains("value is required"), "flag={flag} msg: {m}")
                }
                other => panic!("expected Usage for {flag}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_flags_combine_in_order() {
        let a = parse(s(&["--debug", "-c", "c.yml", "--yolo"])).unwrap();
        assert!(a.debug && a.yolo);
        assert_eq!(a.config.unwrap(), PathBuf::from("c.yml"));
    }
}
