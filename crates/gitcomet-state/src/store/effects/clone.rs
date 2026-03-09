use crate::msg::Msg;
use gitcomet_core::error::{Error, ErrorKind};
use gitcomet_core::services::CommandOutput;
use std::io::{BufRead as _, BufReader, Read as _};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::super::executor::TaskExecutor;
use super::util::send_or_log;

const GIT_COMMAND_TIMEOUT_ENV: &str = "GITCOMET_GIT_COMMAND_TIMEOUT_SECS";
const GIT_COMMAND_TIMEOUT_DEFAULT_SECS: u64 = 300;
const GIT_COMMAND_WAIT_POLL: Duration = Duration::from_millis(100);
const ALLOWED_CLONE_URL_SCHEMES: [&str; 4] = ["https", "ssh", "git", "file"];

fn git_command_timeout() -> Duration {
    std::env::var(GIT_COMMAND_TIMEOUT_ENV)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(GIT_COMMAND_TIMEOUT_DEFAULT_SECS))
}

fn is_windows_drive_path(url: &str) -> bool {
    let bytes = url.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn explicit_url_scheme_end(url: &str) -> Option<usize> {
    if is_windows_drive_path(url) {
        return None;
    }

    let mut chars = url.char_indices();
    let (_, first) = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }

    for (idx, ch) in chars {
        if ch == ':' {
            return Some(idx);
        }
        if !(ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.')) {
            return None;
        }
    }

    None
}

fn validate_clone_url(url: &str) -> Result<(), Error> {
    let url = url.trim();
    if url.is_empty() {
        return Err(Error::new(ErrorKind::Backend(
            "clone URL cannot be empty".to_string(),
        )));
    }

    let Some(scheme_end) = explicit_url_scheme_end(url) else {
        return Ok(());
    };

    let scheme = url[..scheme_end].to_ascii_lowercase();
    if !ALLOWED_CLONE_URL_SCHEMES.contains(&scheme.as_str()) {
        return Err(Error::new(ErrorKind::Backend(format!(
            "unsupported clone URL scheme `{scheme}` (allowed: https, ssh, git, file)"
        ))));
    }

    if !url[scheme_end..].starts_with("://") {
        return Err(Error::new(ErrorKind::Backend(format!(
            "invalid clone URL format for `{scheme}`; expected `{scheme}://...`"
        ))));
    }

    Ok(())
}

pub(super) fn schedule_clone_repo(
    executor: &TaskExecutor,
    msg_tx: mpsc::Sender<Msg>,
    url: String,
    dest: PathBuf,
) {
    executor.spawn(move || {
        if let Err(err) = validate_clone_url(&url) {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::CloneRepoFinished {
                    url,
                    dest,
                    result: Err(err),
                }),
            );
            return;
        }

        let mut cmd = Command::new("git");
        cmd.arg("-c")
            .arg("color.ui=false")
            .arg("clone")
            .arg("--progress")
            .arg(&url)
            .arg(&dest)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .env("GIT_TERMINAL_PROMPT", "0");

        let command_str = format!("git clone --progress {} {}", url, dest.display());

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                let err = Error::new(ErrorKind::Io(e.kind()));
                send_or_log(
                    &msg_tx,
                    Msg::Internal(crate::msg::InternalMsg::CloneRepoFinished {
                        url,
                        dest,
                        result: Err(err),
                    }),
                );
                return;
            }
        };

        let stdout = child.stdout.take();
        let stdout_handle = std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut stdout) = stdout {
                let _ = stdout.read_to_end(&mut buf);
            }
            String::from_utf8_lossy(&buf).into_owned()
        });

        let stderr = child.stderr.take();
        let progress_dest = dest.clone();
        let progress_tx = msg_tx.clone();
        let stderr_handle = std::thread::spawn(move || {
            let mut stderr_acc = String::new();
            if let Some(stderr) = stderr {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    stderr_acc.push_str(&line);
                    stderr_acc.push('\n');
                    send_or_log(
                        &progress_tx,
                        Msg::Internal(crate::msg::InternalMsg::CloneRepoProgress {
                            dest: progress_dest.clone(),
                            line,
                        }),
                    );
                }
            }
            stderr_acc
        });

        let timeout = git_command_timeout();
        let start = Instant::now();
        let mut timed_out = false;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        timed_out = true;
                        let _ = child.kill();
                        break child.wait();
                    }
                    std::thread::sleep(GIT_COMMAND_WAIT_POLL);
                }
                Err(e) => break Err(e),
            }
        };
        let stdout_str = stdout_handle.join().unwrap_or_default();
        let stderr_acc = stderr_handle.join().unwrap_or_default();

        let result = match status {
            Ok(status) => {
                if timed_out {
                    Err(Error::new(ErrorKind::Backend(format!(
                        "{command_str} timed out after {} seconds (set {GIT_COMMAND_TIMEOUT_ENV} to override)",
                        timeout.as_secs()
                    ))))
                } else {
                    let out = CommandOutput {
                        command: command_str,
                        stdout: stdout_str,
                        stderr: stderr_acc,
                        exit_code: status.code(),
                    };
                    if status.success() {
                        Ok(out)
                    } else {
                        let combined = out.combined();
                        let message = if combined.is_empty() {
                            format!("{} failed", out.command)
                        } else {
                            format!("{} failed: {combined}", out.command)
                        };
                        Err(Error::new(ErrorKind::Backend(message)))
                    }
                }
            }
            Err(e) => Err(Error::new(ErrorKind::Io(e.kind()))),
        };

        let ok = result.is_ok();
        send_or_log(
            &msg_tx,
            Msg::Internal(crate::msg::InternalMsg::CloneRepoFinished {
                url: url.clone(),
                dest: dest.clone(),
                result,
            }),
        );

        if ok {
            send_or_log(&msg_tx, Msg::OpenRepo(dest));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::validate_clone_url;

    #[test]
    fn validate_clone_url_accepts_allowlisted_schemes() {
        assert!(validate_clone_url("https://example.com/org/repo.git").is_ok());
        assert!(validate_clone_url("ssh://git@example.com/org/repo.git").is_ok());
        assert!(validate_clone_url("git://example.com/org/repo.git").is_ok());
        assert!(validate_clone_url("file:///tmp/repo.git").is_ok());
    }

    #[test]
    fn validate_clone_url_rejects_unallowlisted_schemes() {
        assert!(validate_clone_url("ext::sh -c touch /tmp/pwned").is_err());
        assert!(validate_clone_url("http://example.com/org/repo.git").is_err());
    }

    #[test]
    fn validate_clone_url_keeps_schemeless_inputs_working() {
        assert!(validate_clone_url("/tmp/repo.git").is_ok());
        assert!(validate_clone_url("git@github.com:org/repo.git").is_ok());
        assert!(validate_clone_url("C:\\repos\\repo.git").is_ok());
    }

    #[test]
    fn validate_clone_url_rejects_malformed_allowlisted_schemes() {
        assert!(validate_clone_url("ssh:git@example.com/org/repo.git").is_err());
    }
}
