use std::{
    fs,
    path::Path,
};

use crate::{
    error::LogError,
    types::LogDeleteResult,
};

/// Delete a specific log file.
pub fn delete_log(
    log_dir: &Path,
    filename: &str,
) -> Result<(), LogError> {
    let path = log_dir.join(filename);
    if !path.exists() {
        return Err(LogError::FileNotFound {
            filename: filename.to_string(),
        });
    }
    fs::remove_file(&path).map_err(LogError::IoError)?;
    Ok(())
}

/// Delete log files, optionally only those older than the given number of days.
///
/// If `older_than_days` is `None`, all `.log` files in the directory are deleted.
pub fn delete_logs(
    log_dir: &Path,
    older_than_days: Option<u32>,
) -> Result<LogDeleteResult, LogError> {
    let mut deleted_count: usize = 0;
    let mut freed_bytes: u64 = 0;

    if !log_dir.exists() {
        return Ok(LogDeleteResult {
            deleted_count: 0,
            freed_bytes: 0,
        });
    }

    let cutoff = older_than_days.map(|days| {
        std::time::SystemTime::now()
            - std::time::Duration::from_secs(u64::from(days) * 86400)
    });

    let entries = fs::read_dir(log_dir).map_err(LogError::IoError)?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "log") {
            let should_delete = if let Some(cutoff_time) = cutoff {
                entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .map(|modified| modified < cutoff_time)
                    .unwrap_or(false)
            } else {
                true
            };

            if should_delete {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if fs::remove_file(&path).is_ok() {
                    deleted_count += 1;
                    freed_bytes += size;
                }
            }
        }
    }

    Ok(LogDeleteResult {
        deleted_count,
        freed_bytes,
    })
}
