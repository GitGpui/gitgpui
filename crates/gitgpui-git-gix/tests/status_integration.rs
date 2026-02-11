use gitgpui_core::domain::{DiffArea, DiffTarget, FileConflictKind, FileStatusKind};
use gitgpui_core::services::ConflictSide;
use gitgpui_core::services::GitBackend;
use gitgpui_git_gix::GixBackend;
use std::fs;
use std::path::{Path, PathBuf};
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

fn run_git_expect_failure(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .status()
        .expect("git command to run");
    assert!(!status.success(), "expected git {:?} to fail", args);
}

fn write(repo: &Path, rel: &str, contents: &str) -> PathBuf {
    let path = repo.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, contents).unwrap();
    path
}

fn write_bytes(repo: &Path, rel: &str, contents: &[u8]) -> PathBuf {
    let path = repo.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, contents).unwrap();
    path
}

fn png_1x1_rgba(r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
    fn push_be_u32(out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&v.to_be_bytes());
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &byte in bytes {
            crc ^= byte as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320u32 & mask);
            }
        }
        !crc
    }

    fn adler32(bytes: &[u8]) -> u32 {
        const MOD: u32 = 65521;
        let mut a = 1u32;
        let mut b = 0u32;
        for &byte in bytes {
            a = (a + byte as u32) % MOD;
            b = (b + a) % MOD;
        }
        (b << 16) | a
    }

    let raw = [0u8, r, g, b, a];
    let len = raw.len() as u16;
    let nlen = !len;

    let mut zlib = Vec::new();
    zlib.push(0x78);
    zlib.push(0x01);
    zlib.push(0x01);
    zlib.extend_from_slice(&len.to_le_bytes());
    zlib.extend_from_slice(&nlen.to_le_bytes());
    zlib.extend_from_slice(&raw);
    push_be_u32(&mut zlib, adler32(&raw));

    let mut out = Vec::new();
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut ihdr = Vec::new();
    push_be_u32(&mut ihdr, 1);
    push_be_u32(&mut ihdr, 1);
    ihdr.push(8);
    ihdr.push(6);
    ihdr.push(0);
    ihdr.push(0);
    ihdr.push(0);
    push_be_u32(&mut out, ihdr.len() as u32);
    out.extend_from_slice(b"IHDR");
    out.extend_from_slice(&ihdr);
    push_be_u32(&mut out, crc32(&[b"IHDR".as_slice(), &ihdr].concat()));

    push_be_u32(&mut out, zlib.len() as u32);
    out.extend_from_slice(b"IDAT");
    out.extend_from_slice(&zlib);
    push_be_u32(&mut out, crc32(&[b"IDAT".as_slice(), &zlib].concat()));

    push_be_u32(&mut out, 0);
    out.extend_from_slice(b"IEND");
    push_be_u32(&mut out, crc32(b"IEND"));

    out
}

#[test]
fn status_separates_staged_and_unstaged() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    write(repo, "a.txt", "one\ntwo\n");
    run_git(repo, &["add", "a.txt"]);
    write(repo, "b.txt", "untracked\n");

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();
    let status = opened.status().unwrap();

    assert_eq!(status.staged.len(), 1);
    assert_eq!(status.staged[0].path, PathBuf::from("a.txt"));
    assert_eq!(status.staged[0].kind, FileStatusKind::Modified);

    assert_eq!(status.unstaged.len(), 1);
    assert_eq!(status.unstaged[0].path, PathBuf::from("b.txt"));
    assert_eq!(status.unstaged[0].kind, FileStatusKind::Untracked);
}

#[test]
fn status_lists_untracked_files_in_directories() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);

    write(repo, "dir/a.txt", "one\n");
    write(repo, "dir/b.txt", "two\n");

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();
    let status = opened.status().unwrap();

    assert_eq!(status.unstaged.len(), 2);
    assert!(
        status
            .unstaged
            .iter()
            .any(|e| e.path == PathBuf::from("dir/a.txt") && e.kind == FileStatusKind::Untracked)
    );
    assert!(
        status
            .unstaged
            .iter()
            .any(|e| e.path == PathBuf::from("dir/b.txt") && e.kind == FileStatusKind::Untracked)
    );
}

#[test]
fn diff_unified_works_for_staged_and_unstaged() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    write(repo, "a.txt", "one\ntwo\n");

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    let unstaged = opened
        .diff_unified(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Unstaged,
        })
        .unwrap();
    assert!(unstaged.contains("@@"));

    run_git(repo, &["add", "a.txt"]);

    let staged = opened
        .diff_unified(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Staged,
        })
        .unwrap();
    assert!(staged.contains("@@"));
}

#[test]
fn diff_file_text_reports_old_and_new_for_working_tree_and_commits() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    write(repo, "a.txt", "one\ntwo\n");

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    let unstaged = opened
        .diff_file_text(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Unstaged,
        })
        .unwrap()
        .expect("file diff for unstaged changes");
    assert_eq!(unstaged.path, PathBuf::from("a.txt"));
    assert_eq!(unstaged.old.as_deref(), Some("one\n"));
    assert_eq!(unstaged.new.as_deref(), Some("one\ntwo\n"));

    run_git(repo, &["add", "a.txt"]);

    let staged = opened
        .diff_file_text(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Staged,
        })
        .unwrap()
        .expect("file diff for staged changes");
    assert_eq!(staged.old.as_deref(), Some("one\n"));
    assert_eq!(staged.new.as_deref(), Some("one\ntwo\n"));

    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "second"],
    );
    let head = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git rev-parse to run");
    assert!(head.status.success());
    let head = String::from_utf8(head.stdout).unwrap().trim().to_string();

    let commit = opened
        .diff_file_text(&DiffTarget::Commit {
            commit_id: gitgpui_core::domain::CommitId(head),
            path: Some(PathBuf::from("a.txt")),
        })
        .unwrap()
        .expect("file diff for commit");
    assert_eq!(commit.old.as_deref(), Some("one\n"));
    assert_eq!(commit.new.as_deref(), Some("one\ntwo\n"));
}

#[test]
fn diff_file_image_reports_old_and_new_for_working_tree_and_commits() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    let old_png = png_1x1_rgba(0, 0, 0, 255);
    let new_png = png_1x1_rgba(255, 0, 0, 255);

    write_bytes(repo, "img.png", &old_png);
    run_git(repo, &["add", "img.png"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    write_bytes(repo, "img.png", &new_png);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    let unstaged = opened
        .diff_file_image(&DiffTarget::WorkingTree {
            path: PathBuf::from("img.png"),
            area: DiffArea::Unstaged,
        })
        .unwrap()
        .expect("image diff for unstaged changes");
    assert_eq!(unstaged.path, PathBuf::from("img.png"));
    assert_eq!(unstaged.old.as_deref(), Some(old_png.as_slice()));
    assert_eq!(unstaged.new.as_deref(), Some(new_png.as_slice()));

    run_git(repo, &["add", "img.png"]);

    let staged = opened
        .diff_file_image(&DiffTarget::WorkingTree {
            path: PathBuf::from("img.png"),
            area: DiffArea::Staged,
        })
        .unwrap()
        .expect("image diff for staged changes");
    assert_eq!(staged.old.as_deref(), Some(old_png.as_slice()));
    assert_eq!(staged.new.as_deref(), Some(new_png.as_slice()));

    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "second"],
    );
    let head = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git rev-parse to run");
    assert!(head.status.success());
    let head = String::from_utf8(head.stdout).unwrap().trim().to_string();

    let commit = opened
        .diff_file_image(&DiffTarget::Commit {
            commit_id: gitgpui_core::domain::CommitId(head),
            path: Some(PathBuf::from("img.png")),
        })
        .unwrap()
        .expect("image diff for commit");
    assert_eq!(commit.old.as_deref(), Some(old_png.as_slice()));
    assert_eq!(commit.new.as_deref(), Some(new_png.as_slice()));
}

#[test]
fn diff_file_text_uses_ours_and_theirs_for_conflicted_paths() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "base\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "base"],
    );

    run_git(repo, &["checkout", "-b", "feature"]);
    write(repo, "a.txt", "theirs\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "theirs"],
    );

    run_git(repo, &["checkout", "-"]);
    write(repo, "a.txt", "ours\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "ours"],
    );

    run_git_expect_failure(repo, &["merge", "feature"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();
    let status = opened.status().unwrap();
    assert_eq!(status.unstaged.len(), 1);
    assert_eq!(status.unstaged[0].path, PathBuf::from("a.txt"));
    assert_eq!(status.unstaged[0].kind, FileStatusKind::Conflicted);
    assert_eq!(status.unstaged[0].conflict, Some(FileConflictKind::BothModified));

    let diff = opened
        .diff_file_text(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Unstaged,
        })
        .unwrap()
        .expect("file diff for conflicted changes");
    assert_eq!(diff.old.as_deref(), Some("ours\n"));
    assert_eq!(diff.new.as_deref(), Some("theirs\n"));
}

#[test]
fn status_reports_single_conflict_for_modify_delete() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "base\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "base"],
    );

    run_git(repo, &["checkout", "-b", "feature"]);
    write(repo, "a.txt", "theirs\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "theirs"],
    );

    run_git(repo, &["checkout", "-"]);
    run_git(repo, &["rm", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "ours_delete"],
    );

    run_git_expect_failure(repo, &["merge", "feature"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();
    let status = opened.status().unwrap();

    let entries = status
        .unstaged
        .iter()
        .filter(|e| e.path == PathBuf::from("a.txt"))
        .collect::<Vec<_>>();
    assert_eq!(
        entries.len(),
        1,
        "expected exactly one status entry for a.txt, got {:#?}",
        status.unstaged
    );
    assert_eq!(entries[0].kind, FileStatusKind::Conflicted);
    assert_eq!(entries[0].conflict, Some(FileConflictKind::DeletedByUs));
}

#[test]
fn status_reports_conflict_kind_for_add_add() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "base.txt", "base\n");
    run_git(repo, &["add", "base.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "base"],
    );

    run_git(repo, &["checkout", "-b", "feature"]);
    write(repo, "a.txt", "theirs\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "theirs_add"],
    );

    run_git(repo, &["checkout", "-"]);
    write(repo, "a.txt", "ours\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "ours_add"],
    );

    run_git_expect_failure(repo, &["merge", "feature"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();
    let status = opened.status().unwrap();
    assert_eq!(status.unstaged.len(), 1);
    assert_eq!(status.unstaged[0].path, PathBuf::from("a.txt"));
    assert_eq!(status.unstaged[0].kind, FileStatusKind::Conflicted);
    assert_eq!(status.unstaged[0].conflict, Some(FileConflictKind::BothAdded));
}

#[test]
fn diff_file_text_handles_modify_delete_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "base\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "base"],
    );

    run_git(repo, &["checkout", "-b", "feature"]);
    write(repo, "a.txt", "theirs\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "theirs"],
    );

    run_git(repo, &["checkout", "-"]);
    run_git(repo, &["rm", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "ours_delete"],
    );

    run_git_expect_failure(repo, &["merge", "feature"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    let diff = opened
        .diff_file_text(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Unstaged,
        })
        .unwrap()
        .expect("file diff for conflicted changes");
    assert_eq!(diff.old, None);
    assert_eq!(diff.new.as_deref(), Some("theirs\n"));
}

#[test]
fn checkout_conflict_side_resolves_modify_delete_using_ours() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "base\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "base"],
    );

    run_git(repo, &["checkout", "-b", "feature"]);
    write(repo, "a.txt", "theirs\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "theirs"],
    );

    run_git(repo, &["checkout", "-"]);
    run_git(repo, &["rm", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "ours_delete"],
    );

    run_git_expect_failure(repo, &["merge", "feature"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();
    opened
        .checkout_conflict_side(Path::new("a.txt"), ConflictSide::Ours)
        .unwrap();

    assert!(
        !repo.join("a.txt").exists(),
        "expected ours resolution to remove file from worktree"
    );
    let status = opened.status().unwrap();
    assert!(
        !status
            .staged
            .iter()
            .chain(status.unstaged.iter())
            .any(|e| e.path == PathBuf::from("a.txt")),
        "expected ours resolution to clear status entries for a.txt, got {status:?}"
    );
}

#[test]
fn checkout_conflict_side_resolves_modify_delete_using_theirs() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "base\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "base"],
    );

    run_git(repo, &["checkout", "-b", "feature"]);
    write(repo, "a.txt", "theirs\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "theirs"],
    );

    run_git(repo, &["checkout", "-"]);
    run_git(repo, &["rm", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "ours_delete"],
    );

    run_git_expect_failure(repo, &["merge", "feature"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();
    opened
        .checkout_conflict_side(Path::new("a.txt"), ConflictSide::Theirs)
        .unwrap();

    assert_eq!(
        fs::read_to_string(repo.join("a.txt")).unwrap(),
        "theirs\n",
        "expected theirs resolution to restore file contents"
    );
    let status = opened.status().unwrap();
    assert_eq!(
        status.unstaged,
        Vec::new(),
        "expected theirs resolution to clear unstaged entries"
    );
    assert!(
        status
            .staged
            .iter()
            .any(|e| e.path == PathBuf::from("a.txt") && e.kind == FileStatusKind::Added),
        "expected theirs resolution to stage file as added, got {status:?}"
    );
}

#[test]
fn checkout_conflict_side_stages_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "base\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "base"],
    );

    run_git(repo, &["checkout", "-b", "feature"]);
    write(repo, "a.txt", "theirs\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "theirs"],
    );

    run_git(repo, &["checkout", "-"]);
    write(repo, "a.txt", "ours\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "ours"],
    );

    run_git_expect_failure(repo, &["merge", "feature"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened
        .checkout_conflict_side(Path::new("a.txt"), ConflictSide::Theirs)
        .unwrap();

    let status = opened.status().unwrap();
    assert!(
        status
            .unstaged
            .iter()
            .all(|s| s.path != PathBuf::from("a.txt"))
    );
    assert!(
        status
            .staged
            .iter()
            .any(|s| s.path == PathBuf::from("a.txt") && s.kind == FileStatusKind::Modified)
    );

    let on_disk = fs::read_to_string(repo.join("a.txt")).unwrap();
    assert_eq!(on_disk, "theirs\n");
}

#[test]
fn stage_and_unstage_paths_update_status() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    write(repo, "a.txt", "one\ntwo\n");
    write(repo, "b.txt", "untracked\n");

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened.stage(&[Path::new("a.txt")]).unwrap();
    let status = opened.status().unwrap();
    assert_eq!(status.staged.len(), 1);
    assert_eq!(status.staged[0].path, PathBuf::from("a.txt"));
    assert_eq!(status.staged[0].kind, FileStatusKind::Modified);
    assert_eq!(status.unstaged.len(), 1);
    assert_eq!(status.unstaged[0].path, PathBuf::from("b.txt"));
    assert_eq!(status.unstaged[0].kind, FileStatusKind::Untracked);

    opened.unstage(&[Path::new("a.txt")]).unwrap();
    let status = opened.status().unwrap();
    assert!(status.staged.is_empty());
    assert_eq!(status.unstaged.len(), 2);
    assert!(
        status
            .unstaged
            .iter()
            .any(|e| e.path == PathBuf::from("a.txt") && e.kind == FileStatusKind::Modified)
    );
    assert!(
        status
            .unstaged
            .iter()
            .any(|e| e.path == PathBuf::from("b.txt") && e.kind == FileStatusKind::Untracked)
    );
}

#[test]
fn commit_creates_new_commit_and_cleans_status() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    write(repo, "a.txt", "one\ntwo\n");
    run_git(repo, &["add", "a.txt"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened.commit("second").unwrap();

    let msg = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["log", "-1", "--pretty=%B"])
        .output()
        .expect("git log to run");
    assert!(msg.status.success());
    assert_eq!(String::from_utf8(msg.stdout).unwrap().trim(), "second");

    let status = opened.status().unwrap();
    assert!(status.staged.is_empty());
    assert!(status.unstaged.is_empty());
}

#[test]
fn reset_soft_moves_head_and_leaves_changes_staged() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(repo, &["-c", "commit.gpgsign=false", "commit", "-m", "c1"]);
    let c1 = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse c1");
    assert!(c1.status.success());
    let c1 = String::from_utf8(c1.stdout).unwrap().trim().to_string();

    write(repo, "a.txt", "two\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(repo, &["-c", "commit.gpgsign=false", "commit", "-m", "c2"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened
        .reset_with_output("HEAD~1", gitgpui_core::services::ResetMode::Soft)
        .unwrap();

    let head = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse head");
    assert!(head.status.success());
    assert_eq!(String::from_utf8(head.stdout).unwrap().trim(), c1);
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "two\n");

    let status = opened.status().unwrap();
    assert_eq!(status.staged.len(), 1);
    assert_eq!(status.staged[0].path, PathBuf::from("a.txt"));
    assert_eq!(status.staged[0].kind, FileStatusKind::Modified);
    assert!(status.unstaged.is_empty());
}

#[test]
fn reset_mixed_moves_head_and_leaves_changes_unstaged() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(repo, &["-c", "commit.gpgsign=false", "commit", "-m", "c1"]);
    let c1 = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse c1");
    assert!(c1.status.success());
    let c1 = String::from_utf8(c1.stdout).unwrap().trim().to_string();

    write(repo, "a.txt", "two\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(repo, &["-c", "commit.gpgsign=false", "commit", "-m", "c2"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened
        .reset_with_output("HEAD~1", gitgpui_core::services::ResetMode::Mixed)
        .unwrap();

    let head = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse head");
    assert!(head.status.success());
    assert_eq!(String::from_utf8(head.stdout).unwrap().trim(), c1);
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "two\n");

    let status = opened.status().unwrap();
    assert!(status.staged.is_empty());
    assert_eq!(status.unstaged.len(), 1);
    assert_eq!(status.unstaged[0].path, PathBuf::from("a.txt"));
    assert_eq!(status.unstaged[0].kind, FileStatusKind::Modified);
}

#[test]
fn reset_hard_moves_head_and_discards_changes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(repo, &["-c", "commit.gpgsign=false", "commit", "-m", "c1"]);
    let c1 = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse c1");
    assert!(c1.status.success());
    let c1 = String::from_utf8(c1.stdout).unwrap().trim().to_string();

    write(repo, "a.txt", "two\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(repo, &["-c", "commit.gpgsign=false", "commit", "-m", "c2"]);

    write(repo, "a.txt", "two-modified\n");

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened
        .reset_with_output("HEAD~1", gitgpui_core::services::ResetMode::Hard)
        .unwrap();

    let head = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse head");
    assert!(head.status.success());
    assert_eq!(String::from_utf8(head.stdout).unwrap().trim(), c1);
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "one\n");

    let status = opened.status().unwrap();
    assert!(status.staged.is_empty());
    assert!(status.unstaged.is_empty());
}

#[test]
fn revert_commit_creates_new_commit_and_reverts_content() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(repo, &["-c", "commit.gpgsign=false", "commit", "-m", "c1"]);

    write(repo, "a.txt", "two\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(repo, &["-c", "commit.gpgsign=false", "commit", "-m", "c2"]);

    let c2 = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse c2");
    assert!(c2.status.success());
    let c2 = String::from_utf8(c2.stdout).unwrap().trim().to_string();

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened
        .revert(&gitgpui_core::domain::CommitId(c2.clone()))
        .unwrap();

    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "one\n");
    let status = opened.status().unwrap();
    assert!(status.staged.is_empty());
    assert!(status.unstaged.is_empty());

    let head = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse head");
    assert!(head.status.success());
    let head = String::from_utf8(head.stdout).unwrap().trim().to_string();
    assert_ne!(head, c2, "expected revert to create a new commit");
}

#[test]
fn amend_rewrites_head_commit_message_and_content() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    let head_before = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse head");
    assert!(head_before.status.success());
    let head_before = String::from_utf8(head_before.stdout)
        .unwrap()
        .trim()
        .to_string();

    write(repo, "a.txt", "one\ntwo\n");
    run_git(repo, &["add", "a.txt"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened.commit_amend("amended").unwrap();

    let head_after = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse head");
    assert!(head_after.status.success());
    let head_after = String::from_utf8(head_after.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_ne!(head_after, head_before);

    let count = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .expect("rev-list --count");
    assert!(count.status.success());
    assert_eq!(String::from_utf8(count.stdout).unwrap().trim(), "1");

    let msg = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["log", "-1", "--pretty=%B"])
        .output()
        .expect("git log to run");
    assert!(msg.status.success());
    assert_eq!(String::from_utf8(msg.stdout).unwrap().trim(), "amended");
    assert_eq!(
        fs::read_to_string(repo.join("a.txt")).unwrap(),
        "one\ntwo\n"
    );

    let status = opened.status().unwrap();
    assert!(status.staged.is_empty());
    assert!(status.unstaged.is_empty());
}

#[test]
fn merge_creates_merge_commit_when_branches_diverged() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "base\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "base"],
    );

    run_git(repo, &["checkout", "-b", "feature"]);
    write(repo, "b.txt", "feature\n");
    run_git(repo, &["add", "b.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "feature"],
    );

    run_git(repo, &["checkout", "-"]);
    write(repo, "c.txt", "main\n");
    run_git(repo, &["add", "c.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "main"],
    );

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened.merge_ref_with_output("feature").unwrap();

    let parents = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-list", "--parents", "-n", "1", "HEAD"])
        .output()
        .expect("rev-list --parents");
    assert!(parents.status.success());
    let parent_count = String::from_utf8(parents.stdout)
        .unwrap()
        .split_whitespace()
        .count()
        .saturating_sub(1);
    assert_eq!(parent_count, 2, "expected merge commit");

    assert!(repo.join("b.txt").exists());
    assert!(repo.join("c.txt").exists());
    assert_eq!(fs::read_to_string(repo.join("b.txt")).unwrap(), "feature\n");
    assert_eq!(fs::read_to_string(repo.join("c.txt")).unwrap(), "main\n");
}

#[test]
fn rebase_replays_commits_onto_target_branch() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "base\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "base"],
    );

    run_git(repo, &["checkout", "-b", "feature"]);
    write(repo, "b.txt", "feature\n");
    run_git(repo, &["add", "b.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "feature"],
    );

    run_git(repo, &["checkout", "-"]);
    write(repo, "c.txt", "main\n");
    run_git(repo, &["add", "c.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "main"],
    );
    let master_head = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse master");
    assert!(master_head.status.success());
    let master_head = String::from_utf8(master_head.stdout)
        .unwrap()
        .trim()
        .to_string();

    run_git(repo, &["checkout", "feature"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened.rebase_with_output("master").unwrap();

    let parent = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD^"])
        .output()
        .expect("rev-parse parent");
    assert!(parent.status.success());
    assert_eq!(
        String::from_utf8(parent.stdout).unwrap().trim(),
        master_head
    );

    assert!(repo.join("b.txt").exists());
    assert_eq!(fs::read_to_string(repo.join("b.txt")).unwrap(), "feature\n");
    let status = opened.status().unwrap();
    assert!(status.staged.is_empty());
    assert!(status.unstaged.is_empty());
}

#[test]
fn create_and_delete_local_branch() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened
        .create_branch(
            "feature",
            &gitgpui_core::domain::CommitId("HEAD".to_string()),
        )
        .unwrap();
    run_git(
        repo,
        &["show-ref", "--verify", "--quiet", "refs/heads/feature"],
    );

    opened.delete_branch("feature").unwrap();
    let deleted = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["show-ref", "--verify", "--quiet", "refs/heads/feature"])
        .status()
        .expect("show-ref");
    assert!(!deleted.success(), "expected branch to be deleted");
}

#[test]
fn create_and_delete_local_tag() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened.create_tag_with_output("v1.0.0", "HEAD").unwrap();
    run_git(
        repo,
        &["show-ref", "--verify", "--quiet", "refs/tags/v1.0.0"],
    );

    opened.delete_tag_with_output("v1.0.0").unwrap();
    let deleted = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["show-ref", "--verify", "--quiet", "refs/tags/v1.0.0"])
        .status()
        .expect("show-ref");
    assert!(!deleted.success(), "expected tag to be deleted");
}

#[test]
fn list_remote_branches_includes_fetched_remote_tracking_refs() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let origin = dir.path().join("origin.git");
    fs::create_dir_all(&repo).unwrap();

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "you@example.com"]);
    run_git(&repo, &["config", "user.name", "You"]);
    run_git(&repo, &["config", "commit.gpgsign", "false"]);

    write(&repo, "a.txt", "one\n");
    run_git(&repo, &["add", "a.txt"]);
    run_git(
        &repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    fs::create_dir_all(&origin).unwrap();
    run_git(&origin, &["init", "--bare"]);
    run_git(
        &repo,
        &["remote", "add", "origin", origin.to_string_lossy().as_ref()],
    );
    run_git(&repo, &["push", "-u", "origin", "master"]);

    run_git(&repo, &["checkout", "-b", "feature"]);
    write(&repo, "b.txt", "feature\n");
    run_git(&repo, &["add", "b.txt"]);
    run_git(
        &repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "feature"],
    );
    run_git(&repo, &["push", "-u", "origin", "feature"]);
    run_git(&repo, &["fetch", "origin"]);

    let backend = GixBackend::default();
    let opened = backend.open(&repo).unwrap();
    let branches = opened.list_remote_branches().unwrap();

    assert!(branches.contains(&gitgpui_core::domain::RemoteBranch {
        remote: "origin".to_string(),
        name: "master".to_string(),
    }));
    assert!(branches.contains(&gitgpui_core::domain::RemoteBranch {
        remote: "origin".to_string(),
        name: "feature".to_string(),
    }));
    assert!(!branches.iter().any(|b| b.name == "HEAD"));
}

#[test]
fn push_with_output_updates_remote_head() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let origin = dir.path().join("origin.git");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&origin).unwrap();

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "you@example.com"]);
    run_git(&repo, &["config", "user.name", "You"]);
    run_git(&repo, &["config", "commit.gpgsign", "false"]);

    write(&repo, "a.txt", "one\n");
    run_git(&repo, &["add", "a.txt"]);
    run_git(
        &repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    run_git(&origin, &["init", "--bare"]);
    run_git(
        &repo,
        &["remote", "add", "origin", origin.to_string_lossy().as_ref()],
    );
    run_git(&repo, &["push", "-u", "origin", "master"]);

    write(&repo, "a.txt", "one\ntwo\n");
    run_git(&repo, &["add", "a.txt"]);
    run_git(
        &repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "second"],
    );
    let head_local = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse HEAD");
    assert!(head_local.status.success());
    let head_local = String::from_utf8(head_local.stdout)
        .unwrap()
        .trim()
        .to_string();

    let backend = GixBackend::default();
    let opened = backend.open(&repo).unwrap();
    opened.push_with_output().unwrap();

    let head_remote = Command::new("git")
        .arg("-C")
        .arg(&origin)
        .args(["rev-parse", "refs/heads/master"])
        .output()
        .expect("rev-parse origin/master");
    assert!(head_remote.status.success());
    let head_remote = String::from_utf8(head_remote.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(head_remote, head_local);
}

#[test]
fn force_push_with_output_updates_remote_head_after_rewrite() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let origin = dir.path().join("origin.git");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&origin).unwrap();

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "you@example.com"]);
    run_git(&repo, &["config", "user.name", "You"]);
    run_git(&repo, &["config", "commit.gpgsign", "false"]);

    write(&repo, "a.txt", "one\n");
    run_git(&repo, &["add", "a.txt"]);
    run_git(
        &repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    run_git(&origin, &["init", "--bare"]);
    run_git(
        &repo,
        &["remote", "add", "origin", origin.to_string_lossy().as_ref()],
    );
    run_git(&repo, &["push", "-u", "origin", "master"]);

    write(&repo, "a.txt", "one\ntwo\n");
    run_git(&repo, &["add", "a.txt"]);
    run_git(
        &repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "second"],
    );
    run_git(&repo, &["push"]);
    run_git(&repo, &["fetch", "origin"]);

    // Rewrite local history so it diverges from the remote.
    run_git(&repo, &["reset", "--hard", "HEAD~1"]);
    write(&repo, "a.txt", "one\ntwo (rewritten)\n");
    run_git(&repo, &["add", "a.txt"]);
    run_git(
        &repo,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "second rewritten",
        ],
    );
    let head_local = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse HEAD");
    assert!(head_local.status.success());
    let head_local = String::from_utf8(head_local.stdout)
        .unwrap()
        .trim()
        .to_string();

    let backend = GixBackend::default();
    let opened = backend.open(&repo).unwrap();
    opened.push_force_with_output().unwrap();

    let head_remote = Command::new("git")
        .arg("-C")
        .arg(&origin)
        .args(["rev-parse", "refs/heads/master"])
        .output()
        .expect("rev-parse refs/heads/master");
    assert!(head_remote.status.success());
    let head_remote = String::from_utf8(head_remote.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(head_remote, head_local);
}

#[test]
fn pull_with_output_fast_forwards_from_remote() {
    let dir = tempfile::tempdir().unwrap();
    let origin = dir.path().join("origin.git");
    let repo_a = dir.path().join("repo-a");
    let repo_b = dir.path().join("repo-b");
    fs::create_dir_all(&origin).unwrap();
    fs::create_dir_all(&repo_a).unwrap();

    run_git(&origin, &["init", "--bare"]);

    run_git(&repo_a, &["init"]);
    run_git(&repo_a, &["config", "user.email", "you@example.com"]);
    run_git(&repo_a, &["config", "user.name", "You"]);
    run_git(&repo_a, &["config", "commit.gpgsign", "false"]);
    write(&repo_a, "a.txt", "one\n");
    run_git(&repo_a, &["add", "a.txt"]);
    run_git(
        &repo_a,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );
    run_git(
        &repo_a,
        &["remote", "add", "origin", origin.to_string_lossy().as_ref()],
    );
    run_git(&repo_a, &["push", "-u", "origin", "master"]);

    run_git(
        dir.path(),
        &[
            "clone",
            origin.to_string_lossy().as_ref(),
            repo_b.to_string_lossy().as_ref(),
        ],
    );

    write(&repo_a, "a.txt", "one\ntwo\n");
    run_git(&repo_a, &["add", "a.txt"]);
    run_git(
        &repo_a,
        &["-c", "commit.gpgsign=false", "commit", "-m", "second"],
    );
    run_git(&repo_a, &["push"]);

    let head_origin = Command::new("git")
        .arg("-C")
        .arg(&origin)
        .args(["rev-parse", "refs/heads/master"])
        .output()
        .expect("rev-parse origin");
    assert!(head_origin.status.success());
    let head_origin = String::from_utf8(head_origin.stdout)
        .unwrap()
        .trim()
        .to_string();

    let backend = GixBackend::default();
    let opened_b = backend.open(&repo_b).unwrap();
    opened_b
        .pull_with_output(gitgpui_core::services::PullMode::FastForwardOnly)
        .unwrap();

    let head_b = Command::new("git")
        .arg("-C")
        .arg(&repo_b)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse b");
    assert!(head_b.status.success());
    let head_b = String::from_utf8(head_b.stdout).unwrap().trim().to_string();
    assert_eq!(head_b, head_origin);
}

#[test]
fn stash_create_list_apply_and_drop_work() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    write(repo, "a.txt", "one\ntwo\n");

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened.stash_create("wip", false).unwrap();
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "one\n");

    let stashes = opened.stash_list().unwrap();
    assert!(!stashes.is_empty());
    assert!(stashes[0].message.contains("wip"));

    opened.stash_apply(0).unwrap();
    assert_eq!(
        fs::read_to_string(repo.join("a.txt")).unwrap(),
        "one\ntwo\n"
    );

    opened.stash_drop(0).unwrap();
    let stashes = opened.stash_list().unwrap();
    assert!(stashes.is_empty());
}

#[test]
fn checkout_commit_detaches_head_at_target() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    let sha = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse HEAD");
    assert!(sha.status.success());
    let sha = String::from_utf8(sha.stdout).unwrap().trim().to_string();

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();
    opened
        .checkout_commit(&gitgpui_core::domain::CommitId(sha.clone()))
        .unwrap();

    let head_name = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .expect("rev-parse --abbrev-ref");
    assert!(head_name.status.success());
    assert_eq!(String::from_utf8(head_name.stdout).unwrap().trim(), "HEAD");

    let head_sha = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse head sha");
    assert!(head_sha.status.success());
    assert_eq!(String::from_utf8(head_sha.stdout).unwrap().trim(), sha);
}

#[test]
fn discard_worktree_changes_reverts_to_index_version() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    write(repo, "a.txt", "one\ntwo\n");
    run_git(repo, &["add", "a.txt"]);
    write(repo, "a.txt", "one\ntwo\nthree\n");

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened
        .discard_worktree_changes(&[Path::new("a.txt")])
        .unwrap();

    assert_eq!(
        fs::read_to_string(repo.join("a.txt")).unwrap(),
        "one\ntwo\n"
    );

    let status = opened.status().unwrap();
    assert!(
        status
            .staged
            .iter()
            .any(|e| e.path == PathBuf::from("a.txt") && e.kind == FileStatusKind::Modified)
    );
    assert!(
        !status
            .unstaged
            .iter()
            .any(|e| e.path == PathBuf::from("a.txt"))
    );
}

#[test]
fn discard_worktree_changes_reverts_modified_file_to_head() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    write(repo, "a.txt", "one\ntwo\n");

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened
        .discard_worktree_changes(&[Path::new("a.txt")])
        .unwrap();

    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "one\n");
    let status = opened.status().unwrap();
    assert!(status.staged.is_empty());
    assert!(status.unstaged.is_empty());
}

#[test]
fn discard_worktree_changes_removes_staged_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    write(repo, "new.txt", "new\n");
    run_git(repo, &["add", "new.txt"]);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened
        .discard_worktree_changes(&[Path::new("new.txt")])
        .unwrap();

    assert!(!repo.join("new.txt").exists());
    let status = opened.status().unwrap();
    assert!(
        !status
            .staged
            .iter()
            .any(|e| e.path == PathBuf::from("new.txt"))
    );
    assert!(
        !status
            .unstaged
            .iter()
            .any(|e| e.path == PathBuf::from("new.txt"))
    );
}

#[test]
fn discard_worktree_changes_removes_untracked_file() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    write(repo, "untracked.txt", "new\n");

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened
        .discard_worktree_changes(&[Path::new("untracked.txt")])
        .unwrap();

    assert!(!repo.join("untracked.txt").exists());
    let status = opened.status().unwrap();
    assert!(
        !status
            .unstaged
            .iter()
            .any(|e| e.path == PathBuf::from("untracked.txt"))
    );
}

#[test]
fn discard_worktree_changes_supports_mixed_selection() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    write(repo, "a.txt", "one\n");
    write(repo, "b.txt", "two\n");
    run_git(repo, &["add", "a.txt", "b.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    write(repo, "a.txt", "one!\n");
    fs::remove_file(repo.join("b.txt")).unwrap();
    write(repo, "c.txt", "three\n");

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    opened
        .discard_worktree_changes(&[Path::new("a.txt"), Path::new("b.txt"), Path::new("c.txt")])
        .unwrap();

    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "one\n");
    assert_eq!(fs::read_to_string(repo.join("b.txt")).unwrap(), "two\n");
    assert!(!repo.join("c.txt").exists());
    let status = opened.status().unwrap();
    assert!(status.staged.is_empty());
    assert!(status.unstaged.is_empty());
}

#[test]
fn stage_hunk_applies_only_part_of_a_file_to_index() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    let mut base = String::new();
    for i in 1..=30 {
        base.push_str(&format!("L{i:02}\n"));
    }
    write(repo, "a.txt", &base);
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    let modified = base
        .replace("L02\n", "L02-mod\n")
        .replace("L25\n", "L25-mod\n");
    write(repo, "a.txt", &modified);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    let unstaged_before = opened
        .diff_unified(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Unstaged,
        })
        .unwrap();
    let hunk_count_before = unstaged_before
        .lines()
        .filter(|l| l.starts_with("@@"))
        .count();
    assert_eq!(
        hunk_count_before, 2,
        "expected two hunks:\n{unstaged_before}"
    );

    let lines = unstaged_before.lines().collect::<Vec<_>>();
    let file_start = lines
        .iter()
        .position(|l| l.starts_with("diff --git "))
        .unwrap_or(0);
    let first_hunk = lines
        .iter()
        .position(|l| l.starts_with("@@"))
        .expect("first hunk header");
    let second_hunk = (first_hunk + 1..lines.len())
        .find(|&ix| lines.get(ix).is_some_and(|l| l.starts_with("@@")))
        .expect("second hunk header");

    let patch = lines[file_start..first_hunk]
        .iter()
        .chain(lines[first_hunk..second_hunk].iter())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    opened
        .apply_unified_patch_to_index_with_output(&patch, false)
        .unwrap();

    let staged_after = opened
        .diff_unified(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Staged,
        })
        .unwrap();
    assert_eq!(
        staged_after.lines().filter(|l| l.starts_with("@@")).count(),
        1,
        "expected one staged hunk:\n{staged_after}"
    );
    assert!(staged_after.contains("-L02"));
    assert!(staged_after.contains("+L02-mod"));
    assert!(!staged_after.contains("L25-mod"));

    let unstaged_after = opened
        .diff_unified(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Unstaged,
        })
        .unwrap();
    assert_eq!(
        unstaged_after
            .lines()
            .filter(|l| l.starts_with("@@"))
            .count(),
        1,
        "expected one remaining unstaged hunk:\n{unstaged_after}"
    );
    assert!(!unstaged_after.contains("L02-mod"));
    assert!(unstaged_after.contains("-L25"));
    assert!(unstaged_after.contains("+L25-mod"));
}

#[test]
fn unstage_hunk_reverts_only_that_part_in_index() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "you@example.com"]);
    run_git(repo, &["config", "user.name", "You"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);

    let mut base = String::new();
    for i in 1..=30 {
        base.push_str(&format!("L{i:02}\n"));
    }
    write(repo, "a.txt", &base);
    run_git(repo, &["add", "a.txt"]);
    run_git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-m", "init"],
    );

    let modified = base
        .replace("L02\n", "L02-mod\n")
        .replace("L25\n", "L25-mod\n");
    write(repo, "a.txt", &modified);

    let backend = GixBackend::default();
    let opened = backend.open(repo).unwrap();

    let unstaged_before = opened
        .diff_unified(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Unstaged,
        })
        .unwrap();
    assert_eq!(
        unstaged_before
            .lines()
            .filter(|l| l.starts_with("@@"))
            .count(),
        2,
        "expected two hunks:\n{unstaged_before}"
    );

    let lines = unstaged_before.lines().collect::<Vec<_>>();
    let file_start = lines
        .iter()
        .position(|l| l.starts_with("diff --git "))
        .unwrap_or(0);
    let first_hunk = lines
        .iter()
        .position(|l| l.starts_with("@@"))
        .expect("first hunk header");
    let second_hunk = (first_hunk + 1..lines.len())
        .find(|&ix| lines.get(ix).is_some_and(|l| l.starts_with("@@")))
        .expect("second hunk header");

    let patch = lines[file_start..first_hunk]
        .iter()
        .chain(lines[first_hunk..second_hunk].iter())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    opened
        .apply_unified_patch_to_index_with_output(&patch, false)
        .unwrap();

    let staged_after_stage = opened
        .diff_unified(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Staged,
        })
        .unwrap();
    assert_eq!(
        staged_after_stage
            .lines()
            .filter(|l| l.starts_with("@@"))
            .count(),
        1,
        "expected one staged hunk:\n{staged_after_stage}"
    );

    opened
        .apply_unified_patch_to_index_with_output(&patch, true)
        .unwrap();

    let staged_after_unstage = opened
        .diff_unified(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Staged,
        })
        .unwrap();
    assert!(
        staged_after_unstage.trim().is_empty(),
        "expected staged diff to be empty:\n{staged_after_unstage}"
    );

    let unstaged_after_unstage = opened
        .diff_unified(&DiffTarget::WorkingTree {
            path: PathBuf::from("a.txt"),
            area: DiffArea::Unstaged,
        })
        .unwrap();
    assert_eq!(
        unstaged_after_unstage
            .lines()
            .filter(|l| l.starts_with("@@"))
            .count(),
        2,
        "expected two unstaged hunks:\n{unstaged_after_unstage}"
    );
    assert!(unstaged_after_unstage.contains("+L02-mod"));
    assert!(unstaged_after_unstage.contains("+L25-mod"));
}
