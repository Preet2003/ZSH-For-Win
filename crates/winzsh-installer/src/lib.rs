//! Detect, backup, install, uninstall, and verify WinZSH (idempotent).

#![forbid(unsafe_code)]

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use winzsh_config::{self as config, Config};
use winzsh_core::{State, VERSION, WinzshPaths};
use winzsh_detect::{DetectionReport, detect_environment};
use winzsh_doctor::{self as doctor, DoctorReport};
use winzsh_error::{Result, message};
use winzsh_fs::{atomic_write, ensure_dir, ensure_layout};
use winzsh_powershell::PowerShellHost;
use winzsh_runtime_gen::{self as runtime_gen, GenerateReport};
use winzsh_shell_host::ShellHost;

/// Options for [`install`].
#[derive(Debug, Clone)]
pub struct InstallOptions {
    /// Re-run install even if already installed.
    pub force: bool,
    /// Override profile path (tests / advanced).
    pub profile_path: Option<PathBuf>,
    /// Require a PowerShell host (`pwsh` or `powershell`) on PATH (disable in hermetic tests).
    pub require_powershell: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            force: false,
            profile_path: None,
            require_powershell: true,
        }
    }
}

/// Options for [`self_install`] (download-and-run Setup.exe flow).
#[derive(Debug, Clone)]
pub struct SelfInstallOptions {
    /// Theme id applied after install (default `modern`).
    pub theme: String,
    /// Skip theme mutation.
    pub skip_theme: bool,
}

impl Default for SelfInstallOptions {
    fn default() -> Self {
        Self {
            theme: "modern".into(),
            skip_theme: false,
        }
    }
}

/// Options for [`uninstall`].
#[derive(Debug, Clone, Default)]
pub struct UninstallOptions {
    /// Delete the entire `~/.winzsh` tree.
    pub purge: bool,
    /// Override profile path (tests / advanced).
    pub profile_path: Option<PathBuf>,
}

/// Outcome of an install/uninstall/verify operation.
#[derive(Debug, Clone, Default, Serialize)]
pub struct InstallReport {
    /// Human-readable steps performed or skipped.
    pub steps: Vec<String>,
    /// WinZSH home used.
    pub home: String,
    /// Profile path managed.
    pub profile_path: Option<String>,
    /// Whether runtime artifacts were rewritten.
    pub runtime_wrote: bool,
}

impl InstallReport {
    fn step(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        info!("{msg}");
        self.steps.push(msg);
    }
}

/// Full "download the .exe and run it" install:
/// copy this binary into `~/.winzsh/bin`, add user PATH, write launcher,
/// then profile hook + runtime (+ optional theme).
pub fn self_install(paths: &WinzshPaths, opts: SelfInstallOptions) -> Result<InstallReport> {
    let env = detect_environment()?;
    self_install_with_detection(paths, opts, &env)
}

/// [`self_install`] with injected detection (tests).
pub fn self_install_with_detection(
    paths: &WinzshPaths,
    opts: SelfInstallOptions,
    env: &DetectionReport,
) -> Result<InstallReport> {
    let mut report = InstallReport {
        home: paths.root.display().to_string(),
        ..InstallReport::default()
    };

    ensure_dir(&paths.bin_dir())?;
    let dest = paths.cli_binary();
    let src = std::env::current_exe().map_err(|e| message(format!("current_exe: {e}")))?;
    if same_file(&src, &dest) {
        report.step(format!("CLI already at {}", dest.display()));
    } else {
        install_cli_binary(&src, &dest, paths)?;
        report.step(format!("installed CLI to {}", dest.display()));
    }

    write_launcher(paths)?;
    report.step(format!(
        "wrote launcher {}",
        paths.bin_dir().join("zsh-for-win.cmd").display()
    ));

    if ensure_user_path(&paths.bin_dir())? {
        report.step(format!("added {} to user PATH", paths.bin_dir().display()));
    } else {
        report.step(format!(
            "user PATH already contains {}",
            paths.bin_dir().display()
        ));
    }

    let install_report = install_with_detection(
        paths,
        InstallOptions {
            force: true,
            require_powershell: true,
            ..InstallOptions::default()
        },
        env,
    )?;
    report.profile_path = install_report.profile_path;
    report.runtime_wrote = install_report.runtime_wrote;
    report.steps.extend(install_report.steps);

    if !opts.skip_theme {
        winzsh_theme::validate_id(&opts.theme)?;
        let mut cfg = config::load(paths)?;
        cfg.theme = opts.theme.clone();
        config::save(paths, &cfg)?;
        let runtime = regenerate_runtime(paths, &cfg)?;
        report.runtime_wrote = runtime.wrote || report.runtime_wrote;
        report.step(format!("theme set to {}", opts.theme));
    }

    report.step(format!("WinZSH {VERSION} ready"));
    Ok(report)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Copy/replace `~/.winzsh/bin/winzsh.exe` using a Windows-friendly rename dance
/// when the destination is locked (running agent / open handle).
fn install_cli_binary(src: &Path, dest: &Path, paths: &WinzshPaths) -> Result<()> {
    // Stage next to dest so rename stays on the same volume.
    let staged = paths
        .bin_dir()
        .join(format!("winzsh.setup-{}.tmp", std::process::id()));
    let _ = fs::remove_file(&staged);
    fs::copy(src, &staged).map_err(|source| {
        message(format!(
            "could not stage installer binary to {}: {source}\n\
             Tip: free disk space, then re-run Setup.",
            staged.display()
        ))
    })?;

    let bak = paths.cli_binary_backup();
    if dest.is_file() {
        let _ = fs::remove_file(&bak);
        if let Err(source) = fs::rename(dest, &bak) {
            let _ = fs::remove_file(&staged);
            return Err(message(format!(
                "could not replace {}: {source}\n\
                 The existing winzsh.exe is probably in use.\n\
                 Close other terminals, run `winzsh agent stop`, then re-run Setup.",
                dest.display()
            )));
        }
    }

    if let Err(source) = fs::rename(&staged, dest) {
        // Fall back to copy; restore backup if we moved one aside.
        let copy_err = fs::copy(&staged, dest);
        let _ = fs::remove_file(&staged);
        if copy_err.is_err() {
            if bak.is_file() {
                let _ = fs::rename(&bak, dest);
            }
            return Err(message(format!(
                "could not install CLI to {}: {source}\n\
                 Close other WinZSH processes (`winzsh agent stop`) and try again.\n\
                 Or from PowerShell:  Copy-Item .\\WinZSH-Setup-x86_64.exe $env:USERPROFILE\\.winzsh\\bin\\winzsh.exe -Force",
                dest.display()
            )));
        }
    }
    let _ = fs::remove_file(&staged);
    Ok(())
}

fn write_launcher(paths: &WinzshPaths) -> Result<()> {
    let launcher = paths.bin_dir().join("zsh-for-win.cmd");
    let contents = concat!(
        "@echo off\r\n",
        "REM Nested WinZSH session. Type \"exit\" to return.\r\n",
        "set WINZSH_SHELL=1\r\n",
        "where pwsh >nul 2>&1\r\n",
        "if errorlevel 1 (\r\n",
        "  powershell %*\r\n",
        "  exit /b %ERRORLEVEL%\r\n",
        ")\r\n",
        "pwsh %*\r\n",
    );
    atomic_write(&launcher, contents)?;
    Ok(())
}

/// Ensure `dir` is on the current user's persistent PATH. Returns true if modified.
pub fn ensure_user_path(dir: &Path) -> Result<bool> {
    #[cfg(windows)]
    {
        ensure_user_path_windows(dir)
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        Ok(false)
    }
}

#[cfg(windows)]
fn ensure_user_path_windows(dir: &Path) -> Result<bool> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};

    let dir_display = dir.display().to_string();
    let dir_canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .map_err(|e| message(format!("open HKCU\\Environment: {e}")))?;
    let current: String = env.get_value("Path").unwrap_or_default();
    let parts: Vec<&str> = current
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let already = parts.iter().any(|p| {
        let candidate = Path::new(p);
        candidate
            .canonicalize()
            .map(|c| c == dir_canon)
            .unwrap_or(false)
            || p.eq_ignore_ascii_case(&dir_display)
    });
    if already {
        return Ok(false);
    }
    let new_path = if current.trim().is_empty() {
        dir_display
    } else if current.ends_with(';') {
        format!("{current}{dir_display}")
    } else {
        format!("{current};{dir_display}")
    };
    env.set_value("Path", &new_path)
        .map_err(|e| message(format!("set user PATH: {e}")))?;
    Ok(true)
}

/// Install or repair WinZSH for the current user.
pub fn install(paths: &WinzshPaths, opts: InstallOptions) -> Result<InstallReport> {
    let env = detect_environment()?;
    install_with_detection(paths, opts, &env)
}

/// Install using a provided detection report (testable).
pub fn install_with_detection(
    paths: &WinzshPaths,
    opts: InstallOptions,
    env: &DetectionReport,
) -> Result<InstallReport> {
    let mut report = InstallReport {
        home: paths.root.display().to_string(),
        ..InstallReport::default()
    };

    if paths.is_installed() {
        report.step("existing installation detected; repairing");
    }

    if opts.require_powershell && !env.has_powershell_host() {
        return Err(winzsh_error::detect(
            "No PowerShell host found on PATH; install PowerShell 7 (recommended) or Windows PowerShell before running winzsh install",
        ));
    }
    if let Some(pwsh) = &env.pwsh {
        report.step(format!("detected PowerShell 7 at {}", pwsh.display()));
    } else if let Some(powershell) = &env.windows_powershell {
        report.step(format!(
            "detected Windows PowerShell at {} (PowerShell 7 recommended)",
            powershell.display()
        ));
    }

    let profile_path = opts
        .profile_path
        .clone()
        .or_else(|| env.profile_path.clone())
        .ok_or_else(|| winzsh_error::profile("could not resolve PowerShell profile path"))?;
    report.profile_path = Some(profile_path.display().to_string());

    ensure_layout(paths)?;
    report.step(format!(
        "ensured directory layout at {}",
        paths.root.display()
    ));

    let cfg = config::load_or_init(paths)?;
    report.step(format!("config ready at {}", paths.config_file().display()));

    let runtime = regenerate_runtime(paths, &cfg)?;
    report.runtime_wrote = runtime.wrote;
    report.step(if runtime.wrote {
        "generated runtime module".to_string()
    } else {
        "runtime module already up to date".to_string()
    });

    let host = PowerShellHost::new(paths.clone(), profile_path);
    if let Some(backup) = host.backup_profile()? {
        report.step(format!("backed up profile to {}", backup.display()));
    } else {
        report.step("no existing profile to backup");
    }
    host.install_hook()?;
    report.step("installed managed profile hook");

    write_state(paths, &cfg)?;
    report.step(format!("wrote state.json (version {VERSION})"));

    Ok(report)
}

/// Remove the managed hook and optionally purge WinZSH data.
pub fn uninstall(paths: &WinzshPaths, opts: UninstallOptions) -> Result<InstallReport> {
    let mut report = InstallReport {
        home: paths.root.display().to_string(),
        ..InstallReport::default()
    };

    if !paths.root.exists() && !paths.is_installed() {
        return Err(winzsh_error::Error::NotInstalled);
    }

    let profile_path = match opts.profile_path {
        Some(p) => Some(p),
        None => detect_environment()?.profile_path,
    };

    if let Some(profile_path) = profile_path {
        report.profile_path = Some(profile_path.display().to_string());
        let host = PowerShellHost::new(paths.clone(), profile_path);
        host.remove_hook()?;
        report.step("removed managed profile hook");
    } else {
        report.step("profile path unresolved; skipped hook removal");
    }

    if opts.purge {
        if paths.root.exists() {
            std::fs::remove_dir_all(&paths.root)
                .map_err(|source| winzsh_error::io(paths.root.clone(), source))?;
            report.step(format!("purged {}", paths.root.display()));
        }
    } else if paths.state_file().is_file() {
        std::fs::remove_file(paths.state_file())
            .map_err(|source| winzsh_error::io(paths.state_file(), source))?;
        report.step("removed state.json (user data retained; use --purge to delete all)");
    }

    Ok(report)
}

/// Verify the installation via doctor checks.
pub fn verify(paths: &WinzshPaths) -> DoctorReport {
    doctor::run(paths)
}

fn regenerate_runtime(paths: &WinzshPaths, cfg: &Config) -> Result<GenerateReport> {
    runtime_gen::generate(paths, cfg)
}

fn write_state(paths: &WinzshPaths, cfg: &Config) -> Result<()> {
    let state = if paths.state_file().is_file() {
        let mut existing = State::load(paths)?;
        existing.installed_version = VERSION.to_string();
        existing.config_schema_version = cfg.schema_version;
        existing.updated_at = State::new_install(cfg.schema_version).updated_at;
        existing
    } else {
        State::new_install(cfg.schema_version)
    };
    let json = state.to_json()?;
    atomic_write(&paths.state_file(), json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use winzsh_detect::DetectionReport;
    use winzsh_test_support::TempHome;

    #[test]
    fn install_uninstall_roundtrip() {
        let home = TempHome::new("install");
        // Keep the fake profile outside ~/.winzsh so --purge does not delete it.
        let profile = std::env::temp_dir().join(format!(
            "winzsh-profile-{}-{}.ps1",
            std::process::id(),
            home.root
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("x")
        ));
        let _ = std::fs::remove_file(&profile);
        let env = DetectionReport {
            pwsh: None,
            windows_powershell: None,
            git: None,
            windows_terminal: None,
            fzf: None,
            zoxide: None,
            profile_path: Some(profile.clone()),
            commands: vec![],
            ..DetectionReport::default()
        };
        let report = install_with_detection(
            &home.paths,
            InstallOptions {
                force: true,
                profile_path: Some(profile.clone()),
                require_powershell: false,
            },
            &env,
        )
        .expect("install");
        assert!(home.paths.is_installed());
        assert!(home.paths.runtime_module().is_file());
        assert!(profile.is_file());
        let contents = std::fs::read_to_string(&profile).expect("read profile");
        assert!(contents.contains("# >>> winzsh >>>"));
        assert!(!report.steps.is_empty());

        uninstall(
            &home.paths,
            UninstallOptions {
                purge: true,
                profile_path: Some(profile.clone()),
            },
        )
        .expect("uninstall");
        let contents = std::fs::read_to_string(&profile).expect("read profile");
        assert!(!contents.contains("# >>> winzsh >>>"));
        assert!(!home.paths.root.exists());
        let _ = std::fs::remove_file(&profile);
    }

    #[test]
    fn install_accepts_windows_powershell_only() {
        let home = TempHome::new("install-winps");
        let profile = std::env::temp_dir().join(format!(
            "winzsh-profile-winps-{}-{}.ps1",
            std::process::id(),
            home.root
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("x")
        ));
        let _ = std::fs::remove_file(&profile);
        let env = DetectionReport {
            pwsh: None,
            windows_powershell: Some(PathBuf::from(
                r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            )),
            git: None,
            windows_terminal: None,
            fzf: None,
            zoxide: None,
            profile_path: Some(profile.clone()),
            commands: vec!["powershell".into()],
            ..DetectionReport::default()
        };
        let report = install_with_detection(
            &home.paths,
            InstallOptions {
                force: true,
                profile_path: Some(profile.clone()),
                require_powershell: true,
            },
            &env,
        )
        .expect("install with Windows PowerShell");
        assert!(
            report
                .steps
                .iter()
                .any(|s| s.contains("Windows PowerShell"))
        );
        assert!(home.paths.is_installed());
        let _ = std::fs::remove_file(&profile);
        let _ = std::fs::remove_dir_all(&home.paths.root);
    }
}
