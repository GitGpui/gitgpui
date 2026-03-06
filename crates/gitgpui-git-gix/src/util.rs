use gitgpui_core::domain::{
    Commit, CommitFileChange, CommitId, FileStatusKind, LogPage, RemoteBranch,
};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{CommandOutput, Result};
use std::path::{Path, PathBuf};
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
        let path_len = path.to_string_lossy().len() + 1;

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

pub(crate) fn run_git_with_output(mut cmd: Command, label: &str) -> Result<CommandOutput> {
    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    let exit_code = output.status.code();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

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
        _ => FileStatusKind::Modified,
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
        let text = String::from_utf8_lossy(patch_bytes);
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
        let raw = String::from_utf8_lossy(GITPY_DIFF_FILE_WITH_COLON);
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
}
