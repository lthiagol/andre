use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use andre::app::App;
use andre::components::profiles::ProfilesComponent;
use andre::discovery::ConfigSearch;
use andre::ui;
use andre_core::ignore::global_ignore_in;
use andre_core::yolo;
use andre_core::{PackageEntry, Result};

/// Parsed CLI flags. Hand-rolled from `std::env::args` (M06 DIY-5) replacing clap;
/// surface is intentionally small and stable.
#[derive(Debug)]
struct Args {
    config: Option<PathBuf>,
    config_folder: Option<PathBuf>,
    profile: Option<String>,
    yolo: bool,
    onboard: bool,
    debug: bool,
    log_file: Option<PathBuf>,
}

const HELP_TEXT: &str = "andre — Another Neat Dotfile Repository Engine

USAGE:
    andre [OPTIONS]

OPTIONS:
    -c, --config <PATH>        Path to YAML config file
        --config-folder <DIR>  Config folder holding andre.yml and profiles/
                               (cannot be combined with --config)
        --profile <NAME>       Work on one profile. With --yolo: stow only its packages.
                               Alone: open the TUI on its screen
        --yolo                 Headless mode: auto-stow all packages without interaction
        --onboard              Setup wizard: open config screen to create a new config file
                               (requires --config or --config-folder)
        --debug                Enable debug logging to file
        --log-file <PATH>      Path for debug log file (default: {config_dir}/andre.log)
    -h, --help                 Print this help

CONFIG DISCOVERY
    --config, else --config-folder, else $XDG_CONFIG_HOME/andre/ (~/.config/andre/).
    A flag that is given is the only place searched.
    Profiles are read from profiles/ next to the andre.yml that is loaded.

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
    path_value(flag, &iter.next().unwrap_or_default())
}

fn name_value(flag: &str, raw: &str) -> std::result::Result<String, ParseError> {
    if raw.is_empty() {
        Err(ParseError::Usage(format!(
            "a value is required for '{flag}' but none was supplied"
        )))
    } else {
        Ok(raw.to_string())
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
        config_folder: None,
        profile: None,
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
            "--config-folder" => {
                args.config_folder = Some(take_value(&mut iter, "--config-folder")?);
            }
            "--log-file" => args.log_file = Some(take_value(&mut iter, "--log-file")?),
            "--profile" => {
                let raw = iter.next().unwrap_or_default();
                args.profile = Some(name_value("--profile", &raw)?);
            }
            s if s.starts_with("--config=") => {
                args.config = Some(path_value("--config", &s["--config=".len()..])?);
            }
            s if s.starts_with("--config-folder=") => {
                args.config_folder = Some(path_value(
                    "--config-folder",
                    &s["--config-folder=".len()..],
                )?);
            }
            s if s.starts_with("--log-file=") => {
                args.log_file = Some(path_value("--log-file", &s["--log-file=".len()..])?);
            }
            s if s.starts_with("--profile=") => {
                args.profile = Some(name_value("--profile", &s["--profile=".len()..])?);
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
    if args.config.is_some() && args.config_folder.is_some() {
        return Err(ParseError::Usage(
            "'--config' cannot be used with '--config-folder'".to_string(),
        ));
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

/// Stows every package in `plan` without asking. One line per package, then the totals.
fn run_yolo(app: &App, plan: &[PackageEntry]) -> Result<()> {
    use andre_core::engine_for;

    // A `groups:` config gives each package its own target, so create every distinct one.
    let targets: BTreeSet<&Path> = plan.iter().map(|entry| entry.target.as_path()).collect();
    for target in targets {
        if !target.exists() {
            fs::create_dir_all(target)?;
        }
    }

    let engine = engine_for(app.ctx.core.config.effective_engine());
    let flags = app.ctx.core.flags;
    let global_ignore = global_ignore_in(&app.ctx.home_dir);

    let mut succeeded = 0usize;
    let mut failed = 0usize;
    for entry in plan {
        match yolo::run_package(entry, engine.as_ref(), &flags, Some(&global_ignore)) {
            Ok(()) => {
                println!("{:<6}{}", "ok", entry.name);
                succeeded += 1;
            }
            Err(err) => {
                eprintln!(
                    "{:<6}{}  (engine={}): {}",
                    "FAIL",
                    entry.name,
                    engine.name(),
                    one_line(&err.to_string())
                );
                failed += 1;
            }
        }
    }

    println!(
        "{} succeeded, {} failed (total: {})",
        succeeded,
        failed,
        plan.len()
    );
    Ok(())
}

/// `message` as one line: each line trimmed, empty lines dropped, the rest joined by a space.
/// GNU stow's errors end in newlines, and a `FAIL` line must stay one line.
fn one_line(message: &str) -> String {
    message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// What `--yolo` stows: every package, or only those of `--profile`. Exits 2 for an
/// unknown profile and 1 for one that does not resolve (a cycle, an unknown include).
fn headless_plan(app: &App, profile: Option<&str>) -> Vec<PackageEntry> {
    let config = &app.ctx.core.config;
    let Some(name) = profile else {
        return yolo::build_plan(config, &app.ctx.config_dir, &app.ctx.home_dir);
    };
    let known = andre_core::profile::known_packages(config, &app.ctx.config_dir, &app.ctx.home_dir);
    let resolved = match app.ctx.core.profiles.resolve(name, &known) {
        Ok(resolved) => resolved,
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    };
    yolo::profile_plan(config, &app.ctx.config_dir, &app.ctx.home_dir, &resolved)
}

/// Exits 2 when `--profile` names no profile, listing the ones there are.
fn check_profile_exists(app: &App, name: &str) {
    if app.ctx.core.profiles.get(name).is_some() {
        return;
    }
    eprintln!(
        "error: no profile named '{name}' in {}",
        app.ctx.core.profiles.dir().display()
    );
    let names: Vec<&str> = app
        .ctx
        .core
        .profiles
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    if names.is_empty() {
        eprintln!("no profiles exist yet");
    } else {
        eprintln!("profiles: {}", names.join(", "));
    }
    std::process::exit(2);
}

fn search_for(args: &Args) -> ConfigSearch {
    let mut search = ConfigSearch::new();
    search.explicit = args.config.clone();
    search.config_folder = args.config_folder.clone();
    search
}

/// Prints the not-found error and exits 1. Names a pre-1.0 config when one exists and
/// no flag was given, so users can point `--config-folder` at it.
fn config_not_found(search: &ConfigSearch, flags_given: bool) -> ! {
    eprintln!("Error: config file not found. Searched:");
    for loc in search.search_locations() {
        eprintln!("  - {}", loc.display());
    }
    if !flags_given {
        if let Some(legacy) = search.legacy_config() {
            let folder = legacy.parent().unwrap_or(&legacy);
            eprintln!();
            eprintln!("Found a config in a pre-1.0 location: {}", legacy.display());
            eprintln!(
                "andre no longer searches there. Use: andre --config-folder {}",
                folder.display()
            );
        }
    }
    eprintln!("Use --config <FILE> or --config-folder <DIR> to choose a config.");
    std::process::exit(1);
}

fn main() {
    // Print Display (not Debug) on failure — std Termination uses {err:?}.
    if let Err(e) = try_main() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

/// What `--onboard` writes when no config exists: no packages folder yet, so the settings
/// screen opens on `packages`.
const ONBOARD_CONFIG: &str = "global: {}\n";

fn try_main() -> Result<()> {
    let args = parse_args();

    // Where `--onboard` creates a new config: the --config file, or andre.yml in --config-folder.
    let onboard_target = args
        .config
        .clone()
        .or_else(|| args.config_folder.as_ref().map(|dir| dir.join("andre.yml")));
    if args.onboard && onboard_target.is_none() {
        eprintln!(
            "Error: --onboard requires --config or --config-folder to specify where to create the config file."
        );
        std::process::exit(1);
    }

    let search = search_for(&args);
    let config_path = match search.discover() {
        Some(p) => p,
        None if args.onboard => {
            // Create empty config file at the specified path
            let path = onboard_target.expect("checked above: --onboard has a target");
            let content = ONBOARD_CONFIG;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, content)?;
            eprintln!("Created empty config at {}", path.display());
            path
        }
        None => config_not_found(
            &search,
            args.config.is_some() || args.config_folder.is_some(),
        ),
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
    let app = match App::with_debug(config_path.clone(), args.debug, log_file, None) {
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
    let known = andre_core::profile::known_packages(
        &app.ctx.core.config,
        &app.ctx.config_dir,
        &app.ctx.home_dir,
    );
    for warning in app.ctx.core.profiles.warnings(&known) {
        eprintln!("profile warning: {}", warning);
    }

    if let Some(name) = &args.profile {
        check_profile_exists(&app, name);
    }

    if args.yolo {
        let config = &app.ctx.core.config;
        // Interim: a `groups:` config has its folders, so it is not "no packages folder".
        if config.packages.is_none() && config.groups.is_empty() {
            eprintln!(
                "no packages folder: set `packages:` in {}",
                config_path.display()
            );
            std::process::exit(1);
        }
        let plan = headless_plan(&app, args.profile.as_deref());
        return run_yolo(&app, &plan);
    }

    let rt = tokio::runtime::Runtime::new()?;
    let _guard = rt.enter();

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stderr(),
            DisableMouseCapture,
            LeaveAlternateScreen,
            Show
        );
        default_hook(info);
    }));

    enable_raw_mode()?;
    crossterm::execute!(std::io::stderr(), EnterAlternateScreen, EnableMouseCapture)?;
    crossterm::execute!(std::io::stderr(), Hide)?;

    let result = (|| -> Result<()> {
        let mut app = app;

        if args.onboard {
            use andre::components::settings::SettingsComponent;
            app.component_stack
                .push(Box::new(SettingsComponent::on_packages()));
            app.onboard_mode = true;
        }

        if let Some(name) = &args.profile {
            app.component_stack
                .push(Box::new(ProfilesComponent::opened_at(name)));
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
                    Ok(event::Event::Mouse(mouse)) => {
                        app.mouse_dispatch(mouse);
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

    crossterm::execute!(std::io::stderr(), DisableMouseCapture)?;
    crossterm::execute!(std::io::stderr(), LeaveAlternateScreen)?;
    crossterm::execute!(std::io::stderr(), Show)?;
    disable_raw_mode()?;

    result
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn onboard_config_loads_and_warns_only_about_the_packages_folder() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("andre.yml");
        std::fs::write(&path, ONBOARD_CONFIG).unwrap();
        let config = andre_core::Config::load(&path).unwrap();
        assert!(config.groups.is_empty());
        assert_eq!(
            config.validate(),
            [andre_core::config::ConfigWarning::NoPackagesFolder]
        );
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
    fn parse_profile_space_and_equals() {
        let a = parse(s(&["--profile", "work"])).unwrap();
        assert_eq!(a.profile.as_deref(), Some("work"));
        let a = parse(s(&["--profile=work"])).unwrap();
        assert_eq!(a.profile.as_deref(), Some("work"));
        assert!(parse(s(&[])).unwrap().profile.is_none());
    }

    #[test]
    fn parse_profile_missing_value_is_usage_error() {
        for argv in [
            &["--profile"][..],
            &["--profile="][..],
            &["--profile", ""][..],
        ] {
            match parse(s(argv)) {
                Err(ParseError::Usage(m)) => {
                    assert!(m.contains("'--profile'"), "argv={argv:?} msg: {m}")
                }
                other => panic!("expected Usage for {argv:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_empty_equals_value_is_usage_error() {
        for flag in ["--config=", "--log-file=", "-c=", "--profile="] {
            match parse(s(&[flag])) {
                Err(ParseError::Usage(m)) => {
                    assert!(m.contains("value is required"), "flag={flag} msg: {m}")
                }
                other => panic!("expected Usage for {flag}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_config_folder_space_and_equals() {
        let a = parse(s(&["--config-folder", "repo/dots"])).unwrap();
        assert_eq!(a.config_folder.unwrap(), PathBuf::from("repo/dots"));
        assert!(a.config.is_none());
        let a = parse(s(&["--config-folder=repo/dots"])).unwrap();
        assert_eq!(a.config_folder.unwrap(), PathBuf::from("repo/dots"));
    }

    #[test]
    fn parse_config_folder_missing_value_is_usage_error() {
        for argv in [s(&["--config-folder"]), s(&["--config-folder="])] {
            match parse(argv.clone()) {
                Err(ParseError::Usage(m)) => assert!(m.contains("--config-folder"), "{m}"),
                other => panic!("expected Usage for {argv:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_config_and_config_folder_are_mutually_exclusive() {
        let cases = [
            s(&["--config", "a.yml", "--config-folder", "d"]),
            s(&["--config-folder=d", "--config=a.yml"]),
            s(&["-c", "a.yml", "--config-folder", "d"]),
        ];
        for argv in cases {
            match parse(argv.clone()) {
                Err(ParseError::Usage(m)) => {
                    assert_eq!(
                        m, "'--config' cannot be used with '--config-folder'",
                        "{argv:?}"
                    )
                }
                other => panic!("expected Usage for {argv:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn help_text_lists_config_folder() {
        assert!(HELP_TEXT.contains("--config-folder <DIR>"));
        assert!(HELP_TEXT.contains("--onboard"));
        assert!(HELP_TEXT.contains("(requires --config or --config-folder)"));
    }

    #[test]
    fn parse_flags_combine_in_order() {
        let a = parse(s(&["--debug", "-c", "c.yml", "--yolo"])).unwrap();
        assert!(a.debug && a.yolo);
        assert_eq!(a.config.unwrap(), PathBuf::from("c.yml"));
    }
}

#[cfg(test)]
mod yolo_output_tests {
    use super::one_line;

    #[test]
    fn one_line_joins_stow_error_lines_and_drops_blank_ones() {
        let stow = "Stow failed: WARNING! conflicts:\n  * cannot stow over .bashrc\nAll operations aborted.\n\n";
        assert_eq!(
            one_line(stow),
            "Stow failed: WARNING! conflicts: * cannot stow over .bashrc All operations aborted."
        );
    }

    #[test]
    fn one_line_keeps_a_single_line_message() {
        assert_eq!(one_line("Stow failed: x"), "Stow failed: x");
    }
}
