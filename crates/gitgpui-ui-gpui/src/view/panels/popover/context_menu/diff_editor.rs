use super::*;

pub(super) fn model(
    repo_id: RepoId,
    area: DiffArea,
    path: &Option<std::path::PathBuf>,
    hunk_patch: &Option<String>,
    hunks_count: usize,
    lines_patch: &Option<String>,
    lines_count: usize,
    copy_text: &Option<String>,
) -> ContextMenuModel {
    let title = path
        .as_ref()
        .and_then(|p| {
            p.file_name()
                .map(|name| name.to_string_lossy().to_string().into())
        })
        .unwrap_or_else(|| "Diff".into());

    let mut items = vec![ContextMenuItem::Header(title)];
    if let Some(path) = path {
        items.push(ContextMenuItem::Label(path.display().to_string().into()));
    }
    items.push(ContextMenuItem::Separator);

    let (line_label, line_icon, line_shortcut, line_reverse) = match area {
        DiffArea::Unstaged => ("Stage line", "+", Some("S"), false),
        DiffArea::Staged => ("Unstage line", "−", Some("U"), true),
    };
    items.push(ContextMenuItem::Entry {
        label: if lines_count > 1 {
            format!("{line_label}s ({lines_count})").into()
        } else {
            line_label.into()
        },
        icon: Some(line_icon.into()),
        shortcut: line_shortcut.map(Into::into),
        disabled: lines_patch.is_none(),
        action: ContextMenuAction::ApplyIndexPatch {
            repo_id,
            patch: lines_patch.clone().unwrap_or_default(),
            reverse: line_reverse,
        },
    });

    if area == DiffArea::Unstaged {
        items.push(ContextMenuItem::Entry {
            label: if lines_count > 1 {
                format!("Discard lines ({lines_count})").into()
            } else {
                "Discard line".into()
            },
            icon: Some("↺".into()),
            shortcut: Some("D".into()),
            disabled: lines_patch.is_none(),
            action: ContextMenuAction::ApplyWorktreePatch {
                repo_id,
                patch: lines_patch.clone().unwrap_or_default(),
                reverse: true,
            },
        });
    }

    items.push(ContextMenuItem::Separator);

    let (hunk_label, hunk_icon, hunk_reverse) = match area {
        DiffArea::Unstaged => ("Stage hunk", "+", false),
        DiffArea::Staged => ("Unstage hunk", "−", true),
    };
    items.push(ContextMenuItem::Entry {
        label: if hunks_count > 1 {
            format!("{}s ({hunks_count})", hunk_label).into()
        } else {
            hunk_label.into()
        },
        icon: Some(hunk_icon.into()),
        shortcut: None,
        disabled: hunk_patch.is_none(),
        action: ContextMenuAction::ApplyIndexPatch {
            repo_id,
            patch: hunk_patch.clone().unwrap_or_default(),
            reverse: hunk_reverse,
        },
    });

    if area == DiffArea::Unstaged {
        items.push(ContextMenuItem::Entry {
            label: if hunks_count > 1 {
                format!("Discard hunks ({hunks_count})").into()
            } else {
                "Discard hunk".into()
            },
            icon: Some("↺".into()),
            shortcut: None,
            disabled: hunk_patch.is_none(),
            action: ContextMenuAction::ApplyWorktreePatch {
                repo_id,
                patch: hunk_patch.clone().unwrap_or_default(),
                reverse: true,
            },
        });
    }

    items.push(ContextMenuItem::Separator);
    items.push(ContextMenuItem::Entry {
        label: "Copy".into(),
        icon: Some("⧉".into()),
        shortcut: Some("C".into()),
        disabled: copy_text
            .as_ref()
            .map(|t| t.trim().is_empty())
            .unwrap_or(true),
        action: ContextMenuAction::CopyText {
            text: copy_text.clone().unwrap_or_default(),
        },
    });

    ContextMenuModel::new(items)
}
