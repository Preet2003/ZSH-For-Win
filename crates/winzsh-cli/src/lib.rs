//! CLI command tree, output modes, and exit-code mapping.
//!
//! Business logic lives in sibling crates; this crate stays thin.

#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::{Command, ExitCode, Stdio};
use tracing::error;
use winzsh_alias::{self as alias};
use winzsh_completion::{self as completion, CompletionPolicy};
use winzsh_config::{self as config};
use winzsh_core::{VERSION, WinzshPaths};
use winzsh_detect::{DetectionReport, detect_environment};
use winzsh_plugin::{self as plugin};
use winzsh_doctor::{self as doctor, Severity};
use winzsh_error::Error;
use winzsh_history::{self as history, HistoryQuery};
use winzsh_installer::{self as installer, InstallOptions, UninstallOptions};
use winzsh_log::LogOptions;
use winzsh_runtime_gen::{self as runtime_gen};
use winzsh_theme::{self as theme};

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
enum PluginCommands {
    /// List first-party and installed plugins.
    List,
    /// Install a first-party plugin (or local path) and enable it.
    Add {
        /// First-party id (docker, git, node, rust) or path to a plugin folder.
        name_or_path: String,
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
    match run_inner() {
        Ok(code) => code,
        Err(err) => {
            let code = err.exit_code();
            error!("{err}");
            eprintln!("{:?}", miette::Report::new(err));
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

    match cli.command.unwrap_or(Commands::Status) {
        Commands::Status => cmd_status(&paths, cli.json),
        Commands::Install { force } => cmd_install(&paths, force, cli.json),
        Commands::Uninstall { purge } => cmd_uninstall(&paths, purge, cli.json),
        Commands::Doctor => cmd_doctor(&paths, cli.json),
        Commands::Reload => cmd_reload(&paths, cli.json),
        Commands::Config(sub) => cmd_config(&paths, sub, cli.json),
        Commands::Theme(sub) => cmd_theme(&paths, sub, cli.json),
        Commands::Alias(sub) => cmd_alias(&paths, sub, cli.json),
        Commands::History(sub) => cmd_history(&paths, sub, cli.json),
        Commands::Completion(sub) => cmd_completion(&paths, sub, cli.json),
        Commands::Plugin(sub) => cmd_plugin(&paths, sub, cli.json),
    }
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
            "phase": "phase-5",
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
            let active_plugins =
                plugin::resolve_active(paths, &cfg.plugins.enabled, &detected)?;
            let plugin_aliases =
                alias::from_plugin_map(&plugin::collect_aliases(&active_plugins))?;
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
                    println!(
                        "{:<12} {:<10} {:<8} {}",
                        p.id, p.command, active, strategy
                    );
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
                    "{:<10} {:<8} {:<9} {:<8} {:<8} DESCRIPTION",
                    "ID", "INSTALLED", "ENABLED", "CMDS_OK", "SOURCE"
                );
                for r in rows {
                    println!(
                        "{:<10} {:<8} {:<9} {:<8} {:<8} {}",
                        r.id,
                        if r.installed { "yes" } else { "no" },
                        if r.enabled { "yes" } else { "no" },
                        if r.commands_ok { "yes" } else { "no" },
                        if r.first_party { "builtin" } else { "local" },
                        r.description
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        PluginCommands::Add { name_or_path } => {
            require_installed(paths)?;
            let path = std::path::Path::new(&name_or_path);
            let manifest = if path.is_dir() || name_or_path.contains(['/', '\\']) {
                eprintln!(
                    "Warning: installing from local path (not signature-verified): {name_or_path}"
                );
                plugin::add_from_path(paths, path)?
            } else {
                plugin::add_first_party(paths, &name_or_path)?
            };
            let mut cfg = config::load(paths)?;
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
