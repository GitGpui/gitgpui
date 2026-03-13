use gitcomet_core::auth::{
    GITCOMET_AUTH_KIND_ENV, GITCOMET_AUTH_KIND_HOST_VERIFICATION, GITCOMET_AUTH_KIND_PASSPHRASE,
    GITCOMET_AUTH_KIND_USERNAME_PASSWORD, GITCOMET_AUTH_SECRET_ENV, GITCOMET_AUTH_USERNAME_ENV,
    GitAuthKind, StagedGitAuth, take_staged_git_auth,
};
use gitcomet_core::domain::{Commit, CommitFileChange, CommitId, FileStatusKind, LogPage};
use gitcomet_core::error::{Error, ErrorKind};
use gitcomet_core::services::{CommandOutput, Result};
use std::ffi::OsString;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::str;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

#[cfg(test)]
use gitcomet_core::domain::RemoteBranch;

const GIT_COMMAND_TIMEOUT_ENV: &str = "GITCOMET_GIT_COMMAND_TIMEOUT_SECS";
const GIT_COMMAND_TIMEOUT_DEFAULT_SECS: u64 = 300;
const GIT_COMMAND_WAIT_POLL: Duration = Duration::from_millis(100);
const GITCOMET_ASKPASS_PROMPT_LOG_ENV: &str = "GITCOMET_ASKPASS_PROMPT_LOG";

struct AskPassScript {
    _dir: tempfile::TempDir,
    path: PathBuf,
    prompt_log_path: PathBuf,
}

fn git_command_timeout() -> Duration {
    std::env::var(GIT_COMMAND_TIMEOUT_ENV)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(GIT_COMMAND_TIMEOUT_DEFAULT_SECS))
}

fn configure_non_interactive_git(cmd: &mut Command) {
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.stdin(Stdio::null());
}

fn command_may_require_auth(cmd: &Command) -> bool {
    let mut args = cmd.get_args();
    while let Some(arg) = args.next() {
        let Some(arg) = arg.to_str() else {
            return false;
        };
        match arg {
            "-C" | "-c" => {
                let _ = args.next();
            }
            "--git-dir" | "--work-tree" | "--namespace" => {
                let _ = args.next();
            }
            value if value.starts_with('-') => {}
            "clone" | "fetch" | "pull" | "push" | "submodule" | "ls-remote" | "commit" => {
                return true;
            }
            _ => return false,
        }
    }
    false
}

fn take_pending_git_auth() -> Option<StagedGitAuth> {
    let auth = take_staged_git_auth()?;
    if auth.secret.is_empty() {
        return None;
    }
    Some(auth)
}

#[cfg(unix)]
fn askpass_script_contents() -> &'static [u8] {
    br#"#!/bin/sh
prompt="$1"
lower_prompt=$(printf '%s' "$prompt" | tr '[:upper:]' '[:lower:]')
if [ -n "${GITCOMET_ASKPASS_PROMPT_LOG:-}" ]; then
  case "$lower_prompt" in
    *authenticity\ of\ host*|*continue\ connecting*|*yes/no*|*fingerprint*)
      printf '%s\n' "$prompt" >> "${GITCOMET_ASKPASS_PROMPT_LOG}" ;;
  esac
fi
kind="${GITCOMET_AUTH_KIND:-}"
if [ "$kind" = "username_password" ]; then
  case "$lower_prompt" in
    *username*) printf '%s\n' "${GITCOMET_AUTH_USERNAME:-}" ;;
    *) printf '%s\n' "${GITCOMET_AUTH_SECRET:-}" ;;
  esac
elif [ "$kind" = "host_verification" ]; then
  case "$lower_prompt" in
    *continue\ connecting*|*yes/no*|*fingerprint*) printf '%s\n' "${GITCOMET_AUTH_SECRET:-}" ;;
    *) printf '\n' ;;
  esac
else
  printf '%s\n' "${GITCOMET_AUTH_SECRET:-}"
fi
"#
}

#[cfg(windows)]
fn askpass_script_contents() -> &'static [u8] {
    br#"@echo off
setlocal EnableDelayedExpansion
set "prompt=%~1"
if not "%GITCOMET_ASKPASS_PROMPT_LOG%"=="" (
  echo %prompt% | findstr /I /C:"authenticity of host" /C:"continue connecting" /C:"yes/no" /C:"fingerprint" >nul
  if not errorlevel 1 (
    >>"%GITCOMET_ASKPASS_PROMPT_LOG%" echo %prompt%
  )
)
if /I "%GITCOMET_AUTH_KIND%"=="username_password" (
  echo %prompt% | findstr /I "username" >nul
  if not errorlevel 1 (
    echo %GITCOMET_AUTH_USERNAME%
    exit /b 0
  )
  echo %GITCOMET_AUTH_SECRET%
  exit /b 0
)
if /I "%GITCOMET_AUTH_KIND%"=="host_verification" (
  echo %prompt% | findstr /I /C:"continue connecting" /C:"yes/no" /C:"fingerprint" >nul
  if not errorlevel 1 (
    echo %GITCOMET_AUTH_SECRET%
  )
  exit /b 0
)
echo %GITCOMET_AUTH_SECRET%
"#
}

fn create_askpass_script() -> Result<AskPassScript> {
    let dir = tempfile::tempdir().map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;
    #[cfg(windows)]
    let script_name = "gitcomet-askpass.cmd";
    #[cfg(not(windows))]
    let script_name = "gitcomet-askpass.sh";
    let path = dir.path().join(script_name);
    let prompt_log_path = dir.path().join("gitcomet-askpass-prompt.log");

    fs::write(&path, askpass_script_contents()).map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;
    fs::write(&prompt_log_path, b"").map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let mut permissions = fs::metadata(&path)
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&path, permissions).map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;
    }

    Ok(AskPassScript {
        _dir: dir,
        path,
        prompt_log_path,
    })
}

fn configure_git_auth_prompt(
    cmd: &mut Command,
    auth: Option<&StagedGitAuth>,
    askpass: &AskPassScript,
) {
    cmd.env("GIT_ASKPASS", &askpass.path);
    cmd.env("SSH_ASKPASS", &askpass.path);
    cmd.env("SSH_ASKPASS_REQUIRE", "force");
    cmd.env(GITCOMET_ASKPASS_PROMPT_LOG_ENV, &askpass.prompt_log_path);
    if cfg!(all(unix, not(target_os = "macos"))) && std::env::var_os("DISPLAY").is_none() {
        cmd.env("DISPLAY", "gitcomet:0");
    }

    if let Some(auth) = auth {
        let kind = match auth.kind {
            GitAuthKind::UsernamePassword => GITCOMET_AUTH_KIND_USERNAME_PASSWORD,
            GitAuthKind::Passphrase => GITCOMET_AUTH_KIND_PASSPHRASE,
            GitAuthKind::HostVerification => GITCOMET_AUTH_KIND_HOST_VERIFICATION,
        };
        cmd.env(GITCOMET_AUTH_KIND_ENV, kind);
        if let Some(username) = &auth.username {
            cmd.env(GITCOMET_AUTH_USERNAME_ENV, username);
        } else {
            cmd.env_remove(GITCOMET_AUTH_USERNAME_ENV);
        }
        cmd.env(GITCOMET_AUTH_SECRET_ENV, &auth.secret);
    } else {
        cmd.env_remove(GITCOMET_AUTH_KIND_ENV);
        cmd.env_remove(GITCOMET_AUTH_USERNAME_ENV);
        cmd.env_remove(GITCOMET_AUTH_SECRET_ENV);
    }
}

fn append_host_prompt_to_stderr(stderr: &mut Vec<u8>, askpass: &AskPassScript) {
    let Ok(raw_prompt_log) = fs::read_to_string(&askpass.prompt_log_path) else {
        return;
    };
    let prompt_log = raw_prompt_log.trim();
    if prompt_log.is_empty() {
        return;
    }

    let stderr_text = String::from_utf8_lossy(stderr);
    if stderr_text.contains(prompt_log) {
        return;
    }

    if !stderr.is_empty() && !stderr.ends_with(b"\n") {
        stderr.push(b'\n');
    }
    stderr.extend_from_slice(b"SSH host verification prompt:\n");
    stderr.extend_from_slice(prompt_log.as_bytes());
    stderr.push(b'\n');
}

fn run_git_output_with_timeout(mut cmd: Command, label: &str) -> Result<Output> {
    configure_non_interactive_git(&mut cmd);
    let askpass_script = if command_may_require_auth(&cmd) {
        let auth = take_pending_git_auth();
        let script = create_askpass_script()?;
        configure_git_auth_prompt(&mut cmd, auth.as_ref(), &script);
        Some(script)
    } else {
        None
    };
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;
    let askpass_script = askpass_script;

    let stdout = child.stdout.take();
    let stdout_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut stdout) = stdout {
            let _ = stdout.read_to_end(&mut buf);
        }
        buf
    });

    let stderr = child.stderr.take();
    let stderr_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut stderr) = stderr {
            let _ = stderr.read_to_end(&mut buf);
        }
        buf
    });

    let timeout = git_command_timeout();
    let start = Instant::now();
    let mut timed_out = false;

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    timed_out = true;
                    let _ = child.kill();
                    match child.wait() {
                        Ok(status) => break status,
                        Err(e) => return Err(Error::new(ErrorKind::Io(e.kind()))),
                    }
                }
                thread::sleep(GIT_COMMAND_WAIT_POLL);
            }
            Err(e) => return Err(Error::new(ErrorKind::Io(e.kind()))),
        }
    };

    let stdout = stdout_handle.join().unwrap_or_default();
    let mut stderr = stderr_handle.join().unwrap_or_default();

    if let Some(askpass_script) = askpass_script.as_ref() {
        append_host_prompt_to_stderr(&mut stderr, askpass_script);
    }

    if timed_out {
        return Err(Error::new(ErrorKind::Backend(format!(
            "{label} timed out after {} seconds (set {GIT_COMMAND_TIMEOUT_ENV} to override)",
            timeout.as_secs()
        ))));
    }

    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

pub(crate) fn run_git_simple(cmd: Command, label: &str) -> Result<()> {
    let output = run_git_output_with_timeout(cmd, label)?;

    if !output.status.success() {
        let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
        return Err(Error::new(ErrorKind::Backend(format!(
            "{label} failed: {stderr}"
        ))));
    }

    Ok(())
}

pub(crate) fn validate_ref_like_arg(value: &str, kind: &str) -> Result<()> {
    if value.is_empty() {
        return Err(Error::new(ErrorKind::Backend(format!(
            "invalid {kind}: value is empty"
        ))));
    }
    if value.starts_with('-') {
        return Err(Error::new(ErrorKind::Backend(format!(
            "invalid {kind}: values starting with '-' are not allowed"
        ))));
    }
    Ok(())
}

pub(crate) fn validate_hex_commit_id(id: &CommitId) -> Result<()> {
    let value = id.as_ref();
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::new(ErrorKind::Backend(
            "invalid commit id: must contain only hexadecimal characters".to_string(),
        )));
    }
    Ok(())
}

pub(crate) fn path_buf_from_git_bytes(path_bytes: &[u8], context: &str) -> Result<PathBuf> {
    #[cfg(unix)]
    {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt as _;

        let _ = context;
        Ok(PathBuf::from(OsStr::from_bytes(path_bytes)))
    }

    #[cfg(windows)]
    {
        let path_text = std::str::from_utf8(path_bytes).map_err(|_| {
            Error::new(ErrorKind::Backend(format!(
                "{context}: non-UTF-8 git path bytes are not representable on Windows",
            )))
        })?;
        Ok(PathBuf::from(path_text))
    }
}

pub(crate) fn git_stage_blob_spec(stage: u8, path: &Path) -> Result<OsString> {
    git_revision_with_path(&format!(":{stage}:"), path, "build conflict stage revision")
}

pub(crate) fn git_stash_untracked_blob_spec(index: usize, path: &Path) -> Result<OsString> {
    git_revision_with_path(
        &format!("stash@{{{index}}}^3:"),
        path,
        "build stash untracked blob revision",
    )
}

fn git_revision_with_path(prefix: &str, path: &Path, context: &str) -> Result<OsString> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};

        let _ = context;
        let path_bytes = path.as_os_str().as_bytes();
        let mut rev = Vec::with_capacity(prefix.len().saturating_add(path_bytes.len()));
        rev.extend_from_slice(prefix.as_bytes());
        rev.extend_from_slice(path_bytes);
        Ok(OsString::from_vec(rev))
    }

    #[cfg(windows)]
    {
        let path_text = path.to_str().ok_or_else(|| {
            Error::new(ErrorKind::Backend(format!(
                "{context}: non-Unicode path cannot be represented for git command arguments",
            )))
        })?;
        Ok(OsString::from(format!(
            "{prefix}{}",
            path_text.replace('\\', "/")
        )))
    }
}

fn command_path_budget_len(path: &Path) -> usize {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;

        path.as_os_str().as_bytes().len().saturating_add(1)
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt as _;

        path.as_os_str()
            .encode_wide()
            .count()
            .saturating_mul(std::mem::size_of::<u16>())
            .saturating_add(std::mem::size_of::<u16>())
    }
}

pub(crate) fn run_git_simple_with_paths(
    workdir: &Path,
    label: &str,
    args: &[&str],
    paths: &[&Path],
) -> Result<()> {
    const MAX_PATH_BYTES_PER_CMD: usize = 28_000;
    const MAX_PATHS_PER_CMD: usize = 1024;

    if paths.is_empty() {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(workdir);
        cmd.args(args);
        return run_git_simple(cmd, label);
    }

    let mut batch: Vec<&Path> = Vec::with_capacity(paths.len().min(MAX_PATHS_PER_CMD));
    let mut bytes: usize = 0;
    for path in paths {
        let path_len = command_path_budget_len(path);

        if !batch.is_empty()
            && (batch.len() >= MAX_PATHS_PER_CMD
                || bytes.saturating_add(path_len) > MAX_PATH_BYTES_PER_CMD)
        {
            let mut cmd = Command::new("git");
            cmd.arg("-C").arg(workdir);
            cmd.args(args);
            cmd.arg("--");
            for p in &batch {
                cmd.arg(p);
            }
            run_git_simple(cmd, label)?;
            batch.clear();
            bytes = 0;
        }

        batch.push(*path);
        bytes = bytes.saturating_add(path_len);
    }

    if !batch.is_empty() {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(workdir);
        cmd.args(args);
        cmd.arg("--");
        for p in &batch {
            cmd.arg(p);
        }
        run_git_simple(cmd, label)?;
    }

    Ok(())
}

fn bytes_to_text_preserving_utf8(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut out = String::with_capacity(bytes.len());
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        match std::str::from_utf8(&bytes[cursor..]) {
            Ok(valid) => {
                out.push_str(valid);
                break;
            }
            Err(err) => {
                let valid_len = err.valid_up_to();
                if valid_len > 0 {
                    let valid = &bytes[cursor..cursor + valid_len];
                    out.push_str(
                        std::str::from_utf8(valid)
                            .expect("slice identified by valid_up_to must be valid UTF-8"),
                    );
                    cursor += valid_len;
                }

                let invalid_len = err.error_len().unwrap_or(1);
                let invalid_end = cursor.saturating_add(invalid_len).min(bytes.len());
                for byte in &bytes[cursor..invalid_end] {
                    let _ = write!(out, "\\x{byte:02x}");
                }
                cursor = invalid_end;
            }
        }
    }

    out
}

pub(crate) fn run_git_with_output(cmd: Command, label: &str) -> Result<CommandOutput> {
    let output = run_git_output_with_timeout(cmd, label)?;

    let exit_code = output.status.code();
    let stdout = bytes_to_text_preserving_utf8(&output.stdout);
    let stderr = bytes_to_text_preserving_utf8(&output.stderr);

    if !output.status.success() {
        let stderr_trimmed = stderr.trim();
        return Err(Error::new(ErrorKind::Backend(
            (if stderr_trimmed.is_empty() {
                format!("{label} failed")
            } else {
                format!("{label} failed: {stderr_trimmed}")
            })
            .to_string(),
        )));
    }

    Ok(CommandOutput {
        command: label.to_string(),
        stdout,
        stderr,
        exit_code,
    })
}

pub(crate) fn run_git_capture(cmd: Command, label: &str) -> Result<String> {
    let output = run_git_output_with_timeout(cmd, label)?;

    if !output.status.success() {
        let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
        return Err(Error::new(ErrorKind::Backend(format!(
            "{label} failed: {stderr}"
        ))));
    }

    Ok(bytes_to_text_preserving_utf8(&output.stdout))
}

pub(crate) fn parse_git_log_pretty_records(output: &str) -> LogPage {
    let approx_commits = output
        .as_bytes()
        .iter()
        .filter(|&&b| b == b'\x1e')
        .count()
        .saturating_add(1);
    let mut commits = Vec::with_capacity(approx_commits);
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

fn pathbuf_from_git_output_path(path: &str) -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(path.replace('/', "\\"))
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(path)
    }
}

pub(crate) fn parse_name_status_line(line: &str) -> Option<CommitFileChange> {
    let line = line.trim_end_matches(&['\n', '\r'][..]);
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
        'T' | 'U' | 'X' | '!' | '?' => FileStatusKind::Modified,
        _ => {
            debug_assert!(false, "unrecognized git status code: {status_kind:?}");
            FileStatusKind::Modified
        }
    };

    let path = match status_kind {
        'R' | 'C' => {
            let _old = parts.next()?;
            parts.next().unwrap_or_default()
        }
        _ => parts.next().unwrap_or_default(),
    };
    if path.is_empty() {
        return None;
    }

    Some(CommitFileChange {
        path: pathbuf_from_git_output_path(path),
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

#[cfg(test)]
pub(crate) fn parse_remote_branches(output: &str) -> Vec<RemoteBranch> {
    let approx_branches = output
        .as_bytes()
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        .saturating_add(1);
    let mut branches = Vec::with_capacity(approx_branches);
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let Some(full_name) = parts.next().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        if full_name.ends_with("/HEAD") {
            continue;
        }
        let Some(sha) = parts.next().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some((remote, name)) = full_name.split_once('/') else {
            continue;
        };
        branches.push(RemoteBranch {
            remote: remote.to_string(),
            name: name.to_string(),
            target: CommitId(sha.to_string()),
        });
    }
    branches.sort_by(|a, b| a.remote.cmp(&b.remote).then_with(|| a.name.cmp(&b.name)));
    branches
}

#[cfg(test)]
mod tests {
    use super::*;

    const GITPY_FOR_EACH_REF_WITH_PATH_COMPONENT: &[u8] =
        include_bytes!("../tests/fixtures/gitpython/for_each_ref_with_path_component");
    const GITPY_DIFF_FILE_WITH_COLON: &[u8] =
        include_bytes!("../tests/fixtures/gitpython/diff_file_with_colon");
    const GITPY_DIFF_FILE_WITH_SPACES: &str =
        include_str!("../tests/fixtures/gitpython/diff_file_with_spaces");
    const GITPY_DIFF_RENAME: &str = include_str!("../tests/fixtures/gitpython/diff_rename");
    const GITPY_DIFF_CHANGE_IN_TYPE_RAW: &str =
        include_str!("../tests/fixtures/gitpython/diff_change_in_type_raw");
    const GITPY_DIFF_COPIED_MODE_RAW: &str =
        include_str!("../tests/fixtures/gitpython/diff_copied_mode_raw");
    const GITPY_DIFF_RENAME_RAW: &str = include_str!("../tests/fixtures/gitpython/diff_rename_raw");
    const GITPY_DIFF_RAW_BINARY: &str = include_str!("../tests/fixtures/gitpython/diff_raw_binary");
    const GITPY_DIFF_INDEX_RAW: &str = include_str!("../tests/fixtures/gitpython/diff_index_raw");
    const GITPY_DIFF_PATCH_UNSAFE_PATHS: &[u8] =
        include_bytes!("../tests/fixtures/gitpython/diff_patch_unsafe_paths");
    const GITPY_UNCOMMON_BRANCH_PREFIX_FETCH_HEAD: &str =
        include_str!("../tests/fixtures/gitpython/uncommon_branch_prefix_FETCH_HEAD");
    const GITPY_REV_LIST_SINGLE: &str = include_str!("../tests/fixtures/gitpython/rev_list_single");
    const GITPY_REV_LIST_COMMIT_STATS: &str =
        include_str!("../tests/fixtures/gitpython/rev_list_commit_stats");

    #[cfg(unix)]
    #[test]
    fn path_buf_from_git_bytes_preserves_non_utf8_bytes() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt as _;

        let raw_path = b"docs/\xff-topic.md";
        let path = path_buf_from_git_bytes(raw_path, "test").expect("path conversion");
        assert_eq!(path.as_os_str(), OsStr::from_bytes(raw_path));
    }

    #[cfg(unix)]
    #[test]
    fn git_stage_blob_spec_preserves_non_utf8_path_bytes() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt as _;

        let path = Path::new(OsStr::from_bytes(b"nested/\xff-file.bin"));
        let rev = git_stage_blob_spec(2, path).expect("stage spec");
        assert_eq!(rev.as_os_str().as_bytes(), b":2:nested/\xff-file.bin");
    }

    #[cfg(windows)]
    #[test]
    fn git_stage_blob_spec_normalizes_windows_separators() {
        let rev = git_stage_blob_spec(3, Path::new(r"nested\file.bin")).expect("stage spec");
        assert_eq!(
            rev.to_str()
                .expect("ascii revision should be valid unicode"),
            ":3:nested/file.bin"
        );
    }

    #[cfg(windows)]
    #[test]
    fn git_stash_untracked_blob_spec_normalizes_windows_separators() {
        let rev =
            git_stash_untracked_blob_spec(4, Path::new(r"nested\file.bin")).expect("stash spec");
        assert_eq!(
            rev.to_str()
                .expect("ascii revision should be valid unicode"),
            "stash@{4}^3:nested/file.bin"
        );
    }

    fn gitpython_raw_to_name_status_line(raw: &str) -> String {
        let mut parts = raw.split_whitespace();
        let _old_mode = parts.next().expect("raw fixture old mode");
        let _new_mode = parts.next().expect("raw fixture new mode");
        let _old_sha = parts.next().expect("raw fixture old sha");
        let _new_sha = parts.next().expect("raw fixture new sha");
        let status = parts.next().expect("raw fixture status");
        let first_path = parts.next().expect("raw fixture path");

        if status.starts_with('R') || status.starts_with('C') {
            let second_path = parts.next().expect("raw fixture second path");
            format!("{status}\t{first_path}\t{second_path}")
        } else {
            format!("{status}\t{first_path}")
        }
    }

    fn gitpython_patch_b_paths(patch_bytes: &[u8]) -> Vec<String> {
        let text = std::str::from_utf8(patch_bytes).expect("test patch bytes should be utf-8");
        let mut out = Vec::new();
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("+++ ") else {
                continue;
            };
            if rest == "/dev/null" {
                continue;
            }
            if let Some(path) = rest.strip_prefix("b/") {
                out.push(path.to_string());
            } else if let Some(quoted) = rest.strip_prefix("\"b/") {
                let path = quoted
                    .strip_suffix('\"')
                    .expect("quoted +++ line should have trailing quote");
                out.push(path.to_string());
            }
        }
        out
    }

    fn gitpython_fetch_head_to_remote_ref_output(fetch_head: &str, remote: &str) -> String {
        let mut out = String::new();
        for line in fetch_head.lines() {
            let Some((sha, rest)) = line.split_once('\t') else {
                continue;
            };
            let sha = sha.trim();
            if sha.is_empty() {
                continue;
            }
            let Some(start) = rest.find("'refs/") else {
                continue;
            };
            let refs_and_after = &rest[start + 1..];
            let Some((full_ref, _)) = refs_and_after.split_once('\'') else {
                continue;
            };
            let short_ref = full_ref.strip_prefix("refs/").unwrap_or(full_ref);
            out.push_str(remote);
            out.push('/');
            out.push_str(short_ref);
            out.push('\t');
            out.push_str(sha);
            out.push('\n');
        }
        out
    }

    fn gitpython_rev_list_fixture_to_pretty_record(fixture: &str) -> String {
        let id = fixture
            .lines()
            .find_map(|line| line.strip_prefix("commit "))
            .expect("rev-list fixture should contain commit id")
            .trim();

        let parents = fixture
            .lines()
            .filter_map(|line| line.strip_prefix("parent "))
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ");

        let author_line = fixture
            .lines()
            .find(|line| line.starts_with("author "))
            .expect("rev-list fixture should contain author line");
        let author = author_line
            .strip_prefix("author ")
            .and_then(|line| line.split_once(" <").map(|(name, _)| name))
            .expect("author line should include actor and email");
        let time = author_line
            .split_whitespace()
            .rev()
            .nth(1)
            .expect("author line should contain unix timestamp")
            .trim();

        let summary = fixture
            .lines()
            .find_map(|line| line.strip_prefix("    "))
            .unwrap_or_default()
            .trim();

        format!("{id}\x1f{parents}\x1f{author}\x1f{time}\x1f{summary}\x1e")
    }

    #[test]
    fn parse_remote_branches_splits_and_skips_head() {
        let output =
            "origin/HEAD\tdeadbeef\norigin/main\t1111111\nupstream/feature/foo\t2222222\n\n";
        let branches = parse_remote_branches(output);
        assert_eq!(
            branches,
            vec![
                RemoteBranch {
                    remote: "origin".to_string(),
                    name: "main".to_string(),
                    target: CommitId("1111111".to_string())
                },
                RemoteBranch {
                    remote: "upstream".to_string(),
                    name: "feature/foo".to_string(),
                    target: CommitId("2222222".to_string())
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

    #[test]
    fn parse_remote_branches_handles_path_components_from_gitpython_fixture() {
        let raw = std::str::from_utf8(GITPY_FOR_EACH_REF_WITH_PATH_COMPONENT)
            .expect("fixture should be valid UTF-8");
        let mut fields = raw.trim().split('\0');
        let full_ref = fields.next().expect("refname field");
        let oid = fields.next().expect("object id field");
        let short = full_ref
            .strip_prefix("refs/heads/")
            .expect("heads ref prefix in fixture");

        let output = format!("origin/{short}\t{oid}\norigin/HEAD\tdeadbeef\n");
        let branches = parse_remote_branches(&output);

        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].remote, "origin");
        assert_eq!(branches[0].name, "refactoring/feature1");
        assert_eq!(branches[0].target, CommitId(oid.to_string()));
    }

    #[test]
    fn parse_git_log_pretty_records_parses_single_commit_from_gitpython_fixture() {
        let output = gitpython_rev_list_fixture_to_pretty_record(GITPY_REV_LIST_SINGLE);
        let page = parse_git_log_pretty_records(&output);

        assert_eq!(page.commits.len(), 1);
        assert!(page.next_cursor.is_none());
        let commit = &page.commits[0];
        assert_eq!(
            commit.id,
            CommitId("4c8124ffcf4039d292442eeccabdeca5af5c5017".to_string())
        );
        assert_eq!(
            commit.parent_ids,
            vec![CommitId(
                "634396b2f541a9f2d58b00be1a07f0c358b999b3".to_string()
            )]
        );
        assert_eq!(commit.author, "Tom Preston-Werner");
        assert_eq!(commit.summary, "implement Grit#heads");
        assert_eq!(
            commit.time,
            SystemTime::UNIX_EPOCH + Duration::from_secs(1_191_999_972)
        );
    }

    #[test]
    fn parse_git_log_pretty_records_parses_multiple_gitpython_fixtures() {
        let output = format!(
            "{}{}",
            gitpython_rev_list_fixture_to_pretty_record(GITPY_REV_LIST_SINGLE),
            gitpython_rev_list_fixture_to_pretty_record(GITPY_REV_LIST_COMMIT_STATS)
        );
        let page = parse_git_log_pretty_records(&output);

        assert_eq!(page.commits.len(), 2);
        assert!(page.next_cursor.is_none());

        assert_eq!(
            page.commits[1].id,
            CommitId("634396b2f541a9f2d58b00be1a07f0c358b999b3".to_string())
        );
        assert!(page.commits[1].parent_ids.is_empty());
        assert_eq!(page.commits[1].author, "Tom Preston-Werner");
        assert_eq!(page.commits[1].summary, "initial grit setup");
        assert_eq!(
            page.commits[1].time,
            SystemTime::UNIX_EPOCH + Duration::from_secs(1_191_997_100)
        );
    }

    #[test]
    fn parse_remote_branches_handles_pull_ref_prefixes_from_gitpython_fixture() {
        let mut output = gitpython_fetch_head_to_remote_ref_output(
            GITPY_UNCOMMON_BRANCH_PREFIX_FETCH_HEAD,
            "origin",
        );
        output.push_str("origin/HEAD\tdeadbeef\n");
        let branches = parse_remote_branches(&output);

        let names = branches.iter().map(|b| b.name.as_str()).collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "pull/1/head",
                "pull/1/merge",
                "pull/2/head",
                "pull/2/merge",
                "pull/3/head",
                "pull/3/merge",
            ]
        );
        assert_eq!(branches.len(), 6);
        assert_eq!(
            branches[0].target,
            CommitId("c2e3c20affa3e2b61a05fdc9ee3061dd416d915e".to_string())
        );
    }

    #[test]
    fn parse_name_status_line_handles_colon_paths_from_gitpython_fixture() {
        let raw = std::str::from_utf8(GITPY_DIFF_FILE_WITH_COLON).expect("fixture should be utf-8");
        let colon_path = raw
            .split('\0')
            .find(|segment| segment.contains("file with : colon.txt"))
            .expect("fixture contains colon path")
            .trim();

        let parsed = parse_name_status_line(&format!("M\t{colon_path}"))
            .expect("name-status line with colon path should parse");

        assert_eq!(parsed.path, PathBuf::from("file with : colon.txt"));
        assert_eq!(parsed.kind, FileStatusKind::Modified);
    }

    #[test]
    fn parse_name_status_line_handles_space_paths_from_gitpython_fixture() {
        let added_path = GITPY_DIFF_FILE_WITH_SPACES
            .lines()
            .find_map(|line| line.strip_prefix("+++ b/"))
            .expect("fixture contains +++ path line")
            .trim();

        let parsed = parse_name_status_line(&format!("A\t{added_path}"))
            .expect("name-status line with spaces should parse");

        assert_eq!(parsed.path, PathBuf::from("file with spaces"));
        assert_eq!(parsed.kind, FileStatusKind::Added);
    }

    #[test]
    fn parse_name_status_line_normalizes_git_separators_to_platform_path() {
        let parsed = parse_name_status_line("M\tnested/file.txt")
            .expect("name-status line with nested path should parse");

        assert_eq!(parsed.path, Path::new("nested").join("file.txt"));
        assert_eq!(parsed.kind, FileStatusKind::Modified);

        #[cfg(windows)]
        assert_eq!(parsed.path.to_str(), Some(r"nested\file.txt"));
    }

    #[test]
    fn parse_name_status_line_handles_unicode_rename_from_gitpython_fixture() {
        let from = GITPY_DIFF_RENAME
            .lines()
            .find_map(|line| line.strip_prefix("rename from "))
            .expect("fixture contains rename-from line")
            .trim();
        let to = GITPY_DIFF_RENAME
            .lines()
            .find_map(|line| line.strip_prefix("rename to "))
            .expect("fixture contains rename-to line")
            .trim();

        let parsed = parse_name_status_line(&format!("R100\t{from}\t{to}"))
            .expect("rename name-status line should parse");

        assert_eq!(parsed.path, PathBuf::from("müller"));
        assert_eq!(parsed.kind, FileStatusKind::Renamed);
    }

    #[test]
    fn parse_name_status_line_handles_copy_status_from_gitpython_raw_fixture() {
        let line = gitpython_raw_to_name_status_line(GITPY_DIFF_COPIED_MODE_RAW.trim());
        let parsed = parse_name_status_line(&line).expect("copied raw status should parse");

        assert_eq!(parsed.path, PathBuf::from("test2.txt"));
        assert_eq!(parsed.kind, FileStatusKind::Added);
    }

    #[test]
    fn parse_name_status_line_maps_type_change_to_modified_from_gitpython_raw_fixture() {
        let line = gitpython_raw_to_name_status_line(GITPY_DIFF_CHANGE_IN_TYPE_RAW.trim());
        let parsed = parse_name_status_line(&line).expect("type-change raw status should parse");

        assert_eq!(parsed.path, PathBuf::from("this"));
        assert_eq!(parsed.kind, FileStatusKind::Modified);
    }

    #[test]
    fn parse_name_status_line_handles_raw_rename_from_gitpython_fixture() {
        let line = gitpython_raw_to_name_status_line(GITPY_DIFF_RENAME_RAW.trim());
        let parsed = parse_name_status_line(&line).expect("rename raw status should parse");

        assert_eq!(parsed.path, PathBuf::from("that"));
        assert_eq!(parsed.kind, FileStatusKind::Renamed);
    }

    #[test]
    fn parse_name_status_line_handles_raw_binary_modified_from_gitpython_fixture() {
        let line = gitpython_raw_to_name_status_line(GITPY_DIFF_RAW_BINARY.trim());
        let parsed = parse_name_status_line(&line).expect("binary raw status should parse");

        assert_eq!(parsed.path, PathBuf::from("rps"));
        assert_eq!(parsed.kind, FileStatusKind::Modified);
    }

    #[test]
    fn parse_name_status_line_preserves_single_space_path_from_gitpython_raw_fixture() {
        let raw = GITPY_DIFF_INDEX_RAW.trim_end_matches('\n');
        let status_start = raw
            .find(" D\t")
            .map(|ix| ix + 1)
            .expect("fixture should contain deleted status with tab-separated path");
        let line = &raw[status_start..];

        let parsed = parse_name_status_line(line).expect("single-space path should parse");
        assert_eq!(parsed.path, PathBuf::from(" "));
        assert_eq!(parsed.kind, FileStatusKind::Deleted);
    }

    #[test]
    fn parse_name_status_line_preserves_unsafe_paths_from_gitpython_patch_fixture() {
        let paths = gitpython_patch_b_paths(GITPY_DIFF_PATCH_UNSAFE_PATHS);
        assert!(paths.iter().any(|p| p == "path/ starting with a space"));
        assert!(paths.iter().any(|p| p == "path/ending in a space "));
        assert!(paths.iter().any(|p| p == r#"path/with\ttab"#));
        assert!(paths.iter().any(|p| p == r#"path/with\nnewline"#));
        assert!(paths.iter().any(|p| p == "path/with spaces"));
        assert!(paths.iter().any(|p| p == "path/with-question-mark?"));

        for path in paths {
            let parsed = parse_name_status_line(&format!("A\t{path}"))
                .expect("unsafe path from fixture should parse");
            assert_eq!(parsed.path, PathBuf::from(&path));
            assert_eq!(parsed.kind, FileStatusKind::Added);
        }
    }

    #[test]
    fn command_may_require_auth_detects_auth_related_git_commands() {
        let mut push = Command::new("git");
        push.args(["-C", "/tmp/repo", "push", "origin", "main"]);
        assert!(command_may_require_auth(&push));

        let mut fetch = Command::new("git");
        fetch.args(["-c", "color.ui=false", "fetch", "--all"]);
        assert!(command_may_require_auth(&fetch));

        let mut ls_remote = Command::new("git");
        ls_remote.args(["ls-remote", "origin"]);
        assert!(command_may_require_auth(&ls_remote));

        let mut commit = Command::new("git");
        commit.args(["commit", "-m", "msg"]);
        assert!(command_may_require_auth(&commit));

        let mut status = Command::new("git");
        status.args(["-C", "/tmp/repo", "status", "--short"]);
        assert!(!command_may_require_auth(&status));

        let mut log = Command::new("git");
        log.args(["log", "--oneline", "-n", "1"]);
        assert!(!command_may_require_auth(&log));
    }

    #[test]
    fn create_askpass_script_writes_expected_content_and_permissions() {
        let askpass = create_askpass_script().expect("askpass script creation");
        assert!(askpass.path.exists());

        let contents =
            std::fs::read_to_string(&askpass.path).expect("askpass script should be readable");
        assert!(contents.contains("GITCOMET_AUTH_SECRET"));
        assert!(contents.contains("GITCOMET_AUTH_KIND"));
        assert!(contents.contains("host_verification"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            let mode = std::fs::metadata(&askpass.path)
                .expect("askpass metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o700);
        }
    }

    #[test]
    fn append_host_prompt_to_stderr_includes_logged_prompt_with_fingerprint() {
        let askpass = create_askpass_script().expect("askpass script creation");
        std::fs::write(
            &askpass.prompt_log_path,
            "The authenticity of host 'github.com (140.82.121.3)' can't be established.\nED25519 key fingerprint is: SHA256:+DiY...\nAre you sure you want to continue connecting (yes/no/[fingerprint])?",
        )
        .expect("write prompt log");

        let mut stderr = b"Host key verification failed.\n".to_vec();
        append_host_prompt_to_stderr(&mut stderr, &askpass);

        let rendered = String::from_utf8(stderr).expect("stderr should be utf-8 for test");
        assert!(rendered.contains("SSH host verification prompt:"));
        assert!(rendered.contains("ED25519 key fingerprint is: SHA256:+DiY..."));
        assert!(rendered.contains("yes/no/[fingerprint]"));
    }

    #[test]
    fn append_host_prompt_to_stderr_skips_when_prompt_already_present() {
        let askpass = create_askpass_script().expect("askpass script creation");
        let prompt = "Are you sure you want to continue connecting (yes/no/[fingerprint])?";
        std::fs::write(&askpass.prompt_log_path, prompt).expect("write prompt log");

        let mut stderr = format!("Host key verification failed.\n{prompt}\n").into_bytes();
        append_host_prompt_to_stderr(&mut stderr, &askpass);

        let rendered = String::from_utf8(stderr).expect("stderr should be utf-8 for test");
        assert_eq!(rendered.matches("SSH host verification prompt:").count(), 0);
        assert_eq!(rendered.matches(prompt).count(), 1);
    }

    fn command_env_value(cmd: &Command, key: &str) -> Option<String> {
        use std::ffi::OsStr;

        cmd.get_envs().find_map(|(k, v)| {
            if k == OsStr::new(key) {
                v.and_then(|value| value.to_str().map(ToOwned::to_owned))
            } else {
                None
            }
        })
    }

    fn command_env_removed(cmd: &Command, key: &str) -> bool {
        use std::ffi::OsStr;

        cmd.get_envs()
            .any(|(k, v)| k == OsStr::new(key) && v.is_none())
    }

    #[test]
    fn configure_git_auth_prompt_sets_username_password_env() {
        let askpass = create_askpass_script().expect("askpass script creation");
        let mut cmd = Command::new("git");
        let auth = StagedGitAuth {
            kind: GitAuthKind::UsernamePassword,
            username: Some("alice".to_string()),
            secret: "secret-token".to_string(),
        };

        configure_git_auth_prompt(&mut cmd, Some(&auth), &askpass);

        let askpass_path = askpass
            .path
            .to_str()
            .expect("temporary askpass path should be unicode")
            .to_string();
        assert_eq!(
            command_env_value(&cmd, "GIT_ASKPASS").as_deref(),
            Some(askpass_path.as_str())
        );
        assert_eq!(
            command_env_value(&cmd, "SSH_ASKPASS").as_deref(),
            Some(askpass_path.as_str())
        );
        assert_eq!(
            command_env_value(&cmd, "SSH_ASKPASS_REQUIRE").as_deref(),
            Some("force")
        );
        assert_eq!(
            command_env_value(&cmd, GITCOMET_ASKPASS_PROMPT_LOG_ENV).as_deref(),
            askpass.prompt_log_path.to_str()
        );
        assert_eq!(
            command_env_value(&cmd, GITCOMET_AUTH_KIND_ENV).as_deref(),
            Some(GITCOMET_AUTH_KIND_USERNAME_PASSWORD)
        );
        assert_eq!(
            command_env_value(&cmd, GITCOMET_AUTH_USERNAME_ENV).as_deref(),
            Some("alice")
        );
        assert_eq!(
            command_env_value(&cmd, GITCOMET_AUTH_SECRET_ENV).as_deref(),
            Some("secret-token")
        );

        if cfg!(all(unix, not(target_os = "macos"))) && std::env::var_os("DISPLAY").is_none() {
            assert_eq!(
                command_env_value(&cmd, "DISPLAY").as_deref(),
                Some("gitcomet:0")
            );
        }
    }

    #[test]
    fn configure_git_auth_prompt_sets_passphrase_env_and_removes_username() {
        let askpass = create_askpass_script().expect("askpass script creation");
        let mut cmd = Command::new("git");
        cmd.env(GITCOMET_AUTH_USERNAME_ENV, "legacy-user");
        let auth = StagedGitAuth {
            kind: GitAuthKind::Passphrase,
            username: None,
            secret: "ssh-passphrase".to_string(),
        };

        configure_git_auth_prompt(&mut cmd, Some(&auth), &askpass);

        assert_eq!(
            command_env_value(&cmd, GITCOMET_AUTH_KIND_ENV).as_deref(),
            Some(GITCOMET_AUTH_KIND_PASSPHRASE)
        );
        assert!(command_env_removed(&cmd, GITCOMET_AUTH_USERNAME_ENV));
        assert_eq!(
            command_env_value(&cmd, GITCOMET_AUTH_SECRET_ENV).as_deref(),
            Some("ssh-passphrase")
        );
    }

    #[test]
    fn configure_git_auth_prompt_sets_host_verification_env_and_removes_username() {
        let askpass = create_askpass_script().expect("askpass script creation");
        let mut cmd = Command::new("git");
        cmd.env(GITCOMET_AUTH_USERNAME_ENV, "legacy-user");
        let auth = StagedGitAuth {
            kind: GitAuthKind::HostVerification,
            username: None,
            secret: "yes".to_string(),
        };

        configure_git_auth_prompt(&mut cmd, Some(&auth), &askpass);

        assert_eq!(
            command_env_value(&cmd, GITCOMET_AUTH_KIND_ENV).as_deref(),
            Some(GITCOMET_AUTH_KIND_HOST_VERIFICATION)
        );
        assert!(command_env_removed(&cmd, GITCOMET_AUTH_USERNAME_ENV));
        assert_eq!(
            command_env_value(&cmd, GITCOMET_AUTH_SECRET_ENV).as_deref(),
            Some("yes")
        );
    }

    #[test]
    fn configure_git_auth_prompt_without_staged_auth_clears_auth_env() {
        let askpass = create_askpass_script().expect("askpass script creation");
        let mut cmd = Command::new("git");
        cmd.env(GITCOMET_AUTH_KIND_ENV, "legacy-kind");
        cmd.env(GITCOMET_AUTH_USERNAME_ENV, "legacy-user");
        cmd.env(GITCOMET_AUTH_SECRET_ENV, "legacy-secret");

        configure_git_auth_prompt(&mut cmd, None, &askpass);

        let askpass_path = askpass
            .path
            .to_str()
            .expect("temporary askpass path should be unicode")
            .to_string();
        assert_eq!(
            command_env_value(&cmd, "GIT_ASKPASS").as_deref(),
            Some(askpass_path.as_str())
        );
        assert!(command_env_removed(&cmd, GITCOMET_AUTH_KIND_ENV));
        assert!(command_env_removed(&cmd, GITCOMET_AUTH_USERNAME_ENV));
        assert!(command_env_removed(&cmd, GITCOMET_AUTH_SECRET_ENV));
    }
}
