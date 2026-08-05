//! Atomic IO, backup rotation, and safe directory helpers under `~/.winzsh`.

#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use winzsh_core::WinzshPaths;
use winzsh_error::{Result, io};

/// Ensure a directory exists (and parents).
pub fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|source| io(path.to_path_buf(), source))
}

/// Create the standard WinZSH directory tree.
pub fn ensure_layout(paths: &WinzshPaths) -> Result<()> {
    ensure_dir(&paths.root)?;
    ensure_dir(&paths.logs_dir())?;
    ensure_dir(&paths.runtime_cache())?;
    ensure_dir(&paths.profile_backups())?;
    ensure_dir(&paths.plugins_dir())?;
    ensure_dir(&paths.themes_dir())?;
    ensure_dir(&paths.locks_dir())?;
    ensure_dir(&paths.history_dir())?;
    Ok(())
}

/// Atomically write `contents` to `path` via temp file + rename.
pub fn atomic_write(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "winzsh.tmp".to_string());
    let tmp = path.with_file_name(format!(".{file_name}.tmp-{}", std::process::id()));
    {
        let mut file = File::create(&tmp).map_err(|source| io(tmp.clone(), source))?;
        file.write_all(contents.as_ref())
            .map_err(|source| io(tmp.clone(), source))?;
        file.sync_all().map_err(|source| io(tmp.clone(), source))?;
    }
    fs::rename(&tmp, path).map_err(|source| {
        let _ = fs::remove_file(&tmp);
        io(path.to_path_buf(), source)
    })?;
    Ok(())
}

/// Read a UTF-8 file to string.
pub fn read_string(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|source| io(path.to_path_buf(), source))
}

/// Copy `src` to a timestamped backup under `backup_dir`, returning the backup path.
pub fn backup_file(src: &Path, backup_dir: &Path, prefix: &str) -> Result<PathBuf> {
    ensure_dir(backup_dir)?;
    let stamp = time_stamp();
    let name = format!("{prefix}-{stamp}");
    let dest = backup_dir.join(name);
    fs::copy(src, &dest).map_err(|source| io(dest.clone(), source))?;
    Ok(dest)
}

/// Append bytes to a file, creating it if needed.
pub fn append_string(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| io(path.to_path_buf(), source))?;
    file.write_all(contents.as_bytes())
        .map_err(|source| io(path.to_path_buf(), source))?;
    Ok(())
}

fn time_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn atomic_write_roundtrip() {
        let dir = std::env::temp_dir().join(format!("winzsh-fs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join("hello.txt");
        atomic_write(&path, b"hello").expect("write");
        assert_eq!(read_string(&path).expect("read"), "hello");
        let _ = fs::remove_dir_all(&dir);
    }
}
