use serde::Serialize;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Atomically writes `bytes` to `path` by writing to a sibling temporary file,
/// flushing and syncing it to disk, and renaming it into place.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("config");
    let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_name = format!(".{}.tmp.{}.{}", file_name, std::process::id(), seq);
    let tmp_path = path.with_file_name(tmp_name);

    let write_res = (|| -> io::Result<()> {
        let mut file = File::create(&tmp_path)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }

    #[cfg(windows)]
    {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }

    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }

    Ok(())
}

/// Serializes `value` as YAML with an optional leading comment header and writes it atomically.
pub fn atomic_write_yaml<T: Serialize>(
    path: &Path,
    header_comment: &str,
    value: &T,
) -> io::Result<()> {
    let yaml = serde_yaml::to_string(value)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("YAML serialize error: {e}")))?;

    let mut out = String::with_capacity(header_comment.len() + yaml.len() + 2);
    if !header_comment.is_empty() {
        out.push_str(header_comment);
        if !header_comment.ends_with('\n') {
            out.push('\n');
        }
    }
    out.push_str(&yaml);
    atomic_write(path, out.as_bytes())
}

/// Non-destructively migrates a legacy config file into `dest` if `dest` does not exist yet.
/// Checks `legacy_candidates` in order and copies the first existing file atomically to `dest`.
/// Returns `Ok(Some(source_path))` if a migration occurred, or `Ok(None)` otherwise.
pub fn migrate_file_if_missing(
    dest: &Path,
    legacy_candidates: &[PathBuf],
) -> io::Result<Option<PathBuf>> {
    if dest.exists() {
        return Ok(None);
    }

    for candidate in legacy_candidates {
        if candidate != dest && candidate.exists() && candidate.is_file() {
            let bytes = fs::read(candidate)?;
            atomic_write(dest, &bytes)?;
            return Ok(Some(candidate.clone()));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct SampleConfig {
        enabled: bool,
        interval_secs: u64,
    }

    #[test]
    fn atomic_write_yaml_creates_parent_and_leaves_no_tmp_files() {
        let root = std::env::temp_dir().join(format!(
            "blockgame_config_io_test_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let cfg_path = root.join("config").join("sample.yml");

        let cfg = SampleConfig {
            enabled: true,
            interval_secs: 120,
        };
        atomic_write_yaml(&cfg_path, "# Sample header", &cfg).unwrap();

        let text = fs::read_to_string(&cfg_path).unwrap();
        assert!(text.starts_with("# Sample header\n"));
        let parsed: SampleConfig = serde_yaml::from_str(&text).unwrap();
        assert_eq!(parsed, cfg);

        let entries: Vec<_> = fs::read_dir(root.join("config"))
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 1, "No temporary files should remain after atomic rename");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn migrate_file_if_missing_copies_non_destructively_and_does_not_overwrite() {
        let root = std::env::temp_dir().join(format!(
            "blockgame_config_migrate_test_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let legacy = root.join("legacy.txt");
        let dest = root.join("config").join("legacy.txt");
        fs::write(&legacy, b"hello-legacy").unwrap();

        let migrated = migrate_file_if_missing(&dest, &[legacy.clone()]).unwrap();
        assert_eq!(migrated, Some(legacy.clone()));
        assert_eq!(fs::read(&dest).unwrap(), b"hello-legacy");
        assert!(legacy.exists(), "Migration must be non-destructive");

        // Updating dest and running again should not overwrite dest
        atomic_write(&dest, b"updated").unwrap();
        let second = migrate_file_if_missing(&dest, &[legacy.clone()]).unwrap();
        assert_eq!(second, None);
        assert_eq!(fs::read(&dest).unwrap(), b"updated");

        let _ = fs::remove_dir_all(&root);
    }
}
