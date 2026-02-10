use super::*;

const COMMIT_DETAILS_MESSAGE_MAX_HEIGHT_PX: f32 = 240.0;

#[derive(Clone)]
enum ContextMenuAction {
    SelectDiff {
        repo_id: RepoId,
        target: DiffTarget,
    },
    CheckoutCommit {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    CherryPickCommit {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    RevertCommit {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    CheckoutBranch {
        repo_id: RepoId,
        name: String,
    },
    CheckoutRemoteBranch {
        repo_id: RepoId,
        remote: String,
        name: String,
    },
    DeleteBranch {
        repo_id: RepoId,
        name: String,
    },
    SetHistoryScope {
        repo_id: RepoId,
        scope: gitgpui_core::domain::LogScope,
    },
    StagePath {
        repo_id: RepoId,
        path: std::path::PathBuf,
    },
    StagePaths {
        repo_id: RepoId,
        paths: Vec<std::path::PathBuf>,
    },
    UnstagePath {
        repo_id: RepoId,
        path: std::path::PathBuf,
    },
    UnstagePaths {
        repo_id: RepoId,
        paths: Vec<std::path::PathBuf>,
    },
    DiscardWorktreeChangesPath {
        repo_id: RepoId,
        path: std::path::PathBuf,
    },
    DiscardWorktreeChangesPaths {
        repo_id: RepoId,
        paths: Vec<std::path::PathBuf>,
    },
    CheckoutConflictSide {
        repo_id: RepoId,
        paths: Vec<std::path::PathBuf>,
        side: gitgpui_core::services::ConflictSide,
    },
    FetchAll {
        repo_id: RepoId,
    },
    Pull {
        repo_id: RepoId,
        mode: PullMode,
    },
    PullBranch {
        repo_id: RepoId,
        remote: String,
        branch: String,
    },
    MergeRef {
        repo_id: RepoId,
        reference: String,
    },
    Push {
        repo_id: RepoId,
    },
    OpenPopover {
        kind: PopoverKind,
    },
    CopyText {
        text: String,
    },
    StageHunk {
        repo_id: RepoId,
        src_ix: usize,
    },
    UnstageHunk {
        repo_id: RepoId,
        src_ix: usize,
    },
    DeleteTag {
        repo_id: RepoId,
        name: String,
    },
}

#[derive(Clone)]
enum ContextMenuItem {
    Separator,
    Header(SharedString),
    Label(SharedString),
    Entry {
        label: SharedString,
        icon: Option<SharedString>,
        shortcut: Option<SharedString>,
        disabled: bool,
        action: ContextMenuAction,
    },
}

#[derive(Clone)]
struct ContextMenuModel {
    items: Vec<ContextMenuItem>,
}

impl ContextMenuModel {
    fn new(items: Vec<ContextMenuItem>) -> Self {
        Self { items }
    }

    fn is_selectable(&self, ix: usize) -> bool {
        matches!(
            self.items.get(ix),
            Some(ContextMenuItem::Entry { disabled, .. }) if !*disabled
        )
    }

    fn first_selectable(&self) -> Option<usize> {
        (0..self.items.len()).find(|&ix| self.is_selectable(ix))
    }

    fn last_selectable(&self) -> Option<usize> {
        (0..self.items.len())
            .rev()
            .find(|&ix| self.is_selectable(ix))
    }

    fn next_selectable(&self, from: Option<usize>, dir: isize) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        let Some(mut ix) = from else {
            return if dir >= 0 {
                self.first_selectable()
            } else {
                self.last_selectable()
            };
        };

        let n = self.items.len() as isize;
        for _ in 0..self.items.len() {
            ix = ((ix as isize + dir).rem_euclid(n)) as usize;
            if self.is_selectable(ix) {
                return Some(ix);
            }
        }
        None
    }
}

struct HistoryColResizeDragGhost;

impl Render for HistoryColResizeDragGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div().w(px(0.0)).h(px(0.0))
    }
}

mod bars;
mod layout;
mod main;
mod popover;

#[cfg(test)]
mod tests;
