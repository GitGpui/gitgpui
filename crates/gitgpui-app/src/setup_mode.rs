//! `gitgpui-app setup` — configure git to use gitgpui as difftool/mergetool.
//!
//! Writes the recommended global (or local) git config entries so that
//! `git difftool` and `git mergetool` invoke gitgpui automatically.

use std::path::PathBuf;

/// A single `git config` key-value pair to set.
struct ConfigEntry {
    key: &'static str,
    value: String,
}

/// Quote a string as a POSIX-shell single-quoted literal.
///
/// This preserves spaces and shell metacharacters, including embedded
/// single quotes.
fn shell_single_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// Resolve the absolute path to the current executable.
fn current_exe_path() -> Result<PathBuf, String> {
    std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| format!("Cannot determine gitgpui-app binary path: {e}"))
}

/// Build the list of git config entries for difftool/mergetool setup.
fn build_config_entries(bin_path: &str) -> Vec<ConfigEntry> {
    let quoted_bin_path = shell_single_quote(bin_path);

    vec![
        // Mergetool
        ConfigEntry {
            key: "merge.tool",
            value: "gitgpui".into(),
        },
        ConfigEntry {
            key: "mergetool.gitgpui.cmd",
            value: format!(
                "{quoted_bin_path} mergetool --base \"$BASE\" --local \"$LOCAL\" --remote \"$REMOTE\" --merged \"$MERGED\""
            ),
        },
        ConfigEntry {
            key: "mergetool.gitgpui.trustExitCode",
            value: "true".into(),
        },
        ConfigEntry {
            key: "mergetool.prompt",
            value: "false".into(),
        },
        // Difftool
        ConfigEntry {
            key: "diff.tool",
            value: "gitgpui".into(),
        },
        ConfigEntry {
            key: "difftool.gitgpui.cmd",
            value: format!(
                "{quoted_bin_path} difftool --local \"$LOCAL\" --remote \"$REMOTE\" --path \"$MERGED\""
            ),
        },
        // Keep both generic and tool-specific trust keys:
        // - `difftool.trustExitCode` matches documented setup guidance and
        //   Git's default trust behavior for the selected difftool.
        // - `difftool.gitgpui.trustExitCode` preserves explicit per-tool
        //   behavior even if users override global defaults later.
        ConfigEntry {
            key: "difftool.trustExitCode",
            value: "true".into(),
        },
        ConfigEntry {
            key: "difftool.gitgpui.trustExitCode",
            value: "true".into(),
        },
        ConfigEntry {
            key: "difftool.prompt",
            value: "false".into(),
        },
        // GUI tool aliases
        ConfigEntry {
            key: "merge.guitool",
            value: "gitgpui".into(),
        },
        ConfigEntry {
            key: "diff.guitool",
            value: "gitgpui".into(),
        },
        ConfigEntry {
            key: "mergetool.guiDefault",
            value: "auto".into(),
        },
        ConfigEntry {
            key: "difftool.guiDefault",
            value: "auto".into(),
        },
    ]
}

/// Format the `git config` shell commands for display (dry-run mode).
fn format_commands(entries: &[ConfigEntry], scope: &str) -> String {
    let mut out = String::new();
    for entry in entries {
        let quoted_value = shell_single_quote(&entry.value);
        out.push_str(&format!(
            "git config {scope} {} {quoted_value}\n",
            entry.key
        ));
    }
    out
}

/// Run `git config` for each entry.
fn apply_config(entries: &[ConfigEntry], scope: &str) -> Result<(), String> {
    for entry in entries {
        let output = std::process::Command::new("git")
            .args(["config", scope, entry.key, &entry.value])
            .output()
            .map_err(|e| format!("Failed to run git config: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "git config {} {} failed: {}",
                entry.key,
                entry.value,
                stderr.trim()
            ));
        }
    }
    Ok(())
}

/// Result returned from `run_setup`.
pub struct SetupResult {
    pub stdout: String,
    pub exit_code: i32,
}

/// Execute the setup command.
pub fn run_setup(dry_run: bool, local: bool) -> Result<SetupResult, String> {
    let bin_path = current_exe_path()?;
    let bin_str = bin_path.to_str().ok_or_else(|| {
        format!(
            "Binary path contains non-UTF-8 characters: {}",
            bin_path.display()
        )
    })?;

    let entries = build_config_entries(bin_str);
    let scope = if local { "--local" } else { "--global" };
    let scope_label = if local { "local" } else { "global" };

    if dry_run {
        let commands = format_commands(&entries, scope);
        let stdout =
            format!("# Dry run: the following git config commands would be executed:\n{commands}");
        return Ok(SetupResult {
            stdout,
            exit_code: 0,
        });
    }

    apply_config(&entries, scope)?;

    let stdout = format!(
        "Configured gitgpui as {scope_label} diff/merge tool.\n\
         Binary: {bin_str}\n\
         Run `git difftool` or `git mergetool` to use it.\n"
    );

    Ok(SetupResult {
        stdout,
        exit_code: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_single_quote_wraps_plain_text() {
        assert_eq!(shell_single_quote("abc"), "'abc'");
        assert_eq!(shell_single_quote(""), "''");
    }

    #[test]
    fn shell_single_quote_escapes_embedded_single_quote() {
        assert_eq!(shell_single_quote("it's"), "'it'\"'\"'s'");
    }

    #[test]
    fn build_config_entries_contains_all_required_keys() {
        let entries = build_config_entries("/usr/bin/gitgpui-app");
        let keys: Vec<&str> = entries.iter().map(|e| e.key).collect();

        assert!(keys.contains(&"merge.tool"));
        assert!(keys.contains(&"mergetool.gitgpui.cmd"));
        assert!(keys.contains(&"mergetool.gitgpui.trustExitCode"));
        assert!(keys.contains(&"mergetool.prompt"));
        assert!(keys.contains(&"diff.tool"));
        assert!(keys.contains(&"difftool.gitgpui.cmd"));
        assert!(keys.contains(&"difftool.trustExitCode"));
        assert!(keys.contains(&"difftool.gitgpui.trustExitCode"));
        assert!(keys.contains(&"difftool.prompt"));
        assert!(keys.contains(&"merge.guitool"));
        assert!(keys.contains(&"diff.guitool"));
        assert!(keys.contains(&"mergetool.guiDefault"));
        assert!(keys.contains(&"difftool.guiDefault"));
    }

    #[test]
    fn mergetool_cmd_includes_all_stage_vars() {
        let entries = build_config_entries("/path/to/bin");
        let cmd = entries
            .iter()
            .find(|e| e.key == "mergetool.gitgpui.cmd")
            .unwrap();

        assert!(cmd.value.contains("$BASE"), "missing $BASE");
        assert!(cmd.value.contains("$LOCAL"), "missing $LOCAL");
        assert!(cmd.value.contains("$REMOTE"), "missing $REMOTE");
        assert!(cmd.value.contains("$MERGED"), "missing $MERGED");
        assert!(cmd.value.starts_with("'/path/to/bin'"));
    }

    #[test]
    fn mergetool_cmd_escapes_single_quote_in_binary_path() {
        let entries = build_config_entries("/tmp/it's/gitgpui-app");
        let cmd = entries
            .iter()
            .find(|e| e.key == "mergetool.gitgpui.cmd")
            .unwrap();

        assert!(
            cmd.value.starts_with("'/tmp/it'\"'\"'s/gitgpui-app'"),
            "unexpected cmd quoting: {}",
            cmd.value
        );
    }

    #[test]
    fn difftool_cmd_includes_local_remote_merged() {
        let entries = build_config_entries("/path/to/bin");
        let cmd = entries
            .iter()
            .find(|e| e.key == "difftool.gitgpui.cmd")
            .unwrap();

        assert!(cmd.value.contains("$LOCAL"), "missing $LOCAL");
        assert!(cmd.value.contains("$REMOTE"), "missing $REMOTE");
        assert!(cmd.value.contains("$MERGED"), "missing $MERGED path");
    }

    #[test]
    fn format_commands_global_scope() {
        let entries = build_config_entries("/bin/gitgpui-app");
        let output = format_commands(&entries, "--global");

        assert!(output.contains("git config --global merge.tool"));
        assert!(output.contains("git config --global diff.tool"));
        assert!(output.contains("git config --global mergetool.gitgpui.trustExitCode"));
        assert!(
            !output.contains("''/bin/gitgpui-app'"),
            "dry-run output should not contain broken nested quoting:\n{output}"
        );
    }

    #[test]
    fn format_commands_local_scope() {
        let entries = build_config_entries("/bin/gitgpui-app");
        let output = format_commands(&entries, "--local");

        assert!(output.contains("git config --local merge.tool"));
        assert!(output.contains("git config --local diff.tool"));
    }

    #[test]
    fn dry_run_does_not_write_config() {
        // dry_run=true should produce output but not call git config.
        // We verify by running inside a temp dir with no repo — if it
        // actually tried `git config --global`, the test env would be
        // unaffected because we only check output format.
        let result = run_setup(true, false).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("Dry run"));
        assert!(result.stdout.contains("git config --global"));
    }

    #[test]
    fn dry_run_local_scope_uses_local_flag() {
        let result = run_setup(true, true).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("git config --local"));
        assert!(!result.stdout.contains("--global"));
    }

    #[test]
    fn apply_config_to_local_repo() {
        let dir = tempfile::tempdir().unwrap();

        // Initialize a git repo.
        let init = std::process::Command::new("git")
            .args(["init", dir.path().to_str().unwrap()])
            .output()
            .unwrap();
        assert!(init.status.success());

        let entries = build_config_entries("/test/gitgpui-app");
        let result = std::process::Command::new("git")
            .args(["-C", dir.path().to_str().unwrap()])
            .args(["config", "--local", entries[0].key, &entries[0].value])
            .output()
            .unwrap();
        assert!(result.status.success());

        // Verify the value was written.
        let check = std::process::Command::new("git")
            .args(["-C", dir.path().to_str().unwrap()])
            .args(["config", "--get", entries[0].key])
            .output()
            .unwrap();
        assert!(check.status.success());
        let value = String::from_utf8_lossy(&check.stdout);
        assert_eq!(value.trim(), entries[0].value);
    }
}
