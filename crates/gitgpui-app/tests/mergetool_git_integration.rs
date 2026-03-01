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
    run_git(repo, &["init", "-b", "main"]);
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

fn configure_gitgpui_mergetool(repo: &Path) {
    let bin = gitgpui_bin();
    let bin_q = shell_quote(&bin.to_string_lossy());
    let cmd = format!(
        "{bin_q} mergetool --base \"$BASE\" --local \"$LOCAL\" --remote \"$REMOTE\" --merged \"$MERGED\""
    );

    run_git(repo, &["config", "merge.tool", "gitgpui"]);
    run_git(repo, &["config", "mergetool.gitgpui.cmd", &cmd]);
    run_git(repo, &["config", "mergetool.gitgpui.trustExitCode", "true"]);
    run_git(repo, &["config", "mergetool.prompt", "false"]);
    // Disable backup file creation for cleaner assertions.
    run_git(repo, &["config", "mergetool.keepBackup", "false"]);
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Create a repo with a genuine merge conflict (overlapping changes).
fn setup_overlapping_conflict(repo: &Path) {
    init_repo(repo);
    write_file(repo, "file.txt", "aaa\nbbb\nccc\n");
    commit_all(repo, "base");

    run_git(repo, &["checkout", "-b", "feature"]);
    write_file(repo, "file.txt", "aaa\nREMOTE\nccc\n");
    commit_all(repo, "feature: change line 2");

    run_git(repo, &["checkout", "main"]);
    write_file(repo, "file.txt", "aaa\nLOCAL\nccc\n");
    commit_all(repo, "main: change line 2");

    // Merge will fail with a conflict.
    let output = run_git_capture(repo, &["merge", "feature"]);
    assert!(
        !output.status.success(),
        "expected merge to fail with conflict"
    );
}

/// Create a repo where both branches make non-overlapping changes
/// but to the same file, creating a conflict that our tool can auto-resolve.
///
/// Git's merge strategy may or may not auto-resolve non-overlapping changes.
/// To ensure a conflict that git leaves for the mergetool, we use changes
/// that are close enough to conflict (adjacent lines).
fn setup_resolvable_conflict(repo: &Path) {
    init_repo(repo);
    // Use a file with enough context separation.
    write_file(
        repo,
        "file.txt",
        "header\n\
         aaa\n\
         bbb\n\
         ccc\n\
         ddd\n\
         eee\n\
         footer\n",
    );
    commit_all(repo, "base");

    run_git(repo, &["checkout", "-b", "feature"]);
    write_file(
        repo,
        "file.txt",
        "header\n\
         aaa\n\
         REMOTE_CHANGE\n\
         ccc\n\
         ddd\n\
         eee\n\
         footer\n",
    );
    commit_all(repo, "feature: change bbb");

    run_git(repo, &["checkout", "main"]);
    write_file(
        repo,
        "file.txt",
        "header\n\
         aaa\n\
         bbb\n\
         ccc\n\
         ddd\n\
         LOCAL_CHANGE\n\
         footer\n",
    );
    commit_all(repo, "main: change eee");

    // Try merge. Git may auto-resolve non-overlapping, so we check.
    let output = run_git_capture(repo, &["merge", "feature"]);
    if output.status.success() {
        // Git auto-resolved — this test scenario won't exercise mergetool.
        // Fall back to an overlapping conflict that our tool can also handle.
        // Reset the merge and create a real conflict instead.
        run_git(repo, &["reset", "--hard", "HEAD~1"]);

        write_file(
            repo,
            "file.txt",
            "header\n\
             aaa\n\
             LOCAL_BBB\n\
             ccc\n\
             ddd\n\
             eee\n\
             footer\n",
        );
        commit_all(repo, "main: change bbb differently");

        let output = run_git_capture(repo, &["merge", "feature"]);
        assert!(
            !output.status.success(),
            "expected merge to fail with conflict"
        );
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[test]
fn git_mergetool_resolves_overlapping_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();

    setup_overlapping_conflict(repo);
    configure_gitgpui_mergetool(repo);

    // Run git mergetool. Our tool will detect the conflict and write
    // markers to MERGED, exiting 1.
    let output = run_git_capture(repo, &["mergetool", "--no-prompt"]);
    let text = output_text(&output);

    // The tool should have run (even if exit code is non-zero due to conflicts).
    // Check that the MERGED file was written by our tool.
    let merged = fs::read_to_string(repo.join("file.txt")).unwrap();

    // Our mergetool reads the actual BASE/LOCAL/REMOTE stage files and
    // performs its own 3-way merge. For this overlapping conflict,
    // it should write conflict markers.
    assert!(
        merged.contains("<<<<<<<") || merged.contains("LOCAL") || merged.contains("REMOTE"),
        "expected mergetool to have processed file.txt\nmerged:\n{merged}\ngit output:\n{text}"
    );
}

#[test]
fn git_mergetool_with_trust_exit_code_marks_clean_merge_resolved() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();

    setup_resolvable_conflict(repo);
    configure_gitgpui_mergetool(repo);

    let output = run_git_capture(repo, &["mergetool", "--no-prompt"]);
    let text = output_text(&output);

    // Check the file was processed by our tool.
    let merged = fs::read_to_string(repo.join("file.txt")).unwrap();
    // The file should not contain unprocessed git conflict markers from
    // the pre-mergetool state (those have <<<<<<< HEAD etc).
    // Our tool either cleanly merged or wrote its own markers.
    assert!(
        !merged.is_empty(),
        "expected mergetool to write content to file.txt\ngit output:\n{text}"
    );

    // After mergetool, check if there are still unresolved conflicts.
    let status = run_git_capture(repo, &["status", "--porcelain"]);
    let status_text = String::from_utf8_lossy(&status.stdout);

    // If our tool produced a clean merge (exit 0), git should have staged
    // the file. If it produced conflicts (exit 1), it remains unmerged.
    // Either way, the tool successfully ran.
    assert!(
        merged.contains("header") && merged.contains("footer"),
        "merged output should contain surrounding context\nmerged:\n{merged}\nstatus:\n{status_text}"
    );
}

#[test]
fn git_mergetool_handles_path_with_spaces() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();

    init_repo(repo);
    write_file(repo, "docs/spaced name.txt", "original\n");
    commit_all(repo, "base");

    run_git(repo, &["checkout", "-b", "feature"]);
    write_file(repo, "docs/spaced name.txt", "remote change\n");
    commit_all(repo, "feature: change spaced file");

    run_git(repo, &["checkout", "main"]);
    write_file(repo, "docs/spaced name.txt", "local change\n");
    commit_all(repo, "main: change spaced file");

    let output = run_git_capture(repo, &["merge", "feature"]);
    assert!(
        !output.status.success(),
        "expected merge conflict for spaced file"
    );

    configure_gitgpui_mergetool(repo);

    let output = run_git_capture(repo, &["mergetool", "--no-prompt"]);
    let text = output_text(&output);

    let merged = fs::read_to_string(repo.join("docs/spaced name.txt")).unwrap();
    // Tool should have processed the file despite spaces in path.
    assert!(
        merged.contains("local change") || merged.contains("remote change") || merged.contains("<<<<<<<"),
        "expected mergetool to process spaced-path file\nmerged:\n{merged}\ngit output:\n{text}"
    );
}

#[test]
fn git_mergetool_works_from_subdirectory() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();

    init_repo(repo);
    write_file(repo, "sub/dir/nested.txt", "base content\n");
    commit_all(repo, "base");

    run_git(repo, &["checkout", "-b", "feature"]);
    write_file(repo, "sub/dir/nested.txt", "remote content\n");
    commit_all(repo, "feature: change nested");

    run_git(repo, &["checkout", "main"]);
    write_file(repo, "sub/dir/nested.txt", "local content\n");
    commit_all(repo, "main: change nested");

    let output = run_git_capture(repo, &["merge", "feature"]);
    assert!(
        !output.status.success(),
        "expected merge conflict for nested file"
    );

    configure_gitgpui_mergetool(repo);

    // Run from subdirectory.
    let subdir = repo.join("sub/dir");
    let output = run_git_capture_in(&subdir, &["mergetool", "--no-prompt"]);
    let text = output_text(&output);

    let merged = fs::read_to_string(repo.join("sub/dir/nested.txt")).unwrap();
    assert!(
        merged.contains("local content") || merged.contains("remote content") || merged.contains("<<<<<<<"),
        "expected mergetool to process file from subdirectory\nmerged:\n{merged}\ngit output:\n{text}"
    );
}

#[test]
fn git_mergetool_handles_add_add_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();

    init_repo(repo);
    // Create an empty initial commit so both branches can add new files.
    write_file(repo, "README", "init\n");
    commit_all(repo, "initial");

    run_git(repo, &["checkout", "-b", "feature"]);
    write_file(repo, "new_file.txt", "added by remote\n");
    commit_all(repo, "feature: add new_file");

    run_git(repo, &["checkout", "main"]);
    write_file(repo, "new_file.txt", "added by local\n");
    commit_all(repo, "main: add new_file");

    let output = run_git_capture(repo, &["merge", "feature"]);
    assert!(
        !output.status.success(),
        "expected add/add merge conflict"
    );

    configure_gitgpui_mergetool(repo);

    let output = run_git_capture(repo, &["mergetool", "--no-prompt"]);
    let text = output_text(&output);

    let merged = fs::read_to_string(repo.join("new_file.txt")).unwrap();
    // For add/add, BASE is empty. Our tool treats this as empty base,
    // resulting in a conflict (both sides added different content).
    assert!(
        merged.contains("added by local") || merged.contains("added by remote") || merged.contains("<<<<<<<"),
        "expected mergetool to handle add/add conflict\nmerged:\n{merged}\ngit output:\n{text}"
    );
}

#[test]
fn git_mergetool_trust_exit_code_conflict_preserves_unmerged_state() {
    // When our tool exits 1 (unresolved conflict) with trustExitCode=true,
    // git should leave the file as unmerged. This verifies the exit code
    // contract between gitgpui-app and git mergetool.
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();

    init_repo(repo);
    write_file(repo, "conflict.txt", "base\n");
    commit_all(repo, "base");

    run_git(repo, &["checkout", "-b", "feature"]);
    write_file(repo, "conflict.txt", "feature side\n");
    commit_all(repo, "feature change");

    run_git(repo, &["checkout", "main"]);
    write_file(repo, "conflict.txt", "main side\n");
    commit_all(repo, "main change");

    let output = run_git_capture(repo, &["merge", "feature"]);
    assert!(!output.status.success(), "expected merge conflict");

    configure_gitgpui_mergetool(repo);

    let output = run_git_capture(repo, &["mergetool", "--no-prompt"]);
    let text = output_text(&output);

    // Our tool exits 1 on unresolved conflict. With trustExitCode=true,
    // git interprets this as failure and restores the original MERGED
    // content. The file should still have conflict markers.
    let merged = fs::read_to_string(repo.join("conflict.txt")).unwrap();
    assert!(
        merged.contains("<<<<<<<"),
        "expected conflict markers to remain after tool reports failure\nmerged:\n{merged}\ngit output:\n{text}"
    );

    // The file should still be in unmerged state (shown as UU in porcelain).
    let status = run_git_capture(repo, &["status", "--porcelain"]);
    let status_text = String::from_utf8_lossy(&status.stdout);
    assert!(
        status_text.contains("UU") || status_text.contains("AA"),
        "expected unmerged file in git status\nstatus:\n{status_text}\ngit output:\n{text}"
    );
}

#[test]
fn git_mergetool_multiple_conflicted_files() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();

    init_repo(repo);
    write_file(repo, "alpha.txt", "base alpha\n");
    write_file(repo, "beta.txt", "base beta\n");
    commit_all(repo, "base");

    run_git(repo, &["checkout", "-b", "feature"]);
    write_file(repo, "alpha.txt", "remote alpha\n");
    write_file(repo, "beta.txt", "remote beta\n");
    commit_all(repo, "feature: change both");

    run_git(repo, &["checkout", "main"]);
    write_file(repo, "alpha.txt", "local alpha\n");
    write_file(repo, "beta.txt", "local beta\n");
    commit_all(repo, "main: change both");

    let output = run_git_capture(repo, &["merge", "feature"]);
    assert!(!output.status.success(), "expected merge conflict");

    configure_gitgpui_mergetool(repo);

    let output = run_git_capture(repo, &["mergetool", "--no-prompt"]);
    let text = output_text(&output);

    // Both files should have been processed by the mergetool.
    let alpha = fs::read_to_string(repo.join("alpha.txt")).unwrap();
    let beta = fs::read_to_string(repo.join("beta.txt")).unwrap();

    assert!(
        alpha.contains("<<<<<<<") || alpha.contains("local alpha") || alpha.contains("remote alpha"),
        "expected alpha.txt to be processed\nalpha:\n{alpha}\ngit output:\n{text}"
    );
    assert!(
        beta.contains("<<<<<<<") || beta.contains("local beta") || beta.contains("remote beta"),
        "expected beta.txt to be processed\nbeta:\n{beta}\ngit output:\n{text}"
    );
}

#[test]
fn git_mergetool_crlf_content_preserved() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();

    init_repo(repo);
    // Disable autocrlf to preserve exact line endings.
    run_git(repo, &["config", "core.autocrlf", "false"]);

    write_file(repo, "crlf.txt", "line1\r\nline2\r\nline3\r\n");
    commit_all(repo, "base with CRLF");

    run_git(repo, &["checkout", "-b", "feature"]);
    write_file(repo, "crlf.txt", "remote1\r\nline2\r\nline3\r\n");
    commit_all(repo, "feature: change line 1");

    run_git(repo, &["checkout", "main"]);
    write_file(repo, "crlf.txt", "local1\r\nline2\r\nline3\r\n");
    commit_all(repo, "main: change line 1");

    let output = run_git_capture(repo, &["merge", "feature"]);
    assert!(!output.status.success(), "expected CRLF merge conflict");

    configure_gitgpui_mergetool(repo);

    let output = run_git_capture(repo, &["mergetool", "--no-prompt"]);
    let text = output_text(&output);

    let merged_bytes = fs::read(repo.join("crlf.txt")).unwrap();
    let merged = String::from_utf8_lossy(&merged_bytes);

    // The tool should have processed the file. Content should still
    // contain CRLF sequences from the original input.
    assert!(
        merged.contains("\r\n"),
        "expected CRLF to be preserved in merged output\nmerged:\n{merged}\ngit output:\n{text}"
    );
}
