//! CLI command tree, output modes, and exit-code mapping.
//!
//! Business logic lives in sibling crates; this crate stays thin.

#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use tracing::error;
use winzsh_agent::{self as agent};
use winzsh_ai::{self as ai, AiProvider, AiSettings};
use winzsh_alias::{self as alias};
use winzsh_completion::{self as completion, CompletionPolicy};
use winzsh_config::{self as config, Config};
use winzsh_core::{VERSION, WinzshPaths};
use winzsh_detect::{DetectionReport, detect_environment};
use winzsh_doctor::{self as doctor, Severity};
use winzsh_error::Error;
use winzsh_history::{self as history, HistoryQuery};
use winzsh_installer::{self as installer, InstallOptions, SelfInstallOptions, UninstallOptions};
use winzsh_log::LogOptions;
use winzsh_plugin::{self as plugin};
use winzsh_powershell::PowerShellHost;
use winzsh_registry::{self as registry};
use winzsh_runtime_gen::{self as runtime_gen};
use winzsh_shell_host::{
    self as shell_host, BashHost, CmdHost, NuHost, ShellHost, ShellId, catalog_entry,
};
use winzsh_sync::{self as sync, ExportOptions, ImportOptions};
use winzsh_theme::{self as theme};
use winzsh_update::{self as update, FromSourceOptions};

/// WinZSH — Oh My Zsh–style developer experience for Windows shells.
#[derive(Debug, Parser)]
#[command(name = "winzsh", version = VERSION, about, long_about = None)]
struct Cli {
    /// Emit machine-readable JSON on stdout where supported.
    #[arg(long, global = true)]
    json: bool,

    /// Enable debug logging.
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Show installation / version status.
    Status,
    /// Install or repair WinZSH (profile hook + runtime + config).
    Install {
        /// Repair even if already installed.
        #[arg(long)]
        force: bool,
    },
    /// Download-and-run setup: copy this .exe to `~/.winzsh/bin`, PATH, profile, theme.
    ///
    /// Running a release file named `WinZSH-Setup*.exe` with no args also runs this.
    Setup {
        /// Non-interactive (skip confirmation prompts).
        #[arg(long, short = 'y')]
        yes: bool,
        /// Theme to apply (default: modern).
        #[arg(long, default_value = "modern")]
        theme: String,
    },
    /// Remove the managed profile hook; keep data unless `--purge`.
    Uninstall {
        /// Delete `~/.winzsh` entirely.
        #[arg(long)]
        purge: bool,
    },
    /// Run health checks and print remediation hints.
    Doctor,
    /// Rebuild the cached PowerShell runtime from config.
    Reload,
    /// Configuration helpers.
    #[command(subcommand)]
    Config(ConfigCommands),
    /// Theme helpers.
    #[command(subcommand)]
    Theme(ThemeCommands),
    /// Alias helpers.
    #[command(subcommand)]
    Alias(AliasCommands),
    /// History helpers.
    #[command(subcommand)]
    History(HistoryCommands),
    /// Completion pack helpers.
    #[command(subcommand)]
    Completion(CompletionCommands),
    /// Plugin helpers.
    #[command(subcommand)]
    Plugin(PluginCommands),
    /// AI helpers (Phase 6; opt-in, local offline only).
    #[command(subcommand)]
    Ai(AiCommands),
    /// Settings sync (export/import across machines).
    #[command(subcommand)]
    Sync(SyncCommands),
    /// Opt-in multi-shell integrations (CMD / Nu / Bash bridges).
    #[command(subcommand)]
    Shell(ShellCommands),
    /// Background maintenance agent (same `winzsh` binary).
    #[command(subcommand)]
    Agent(AgentCommands),
    /// Check for or apply CLI self-updates.
    Update {
        /// Only check; do not download or rebuild.
        #[arg(long)]
        check: bool,
        /// Rebuild from a local git/cargo checkout (optional path).
        ///
        /// Path resolution order when omitted: `WINZSH_SOURCE`, `[update].source_dir`, walk cwd.
        #[arg(long, num_args = 0..=1, default_missing_value = "", value_name = "PATH")]
        from_source: Option<String>,
        /// With `--from-source`, run `git pull --ff-only` before building.
        #[arg(long)]
        pull: bool,
        /// Restore the previous CLI binary from `winzsh.exe.bak`.
        #[arg(long)]
        rollback: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommands {
    /// Print the active config (TOML) or path metadata.
    Show,
    /// Validate `config.toml`.
    Validate,
    /// Print the config file path.
    Path,
}

#[derive(Debug, Subcommand)]
enum ThemeCommands {
    /// List built-in themes.
    List,
    /// Show the active or named theme.
    Show {
        /// Theme id (defaults to active config theme).
        id: Option<String>,
    },
    /// Set the active theme and regenerate runtime.
    Set {
        /// Theme id.
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum AliasCommands {
    /// List effective aliases (builtins + plugins + user).
    List,
    /// Set a user alias and regenerate runtime.
    Set {
        /// Alias name.
        name: String,
        /// Expansion (quote if it contains spaces).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        value: Vec<String>,
    },
    /// Remove a user alias and regenerate runtime.
    Remove {
        /// Alias name.
        name: String,
    },
}

#[derive(Debug, Subcommand)]
enum HistoryCommands {
    /// List recent history entries.
    List {
        /// Max entries.
        #[arg(long, short = 'n', default_value_t = 20)]
        limit: usize,
        /// Case-insensitive substring filter.
        #[arg(long)]
        contains: Option<String>,
    },
    /// Compact spool into the history store.
    Compact,
}

#[derive(Debug, Subcommand)]
enum CompletionCommands {
    /// List built-in packs and which are active for this machine.
    List,
}

#[derive(Debug, Subcommand)]
enum AiCommands {
    /// Show AI enablement / provider / key status.
    Status,
    /// Enable AI helpers (`features.ai=true`).
    Enable,
    /// Disable AI helpers.
    Disable,
    /// Explain a shell command.
    Explain {
        /// Command words (quote the whole command if needed).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        command: Vec<String>,
    },
    /// Convert English to a PowerShell command.
    Ask {
        /// Natural-language request.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        prompt: Vec<String>,
    },
    /// Scan a command for dangerous patterns (works even when AI is disabled).
    Check {
        /// Command to scan.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        command: Vec<String>,
    },
    /// Suggest an alias from a description.
    Alias {
        /// What the alias should do.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        description: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum SyncCommands {
    /// Show sync destination and last export/import.
    Status,
    /// Write a sync bundle JSON (settings; optional plugins/history).
    Export {
        /// Output file or directory (default: ./winzsh-sync.json).
        #[arg(long, short = 'p')]
        path: Option<PathBuf>,
        /// Include installed plugin files in the bundle.
        #[arg(long)]
        plugins: bool,
        /// Include command history in the bundle.
        #[arg(long)]
        history: bool,
    },
    /// Apply a sync bundle from a file or HTTPS URL.
    Import {
        /// Bundle path, directory with winzsh-sync.json, or https URL.
        #[arg(long, short = 'p')]
        path: Option<String>,
        /// Allow import even when unusual (reserved; always writes with backup).
        #[arg(long)]
        force: bool,
    },
    /// Export to `[sync].destination` / `WINZSH_SYNC_DEST`.
    Push {
        #[arg(long)]
        plugins: bool,
        #[arg(long)]
        history: bool,
    },
    /// Import from `[sync].destination` / `WINZSH_SYNC_DEST`.
    Pull {
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ShellCommands {
    /// List supported shells (detected / hooked / enabled).
    List,
    /// Show multi-shell status summary.
    Status,
    /// Install a managed hook/launcher and opt-in via config.
    Enable {
        /// Shell id: `cmd`, `nu`, `bash` (PowerShell is install-managed).
        id: String,
    },
    /// Remove a managed hook/launcher and drop from `[shells].enabled`.
    Disable {
        /// Shell id: `cmd`, `nu`, `bash`.
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum AgentCommands {
    /// Show whether the background agent is running.
    Status,
    /// Start a detached background agent (`winzsh agent run`).
    Start,
    /// Stop a running agent.
    Stop,
    /// Run one maintenance tick in the foreground.
    #[command(name = "run-once")]
    RunOnce,
    /// Foreground loop (used by `agent start`; Ctrl+C or `agent stop` to end).
    Run,
}

#[derive(Debug, Subcommand)]
enum PluginCommands {
    /// List first-party, registry-known, and installed plugins.
    List,
    /// Search the plugin registry.
    Search {
        /// Substring match on id, description, author, tags (empty = list all).
        #[arg(default_value = "")]
        query: String,
    },
    /// Show registry metadata for a plugin id.
    Info {
        /// Plugin id.
        id: String,
    },
    /// Install a first-party, registry, or local-path plugin and enable it.
    Add {
        /// First-party id, registry id, or path to a plugin folder.
        name_or_path: String,
    },
    /// Update registry-installed plugins (or one id).
    Update {
        /// Plugin id (default: all registry-origin plugins with newer versions).
        id: Option<String>,
    },
    /// Remove an installed plugin and disable it.
    Remove {
        /// Plugin id.
        id: String,
    },
    /// Enable an installed plugin and regenerate runtime.
    Enable {
        /// Plugin id.
        id: String,
    },
    /// Disable a plugin (keeps files) and regenerate runtime.
    Disable {
        /// Plugin id.
        id: String,
    },
}

/// Parse CLI args and dispatch. Returns a process exit code.
pub fn run() -> ExitCode {
    // Double-clicked Setup.exe: keep the console open on panics so users can read errors.
    if launched_as_setup_window() {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            prev(info);
            eprintln!();
            eprintln!("WinZSH Setup crashed unexpectedly.");
            eprintln!(
                "If this keeps happening, open PowerShell and run the downloaded file from there:"
            );
            eprintln!("  .\\WinZSH-Setup-x86_64.exe");
            pause_setup_window();
        }));
    }

    match run_inner() {
        Ok(code) => code,
        Err(err) => {
            let code = err.exit_code();
            error!("{err}");
            if launched_as_setup_window() {
                eprintln!();
                eprintln!("========== WinZSH Setup failed ==========");
                eprintln!("{:?}", miette::Report::new(err));
                eprintln!();
                eprintln!("Common fixes:");
                eprintln!("  - Close other WinZSH / PowerShell windows using the old install");
                eprintln!("  - Run:  winzsh agent stop   (if a background agent is running)");
                eprintln!("  - Right-click the downloaded file → Properties → Unblock → Apply");
                eprintln!("  - Or open PowerShell in your Downloads folder and run:");
                eprintln!("      .\\WinZSH-Setup-x86_64.exe");
                eprintln!();
                pause_setup_window();
            } else {
                eprintln!("{:?}", miette::Report::new(err));
            }
            ExitCode::from(code)
        }
    }
}

fn run_inner() -> Result<ExitCode, Error> {
    let cli = Cli::parse();
    let paths = WinzshPaths::discover()?;

    let _ = winzsh_log::init(LogOptions {
        verbose: cli.verbose,
        paths: Some(paths.clone()),
    });

    let command = match cli.command {
        Some(cmd) => cmd,
        None if exe_looks_like_setup() => Commands::Setup {
            yes: true,
            theme: "modern".into(),
        },
        None => Commands::Status,
    };

    match command {
        Commands::Status => cmd_status(&paths, cli.json),
        Commands::Install { force } => cmd_install(&paths, force, cli.json),
        Commands::Setup { yes, theme } => cmd_setup(&paths, yes, theme, cli.json),
        Commands::Uninstall { purge } => cmd_uninstall(&paths, purge, cli.json),
        Commands::Doctor => cmd_doctor(&paths, cli.json),
        Commands::Reload => cmd_reload(&paths, cli.json),
        Commands::Config(sub) => cmd_config(&paths, sub, cli.json),
        Commands::Theme(sub) => cmd_theme(&paths, sub, cli.json),
        Commands::Alias(sub) => cmd_alias(&paths, sub, cli.json),
        Commands::History(sub) => cmd_history(&paths, sub, cli.json),
        Commands::Completion(sub) => cmd_completion(&paths, sub, cli.json),
        Commands::Plugin(sub) => cmd_plugin(&paths, sub, cli.json),
        Commands::Ai(sub) => cmd_ai(&paths, sub, cli.json),
        Commands::Sync(sub) => cmd_sync(&paths, sub, cli.json),
        Commands::Shell(sub) => cmd_shell(&paths, sub, cli.json),
        Commands::Agent(sub) => cmd_agent(&paths, sub, cli.json),
        Commands::Update {
            check,
            from_source,
            pull,
            rollback,
        } => cmd_update(&paths, check, from_source, pull, rollback, cli.json),
    }
}

fn exe_looks_like_setup() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_ascii_lowercase().contains("setup"))
        })
        .unwrap_or(false)
}

/// True when the user likely double-clicked `WinZSH-Setup*.exe` (no CLI args).
///
/// Do **not** gate the end-of-setup pause on `stdin().is_terminal()` — Explorer
/// launches often flash a console and exit before the user can read anything.
fn launched_as_setup_window() -> bool {
    exe_looks_like_setup() && std::env::args_os().len() <= 1
}

fn pause_setup_window() {
    if !launched_as_setup_window() {
        return;
    }
    println!();
    print!("Press Enter to close this window... ");
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
}

fn cmd_setup(paths: &WinzshPaths, yes: bool, theme: String, json: bool) -> Result<ExitCode, Error> {
    let already = paths.is_installed();
    let setup_window = launched_as_setup_window();

    if !json {
        println!("WinZSH Setup {VERSION}");
        println!("Install location: {}", paths.root.display());
        if already {
            println!();
            println!("An existing WinZSH install was found.");
            println!(
                "Setup will repair/update the CLI, profile hook, and runtime (your config is kept)."
            );
        }
        println!();
        let _ = io::stdout().flush();
    }

    if !yes && !json && io::stdin().is_terminal() {
        eprint!(
            "Install / repair WinZSH {VERSION} to {} ? [Y/n] ",
            paths.root.display()
        );
        let _ = io::stderr().flush();
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|source| winzsh_error::io("<stdin>", source))?;
        let t = line.trim();
        if !(t.is_empty() || t.eq_ignore_ascii_case("y") || t.eq_ignore_ascii_case("yes")) {
            println!("Cancelled — nothing was changed.");
            pause_setup_window();
            return Ok(ExitCode::from(1));
        }
    }

    let report = installer::self_install(
        paths,
        SelfInstallOptions {
            theme,
            skip_theme: false,
        },
    )?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| Error::Message(e.to_string()))?
        );
    } else {
        println!();
        if already {
            println!("WinZSH repaired / updated successfully.");
        } else {
            println!("WinZSH installed successfully.");
        }
        for step in &report.steps {
            println!("  - {step}");
        }
        println!();
        println!("Next steps:");
        println!("  1. Close this window.");
        println!("  2. Open a NEW PowerShell or Windows Terminal tab");
        println!("     (so PATH and profile refresh).");
        println!("  3. Run:  zsh-for-win");
        println!("  4. Try:  Get-WinZshInfo");
        println!("  5. Type: exit    (returns to stock PowerShell)");
        println!();
        println!("Already using WinZSH? You can keep your config; just open a new tab.");
        if !setup_window {
            println!("Tip: double-clicking Setup.exe shows this window until you press Enter.");
        }
    }

    pause_setup_window();
    Ok(ExitCode::SUCCESS)
}

fn cmd_status(paths: &WinzshPaths, json: bool) -> Result<ExitCode, Error> {
    let installed = paths.is_installed();
    let state = if installed {
        StateView::from_paths(paths)
    } else {
        None
    };
    let theme_id = config::load(paths).ok().map(|c| c.theme);

    if json {
        let payload = serde_json::json!({
            "name": "winzsh",
            "version": VERSION,
            "phase": "phase-6",
            "installed": installed,
            "home": paths.root,
            "theme": theme_id,
            "state": state,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).map_err(|e| Error::Message(e.to_string()))?
        );
    } else {
        println!("WinZSH {VERSION}");
        println!("Home: {}", paths.root.display());
        if installed {
            println!("Status: installed");
            if let Some(theme) = theme_id {
                println!("Theme: {theme}");
            }
            if let Some(state) = state {
                println!("Install ID: {}", state.install_id);
                println!("Installed version: {}", state.installed_version);
            }
        } else {
            println!("Status: not installed (run `winzsh install`)");
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_install(paths: &WinzshPaths, force: bool, json: bool) -> Result<ExitCode, Error> {
    let env = prepare_install_environment(json)?;
    let report = installer::install_with_detection(
        paths,
        InstallOptions {
            force,
            ..InstallOptions::default()
        },
        &env,
    )?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| Error::Message(e.to_string()))?
        );
    } else {
        println!("WinZSH installed successfully.");
        for step in &report.steps {
            println!("  - {step}");
        }
        if let Some(profile) = &report.profile_path {
            println!("Profile: {profile}");
        }
        println!("Restart PowerShell (or open a new tab) to load WinZSH.");
    }
    Ok(ExitCode::SUCCESS)
}

fn prepare_install_environment(json: bool) -> Result<DetectionReport, Error> {
    let mut env = detect_environment()?;

    if env.has_pwsh() {
        return Ok(env);
    }

    if !env.has_windows_powershell() {
        return Err(winzsh_error::detect(
            "No PowerShell host found on PATH; install PowerShell 7 from https://aka.ms/powershell",
        ));
    }

    if json || !io::stdin().is_terminal() {
        return Ok(env);
    }

    eprintln!(
        "Windows PowerShell detected at {}.",
        env.windows_powershell
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "powershell".into())
    );

    if confirm_yes_default(
        "PowerShell 7 is recommended for the best WinZSH experience. Install it now? (Y/n)",
    )? {
        match try_install_powershell7() {
            Ok(()) => {
                env = detect_environment()?;
                if let Some(pwsh) = &env.pwsh {
                    eprintln!("PowerShell 7 is ready at {}.", pwsh.display());
                } else {
                    eprintln!(
                        "PowerShell 7 may need a new terminal session for PATH updates; continuing with Windows PowerShell."
                    );
                }
            }
            Err(err) => {
                eprintln!("{err}");
                eprintln!("Continuing with Windows PowerShell.");
            }
        }
    } else {
        eprintln!("Continuing with Windows PowerShell.");
    }

    Ok(env)
}

fn confirm_yes_default(prompt: &str) -> Result<bool, Error> {
    eprint!("{prompt} ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|source| winzsh_error::io("<stdin>", source))?;
    let answer = line.trim();
    if answer.is_empty() {
        return Ok(true);
    }
    Ok(matches!(answer.to_ascii_lowercase().as_str(), "y" | "yes"))
}

fn try_install_powershell7() -> Result<(), Error> {
    eprintln!("Installing PowerShell 7 via winget…");
    let status = Command::new("winget")
        .args([
            "install",
            "--id",
            "Microsoft.PowerShell",
            "--source",
            "winget",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ])
        .stdin(Stdio::null())
        .status()
        .map_err(|source| {
            winzsh_error::message(format!(
                "Could not run winget ({source}). Install PowerShell 7 from https://aka.ms/powershell"
            ))
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(winzsh_error::message(format!(
            "winget exited with {status}. Install PowerShell 7 from https://aka.ms/powershell"
        )))
    }
}

fn cmd_uninstall(paths: &WinzshPaths, purge: bool, json: bool) -> Result<ExitCode, Error> {
    let report = installer::uninstall(
        paths,
        UninstallOptions {
            purge,
            profile_path: None,
        },
    )?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| Error::Message(e.to_string()))?
        );
    } else {
        println!("WinZSH uninstalled.");
        for step in &report.steps {
            println!("  - {step}");
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_doctor(paths: &WinzshPaths, json: bool) -> Result<ExitCode, Error> {
    let report = doctor::run(paths);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| Error::Message(e.to_string()))?
        );
    } else {
        println!("WinZSH doctor");
        for d in &report.diagnostics {
            let tag = match d.severity {
                Severity::Info => "INFO",
                Severity::Warning => "WARN",
                Severity::Error => "ERROR",
            };
            println!("[{tag}] {}: {}", d.code, d.message);
            if let Some(hint) = &d.hint {
                println!("       hint: {hint}");
            }
        }
        if report.ok {
            println!("All critical checks passed.");
        } else {
            println!("Problems detected. See hints above.");
        }
    }
    Ok(if report.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn cmd_reload(paths: &WinzshPaths, json: bool) -> Result<ExitCode, Error> {
    require_installed(paths)?;
    let report = runtime_gen::regenerate_from_disk(paths)?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "wrote": report.wrote, "hash": report.lock.input_hash })
        );
    } else if report.wrote {
        println!("Runtime regenerated. Open a new PowerShell tab (or `Import-Module` again).");
    } else {
        println!("Runtime already up to date.");
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_config(paths: &WinzshPaths, sub: ConfigCommands, json: bool) -> Result<ExitCode, Error> {
    match sub {
        ConfigCommands::Path => {
            if json {
                println!("{}", serde_json::json!({ "path": paths.config_file() }));
            } else {
                println!("{}", paths.config_file().display());
            }
            Ok(ExitCode::SUCCESS)
        }
        ConfigCommands::Show => {
            let cfg = config::load(paths)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&cfg)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                let rendered =
                    toml::to_string_pretty(&cfg).map_err(|e| Error::Message(e.to_string()))?;
                print!("{rendered}");
            }
            Ok(ExitCode::SUCCESS)
        }
        ConfigCommands::Validate => {
            let cfg = config::load(paths)?;
            config::validate(&cfg)?;
            theme::validate_id(&cfg.theme)?;
            if json {
                println!("{{\"ok\":true}}");
            } else {
                println!("config OK");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn cmd_theme(paths: &WinzshPaths, sub: ThemeCommands, json: bool) -> Result<ExitCode, Error> {
    match sub {
        ThemeCommands::List => {
            let themes = theme::builtin_themes();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&themes)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                for t in themes {
                    println!("{:<14} {}", t.id, t.name);
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        ThemeCommands::Show { id } => {
            let id = match id {
                Some(id) => id,
                None => {
                    require_installed(paths)?;
                    config::load(paths)?.theme
                }
            };
            let resolved = theme::resolve(&id)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&resolved.theme)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!("{} ({})", resolved.theme.name, resolved.theme.id);
                println!("prompt char: {}", resolved.theme.symbols.prompt);
            }
            Ok(ExitCode::SUCCESS)
        }
        ThemeCommands::Set { id } => {
            require_installed(paths)?;
            theme::validate_id(&id)?;
            let mut cfg = config::load(paths)?;
            cfg.theme = id.clone();
            config::save(paths, &cfg)?;
            let report = runtime_gen::generate(paths, &cfg)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "theme": id, "runtime_wrote": report.wrote })
                );
            } else {
                println!("Theme set to '{id}'.");
                println!("Open a new PowerShell tab to see the prompt.");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn cmd_alias(paths: &WinzshPaths, sub: AliasCommands, json: bool) -> Result<ExitCode, Error> {
    match sub {
        AliasCommands::List => {
            require_installed(paths)?;
            let cfg = config::load(paths)?;
            let detected = detect_environment().unwrap_or_default();
            let active_plugins = plugin::resolve_active(paths, &cfg.plugins.enabled, &detected)?;
            let plugin_aliases = alias::from_plugin_map(&plugin::collect_aliases(&active_plugins))?;
            let user = alias::from_user_map(&cfg.aliases)?;
            let set = alias::merge(alias::builtin_aliases(), plugin_aliases, user);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&set.aliases)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                for a in set.aliases.values() {
                    println!("{:<8} = {}  [{:?}]", a.name, a.value, a.source);
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        AliasCommands::Set { name, value } => {
            require_installed(paths)?;
            let expansion = value.join(" ");
            let mut cfg = config::load(paths)?;
            cfg.aliases.insert(name.clone(), expansion.clone());
            let _ = alias::from_user_map(&cfg.aliases)?;
            config::save(paths, &cfg)?;
            runtime_gen::generate(paths, &cfg)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "name": name, "value": expansion })
                );
            } else {
                println!("Alias set: {name} = {expansion}");
                println!("Open a new PowerShell tab (or run `winzsh reload`) to apply.");
            }
            Ok(ExitCode::SUCCESS)
        }
        AliasCommands::Remove { name } => {
            require_installed(paths)?;
            let mut cfg = config::load(paths)?;
            if cfg.aliases.remove(&name).is_none() {
                return Err(winzsh_error::message(format!(
                    "no user alias named '{name}'"
                )));
            }
            config::save(paths, &cfg)?;
            runtime_gen::generate(paths, &cfg)?;
            if json {
                println!("{}", serde_json::json!({ "removed": name }));
            } else {
                println!("Removed user alias '{name}'.");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn cmd_history(paths: &WinzshPaths, sub: HistoryCommands, json: bool) -> Result<ExitCode, Error> {
    require_installed(paths)?;
    match sub {
        HistoryCommands::List { limit, contains } => {
            let items = history::query(paths, &HistoryQuery { limit, contains })?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&items)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else if items.is_empty() {
                println!("No history entries yet.");
            } else {
                for (idx, entry) in items.iter().enumerate() {
                    println!("{:>4}  {}  {}", idx + 1, entry.timestamp, entry.command);
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        HistoryCommands::Compact => {
            let cfg = config::load(paths)?;
            let n = history::compact(paths, cfg.history.max_entries)?;
            if json {
                println!("{}", serde_json::json!({ "entries": n }));
            } else {
                println!("Compacted history store ({n} entries).");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn cmd_completion(
    paths: &WinzshPaths,
    sub: CompletionCommands,
    json: bool,
) -> Result<ExitCode, Error> {
    match sub {
        CompletionCommands::List => {
            let detected = detect_environment().unwrap_or_default();
            let policy = match config::load(paths) {
                Ok(cfg) => CompletionPolicy {
                    enabled: cfg.completions.enabled,
                    only: cfg.completions.only.clone(),
                },
                Err(_) => CompletionPolicy::default(),
            };
            let packs = completion::builtin_packs();
            if json {
                let rows: Vec<_> = packs
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "id": p.id,
                            "command": p.command,
                            "strategy": p.strategy,
                            "active": completion::pack_enabled(p, &detected, &policy),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&rows)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                if !policy.enabled {
                    println!("Completions disabled in config ([completions] enabled=false).");
                }
                println!("{:<12} {:<10} {:<8} STRATEGY", "ID", "COMMAND", "ACTIVE");
                for p in &packs {
                    let active = if completion::pack_enabled(p, &detected, &policy) {
                        "yes"
                    } else {
                        "no"
                    };
                    let strategy = match p.strategy {
                        completion::CompletionStrategy::Builtin => "builtin",
                        completion::CompletionStrategy::NativeGenerate => "native",
                        completion::CompletionStrategy::SshHosts => "ssh_hosts",
                    };
                    println!("{:<12} {:<10} {:<8} {}", p.id, p.command, active, strategy);
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn cmd_plugin(paths: &WinzshPaths, sub: PluginCommands, json: bool) -> Result<ExitCode, Error> {
    match sub {
        PluginCommands::List => {
            let detected = detect_environment().unwrap_or_default();
            let enabled = config::load(paths)
                .map(|c| c.plugins.enabled)
                .unwrap_or_default();
            let rows = plugin::list_entries(paths, &enabled, &detected)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&rows)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!(
                    "{:<14} {:<8} {:<9} {:<8} {:<10} DESCRIPTION",
                    "ID", "INSTALLED", "ENABLED", "CMDS_OK", "SOURCE"
                );
                for r in rows {
                    let source = plugin::read_origin_source(paths, &r.id).unwrap_or_else(|| {
                        if r.first_party {
                            "builtin".into()
                        } else {
                            "local".into()
                        }
                    });
                    println!(
                        "{:<14} {:<8} {:<9} {:<8} {:<10} {}",
                        r.id,
                        if r.installed { "yes" } else { "no" },
                        if r.enabled { "yes" } else { "no" },
                        if r.commands_ok { "yes" } else { "no" },
                        source,
                        r.description
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        PluginCommands::Search { query } => {
            require_installed(paths)?;
            let cfg = config::load(paths)?;
            let loaded = registry::fetch_index(paths, &cfg.registry)?;
            let hits = registry::search(&loaded.index, &query);
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "source": loaded.source,
                        "count": hits.len(),
                        "plugins": hits,
                    })
                );
            } else {
                println!("Registry ({}): {} plugin(s)", loaded.source, hits.len());
                println!("{:<14} {:<8} {:<18} DESCRIPTION", "ID", "VERSION", "AUTHOR");
                for p in hits {
                    println!(
                        "{:<14} {:<8} {:<18} {}",
                        p.id, p.version, p.author, p.description
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        PluginCommands::Info { id } => {
            require_installed(paths)?;
            let cfg = config::load(paths)?;
            let loaded = registry::fetch_index(paths, &cfg.registry)?;
            let Some(entry) = registry::find(&loaded.index, &id) else {
                return Err(Error::Message(format!(
                    "plugin '{id}' not in registry (source={})",
                    loaded.source
                )));
            };
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(entry)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!("{} v{}", entry.id, entry.version);
                println!("  {}", entry.description);
                println!("  author:    {}", entry.author);
                println!("  tags:      {}", entry.tags.join(", "));
                println!("  sha256:    {}", entry.sha256);
                println!(
                    "  signature: {}",
                    entry
                        .signature
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or("(none)")
                );
                if !entry.homepage.is_empty() {
                    println!("  homepage:  {}", entry.homepage);
                }
                println!("  download:  {}", entry.download_url);
                if let Ok(installed) = plugin::load(paths, &id) {
                    println!(
                        "  installed: yes (v{}, origin={})",
                        installed.manifest.version,
                        plugin::read_origin_source(paths, &id).unwrap_or_else(|| "?".into())
                    );
                } else {
                    println!("  installed: no");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        PluginCommands::Add { name_or_path } => {
            require_installed(paths)?;
            let path = std::path::Path::new(&name_or_path);
            let mut cfg = config::load(paths)?;
            let manifest = if path.is_dir() || name_or_path.contains(['/', '\\']) {
                eprintln!(
                    "Warning: installing from local path (not signature-verified): {name_or_path}"
                );
                plugin::add_from_path(paths, path)?
            } else if plugin::first_party_ids().contains(&name_or_path.as_str()) {
                plugin::add_first_party(paths, &name_or_path)?
            } else {
                let loaded = registry::fetch_index(paths, &cfg.registry)?;
                registry::install(paths, &cfg.registry, &loaded, &name_or_path, false)?
            };
            if !cfg.plugins.enabled.iter().any(|e| e == &manifest.name) {
                cfg.plugins.enabled.push(manifest.name.clone());
            }
            config::save(paths, &cfg)?;
            let report = runtime_gen::generate(paths, &cfg)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": manifest.name,
                        "version": manifest.version,
                        "runtime_wrote": report.wrote,
                    })
                );
            } else {
                println!(
                    "Installed and enabled plugin '{}' v{}.",
                    manifest.name, manifest.version
                );
                println!("Open a new PowerShell tab to load plugin aliases/hooks.");
            }
            Ok(ExitCode::SUCCESS)
        }
        PluginCommands::Update { id } => {
            require_installed(paths)?;
            let mut cfg = config::load(paths)?;
            let loaded = registry::fetch_index(paths, &cfg.registry)?;
            let updated = registry::update(paths, &cfg.registry, &loaded, id.as_deref())?;
            for m in &updated {
                if !cfg.plugins.enabled.iter().any(|e| e == &m.name) {
                    cfg.plugins.enabled.push(m.name.clone());
                }
            }
            if !updated.is_empty() {
                config::save(paths, &cfg)?;
                runtime_gen::generate(paths, &cfg)?;
            }
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "updated": updated.iter().map(|m| &m.name).collect::<Vec<_>>(),
                        "count": updated.len(),
                    })
                );
            } else if updated.is_empty() {
                println!("No registry plugin updates applied.");
            } else {
                println!("Updated {} plugin(s):", updated.len());
                for m in updated {
                    println!("  - {} v{}", m.name, m.version);
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        PluginCommands::Remove { id } => {
            require_installed(paths)?;
            plugin::remove(paths, &id)?;
            let mut cfg = config::load(paths)?;
            cfg.plugins.enabled.retain(|e| e != &id);
            config::save(paths, &cfg)?;
            runtime_gen::generate(paths, &cfg)?;
            if json {
                println!("{}", serde_json::json!({ "removed": id }));
            } else {
                println!("Removed plugin '{id}'.");
            }
            Ok(ExitCode::SUCCESS)
        }
        PluginCommands::Enable { id } => {
            require_installed(paths)?;
            let _ = plugin::load(paths, &id)?;
            let mut cfg = config::load(paths)?;
            if !cfg.plugins.enabled.iter().any(|e| e == &id) {
                cfg.plugins.enabled.push(id.clone());
            }
            config::save(paths, &cfg)?;
            runtime_gen::generate(paths, &cfg)?;
            if json {
                println!("{}", serde_json::json!({ "enabled": id }));
            } else {
                println!("Enabled plugin '{id}'. Open a new PowerShell tab to apply.");
            }
            Ok(ExitCode::SUCCESS)
        }
        PluginCommands::Disable { id } => {
            require_installed(paths)?;
            let mut cfg = config::load(paths)?;
            let before = cfg.plugins.enabled.len();
            cfg.plugins.enabled.retain(|e| e != &id);
            if cfg.plugins.enabled.len() == before {
                return Err(winzsh_error::message(format!(
                    "plugin '{id}' is not in the enabled list"
                )));
            }
            config::save(paths, &cfg)?;
            runtime_gen::generate(paths, &cfg)?;
            if json {
                println!("{}", serde_json::json!({ "disabled": id }));
            } else {
                println!("Disabled plugin '{id}' (files kept).");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn cmd_sync(paths: &WinzshPaths, sub: SyncCommands, json: bool) -> Result<ExitCode, Error> {
    require_installed(paths)?;
    let cfg = config::load(paths)?;

    match sub {
        SyncCommands::Status => {
            let report = sync::status(paths, &cfg.sync);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!(
                    "Sync destination: {}",
                    blank_or(&report.destination, "(not set)")
                );
                println!(
                    "Defaults: include_plugins={} include_history={}",
                    report.include_plugins, report.include_history
                );
                println!(
                    "Last export: {}",
                    blank_or(&report.state.last_export_at, "never")
                );
                println!(
                    "Last import: {}",
                    blank_or(&report.state.last_import_at, "never")
                );
                if !report.state.last_destination.is_empty() {
                    println!("Last path/url: {}", report.state.last_destination);
                }
                if !report.state.last_sha256.is_empty() {
                    println!("Last sha256:  {}", report.state.last_sha256);
                }
                println!();
                println!(
                    "Tip: set [sync].destination to an OneDrive/USB path, then `winzsh sync push` / `pull`."
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        SyncCommands::Export {
            path,
            plugins,
            history,
        } => {
            let mut opts = ExportOptions::from_config(&cfg.sync);
            if plugins {
                opts.include_plugins = true;
            }
            if history {
                opts.include_history = true;
            }
            let dest = path.unwrap_or_else(|| PathBuf::from("winzsh-sync.json"));
            let report = sync::export_to_path(paths, &dest, &opts)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!("Exported sync bundle → {}", report.destination);
                println!("  sha256: {}", report.sha256);
                if report.included_plugins {
                    println!("  plugins: included");
                }
                if report.history_count > 0 {
                    println!("  history: {} entries", report.history_count);
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        SyncCommands::Import { path, force } => {
            let source = path
                .or_else(|| {
                    let d = cfg.sync.destination.trim();
                    if d.is_empty() {
                        None
                    } else {
                        Some(d.to_string())
                    }
                })
                .ok_or_else(|| {
                    Error::Message(
                        "pass --path or set [sync].destination / WINZSH_SYNC_DEST".into(),
                    )
                })?;
            let report = sync::import_from(
                paths,
                &source,
                &ImportOptions {
                    force,
                    prefer_bundled_plugins: true,
                },
            )?;
            let cfg_after = config::load(paths)?;
            runtime_gen::generate(paths, &cfg_after)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!("Imported sync bundle from {}", report.source);
                println!("  theme: {}", report.theme);
                println!("  plugins enabled: {}", report.enabled_plugins.join(", "));
                for step in &report.steps {
                    println!("  - {step}");
                }
                println!("Open a new PowerShell tab (or `winzsh reload`) to apply.");
            }
            Ok(ExitCode::SUCCESS)
        }
        SyncCommands::Push { plugins, history } => {
            let mut opts = ExportOptions::from_config(&cfg.sync);
            if plugins {
                opts.include_plugins = true;
            }
            if history {
                opts.include_history = true;
            }
            let report = sync::push(paths, &cfg.sync, &opts)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!("Pushed sync bundle → {}", report.destination);
                println!("  sha256: {}", report.sha256);
            }
            Ok(ExitCode::SUCCESS)
        }
        SyncCommands::Pull { force } => {
            let report = sync::pull(
                paths,
                &cfg.sync,
                &ImportOptions {
                    force,
                    prefer_bundled_plugins: true,
                },
            )?;
            let cfg_after = config::load(paths)?;
            runtime_gen::generate(paths, &cfg_after)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!("Pulled sync bundle from {}", report.source);
                println!("  theme: {}", report.theme);
                for step in &report.steps {
                    println!("  - {step}");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn cmd_shell(paths: &WinzshPaths, sub: ShellCommands, json: bool) -> Result<ExitCode, Error> {
    require_installed(paths)?;
    let mut cfg = config::load(paths)?;
    let detected = detect_environment()?;

    match sub {
        ShellCommands::List | ShellCommands::Status => {
            let entries = build_shell_catalog(paths, &detected, &cfg)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&entries)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!(
                    "{:<12} {:<8} {:<8} {:<10} STATUS",
                    "SHELL", "DETECT", "HOOKED", "ENABLED"
                );
                for e in &entries {
                    println!(
                        "{:<12} {:<8} {:<8} {:<10} {}",
                        e.id,
                        if e.detected { "yes" } else { "no" },
                        if e.hooked { "yes" } else { "no" },
                        if e.enabled { "yes" } else { "no" },
                        e.status
                    );
                    if let Some(p) = &e.path {
                        println!("             {p}");
                    }
                }
                println!();
                println!(
                    "Tip: winzsh shell enable cmd|nu|bash  (PowerShell is managed by install)"
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        ShellCommands::Enable { id } => {
            let shell_id = parse_shell_id(&id)?;
            if shell_id == ShellId::PowerShell {
                let host = powershell_host(paths, &detected)?;
                host.install_hook()?;
                if json {
                    println!(
                        "{}",
                        serde_json::json!({ "id": "powershell", "enabled": true, "hooked": true })
                    );
                } else {
                    println!(
                        "PowerShell is the primary host (managed by `winzsh install`). Hook refreshed."
                    );
                }
                return Ok(ExitCode::SUCCESS);
            }
            let host = host_for(paths, shell_id)?;
            host.install_hook()?;
            set_shell_enabled(&mut cfg, shell_id, true);
            config::save(paths, &cfg)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": shell_id.as_str(),
                        "enabled": true,
                        "hooked": host.hook_installed()?,
                        "experimental": host.capabilities().experimental,
                    })
                );
            } else {
                println!(
                    "Enabled {} hook → {}",
                    shell_id,
                    host.profile_path()?.display()
                );
                if host.capabilities().experimental {
                    println!(
                        "Note: this bridge is experimental; full runtime still runs in PowerShell via zsh-for-win."
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        ShellCommands::Disable { id } => {
            let shell_id = parse_shell_id(&id)?;
            if shell_id == ShellId::PowerShell {
                return Err(Error::Message(
                    "Cannot disable PowerShell via shell disable; use `winzsh uninstall`".into(),
                ));
            }
            let host = host_for(paths, shell_id)?;
            host.remove_hook()?;
            set_shell_enabled(&mut cfg, shell_id, false);
            config::save(paths, &cfg)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "id": shell_id.as_str(), "enabled": false, "hooked": false })
                );
            } else {
                println!("Disabled {} integration.", shell_id);
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn cmd_agent(paths: &WinzshPaths, sub: AgentCommands, json: bool) -> Result<ExitCode, Error> {
    require_installed(paths)?;
    let cfg = config::load(paths)?;

    match sub {
        AgentCommands::Status => {
            let report = agent::status(paths, &cfg.agent)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!(
                    "Agent config: enabled={} interval={}s",
                    report.config_enabled, report.interval_secs
                );
                println!(
                    "Running: {}",
                    if report.running {
                        format!("yes (pid {})", report.pid.unwrap_or(0))
                    } else {
                        "no".into()
                    }
                );
                if let Some(hb) = &report.heartbeat {
                    println!("Last tick: {}", blank_or(&hb.last_tick_at, "never"));
                    if let Some(n) = hb.last_tick.history_entries {
                        println!("  history entries: {n}");
                    }
                    if let Some(s) = &hb.last_tick.registry_source {
                        println!("  registry: {s}");
                    }
                    if let Some(u) = hb.last_tick.update_available {
                        println!("  update available: {u}");
                    }
                }
                for note in &report.notes {
                    println!("Note: {note}");
                }
                if !report.running {
                    println!("Tip: winzsh agent start");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        AgentCommands::Start => {
            let pid = agent::start(paths, &cfg.agent)?;
            if json {
                println!("{}", serde_json::json!({ "started": true, "pid": pid }));
            } else {
                println!("Agent started (pid {pid}).");
                println!("Status: winzsh agent status");
            }
            Ok(ExitCode::SUCCESS)
        }
        AgentCommands::Stop => {
            let stopped = agent::stop(paths)?;
            if json {
                println!("{}", serde_json::json!({ "stopped": stopped }));
            } else if stopped {
                println!("Agent stopped.");
            } else {
                println!("Agent was not running.");
            }
            Ok(ExitCode::SUCCESS)
        }
        AgentCommands::RunOnce => {
            let report = agent::tick(paths, &cfg)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!("Agent tick complete.");
                if let Some(n) = report.history_entries {
                    println!("  history entries: {n}");
                }
                if let Some(s) = report.registry_source {
                    println!("  registry: {s}");
                }
                if let Some(u) = report.update_available {
                    println!("  update available: {u}");
                }
                for note in &report.notes {
                    println!("  note: {note}");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        AgentCommands::Run => {
            if !json {
                eprintln!(
                    "WinZSH agent running (interval {}s). Stop with: winzsh agent stop",
                    cfg.agent.interval_secs
                );
            }
            agent::run_loop(paths, &cfg)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn build_shell_catalog(
    paths: &WinzshPaths,
    detected: &DetectionReport,
    cfg: &Config,
) -> Result<Vec<shell_host::ShellCatalogEntry>, Error> {
    let enabled = &cfg.shells.enabled;
    let mut out = Vec::new();

    let ps = powershell_host(paths, detected)?;
    out.push(catalog_entry(
        &ps,
        detected.has_powershell_host(),
        detected.preferred_shell().map(|p| p.display().to_string()),
        true,
    )?);

    let cmd = CmdHost::new(paths.clone());
    out.push(catalog_entry(
        &cmd,
        detected.cmd.is_some(),
        detected.cmd.as_ref().map(|p| p.display().to_string()),
        shell_config_enabled(enabled, ShellId::Cmd),
    )?);

    let nu = NuHost::new(paths.clone());
    out.push(catalog_entry(
        &nu,
        detected.nu.is_some(),
        detected.nu.as_ref().map(|p| p.display().to_string()),
        shell_config_enabled(enabled, ShellId::Nu),
    )?);

    let bash = BashHost::new(paths.clone());
    out.push(catalog_entry(
        &bash,
        detected.bash.is_some(),
        detected.bash.as_ref().map(|p| p.display().to_string()),
        shell_config_enabled(enabled, ShellId::Bash),
    )?);

    Ok(out)
}

fn powershell_host(
    paths: &WinzshPaths,
    detected: &DetectionReport,
) -> Result<PowerShellHost, Error> {
    let profile = detected
        .profile_path
        .clone()
        .ok_or_else(|| Error::Message("Could not resolve PowerShell profile path".into()))?;
    Ok(PowerShellHost::new(paths.clone(), profile))
}

fn host_for(paths: &WinzshPaths, id: ShellId) -> Result<Box<dyn ShellHost>, Error> {
    match id {
        ShellId::PowerShell => Err(Error::Message(
            "PowerShell is managed by install/uninstall, not shell enable/disable hosts".into(),
        )),
        ShellId::Cmd => Ok(Box::new(CmdHost::new(paths.clone()))),
        ShellId::Nu => Ok(Box::new(NuHost::new(paths.clone()))),
        ShellId::Bash => Ok(Box::new(BashHost::new(paths.clone()))),
    }
}

fn parse_shell_id(raw: &str) -> Result<ShellId, Error> {
    ShellId::parse(raw).ok_or_else(|| {
        Error::Message(format!(
            "Unknown shell `{raw}` (expected powershell|cmd|nu|bash)"
        ))
    })
}

fn shell_config_enabled(enabled: &[String], id: ShellId) -> bool {
    enabled.iter().any(|s| ShellId::parse(s) == Some(id))
}

fn set_shell_enabled(cfg: &mut Config, id: ShellId, on: bool) {
    let key = id.as_str().to_string();
    cfg.shells.enabled.retain(|s| ShellId::parse(s) != Some(id));
    if on {
        cfg.shells.enabled.push(key);
    }
}

fn blank_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn cmd_ai(paths: &WinzshPaths, sub: AiCommands, json: bool) -> Result<ExitCode, Error> {
    match sub {
        AiCommands::Status => {
            let cfg = config::load(paths).unwrap_or_else(|_| Config::default());
            let settings = ai_settings_from_config(&cfg);
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "enabled": settings.enabled,
                        "provider": "local",
                        "mode": "offline-heuristics",
                    })
                );
            } else {
                println!("AI enabled: {}", settings.enabled);
                println!("Provider: local (offline heuristics only)");
                if !settings.enabled {
                    println!("Tip: winzsh ai enable");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        AiCommands::Enable => {
            require_installed(paths)?;
            let mut cfg = config::load(paths)?;
            cfg.features.ai = true;
            cfg.ai.provider = "local".into();
            config::save(paths, &cfg)?;
            if json {
                println!("{{\"enabled\":true,\"provider\":\"local\"}}");
            } else {
                println!("AI enabled (features.ai=true, provider=local).");
                println!("Try: winzsh ai explain git status");
            }
            Ok(ExitCode::SUCCESS)
        }
        AiCommands::Disable => {
            require_installed(paths)?;
            let mut cfg = config::load(paths)?;
            cfg.features.ai = false;
            config::save(paths, &cfg)?;
            if json {
                println!("{{\"enabled\":false}}");
            } else {
                println!("AI disabled.");
            }
            Ok(ExitCode::SUCCESS)
        }
        AiCommands::Explain { command } => {
            require_installed(paths)?;
            let cfg = config::load(paths)?;
            let settings = ai_settings_from_config(&cfg);
            let joined = command.join(" ");
            let result = ai::explain(&settings, &joined)?;
            print_ai_text(&result, json)?;
            Ok(ExitCode::SUCCESS)
        }
        AiCommands::Ask { prompt } => {
            require_installed(paths)?;
            let cfg = config::load(paths)?;
            let settings = ai_settings_from_config(&cfg);
            let joined = prompt.join(" ");
            let result = ai::ask(&settings, &joined)?;
            print_ai_text(&result, json)?;
            Ok(ExitCode::SUCCESS)
        }
        AiCommands::Check { command } => {
            let joined = command.join(" ");
            let report = ai::check_safety(&joined);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!("Command: {}", report.command);
                println!("Level: {:?}", report.level);
                if report.findings.is_empty() {
                    println!("No dangerous patterns detected (heuristics only).");
                } else {
                    for f in &report.findings {
                        println!("- [{:?}] {}: {}", f.level, f.code, f.message);
                        if let Some(safer) = &f.safer {
                            println!("  safer: {safer}");
                        }
                    }
                }
            }
            Ok(if matches!(report.level, ai::SafetyLevel::Danger) {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            })
        }
        AiCommands::Alias { description } => {
            require_installed(paths)?;
            let cfg = config::load(paths)?;
            let settings = ai_settings_from_config(&cfg);
            let joined = description.join(" ");
            let result = ai::suggest_alias(&settings, &joined)?;
            print_ai_text(&result, json)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn cmd_update(
    paths: &WinzshPaths,
    check_only: bool,
    from_source: Option<String>,
    pull: bool,
    rollback: bool,
    json: bool,
) -> Result<ExitCode, Error> {
    require_installed(paths)?;
    let mut cfg = config::load(paths)?;

    if rollback {
        if check_only || from_source.is_some() {
            return Err(Error::Message(
                "--rollback cannot be combined with --check or --from-source".into(),
            ));
        }
        let report = update::rollback(paths)?;
        post_update_repair(paths)?;
        print_apply_report(&report, json)?;
        return Ok(ExitCode::SUCCESS);
    }

    if let Some(raw) = from_source {
        let explicit = {
            let t = raw.trim();
            if t.is_empty() {
                None
            } else {
                Some(PathBuf::from(t))
            }
        };
        let source_dir = update::resolve_source_dir(explicit, &cfg.update)?;
        if check_only {
            let payload = serde_json::json!({
                "mode": "from-source",
                "source_dir": source_dir,
                "pull": pull,
                "current_version": VERSION,
                "notes": ["--check with --from-source only resolves the checkout path"],
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .map_err(|e| Error::Message(e.to_string()))?
                );
            } else {
                println!("From-source update ready");
                println!("  Checkout: {}", source_dir.display());
                println!("  Current:  {VERSION}");
                if pull {
                    println!("  Will run: git pull --ff-only");
                }
                println!("Apply with: winzsh update --from-source");
            }
            return Ok(ExitCode::SUCCESS);
        }

        let report = update::apply_from_source(
            paths,
            &FromSourceOptions {
                source_dir: source_dir.clone(),
                pull,
            },
        )?;
        if cfg.update.source_dir.trim().is_empty() {
            cfg.update.source_dir = source_dir.display().to_string();
            config::save(paths, &cfg)?;
        }
        post_update_repair(paths)?;
        print_apply_report(&report, json)?;
        if !json {
            println!("Tip: open a new WinZSH session (`zsh-for-win`) so the new CLI is on PATH.");
        }
        return Ok(ExitCode::SUCCESS);
    }

    let report = update::check(paths, &cfg.update)?;
    if check_only || !report.update_available {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| Error::Message(e.to_string()))?
            );
        } else {
            println!("Current: {}", report.current_version);
            if let Some(latest) = &report.latest_version {
                println!("Latest:  {latest} ({})", report.source);
            } else {
                println!("Latest:  (none)");
            }
            println!(
                "Update available: {}",
                if report.update_available { "yes" } else { "no" }
            );
            for note in &report.notes {
                println!("  note: {note}");
            }
            if !report.update_available && report.source == "none" {
                println!();
                println!("Source installs: winzsh update --from-source [--pull]");
            }
        }
        return Ok(ExitCode::SUCCESS);
    }

    let applied = update::apply_github(paths, &cfg.update)?;
    post_update_repair(paths)?;
    print_apply_report(&applied, json)?;
    Ok(ExitCode::SUCCESS)
}

fn post_update_repair(paths: &WinzshPaths) -> Result<(), Error> {
    let env = detect_environment()?;
    let _ = installer::install_with_detection(
        paths,
        InstallOptions {
            force: true,
            ..InstallOptions::default()
        },
        &env,
    )?;
    Ok(())
}

fn print_apply_report(report: &update::ApplyReport, json: bool) -> Result<(), Error> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(report).map_err(|e| Error::Message(e.to_string()))?
        );
    } else {
        println!(
            "Updated {} → {} ({})",
            report.previous_version, report.installed_version, report.method
        );
        println!("Binary: {}", report.binary);
        for step in &report.steps {
            println!("  - {step}");
        }
        println!("Runtime refreshed via install --force.");
    }
    Ok(())
}

fn ai_settings_from_config(cfg: &Config) -> AiSettings {
    AiSettings {
        enabled: cfg.features.ai,
        provider: AiProvider::Local,
    }
}

fn print_ai_text(result: &ai::AiTextResult, json: bool) -> Result<(), Error> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(result).map_err(|e| Error::Message(e.to_string()))?
        );
    } else {
        println!("{}", result.text);
        for note in &result.notes {
            eprintln!("note: {note}");
        }
        eprintln!("provider: {}", result.provider);
    }
    Ok(())
}

fn require_installed(paths: &WinzshPaths) -> Result<(), Error> {
    if paths.is_installed() {
        Ok(())
    } else {
        Err(Error::NotInstalled)
    }
}

#[derive(Debug, serde::Serialize)]
struct StateView {
    install_id: String,
    installed_version: String,
}

impl StateView {
    fn from_paths(paths: &WinzshPaths) -> Option<Self> {
        winzsh_core::State::load(paths).ok().map(|s| Self {
            install_id: s.install_id,
            installed_version: s.installed_version,
        })
    }
}
