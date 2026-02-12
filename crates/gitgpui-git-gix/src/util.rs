use gitgpui_core::domain::{
    Commit, CommitFileChange, CommitId, FileStatusKind, LogPage, RemoteBranch,
};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{CommandOutput, Result};
use std::path::PathBuf;
use std::process::Command;
use std::str;
use std::time::{Duration, SystemTime};

pub(crate) fn run_git_simple(mut cmd: Command, label: &str) -> Result<()> {
    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    if !output.status.success() {
        let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
        return Err(Error::new(ErrorKind::Backend(format!(
            "{label} failed: {stderr}"
        ))));
    }

    Ok(())
}

pub(crate) fn run_git_with_output(mut cmd: Command, label: &str) -> Result<CommandOutput> {
    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    let exit_code = output.status.code();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let stderr_trimmed = stderr.trim();
        return Err(Error::new(ErrorKind::Backend(format!(
            "{}",
            if stderr_trimmed.is_empty() {
                format!("{label} failed")
            } else {
                format!("{label} failed: {stderr_trimmed}")
            }
        ))));
    }

    Ok(CommandOutput {
        command: label.to_string(),
        stdout,
        stderr,
        exit_code,
    })
}

pub(crate) fn run_git_capture(mut cmd: Command, label: &str) -> Result<String> {
    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    if !output.status.success() {
        let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
        return Err(Error::new(ErrorKind::Backend(format!(
            "{label} failed: {stderr}"
        ))));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(crate) fn parse_git_log_pretty_records(output: &str) -> LogPage {
    let mut commits = Vec::new();
    for record in output.split('\u{001e}') {
        let record = record.trim();
        if record.is_empty() {
            continue;
        }
        let mut parts = record.split('\u{001f}');
        let Some(id) = parts
            .next()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let parents = parts.next().unwrap_or_default();
        let author = parts.next().unwrap_or_default().to_string();
        let time_secs = parts
            .next()
            .and_then(|s| s.trim().parse::<i64>().ok())
            .unwrap_or(0);
        let summary = parts.next().unwrap_or_default().to_string();

        let time = if time_secs >= 0 {
            SystemTime::UNIX_EPOCH + Duration::from_secs(time_secs as u64)
        } else {
            SystemTime::UNIX_EPOCH
        };

        let parent_ids = parents
            .split_whitespace()
            .filter(|p| !p.trim().is_empty())
            .map(|p| CommitId(p.to_string()))
            .collect::<Vec<_>>();

        commits.push(Commit {
            id: CommitId(id),
            parent_ids,
            summary,
            author,
            time,
        });
    }

    LogPage {
        commits,
        next_cursor: None,
    }
}

pub(crate) fn parse_name_status_line(line: &str) -> Option<CommitFileChange> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let mut parts = line.split('\t');
    let status = parts.next()?.trim();
    if status.is_empty() {
        return None;
    }

    let status_kind = status.chars().next()?;
    let kind = match status_kind {
        'A' => FileStatusKind::Added,
        'M' => FileStatusKind::Modified,
        'D' => FileStatusKind::Deleted,
        'R' => FileStatusKind::Renamed,
        'C' => FileStatusKind::Added,
        _ => FileStatusKind::Modified,
    };

    let path = match status_kind {
        'R' | 'C' => {
            let _old = parts.next()?;
            parts.next().unwrap_or_default()
        }
        _ => parts.next().unwrap_or_default(),
    };
    let path = path.trim();
    if path.is_empty() {
        return None;
    }

    Some(CommitFileChange {
        path: PathBuf::from(path),
        kind,
    })
}

pub(crate) fn unix_seconds_to_system_time(seconds: i64) -> Option<SystemTime> {
    if seconds >= 0 {
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds as u64))
    } else {
        None
    }
}

pub(crate) fn unix_seconds_to_system_time_or_epoch(seconds: i64) -> SystemTime {
    unix_seconds_to_system_time(seconds).unwrap_or(SystemTime::UNIX_EPOCH)
}

pub(crate) fn parse_reflog_index(selector: &str) -> Option<usize> {
    let start = selector.rfind("@{")? + 2;
    let end = selector[start..].find('}')? + start;
    selector[start..end].parse::<usize>().ok()
}

pub(crate) fn parse_remote_branches(output: &str) -> Vec<RemoteBranch> {
    let mut branches = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.ends_with("/HEAD") {
            continue;
        }
        let Some((remote, name)) = line.split_once('/') else {
            continue;
        };
        branches.push(RemoteBranch {
            remote: remote.to_string(),
            name: name.to_string(),
        });
    }
    branches.sort_by(|a, b| a.remote.cmp(&b.remote).then_with(|| a.name.cmp(&b.name)));
    branches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_remote_branches_splits_and_skips_head() {
        let output = "origin/HEAD\norigin/main\nupstream/feature/foo\n\n";
        let branches = parse_remote_branches(output);
        assert_eq!(
            branches,
            vec![
                RemoteBranch {
                    remote: "origin".to_string(),
                    name: "main".to_string()
                },
                RemoteBranch {
                    remote: "upstream".to_string(),
                    name: "feature/foo".to_string()
                },
            ]
        );
    }

    #[test]
    fn unix_seconds_to_system_time_clamps_negative_to_epoch() {
        assert_eq!(
            unix_seconds_to_system_time_or_epoch(-1),
            SystemTime::UNIX_EPOCH
        );
        assert_eq!(
            unix_seconds_to_system_time_or_epoch(1),
            SystemTime::UNIX_EPOCH + Duration::from_secs(1)
        );
    }
}
