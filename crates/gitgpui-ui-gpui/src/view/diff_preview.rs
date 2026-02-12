use gitgpui_core::diff::AnnotatedDiffLine;
use gitgpui_core::domain::DiffTarget;

pub(super) fn build_new_file_preview_from_diff(
    diff: &[AnnotatedDiffLine],
    workdir: &std::path::Path,
    target: Option<&DiffTarget>,
) -> Option<(std::path::PathBuf, Vec<String>)> {
    let mut file_header_count = 0usize;
    let mut is_new_file = false;
    let mut has_remove = false;

    for line in diff {
        if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
            && line.text.starts_with("diff --git ")
        {
            file_header_count += 1;
        }
        if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
            && (line.text.starts_with("new file mode ") || line.text.eq_ignore_ascii_case("--- /dev/null"))
        {
            is_new_file = true;
        }
        if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Remove) {
            has_remove = true;
        }
    }

    if file_header_count != 1 || !is_new_file || has_remove {
        return None;
    }

    let rel_path = match target? {
        DiffTarget::WorkingTree { path, .. } => path.clone(),
        DiffTarget::Commit {
            path: Some(path), ..
        } => path.clone(),
        _ => return None,
    };

    let abs_path = if rel_path.is_absolute() {
        rel_path
    } else {
        workdir.join(rel_path)
    };

    let lines = diff
        .iter()
        .filter(|l| matches!(l.kind, gitgpui_core::domain::DiffLineKind::Add))
        .map(|l| l.text.strip_prefix('+').unwrap_or(&l.text).to_string())
        .collect::<Vec<_>>();

    Some((abs_path, lines))
}

pub(super) fn build_deleted_file_preview_from_diff(
    diff: &[AnnotatedDiffLine],
    workdir: &std::path::Path,
    target: Option<&DiffTarget>,
) -> Option<(std::path::PathBuf, Vec<String>)> {
    let mut file_header_count = 0usize;
    let mut is_deleted_file = false;
    let mut has_add = false;

    for line in diff {
        if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
            && line.text.starts_with("diff --git ")
        {
            file_header_count += 1;
        }
        if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
            && (line.text.starts_with("deleted file mode ") || line.text.eq_ignore_ascii_case("+++ /dev/null"))
        {
            is_deleted_file = true;
        }
        if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Add) {
            has_add = true;
        }
    }

    if file_header_count != 1 || !is_deleted_file || has_add {
        return None;
    }

    let rel_path = match target? {
        DiffTarget::WorkingTree { path, .. } => path.clone(),
        DiffTarget::Commit {
            path: Some(path), ..
        } => path.clone(),
        _ => return None,
    };

    let abs_path = if rel_path.is_absolute() {
        rel_path
    } else {
        workdir.join(rel_path)
    };

    let lines = diff
        .iter()
        .filter(|l| matches!(l.kind, gitgpui_core::domain::DiffLineKind::Remove))
        .map(|l| l.text.strip_prefix('-').unwrap_or(&l.text).to_string())
        .collect::<Vec<_>>();

    Some((abs_path, lines))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitgpui_core::domain::{DiffArea, DiffLineKind};
    use std::path::PathBuf;

    fn line(kind: DiffLineKind, text: &str) -> AnnotatedDiffLine {
        AnnotatedDiffLine {
            kind,
            text: text.into(),
            old_line: None,
            new_line: None,
        }
    }

    #[test]
    fn new_file_preview_extracts_added_lines() {
        let workdir = PathBuf::from("repo");
        let target = DiffTarget::WorkingTree {
            path: PathBuf::from("new.txt"),
            area: DiffArea::Unstaged,
        };
        let diff = vec![
            line(DiffLineKind::Header, "diff --git a/new.txt b/new.txt"),
            line(DiffLineKind::Header, "new file mode 100644"),
            line(DiffLineKind::Add, "+hello"),
            line(DiffLineKind::Add, "+world"),
        ];

        let (abs_path, lines) =
            build_new_file_preview_from_diff(&diff, &workdir, Some(&target)).unwrap();
        assert_eq!(abs_path, workdir.join("new.txt"));
        assert_eq!(lines, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn new_file_preview_returns_none_when_diff_has_removes() {
        let workdir = PathBuf::from("repo");
        let target = DiffTarget::WorkingTree {
            path: PathBuf::from("new.txt"),
            area: DiffArea::Unstaged,
        };
        let diff = vec![
            line(DiffLineKind::Header, "diff --git a/new.txt b/new.txt"),
            line(DiffLineKind::Header, "new file mode 100644"),
            line(DiffLineKind::Remove, "-nope"),
            line(DiffLineKind::Add, "+ok"),
        ];

        assert!(build_new_file_preview_from_diff(&diff, &workdir, Some(&target)).is_none());
    }

    #[test]
    fn deleted_file_preview_extracts_removed_lines() {
        let workdir = PathBuf::from("repo");
        let target = DiffTarget::WorkingTree {
            path: PathBuf::from("old.txt"),
            area: DiffArea::Unstaged,
        };
        let diff = vec![
            line(DiffLineKind::Header, "diff --git a/old.txt b/old.txt"),
            line(DiffLineKind::Header, "deleted file mode 100644"),
            line(DiffLineKind::Remove, "-hello"),
            line(DiffLineKind::Remove, "-world"),
        ];

        let (abs_path, lines) =
            build_deleted_file_preview_from_diff(&diff, &workdir, Some(&target)).unwrap();
        assert_eq!(abs_path, workdir.join("old.txt"));
        assert_eq!(lines, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn deleted_file_preview_returns_none_when_diff_has_adds() {
        let workdir = PathBuf::from("repo");
        let target = DiffTarget::WorkingTree {
            path: PathBuf::from("old.txt"),
            area: DiffArea::Unstaged,
        };
        let diff = vec![
            line(DiffLineKind::Header, "diff --git a/old.txt b/old.txt"),
            line(DiffLineKind::Header, "deleted file mode 100644"),
            line(DiffLineKind::Remove, "-ok"),
            line(DiffLineKind::Add, "+nope"),
        ];

        assert!(build_deleted_file_preview_from_diff(&diff, &workdir, Some(&target)).is_none());
    }
}
