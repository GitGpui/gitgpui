use gitgpui_core::services::GitBackend;
use gitgpui_git_gix::GixBackend;
use std::fs;
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
fn list_submodules_ignores_missing_gitmodules_mapping() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    let sub_repo = root.join("sub");
    let parent_repo = root.join("parent");
    fs::create_dir_all(&sub_repo).unwrap();
    fs::create_dir_all(&parent_repo).unwrap();

    run_git(&sub_repo, &["init"]);
    run_git(&sub_repo, &["config", "user.email", "you@example.com"]);
    run_git(&sub_repo, &["config", "user.name", "You"]);
    run_git(&sub_repo, &["config", "commit.gpgsign", "false"]);
    fs::write(sub_repo.join("file.txt"), "hi\n").unwrap();
    run_git(&sub_repo, &["add", "file.txt"]);
    run_git(
        &sub_repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    run_git(&parent_repo, &["init"]);
    run_git(&parent_repo, &["config", "user.email", "you@example.com"]);
    run_git(&parent_repo, &["config", "user.name", "You"]);
    run_git(&parent_repo, &["config", "commit.gpgsign", "false"]);

    let status = Command::new("git")
        .arg("-C")
        .arg(&parent_repo)
        .arg("-c")
        .arg("protocol.file.allow=always")
        .arg("submodule")
        .arg("add")
        .arg(&sub_repo)
        .arg("submod")
        .status()
        .expect("git submodule add to run");
    assert!(status.success(), "git submodule add failed");

    run_git(
        &parent_repo,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "add submodule",
        ],
    );

    fs::write(parent_repo.join(".gitmodules"), "").unwrap();
    run_git(&parent_repo, &["add", ".gitmodules"]);

    let output = Command::new("git")
        .arg("-C")
        .arg(&parent_repo)
        .args(["submodule", "status", "--recursive"])
        .output()
        .expect("git submodule status to run");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("no submodule mapping found in .gitmodules for path"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let backend = GixBackend;
    let opened = backend.open(&parent_repo).unwrap();

    let submodules = opened.list_submodules().unwrap();
    assert!(submodules.is_empty());
}
