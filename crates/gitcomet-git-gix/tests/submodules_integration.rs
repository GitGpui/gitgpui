use gitcomet_core::services::GitBackend;
use gitcomet_git_gix::GixBackend;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(windows)]
use std::sync::OnceLock;

#[cfg(windows)]
const NULL_DEVICE: &str = "NUL";
#[cfg(not(windows))]
const NULL_DEVICE: &str = "/dev/null";

fn git_command() -> Command {
    let mut cmd = Command::new("git");
    cmd.env("GIT_CONFIG_NOSYSTEM", "1");
    cmd.env("GIT_CONFIG_GLOBAL", NULL_DEVICE);
    cmd
}

fn run_git(repo: &Path, args: &[&str]) {
    let status = git_command()
        .arg("-C")
        .arg(repo)
        .args(args)
        .status()
        .expect("git command to run");
    assert!(status.success(), "git {:?} failed", args);
}

fn init_repo_with_seed(repo: &Path, file: &str, contents: &str, message: &str) {
    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);
    run_git(repo, &["config", "core.autocrlf", "false"]);
    run_git(repo, &["config", "core.eol", "lf"]);

    fs::write(repo.join(file), contents).expect("write seed file");
    run_git(repo, &["add", file]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", message],
    );
}

#[cfg(windows)]
fn is_git_shell_startup_failure(text: &str) -> bool {
    text.contains("sh.exe: *** fatal error -")
        && (text.contains("couldn't create signal pipe") || text.contains("CreateFileMapping"))
}

#[cfg(windows)]
fn git_shell_available_for_submodule_tests() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let output = match Command::new("git")
            .args(["difftool", "--tool-help"])
            .output()
        {
            Ok(output) => output,
            Err(_) => return true,
        };
        if output.status.success() {
            return true;
        }
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        !is_git_shell_startup_failure(&text)
    })
}

fn require_git_shell_for_submodule_tests() -> bool {
    #[cfg(windows)]
    {
        if !git_shell_available_for_submodule_tests() {
            eprintln!(
                "skipping submodule integration test: Git-for-Windows shell startup failed in this environment"
            );
            return false;
        }
    }
    true
}

#[test]
fn list_submodules_ignores_missing_gitmodules_mapping() {
    if !require_git_shell_for_submodule_tests() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    let sub_repo = root.join("sub");
    let parent_repo = root.join("parent");
    fs::create_dir_all(&sub_repo).unwrap();
    fs::create_dir_all(&parent_repo).unwrap();

    init_repo_with_seed(&sub_repo, "file.txt", "hi\n", "init");
    init_repo_with_seed(&parent_repo, "seed.txt", "seed\n", "seed");

    let status = git_command()
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

#[test]
fn submodule_add_update_remove_round_trip() {
    if !require_git_shell_for_submodule_tests() {
        return;
    }
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path();

    let sub_repo = root.join("sub source");
    let parent_repo = root.join("parent repo");
    fs::create_dir_all(&sub_repo).expect("create sub repository directory");
    fs::create_dir_all(&parent_repo).expect("create parent repository directory");

    init_repo_with_seed(&sub_repo, "file.txt", "hello\n", "seed submodule");
    init_repo_with_seed(&parent_repo, "seed.txt", "seed\n", "seed parent");
    run_git(&parent_repo, &["config", "protocol.file.allow", "always"]);

    let backend = GixBackend;
    let opened = backend.open(&parent_repo).expect("open parent repository");

    let submodule_path = Path::new("mods/sub-one");
    let add_output = opened
        .add_submodule_with_output(sub_repo.to_string_lossy().as_ref(), submodule_path)
        .expect("add submodule");
    assert_eq!(add_output.exit_code, Some(0));

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

    let listed = opened.list_submodules().expect("list submodules after add");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].path, PathBuf::from("mods/sub-one"));
    assert_eq!(
        listed[0].status,
        gitcomet_core::domain::SubmoduleStatus::UpToDate
    );
    assert_eq!(listed[0].head.as_ref().len(), 40);

    let update_output = opened
        .update_submodules_with_output()
        .expect("update submodules");
    assert_eq!(update_output.exit_code, Some(0));

    let remove_output = opened
        .remove_submodule_with_output(submodule_path)
        .expect("remove submodule");
    assert_eq!(remove_output.exit_code, Some(0));
    assert!(remove_output.command.contains("Remove submodule"));

    let listed_after_remove = opened
        .list_submodules()
        .expect("list submodules after remove");
    assert!(listed_after_remove.is_empty());
    assert!(!parent_repo.join("mods/sub-one").exists());
}
