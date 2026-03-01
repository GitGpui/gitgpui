use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn gitgpui_bin() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_gitgpui-app")
        .map(PathBuf::from)
        .expect("CARGO_BIN_EXE_gitgpui-app is not set for integration tests")
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn run_git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git command to run");
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_git_capture(repo: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git command to run")
}

fn run_git_capture_in(cwd: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("git command to run")
}

fn write_file(repo: &Path, rel: &str, contents: &str) {
    let path = repo.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent directories");
    }
    fs::write(path, contents).expect("write file");
}

fn init_repo(repo: &Path) {
    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);
}

fn commit_all(repo: &Path, message: &str) {
    run_git(repo, &["add", "-A"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", message],
    );
}

fn configure_gitgpui_difftool(repo: &Path) {
    let bin = gitgpui_bin();
    let bin_q = shell_quote(&bin.to_string_lossy());
    let cmd = format!(
        "if [ -n \"$MERGED\" ]; then {bin_q} difftool --local \"$LOCAL\" --remote \"$REMOTE\" --path \"$MERGED\"; else {bin_q} difftool --local \"$LOCAL\" --remote \"$REMOTE\"; fi"
    );

    run_git(repo, &["config", "diff.tool", "gitgpui"]);
    run_git(repo, &["config", "difftool.gitgpui.cmd", &cmd]);
    run_git(repo, &["config", "difftool.gitgpui.trustExitCode", "true"]);
    run_git(repo, &["config", "difftool.prompt", "false"]);
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn git_difftool_invokes_gitgpui_app_for_basic_diff() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();

    init_repo(repo);
    write_file(repo, "a.txt", "before\n");
    commit_all(repo, "base");

    write_file(repo, "a.txt", "after\n");
    configure_gitgpui_difftool(repo);

    let output = run_git_capture(repo, &["difftool", "--no-prompt", "--", "a.txt"]);
    let text = output_text(&output);
    assert!(output.status.success(), "git difftool failed\n{text}");
    assert!(
        text.contains("-before"),
        "missing removed line in output\n{text}"
    );
    assert!(
        text.contains("+after"),
        "missing added line in output\n{text}"
    );
}

#[test]
fn git_difftool_handles_path_with_spaces() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();

    init_repo(repo);
    write_file(repo, "docs/spaced name.txt", "left side\n");
    commit_all(repo, "base");

    write_file(repo, "docs/spaced name.txt", "right side\n");
    configure_gitgpui_difftool(repo);

    let output = run_git_capture(
        repo,
        &["difftool", "--no-prompt", "--", "docs/spaced name.txt"],
    );
    let text = output_text(&output);
    assert!(
        output.status.success(),
        "git difftool failed for spaced path\n{text}"
    );
    assert!(
        text.contains("spaced name.txt"),
        "expected spaced filename in output\n{text}"
    );
    assert!(
        text.contains("-left side") && text.contains("+right side"),
        "missing expected line delta in output\n{text}"
    );
}

#[test]
fn git_difftool_works_from_subdirectory() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();

    init_repo(repo);
    write_file(repo, "nested/deeper/file.txt", "old value\n");
    commit_all(repo, "base");

    write_file(repo, "nested/deeper/file.txt", "new value\n");
    configure_gitgpui_difftool(repo);

    let subdir = repo.join("nested/deeper");
    let output = run_git_capture_in(&subdir, &["difftool", "--no-prompt", "--", "file.txt"]);
    let text = output_text(&output);
    assert!(
        output.status.success(),
        "git difftool failed from subdirectory\n{text}"
    );
    assert!(
        text.contains("-old value") && text.contains("+new value"),
        "missing expected delta for subdirectory invocation\n{text}"
    );
}

#[test]
fn git_difftool_dir_diff_mode_works() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();

    init_repo(repo);
    write_file(repo, "tracked.txt", "one\n");
    commit_all(repo, "base");

    write_file(repo, "tracked.txt", "two\n");
    configure_gitgpui_difftool(repo);

    let output = run_git_capture(repo, &["difftool", "--dir-diff", "--no-prompt"]);
    let text = output_text(&output);
    assert!(
        output.status.success(),
        "git difftool --dir-diff failed\n{text}"
    );
    assert!(
        text.contains("tracked.txt"),
        "expected tracked filename in dir-diff output\n{text}"
    );
}
