use gitgpui_core::services::GitBackend;
use gitgpui_git_gix::GixBackend;
use std::path::Path;
use std::process::Command;

fn run_git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
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
    run_git(&repo, &["remote", "add", "origin", origin.to_str().unwrap()]);
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
