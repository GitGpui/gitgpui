use gitgpui_core::services::GitBackend;
use gitgpui_git_gix::GixBackend;
use std::path::Path;
use std::process::Command;

fn run_git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .env("EDITOR", "true")
        .env("VISUAL", "true")
        .status()
        .expect("git command to run");
    assert!(status.success(), "git {:?} failed", args);
}

#[test]
fn log_all_branches_includes_remote_tracking_branches() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let origin = dir.path().join("origin.git");

    std::fs::create_dir_all(&repo).unwrap();
    run_git(&repo, &["init", "-b", "main"]);
    run_git(&repo, &["config", "user.email", "you@example.com"]);
    run_git(&repo, &["config", "user.name", "You"]);
    run_git(&repo, &["config", "commit.gpgsign", "false"]);

    std::fs::write(repo.join("a.txt"), "one\n").unwrap();
    run_git(&repo, &["add", "a.txt"]);
    run_git(&repo, &["-c", "commit.gpgsign=false", "commit", "-m", "A"]);

    run_git(&repo, &["checkout", "-b", "feature"]);
    std::fs::write(repo.join("b.txt"), "two\n").unwrap();
    run_git(&repo, &["add", "b.txt"]);
    run_git(&repo, &["-c", "commit.gpgsign=false", "commit", "-m", "C"]);
    let feature_tip = {
        let out = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git rev-parse to run");
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };

    run_git(
        dir.path(),
        &["init", "--bare", "-b", "main", origin.to_str().unwrap()],
    );
    run_git(
        &repo,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    run_git(&repo, &["push", "-u", "origin", "feature"]);

    run_git(&repo, &["checkout", "main"]);
    run_git(&repo, &["branch", "-D", "feature"]);
    run_git(&repo, &["fetch", "origin"]);

    let backend = GixBackend::default();
    let opened = backend.open(&repo).unwrap();

    let head = opened.log_head_page(200, None).unwrap();
    assert!(
        !head.commits.iter().any(|c| c.id.0 == feature_tip),
        "head log unexpectedly contains feature commit"
    );

    let all = opened.log_all_branches_page(200, None).unwrap();
    assert!(
        all.commits.iter().any(|c| c.id.0 == feature_tip),
        "all-branches log should include remote-tracking branch commit"
    );
}

#[test]
fn log_all_branches_includes_nonstandard_ref_namespaces() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init", "-b", "main"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    std::fs::write(repo.join("a.txt"), "one\n").unwrap();
    run_git(repo, &["add", "a.txt"]);
    run_git(repo, &["-c", "commit.gpgsign=false", "commit", "-m", "A"]);

    run_git(repo, &["checkout", "-b", "feature"]);
    std::fs::write(repo.join("b.txt"), "two\n").unwrap();
    run_git(repo, &["add", "b.txt"]);
    run_git(repo, &["-c", "commit.gpgsign=false", "commit", "-m", "C"]);
    let feature_tip = {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git rev-parse to run");
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };

    run_git(repo, &["checkout", "main"]);
    run_git(repo, &["branch", "-D", "feature"]);
    run_git(
        repo,
        &[
            "update-ref",
            "refs/branch-heads/feature",
            feature_tip.as_str(),
        ],
    );

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();
    let all = opened.log_all_branches_page(200, None).unwrap();
    assert!(
        all.commits.iter().any(|c| c.id.0 == feature_tip),
        "all-branches log should include commits reachable from refs outside refs/heads and refs/remotes"
    );
}

#[test]
fn log_all_branches_does_not_include_tag_only_tips() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init", "-b", "main"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    std::fs::write(repo.join("a.txt"), "one\n").unwrap();
    run_git(repo, &["add", "a.txt"]);
    run_git(repo, &["-c", "commit.gpgsign=false", "commit", "-m", "A"]);

    run_git(repo, &["checkout", "-b", "tag-only"]);
    std::fs::write(repo.join("b.txt"), "two\n").unwrap();
    run_git(repo, &["add", "b.txt"]);
    run_git(repo, &["-c", "commit.gpgsign=false", "commit", "-m", "B"]);
    let tag_only_tip = {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git rev-parse to run");
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };

    run_git(
        repo,
        &[
            "-c",
            "tag.gpgSign=false",
            "tag",
            "-a",
            "-m",
            "tag",
            "v0.0",
            tag_only_tip.as_str(),
        ],
    );
    run_git(repo, &["checkout", "main"]);
    run_git(repo, &["branch", "-D", "tag-only"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();
    let all = opened.log_all_branches_page(200, None).unwrap();
    assert!(
        !all.commits.iter().any(|c| c.id.0 == tag_only_tip),
        "all-branches log should not be expanded by tag-only tips"
    );
}

#[test]
fn empty_repo_log_and_head_branch_do_not_error() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init", "-b", "main"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    assert_eq!(opened.current_branch().unwrap(), "main");
    assert!(opened.log_head_page(200, None).unwrap().commits.is_empty());
    assert!(
        opened
            .log_all_branches_page(200, None)
            .unwrap()
            .commits
            .is_empty()
    );
}
