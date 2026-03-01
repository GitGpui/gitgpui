use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn gitgpui_bin() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_gitgpui-app")
        .map(PathBuf::from)
        .expect("CARGO_BIN_EXE_gitgpui-app is not set for integration tests")
}

fn run_gitgpui<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(gitgpui_bin())
        .args(args)
        .output()
        .expect("gitgpui-app command to run")
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent directories");
    }
    fs::write(path, contents).expect("write file");
}

fn output_text(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn standalone_mergetool_clean_merge_exits_zero_and_writes_output() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.txt");
    let local = dir.path().join("local.txt");
    let remote = dir.path().join("remote.txt");
    let merged = dir.path().join("nested/out/merged.txt");

    write_file(&base, "line1\nline2\nline3\n");
    write_file(&local, "LINE1\nline2\nline3\n");
    write_file(&remote, "line1\nline2\nLINE3\n");

    let output = run_gitgpui([
        OsString::from("mergetool"),
        OsString::from("--base"),
        base.as_os_str().to_owned(),
        OsString::from("--local"),
        local.as_os_str().to_owned(),
        OsString::from("--remote"),
        remote.as_os_str().to_owned(),
        OsString::from("--merged"),
        merged.as_os_str().to_owned(),
    ]);

    let text = output_text(&output);
    assert_eq!(output.status.code(), Some(0), "expected exit 0\n{text}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Auto-merged"),
        "expected auto-merge message\n{text}"
    );
    let merged_text = fs::read_to_string(&merged).expect("merged output to exist");
    assert_eq!(merged_text, "LINE1\nline2\nLINE3\n");
}

#[test]
fn standalone_mergetool_conflict_exits_one_and_writes_markers() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.txt");
    let local = dir.path().join("local.txt");
    let remote = dir.path().join("remote.txt");
    let merged = dir.path().join("merged.txt");

    write_file(&base, "line\n");
    write_file(&local, "ours\n");
    write_file(&remote, "theirs\n");

    let output = run_gitgpui([
        OsString::from("mergetool"),
        OsString::from("--base"),
        base.as_os_str().to_owned(),
        OsString::from("--local"),
        local.as_os_str().to_owned(),
        OsString::from("--remote"),
        remote.as_os_str().to_owned(),
        OsString::from("--merged"),
        merged.as_os_str().to_owned(),
    ]);

    let text = output_text(&output);
    assert_eq!(output.status.code(), Some(1), "expected exit 1\n{text}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("CONFLICT (content)"),
        "expected conflict message\n{text}"
    );

    let merged_text = fs::read_to_string(&merged).expect("merged output to exist");
    assert!(merged_text.contains("<<<<<<<"), "output:\n{merged_text}");
    assert!(merged_text.contains("======="), "output:\n{merged_text}");
    assert!(merged_text.contains(">>>>>>>"), "output:\n{merged_text}");
}

#[test]
fn standalone_mergetool_invalid_path_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.txt");
    let local = dir.path().join("local.txt");
    let missing_remote = dir.path().join("missing_remote.txt");
    let merged = dir.path().join("merged.txt");

    write_file(&base, "line\n");
    write_file(&local, "line\n");

    let output = run_gitgpui([
        OsString::from("mergetool"),
        OsString::from("--base"),
        base.as_os_str().to_owned(),
        OsString::from("--local"),
        local.as_os_str().to_owned(),
        OsString::from("--remote"),
        missing_remote.as_os_str().to_owned(),
        OsString::from("--merged"),
        merged.as_os_str().to_owned(),
    ]);

    let text = output_text(&output);
    assert_eq!(output.status.code(), Some(2), "expected exit 2\n{text}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Remote path does not exist"),
        "expected validation error\n{text}"
    );
}

#[test]
fn standalone_difftool_changed_files_exits_zero_and_prints_diff() {
    let dir = tempfile::tempdir().unwrap();
    let local = dir.path().join("left.txt");
    let remote = dir.path().join("right.txt");

    write_file(&local, "left\n");
    write_file(&remote, "right\n");

    let output = run_gitgpui([
        OsString::from("difftool"),
        OsString::from("--local"),
        local.as_os_str().to_owned(),
        OsString::from("--remote"),
        remote.as_os_str().to_owned(),
        OsString::from("--path"),
        OsString::from("src/file.txt"),
    ]);

    let text = output_text(&output);
    assert_eq!(output.status.code(), Some(0), "expected exit 0\n{text}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("@@"), "expected unified diff hunk\n{text}");
    assert!(stdout.contains("--- a/src/file.txt"), "expected left label\n{text}");
    assert!(
        stdout.contains("+++ b/src/file.txt"),
        "expected right label\n{text}"
    );
}

#[test]
fn standalone_difftool_missing_input_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    let local = dir.path().join("left.txt");
    let missing_remote = dir.path().join("missing.txt");
    write_file(&local, "left\n");

    let output = run_gitgpui([
        OsString::from("difftool"),
        OsString::from("--local"),
        local.as_os_str().to_owned(),
        OsString::from("--remote"),
        missing_remote.as_os_str().to_owned(),
    ]);

    let text = output_text(&output);
    assert_eq!(output.status.code(), Some(2), "expected exit 2\n{text}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Remote path does not exist"),
        "expected validation error\n{text}"
    );
}
