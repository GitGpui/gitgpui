use super::*;

#[derive(Clone, Debug)]
pub(super) struct HistoryCache {
    pub(super) request: HistoryCacheRequest,
    pub(super) visible_indices: Vec<usize>,
    pub(super) graph_rows: Vec<history_graph::GraphRow>,
    pub(super) commit_row_vms: Vec<HistoryCommitRowVm>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct HistoryCacheRequest {
    pub(super) repo_id: RepoId,
    pub(super) log_fingerprint: u64,
    pub(super) head_branch_rev: u64,
    pub(super) branches_rev: u64,
    pub(super) tags_rev: u64,
    pub(super) date_time_format: DateTimeFormat,
}

#[derive(Clone, Debug)]
pub(super) struct HistoryCommitRowVm {
    pub(super) branches_text: SharedString,
    pub(super) tag_names: Arc<[SharedString]>,
    pub(super) when: SharedString,
    pub(super) short_sha: SharedString,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BranchSidebarFingerprint {
    head_branch_rev: u64,
    branches_rev: u64,
    remotes_rev: u64,
    remote_branches_rev: u64,
    stashes_rev: u64,
}

impl BranchSidebarFingerprint {
    fn from_repo(repo: &RepoState) -> Self {
        Self {
            head_branch_rev: repo.head_branch_rev,
            branches_rev: repo.branches_rev,
            remotes_rev: repo.remotes_rev,
            remote_branches_rev: repo.remote_branches_rev,
            stashes_rev: repo.stashes_rev,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct BranchSidebarCache {
    repo_id: RepoId,
    fingerprint: BranchSidebarFingerprint,
    rows: Arc<[BranchSidebarRow]>,
}

#[derive(Clone, Debug)]
pub(super) struct HistoryWorktreeSummaryCache {
    repo_id: RepoId,
    status: Arc<RepoStatus>,
    show_row: bool,
    counts: (usize, usize, usize),
}

#[derive(Clone, Debug)]
pub(super) struct HistoryStashIdsCache {
    repo_id: RepoId,
    stashes_rev: u64,
    ids: Arc<HashSet<CommitId>>,
}

impl GitGpuiView {
    pub(super) fn branch_sidebar_rows(repo: &RepoState) -> Vec<BranchSidebarRow> {
        branch_sidebar::branch_sidebar_rows(repo)
    }

    pub(super) fn branch_sidebar_rows_cached(&mut self) -> Option<Arc<[BranchSidebarRow]>> {
        if self.active_repo().is_none() {
            self.branch_sidebar_cache = None;
            return None;
        }

        let (repo_id, fingerprint, rows) = {
            let repo = self.active_repo()?;
            let fingerprint = BranchSidebarFingerprint::from_repo(repo);
            if let Some(cache) = &self.branch_sidebar_cache
                && cache.repo_id == repo.id
                && cache.fingerprint == fingerprint
            {
                return Some(Arc::clone(&cache.rows));
            }

            let rows: Arc<[BranchSidebarRow]> = Self::branch_sidebar_rows(repo).into();
            (repo.id, fingerprint, rows)
        };

        self.branch_sidebar_cache = Some(BranchSidebarCache {
            repo_id,
            fingerprint,
            rows: Arc::clone(&rows),
        });
        Some(rows)
    }

    pub(super) fn ensure_history_worktree_summary_cache(
        &mut self,
    ) -> (bool, (usize, usize, usize)) {
        enum Action {
            Clear,
            CacheOk {
                show_row: bool,
                counts: (usize, usize, usize),
            },
            Rebuild {
                repo_id: RepoId,
                status: Arc<RepoStatus>,
                show_row: bool,
                counts: (usize, usize, usize),
            },
        }

        let action = (|| {
            let Some(repo) = self.active_repo() else {
                return Action::Clear;
            };
            let Loadable::Ready(status) = &repo.status else {
                return Action::Clear;
            };

            if let Some(cache) = &self.history_worktree_summary_cache
                && cache.repo_id == repo.id
                && Arc::ptr_eq(&cache.status, status)
            {
                return Action::CacheOk {
                    show_row: cache.show_row,
                    counts: cache.counts,
                };
            }

            let count_for = |entries: &[FileStatus]| {
                let mut added = 0usize;
                let mut modified = 0usize;
                let mut deleted = 0usize;
                for entry in entries {
                    match entry.kind {
                        FileStatusKind::Untracked | FileStatusKind::Added => added += 1,
                        FileStatusKind::Deleted => deleted += 1,
                        FileStatusKind::Modified
                        | FileStatusKind::Renamed
                        | FileStatusKind::Conflicted => modified += 1,
                    }
                }
                (added, modified, deleted)
            };

            let unstaged_counts = count_for(&status.unstaged);
            let staged_counts = count_for(&status.staged);
            let show_row = !status.unstaged.is_empty() || !status.staged.is_empty();
            let counts = (
                unstaged_counts.0 + staged_counts.0,
                unstaged_counts.1 + staged_counts.1,
                unstaged_counts.2 + staged_counts.2,
            );

            Action::Rebuild {
                repo_id: repo.id,
                status: Arc::clone(status),
                show_row,
                counts,
            }
        })();

        match action {
            Action::Clear => {
                self.history_worktree_summary_cache = None;
                (false, (0, 0, 0))
            }
            Action::CacheOk { show_row, counts } => (show_row, counts),
            Action::Rebuild {
                repo_id,
                status,
                show_row,
                counts,
            } => {
                self.history_worktree_summary_cache = Some(HistoryWorktreeSummaryCache {
                    repo_id,
                    status,
                    show_row,
                    counts,
                });
                (show_row, counts)
            }
        }
    }

    pub(super) fn ensure_history_stash_ids_cache(&mut self) -> Option<Arc<HashSet<CommitId>>> {
        enum Action {
            Clear,
            CacheOk(Arc<HashSet<CommitId>>),
            Rebuild {
                repo_id: RepoId,
                stashes_rev: u64,
                ids: Arc<HashSet<CommitId>>,
            },
        }

        let action = (|| {
            let Some(repo) = self.active_repo() else {
                return Action::Clear;
            };
            let Loadable::Ready(stashes) = &repo.stashes else {
                return Action::Clear;
            };
            if stashes.is_empty() {
                return Action::Clear;
            }

            let stashes_rev = repo.stashes_rev;
            if let Some(cache) = &self.history_stash_ids_cache
                && cache.repo_id == repo.id
                && cache.stashes_rev == stashes_rev
            {
                return Action::CacheOk(Arc::clone(&cache.ids));
            }

            let ids: HashSet<_> = stashes.iter().map(|s| s.id.clone()).collect();
            let ids = Arc::new(ids);
            Action::Rebuild {
                repo_id: repo.id,
                stashes_rev,
                ids: Arc::clone(&ids),
            }
        })();

        match action {
            Action::Clear => {
                self.history_stash_ids_cache = None;
                None
            }
            Action::CacheOk(ids) => Some(ids),
            Action::Rebuild {
                repo_id,
                stashes_rev,
                ids,
            } => {
                self.history_stash_ids_cache = Some(HistoryStashIdsCache {
                    repo_id,
                    stashes_rev,
                    ids: Arc::clone(&ids),
                });
                Some(ids)
            }
        }
    }

    pub(super) fn ensure_history_cache(&mut self, cx: &mut gpui::Context<Self>) {
        enum Next {
            Clear,
            CacheOk,
            Inflight,
            Build {
                request: HistoryCacheRequest,
                commits: Vec<Commit>,
                head_branch: Option<String>,
                branches: Vec<Branch>,
                tags: Vec<Tag>,
            },
        }

        let next = if let Some(repo) = self.active_repo() {
            if let Loadable::Ready(page) = &repo.log {
                let request = HistoryCacheRequest {
                    repo_id: repo.id,
                    log_fingerprint: Self::log_fingerprint(&page.commits),
                    head_branch_rev: repo.head_branch_rev,
                    branches_rev: repo.branches_rev,
                    tags_rev: repo.tags_rev,
                    date_time_format: self.date_time_format,
                };

                let cache_ok = self
                    .history_cache
                    .as_ref()
                    .is_some_and(|c| c.request == request);
                if cache_ok {
                    Next::CacheOk
                } else if self.history_cache_inflight.as_ref() == Some(&request) {
                    Next::Inflight
                } else {
                    Next::Build {
                        request,
                        commits: page.commits.clone(),
                        head_branch: match &repo.head_branch {
                            Loadable::Ready(h) => Some(h.clone()),
                            _ => None,
                        },
                        branches: match &repo.branches {
                            Loadable::Ready(b) => b.clone(),
                            _ => Vec::new(),
                        },
                        tags: match &repo.tags {
                            Loadable::Ready(t) => t.clone(),
                            _ => Vec::new(),
                        },
                    }
                }
            } else {
                Next::Clear
            }
        } else {
            Next::Clear
        };

        let (request_for_task, commits, head_branch, branches, tags) = match next {
            Next::Clear => {
                self.history_cache_inflight = None;
                self.history_cache = None;
                return;
            }
            Next::CacheOk => {
                self.history_cache_inflight = None;
                return;
            }
            Next::Inflight => {
                return;
            }
            Next::Build {
                request,
                commits,
                head_branch,
                branches,
                tags,
            } => (request, commits, head_branch, branches, tags),
        };

        self.history_cache_seq = self.history_cache_seq.wrapping_add(1);
        let seq = self.history_cache_seq;
        self.history_cache_inflight = Some(request_for_task.clone());

        let theme = self.theme;

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
                let visible_indices = (0..commits.len()).collect::<Vec<_>>();

                let visible_commits = visible_indices
                    .iter()
                    .filter_map(|ix| commits.get(*ix))
                    .collect::<Vec<_>>();

                let graph_rows = history_graph::compute_graph(&visible_commits, theme);
                let max_lanes = graph_rows
                    .iter()
                    .map(|r| r.lanes_now.len().max(r.lanes_next.len()))
                    .max()
                    .unwrap_or(1);

                let head_target = head_branch
                    .as_deref()
                    .and_then(|head| branches.iter().find(|b| b.name == head))
                    .map(|b| b.target.as_ref());

                let mut branch_names_by_target: HashMap<&str, Vec<&str>> = HashMap::new();
                for branch in &branches {
                    let should_skip = head_branch
                        .as_ref()
                        .is_some_and(|head| branch.name == *head)
                        && head_target == Some(branch.target.as_ref());
                    if should_skip {
                        continue;
                    }
                    branch_names_by_target
                        .entry(branch.target.as_ref())
                        .or_default()
                        .push(branch.name.as_str());
                }
                for names in branch_names_by_target.values_mut() {
                    names.sort_unstable();
                    names.dedup();
                }

                let mut tag_names_by_target: HashMap<&str, Vec<&str>> = HashMap::new();
                for tag in &tags {
                    tag_names_by_target
                        .entry(tag.target.as_ref())
                        .or_default()
                        .push(tag.name.as_str());
                }
                for names in tag_names_by_target.values_mut() {
                    names.sort_unstable();
                    names.dedup();
                }

                let empty_tags: Arc<[SharedString]> = Vec::new().into();
                let commit_row_vms = visible_indices
                    .iter()
                    .filter_map(|ix| commits.get(*ix))
                    .map(|commit| {
                        let commit_id = commit.id.as_ref();

                        let branches_text = {
                            let mut names: Vec<String> = Vec::new();
                            if head_target == Some(commit_id)
                                && let Some(head) = head_branch.as_ref()
                            {
                                names.push(format!("HEAD → {head}"));
                            }
                            if let Some(branches) = branch_names_by_target.get(commit_id) {
                                names.extend(branches.iter().copied().map(ToOwned::to_owned));
                            }
                            names.sort();
                            names.dedup();
                            if names.is_empty() {
                                SharedString::from("")
                            } else {
                                SharedString::from(names.join(", "))
                            }
                        };

                        let tag_names = tag_names_by_target.get(commit_id).map_or_else(
                            || Arc::clone(&empty_tags),
                            |names| {
                                let tag_names: Vec<SharedString> = names
                                    .iter()
                                    .copied()
                                    .map(|n| n.to_string().into())
                                    .collect();
                                tag_names.into()
                            },
                        );

                        let when: SharedString =
                            format_datetime_utc(commit.time, request_for_task.date_time_format)
                                .into();

                        let id: &str = commit.id.as_ref();
                        let short = id.get(0..8).unwrap_or(id);
                        let short_sha: SharedString = short.to_string().into();

                        HistoryCommitRowVm {
                            branches_text,
                            tag_names,
                            when,
                            short_sha,
                        }
                    })
                    .collect::<Vec<_>>();

                let _ = view.update(cx, |this, cx| {
                    if this.history_cache_seq != seq {
                        return;
                    }
                    if this.history_cache_inflight.as_ref() != Some(&request_for_task) {
                        return;
                    }
                    if this.active_repo_id() != Some(request_for_task.repo_id) {
                        return;
                    }

                    if this.history_col_graph_auto && this.history_col_resize.is_none() {
                        let required = px(HISTORY_GRAPH_MARGIN_X_PX * 2.0
                            + HISTORY_GRAPH_COL_GAP_PX * (max_lanes as f32));
                        this.history_col_graph = required
                            .min(px(HISTORY_COL_GRAPH_MAX_PX))
                            .max(px(HISTORY_COL_GRAPH_MIN_PX));
                    }

                    this.history_cache_inflight = None;
                    this.history_cache = Some(HistoryCache {
                        request: request_for_task.clone(),
                        visible_indices,
                        graph_rows,
                        commit_row_vms,
                    });
                    cx.notify();
                });
            },
        )
        .detach();
    }
}
