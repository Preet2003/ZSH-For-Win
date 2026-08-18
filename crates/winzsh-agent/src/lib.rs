//! Background maintenance agent (invoked as `winzsh agent …`, not a separate binary).

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use time::OffsetDateTime;
use tracing::{info, warn};
use winzsh_config::{AgentConfig, Config};
use winzsh_core::WinzshPaths;
use winzsh_error::{Result, message};
use winzsh_fs::{atomic_write, ensure_dir};
use winzsh_history as history;
use winzsh_registry as registry;
use winzsh_update as update;

/// One maintenance tick outcome.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TickReport {
    /// History entries retained after compact (when run).
    pub history_entries: Option<usize>,
    /// Registry refresh source label when run.
    pub registry_source: Option<String>,
    /// Whether an update appears available (when checked).
    pub update_available: Option<bool>,
    /// Notes / skipped steps.
    pub notes: Vec<String>,
}

/// Agent process heartbeat written under `locks/agent.heartbeat.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Heartbeat {
    /// Agent process id.
    pub pid: u32,
    /// RFC3339 start time.
    pub started_at: String,
    /// RFC3339 last tick time.
    pub last_tick_at: String,
    /// Last tick summary.
    pub last_tick: TickReport,
}

/// Status snapshot for `winzsh agent status`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentStatus {
    /// Whether `[agent].enabled` is true.
    pub config_enabled: bool,
    /// Configured interval.
    pub interval_secs: u64,
    /// PID from pid file when present.
    pub pid: Option<u32>,
    /// Whether that PID appears alive.
    pub running: bool,
    /// Heartbeat when present.
    pub heartbeat: Option<Heartbeat>,
    /// Human notes.
    pub notes: Vec<String>,
}

/// Run a single maintenance tick.
pub fn tick(paths: &WinzshPaths, cfg: &Config) -> Result<TickReport> {
    let agent = &cfg.agent;
    let mut report = TickReport::default();

    if agent.compact_history {
        match history::compact(paths, cfg.history.max_entries) {
            Ok(n) => {
                report.history_entries = Some(n);
                info!(entries = n, "agent: history compacted");
            }
            Err(err) => {
                report
                    .notes
                    .push(format!("history compact failed: {err}"));
                warn!(error = %err, "agent: history compact failed");
            }
        }
    } else {
        report.notes.push("history compact skipped (disabled)".into());
    }

    if agent.refresh_registry {
        match registry::fetch_index(paths, &cfg.registry) {
            Ok(loaded) => {
                report.registry_source = Some(loaded.source.clone());
                info!(source = %loaded.source, "agent: registry refreshed");
            }
            Err(err) => {
                report
                    .notes
                    .push(format!("registry refresh failed: {err}"));
                warn!(error = %err, "agent: registry refresh failed");
            }
        }
    } else {
        report
            .notes
            .push("registry refresh skipped (disabled)".into());
    }

    if agent.check_updates {
        match update::check(paths, &cfg.update) {
            Ok(check) => {
                report.update_available = Some(check.update_available);
                if check.update_available {
                    report.notes.push(format!(
                        "update available: {} → {}",
                        check.current_version,
                        check.latest_version.unwrap_or_default()
                    ));
                }
            }
            Err(err) => {
                report.notes.push(format!("update check failed: {err}"));
                warn!(error = %err, "agent: update check failed");
            }
        }
    }

    write_heartbeat(paths, &report)?;
    Ok(report)
}

/// Foreground agent loop until stop sentinel or process exit.
pub fn run_loop(paths: &WinzshPaths, cfg: &Config) -> Result<()> {
    if !cfg.agent.enabled {
        return Err(message(
            "Agent is disabled in config ([agent].enabled = false)",
        ));
    }

    ensure_dir(&paths.locks_dir())?;
    let pid = std::process::id();
    atomic_write(&paths.agent_pid_file(), format!("{pid}\n"))?;
    clear_stop_flag(paths)?;

    let started = now_rfc3339();
    info!(pid, interval = cfg.agent.interval_secs, "agent: loop starting");

    // Initial heartbeat before first sleep.
    let mut hb = Heartbeat {
        pid,
        started_at: started.clone(),
        last_tick_at: started,
        last_tick: TickReport::default(),
    };
    write_heartbeat_full(paths, &hb)?;

    loop {
        if stop_requested(paths) {
            info!("agent: stop requested");
            break;
        }

        match tick(paths, cfg) {
            Ok(report) => {
                hb.last_tick = report;
                hb.last_tick_at = now_rfc3339();
                write_heartbeat_full(paths, &hb)?;
            }
            Err(err) => {
                warn!(error = %err, "agent: tick failed");
                hb.last_tick.notes.push(format!("tick error: {err}"));
                hb.last_tick_at = now_rfc3339();
                let _ = write_heartbeat_full(paths, &hb);
            }
        }

        let interval = cfg.agent.interval_secs.max(30);
        let mut slept = 0u64;
        while slept < interval {
            if stop_requested(paths) {
                info!("agent: stop requested during sleep");
                cleanup_pid(paths, pid);
                return Ok(());
            }
            thread::sleep(Duration::from_secs(1));
            slept += 1;
        }
    }

    cleanup_pid(paths, pid);
    Ok(())
}

/// Start a detached `winzsh agent run` process.
pub fn start(paths: &WinzshPaths, cfg: &AgentConfig) -> Result<u32> {
    if !cfg.enabled {
        return Err(message(
            "Agent is disabled in config ([agent].enabled = false)",
        ));
    }

    let status = status(paths, cfg)?;
    if status.running {
        return Err(message(format!(
            "Agent already running (pid {})",
            status.pid.unwrap_or(0)
        )));
    }

    ensure_dir(&paths.locks_dir())?;
    clear_stop_flag(paths)?;

    let exe = std::env::current_exe().map_err(|source| winzsh_error::io("current_exe", source))?;

    let mut cmd = Command::new(&exe);
    cmd.args(["agent", "run"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
    }

    let child = cmd
        .spawn()
        .map_err(|source| winzsh_error::io(exe.display().to_string(), source))?;
    let pid = child.id();

    // Give the child a moment to write its pid file.
    thread::sleep(Duration::from_millis(400));
    if let Some(written) = read_pid(paths) {
        return Ok(written);
    }
    Ok(pid)
}

/// Request stop (sentinel) and force-kill if still alive.
pub fn stop(paths: &WinzshPaths) -> Result<bool> {
    ensure_dir(&paths.locks_dir())?;
    request_stop(paths)?;

    let Some(pid) = read_pid(paths) else {
        clear_stop_flag(paths)?;
        return Ok(false);
    };

    for _ in 0..20 {
        if !pid_alive(pid) {
            cleanup_pid(paths, pid);
            clear_stop_flag(paths)?;
            return Ok(true);
        }
        thread::sleep(Duration::from_millis(150));
    }

    force_kill(pid);
    thread::sleep(Duration::from_millis(200));
    cleanup_pid(paths, pid);
    clear_stop_flag(paths)?;
    Ok(true)
}

/// Read pid / heartbeat / liveness.
pub fn status(paths: &WinzshPaths, cfg: &AgentConfig) -> Result<AgentStatus> {
    let pid = read_pid(paths);
    let running = pid.map(pid_alive).unwrap_or(false);
    let heartbeat = read_heartbeat(paths).ok().flatten();
    let mut notes = Vec::new();
    if pid.is_some() && !running {
        notes.push("Stale pid file (process not running)".into());
    }
    if !cfg.enabled {
        notes.push("Agent disabled in config".into());
    }
    Ok(AgentStatus {
        config_enabled: cfg.enabled,
        interval_secs: cfg.interval_secs,
        pid,
        running,
        heartbeat,
        notes,
    })
}

fn write_heartbeat(paths: &WinzshPaths, tick: &TickReport) -> Result<()> {
    let pid = read_pid(paths).unwrap_or_else(std::process::id);
    let existing = read_heartbeat(paths).ok().flatten();
    let hb = Heartbeat {
        pid,
        started_at: existing
            .as_ref()
            .map(|h| h.started_at.clone())
            .unwrap_or_else(now_rfc3339),
        last_tick_at: now_rfc3339(),
        last_tick: tick.clone(),
    };
    write_heartbeat_full(paths, &hb)
}

fn write_heartbeat_full(paths: &WinzshPaths, hb: &Heartbeat) -> Result<()> {
    ensure_dir(&paths.locks_dir())?;
    let json = serde_json::to_string_pretty(hb)
        .map_err(|e| message(format!("serialize agent heartbeat: {e}")))?;
    atomic_write(&paths.agent_heartbeat_file(), format!("{json}\n"))
}

fn read_heartbeat(paths: &WinzshPaths) -> Result<Option<Heartbeat>> {
    let path = paths.agent_heartbeat_file();
    if !path.is_file() {
        return Ok(None);
    }
    let raw = winzsh_fs::read_string(&path)?;
    let hb: Heartbeat = serde_json::from_str(&raw)
        .map_err(|e| message(format!("parse agent heartbeat: {e}")))?;
    Ok(Some(hb))
}

fn read_pid(paths: &WinzshPaths) -> Option<u32> {
    let path = paths.agent_pid_file();
    let raw = fs::read_to_string(path).ok()?;
    raw.trim().parse().ok()
}

fn cleanup_pid(paths: &WinzshPaths, pid: u32) {
    if read_pid(paths) == Some(pid) {
        let _ = fs::remove_file(paths.agent_pid_file());
    }
}

fn stop_flag(paths: &WinzshPaths) -> std::path::PathBuf {
    paths.locks_dir().join("agent.stop")
}

fn stop_requested(paths: &WinzshPaths) -> bool {
    stop_flag(paths).is_file()
}

fn request_stop(paths: &WinzshPaths) -> Result<()> {
    atomic_write(&stop_flag(paths), "1\n")
}

fn clear_stop_flag(paths: &WinzshPaths) -> Result<()> {
    let path = stop_flag(paths);
    if path.is_file() {
        fs::remove_file(&path).map_err(|source| winzsh_error::io(path, source))?;
    }
    Ok(())
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".into())
}

fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(windows)]
    {
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output();
        match output {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                text.contains(&pid.to_string())
            }
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
}

fn force_kill(pid: u32) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winzsh_config::Config;
    use winzsh_test_support::TempHome;

    #[test]
    fn tick_runs_without_network_panic() {
        let home = TempHome::new("agent");
        let paths = home.paths.clone();
        let mut cfg = Config::default();
        cfg.agent.refresh_registry = false;
        cfg.agent.check_updates = false;
        let report = tick(&paths, &cfg).expect("tick");
        assert!(report.history_entries.is_some());
        assert!(report.notes.iter().any(|n| n.contains("registry")));
    }
}
