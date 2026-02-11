use super::super::*;

pub(in super::super) struct MainPaneView {
    pub(in super::super) store: Arc<AppStore>,
    state: Arc<AppState>,
    pub(in super::super) theme: AppTheme,
    pub(in super::super) date_time_format: DateTimeFormat,
    _ui_model_subscription: gpui::Subscription,
    root_view: WeakEntity<GitGpuiView>,

    pub(in super::super) last_window_size: Size<Pixels>,

    pub(in super::super) diff_view: DiffViewMode,
    pub(in super::super) diff_cache_repo_id: Option<RepoId>,
    pub(in super::super) diff_cache_rev: u64,
    pub(in super::super) diff_cache_target: Option<DiffTarget>,
    pub(in super::super) diff_cache: Vec<AnnotatedDiffLine>,
    pub(in super::super) diff_file_for_src_ix: Vec<Option<Arc<str>>>,
    pub(in super::super) diff_split_cache: Vec<PatchSplitRow>,
    pub(in super::super) diff_split_cache_len: usize,
    pub(in super::super) diff_panel_focus_handle: FocusHandle,
    pub(in super::super) diff_autoscroll_pending: bool,
    pub(in super::super) diff_raw_input: Entity<zed::TextInput>,
    pub(in super::super) diff_visible_indices: Vec<usize>,
    pub(in super::super) diff_visible_cache_len: usize,
    pub(in super::super) diff_visible_view: DiffViewMode,
    pub(in super::super) diff_visible_is_file_view: bool,
    pub(in super::super) diff_scrollbar_markers_cache: Vec<zed::ScrollbarMarker>,
    pub(in super::super) diff_word_highlights: Vec<Option<Vec<Range<usize>>>>,
    pub(in super::super) diff_file_stats: Vec<Option<(usize, usize)>>,
    pub(in super::super) diff_text_segments_cache: Vec<Option<CachedDiffStyledText>>,
    pub(in super::super) diff_selection_anchor: Option<usize>,
    pub(in super::super) diff_selection_range: Option<(usize, usize)>,
    pub(in super::super) diff_text_selecting: bool,
    pub(in super::super) diff_text_anchor: Option<DiffTextPos>,
    pub(in super::super) diff_text_head: Option<DiffTextPos>,
    pub(in super::super) diff_suppress_clicks_remaining: u8,
    pub(in super::super) diff_text_hitboxes: HashMap<(usize, DiffTextRegion), DiffTextHitbox>,
    pub(in super::super) diff_text_layout_cache_epoch: u64,
    pub(in super::super) diff_text_layout_cache: HashMap<u64, DiffTextLayoutCacheEntry>,
    pub(in super::super) diff_hunk_picker_search_input: Option<Entity<zed::TextInput>>,
    pub(in super::super) diff_search_active: bool,
    pub(in super::super) diff_search_query: SharedString,
    pub(in super::super) diff_search_matches: Vec<usize>,
    pub(in super::super) diff_search_match_ix: Option<usize>,
    pub(in super::super) diff_search_input: Entity<zed::TextInput>,
    _diff_search_subscription: gpui::Subscription,

    pub(in super::super) file_diff_cache_repo_id: Option<RepoId>,
    pub(in super::super) file_diff_cache_rev: u64,
    pub(in super::super) file_diff_cache_target: Option<DiffTarget>,
    pub(in super::super) file_diff_cache_path: Option<std::path::PathBuf>,
    pub(in super::super) file_diff_cache_rows: Vec<FileDiffRow>,
    pub(in super::super) file_diff_inline_cache: Vec<AnnotatedDiffLine>,
    pub(in super::super) file_diff_inline_word_highlights: Vec<Option<Vec<Range<usize>>>>,
    pub(in super::super) file_diff_split_word_highlights_old: Vec<Option<Vec<Range<usize>>>>,
    pub(in super::super) file_diff_split_word_highlights_new: Vec<Option<Vec<Range<usize>>>>,

    pub(in super::super) file_image_diff_cache_repo_id: Option<RepoId>,
    pub(in super::super) file_image_diff_cache_rev: u64,
    pub(in super::super) file_image_diff_cache_target: Option<DiffTarget>,
    pub(in super::super) file_image_diff_cache_path: Option<std::path::PathBuf>,
    pub(in super::super) file_image_diff_cache_old: Option<Arc<gpui::Image>>,
    pub(in super::super) file_image_diff_cache_new: Option<Arc<gpui::Image>>,

    pub(in super::super) worktree_preview_path: Option<std::path::PathBuf>,
    pub(in super::super) worktree_preview: Loadable<Arc<Vec<String>>>,
    pub(in super::super) worktree_preview_segments_cache_path: Option<std::path::PathBuf>,
    pub(in super::super) worktree_preview_segments_cache: HashMap<usize, CachedDiffStyledText>,
    pub(in super::super) diff_preview_is_new_file: bool,
    pub(in super::super) diff_preview_new_file_lines: Arc<Vec<String>>,

    pub(in super::super) conflict_resolver_input: Entity<zed::TextInput>,
    pub(in super::super) conflict_resolver: ConflictResolverUiState,
    pub(in super::super) conflict_diff_segments_cache_split:
        HashMap<(usize, ConflictPickSide), CachedDiffStyledText>,
    pub(in super::super) conflict_diff_segments_cache_inline: HashMap<usize, CachedDiffStyledText>,
    pub(in super::super) conflict_resolved_preview_path: Option<std::path::PathBuf>,
    pub(in super::super) conflict_resolved_preview_source_hash: Option<u64>,
    pub(in super::super) conflict_resolved_preview_lines: Vec<String>,
    pub(in super::super) conflict_resolved_preview_segments_cache:
        HashMap<usize, CachedDiffStyledText>,

    pub(in super::super) history_cache_seq: u64,
    pub(in super::super) history_cache_inflight: Option<HistoryCacheRequest>,
    pub(in super::super) history_col_branch: Pixels,
    pub(in super::super) history_col_graph: Pixels,
    pub(in super::super) history_col_date: Pixels,
    pub(in super::super) history_col_sha: Pixels,
    pub(in super::super) history_col_graph_auto: bool,
    pub(in super::super) history_col_resize: Option<HistoryColResizeState>,
    pub(in super::super) history_cache: Option<HistoryCache>,
    pub(in super::super) history_worktree_summary_cache: Option<HistoryWorktreeSummaryCache>,
    pub(in super::super) history_stash_ids_cache: Option<HistoryStashIdsCache>,

    pub(in super::super) history_scroll: UniformListScrollHandle,
    pub(in super::super) diff_scroll: UniformListScrollHandle,
    pub(in super::super) conflict_resolver_diff_scroll: UniformListScrollHandle,
    pub(in super::super) conflict_resolved_preview_scroll: UniformListScrollHandle,
    pub(in super::super) worktree_preview_scroll: UniformListScrollHandle,

    path_display_cache: std::cell::RefCell<HashMap<std::path::PathBuf, SharedString>>,
}

impl MainPaneView {
    pub(in super::super) fn new(
        store: Arc<AppStore>,
        ui_model: Entity<AppUiModel>,
        theme: AppTheme,
        date_time_format: DateTimeFormat,
        root_view: WeakEntity<GitGpuiView>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let state = Arc::clone(&ui_model.read(cx).state);
        let subscription = cx.observe(&ui_model, |this, model, cx| {
            let next = Arc::clone(&model.read(cx).state);
            this.apply_state_snapshot(next, cx);
            cx.notify();
        });

        let diff_raw_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "".into(),
                    multiline: true,
                    read_only: true,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let conflict_resolver_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Resolve file contents…".into(),
                    multiline: true,
                    read_only: false,
                    chromeless: true,
                    soft_wrap: true,
                },
                window,
                cx,
            )
        });

        let diff_search_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Search diff".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });
        let diff_search_subscription = cx.observe(&diff_search_input, |this, input, cx| {
            let next: SharedString = input.read(cx).text().to_string().into();
            if this.diff_search_query != next {
                this.diff_search_query = next;
                this.diff_text_segments_cache.clear();
                this.worktree_preview_segments_cache_path = None;
                this.worktree_preview_segments_cache.clear();
                this.conflict_diff_segments_cache_split.clear();
                this.conflict_diff_segments_cache_inline.clear();
                this.diff_search_recompute_matches();
                cx.notify();
            }
        });

        let diff_panel_focus_handle = cx.focus_handle().tab_index(0).tab_stop(false);

        let mut pane = Self {
            store,
            state,
            theme,
            date_time_format,
            _ui_model_subscription: subscription,
            root_view,
            last_window_size: size(px(0.0), px(0.0)),
            diff_view: DiffViewMode::Split,
            diff_cache_repo_id: None,
            diff_cache_rev: 0,
            diff_cache_target: None,
            diff_cache: Vec::new(),
            diff_file_for_src_ix: Vec::new(),
            diff_split_cache: Vec::new(),
            diff_split_cache_len: 0,
            diff_panel_focus_handle,
            diff_autoscroll_pending: false,
            diff_raw_input,
            diff_visible_indices: Vec::new(),
            diff_visible_cache_len: 0,
            diff_visible_view: DiffViewMode::Split,
            diff_visible_is_file_view: false,
            diff_scrollbar_markers_cache: Vec::new(),
            diff_word_highlights: Vec::new(),
            diff_file_stats: Vec::new(),
            diff_text_segments_cache: Vec::new(),
            diff_selection_anchor: None,
            diff_selection_range: None,
            diff_text_selecting: false,
            diff_text_anchor: None,
            diff_text_head: None,
            diff_suppress_clicks_remaining: 0,
            diff_text_hitboxes: HashMap::new(),
            diff_text_layout_cache_epoch: 0,
            diff_text_layout_cache: HashMap::new(),
            diff_hunk_picker_search_input: None,
            diff_search_active: false,
            diff_search_query: "".into(),
            diff_search_matches: Vec::new(),
            diff_search_match_ix: None,
            diff_search_input,
            _diff_search_subscription: diff_search_subscription,
            file_diff_cache_repo_id: None,
            file_diff_cache_rev: 0,
            file_diff_cache_target: None,
            file_diff_cache_path: None,
            file_diff_cache_rows: Vec::new(),
            file_diff_inline_cache: Vec::new(),
            file_diff_inline_word_highlights: Vec::new(),
            file_diff_split_word_highlights_old: Vec::new(),
            file_diff_split_word_highlights_new: Vec::new(),
            file_image_diff_cache_repo_id: None,
            file_image_diff_cache_rev: 0,
            file_image_diff_cache_target: None,
            file_image_diff_cache_path: None,
            file_image_diff_cache_old: None,
            file_image_diff_cache_new: None,
            worktree_preview_path: None,
            worktree_preview: Loadable::NotLoaded,
            worktree_preview_segments_cache_path: None,
            worktree_preview_segments_cache: HashMap::new(),
            diff_preview_is_new_file: false,
            diff_preview_new_file_lines: Arc::new(Vec::new()),
            conflict_resolver_input,
            conflict_resolver: ConflictResolverUiState::default(),
            conflict_diff_segments_cache_split: HashMap::new(),
            conflict_diff_segments_cache_inline: HashMap::new(),
            conflict_resolved_preview_path: None,
            conflict_resolved_preview_source_hash: None,
            conflict_resolved_preview_lines: Vec::new(),
            conflict_resolved_preview_segments_cache: HashMap::new(),
            history_cache_seq: 0,
            history_cache_inflight: None,
            history_col_branch: px(HISTORY_COL_BRANCH_PX),
            history_col_graph: px(HISTORY_COL_GRAPH_PX),
            history_col_date: px(HISTORY_COL_DATE_PX),
            history_col_sha: px(HISTORY_COL_SHA_PX),
            history_col_graph_auto: true,
            history_col_resize: None,
            history_cache: None,
            history_worktree_summary_cache: None,
            history_stash_ids_cache: None,
            history_scroll: UniformListScrollHandle::default(),
            diff_scroll: UniformListScrollHandle::default(),
            conflict_resolver_diff_scroll: UniformListScrollHandle::default(),
            conflict_resolved_preview_scroll: UniformListScrollHandle::default(),
            worktree_preview_scroll: UniformListScrollHandle::default(),
            path_display_cache: std::cell::RefCell::new(HashMap::new()),
        };

        pane.set_theme(theme, cx);
        pane.rebuild_diff_cache();
        pane
    }

    pub(in super::super) fn set_theme(&mut self, theme: AppTheme, cx: &mut gpui::Context<Self>) {
        self.theme = theme;
        self.diff_text_segments_cache.clear();
        self.worktree_preview_segments_cache_path = None;
        self.worktree_preview_segments_cache.clear();
        self.diff_raw_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.diff_search_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.conflict_resolver_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        if let Some(input) = &self.diff_hunk_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        cx.notify();
    }

    pub(in super::super) fn set_date_time_format(
        &mut self,
        next: DateTimeFormat,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.date_time_format == next {
            return;
        }
        self.date_time_format = next;
        self.history_cache = None;
        self.history_cache_inflight = None;
        cx.notify();
    }

    pub(in super::super) fn active_repo_id(&self) -> Option<RepoId> {
        self.state.active_repo
    }

    pub(in super::super) fn active_repo(&self) -> Option<&RepoState> {
        let repo_id = self.active_repo_id()?;
        self.state.repos.iter().find(|r| r.id == repo_id)
    }

    pub(in super::super) fn open_popover_at(
        &mut self,
        kind: PopoverKind,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let _ = self.root_view.update(cx, |root, cx| {
            root.open_popover_at(kind, anchor, window, cx);
        });
    }

    pub(in super::super) fn open_popover_at_cursor(
        &mut self,
        kind: PopoverKind,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let _ = self.root_view.update(cx, |root, cx| {
            root.open_popover_at(kind, root.last_mouse_pos, window, cx);
        });
    }

    pub(in super::super) fn clear_status_multi_selection(
        &mut self,
        repo_id: RepoId,
        cx: &mut gpui::Context<Self>,
    ) {
        let _ = self.root_view.update(cx, |root, cx| {
            let _ = root.details_pane.update(cx, |pane, cx| {
                pane.status_multi_selection.remove(&repo_id);
                cx.notify();
            });
        });
    }

    pub(in super::super) fn scroll_status_list_to_ix(
        &mut self,
        area: DiffArea,
        ix: usize,
        cx: &mut gpui::Context<Self>,
    ) {
        let _ = self.root_view.update(cx, |root, cx| {
            let _ = root
                .details_pane
                .update(cx, |pane: &mut DetailsPaneView, cx| {
                    match area {
                        DiffArea::Unstaged => pane
                            .unstaged_scroll
                            .scroll_to_item_strict(ix, gpui::ScrollStrategy::Center),
                        DiffArea::Staged => pane
                            .staged_scroll
                            .scroll_to_item_strict(ix, gpui::ScrollStrategy::Center),
                    }
                    cx.notify();
                });
        });
    }

    pub(in super::super) fn set_tooltip_text_if_changed(
        &mut self,
        next: Option<SharedString>,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        self.root_view
            .update(cx, |root, cx| {
                let changed = root.set_tooltip_text_if_changed(next);
                if changed {
                    cx.notify();
                }
                changed
            })
            .unwrap_or(false)
    }

    pub(in super::super) fn clear_tooltip_if_matches(
        &mut self,
        tooltip: &SharedString,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let tooltip = tooltip.clone();
        self.root_view
            .update(cx, |root, cx| {
                if root.tooltip_text.as_ref() != Some(&tooltip) {
                    return false;
                }
                let changed = root.set_tooltip_text_if_changed(None);
                if changed {
                    cx.notify();
                }
                changed
            })
            .unwrap_or(false)
    }

    pub(super) fn apply_state_snapshot(
        &mut self,
        next: Arc<AppState>,
        cx: &mut gpui::Context<Self>,
    ) {
        let prev_active_repo_id = self.state.active_repo;
        let prev_diff_target = self
            .active_repo()
            .and_then(|r| r.diff_target.as_ref())
            .cloned();

        let next_repo_id = next.active_repo;
        let next_repo = next_repo_id.and_then(|id| next.repos.iter().find(|r| r.id == id));
        let next_diff_target = next_repo.and_then(|r| r.diff_target.as_ref()).cloned();
        let next_diff_rev = next_repo.map(|r| r.diff_rev).unwrap_or(0);

        if prev_diff_target != next_diff_target {
            self.diff_selection_anchor = None;
            self.diff_selection_range = None;
            self.diff_autoscroll_pending = next_diff_target.is_some();
        }

        self.state = next;

        self.sync_conflict_resolver(cx);

        if prev_active_repo_id != next_repo_id {
            self.history_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
        }

        let should_rebuild_diff_cache = self.diff_cache_repo_id != next_repo_id
            || self.diff_cache_rev != next_diff_rev
            || self.diff_cache_target != next_diff_target;
        if should_rebuild_diff_cache {
            self.rebuild_diff_cache();
        }

        // Precompute derived data that would otherwise be recalculated in hot render paths.
        let _ = self.ensure_history_worktree_summary_cache();
        let _ = self.ensure_history_stash_ids_cache();
    }

    pub(in super::super) fn cached_path_display(&self, path: &std::path::PathBuf) -> SharedString {
        const MAX_ENTRIES: usize = 8_192;
        let mut cache = self.path_display_cache.borrow_mut();
        if cache.len() > MAX_ENTRIES {
            cache.clear();
        }
        if let Some(s) = cache.get(path) {
            return s.clone();
        }
        let s: SharedString = path.display().to_string().into();
        cache.insert(path.clone(), s.clone());
        s
    }

    pub(in super::super) fn is_file_diff_target(target: Option<&DiffTarget>) -> bool {
        matches!(
            target,
            Some(DiffTarget::WorkingTree { .. } | DiffTarget::Commit { path: Some(_), .. })
        )
    }

    fn is_file_preview_active(&self) -> bool {
        self.untracked_worktree_preview_path().is_some()
            || self.added_file_preview_abs_path().is_some()
            || self.deleted_file_preview_abs_path().is_some()
    }

    fn worktree_preview_line_count(&self) -> Option<usize> {
        match &self.worktree_preview {
            Loadable::Ready(lines) => Some(lines.len()),
            _ => None,
        }
    }

    pub(in super::super) fn untracked_worktree_preview_path(&self) -> Option<std::path::PathBuf> {
        let repo = self.active_repo()?;
        let status = match &repo.status {
            Loadable::Ready(s) => s,
            _ => return None,
        };
        let workdir = repo.spec.workdir.clone();
        let DiffTarget::WorkingTree { path, area } = repo.diff_target.as_ref()? else {
            return None;
        };
        if *area != DiffArea::Unstaged {
            return None;
        }
        let is_untracked = status
            .unstaged
            .iter()
            .any(|e| e.kind == FileStatusKind::Untracked && &e.path == path);
        is_untracked.then(|| {
            if path.is_absolute() {
                path.clone()
            } else {
                workdir.join(path)
            }
        })
    }

    pub(in super::super) fn added_file_preview_abs_path(&self) -> Option<std::path::PathBuf> {
        let repo = self.active_repo()?;
        let workdir = repo.spec.workdir.clone();
        let target = repo.diff_target.as_ref()?;

        match target {
            DiffTarget::WorkingTree { path, area } => {
                if *area != DiffArea::Staged {
                    return None;
                }
                let status = match &repo.status {
                    Loadable::Ready(s) => s,
                    _ => return None,
                };
                let is_added = status
                    .staged
                    .iter()
                    .any(|e| e.kind == FileStatusKind::Added && &e.path == path);
                if !is_added {
                    return None;
                }
                Some(if path.is_absolute() {
                    path.clone()
                } else {
                    workdir.join(path)
                })
            }
            DiffTarget::Commit {
                commit_id,
                path: Some(path),
            } => {
                let details = match &repo.commit_details {
                    Loadable::Ready(d) => d,
                    _ => return None,
                };
                if &details.id != commit_id {
                    return None;
                }
                let is_added = details
                    .files
                    .iter()
                    .any(|f| f.kind == FileStatusKind::Added && &f.path == path);
                if !is_added {
                    return None;
                }
                Some(workdir.join(path))
            }
            _ => None,
        }
    }

    pub(in super::super) fn deleted_file_preview_abs_path(&self) -> Option<std::path::PathBuf> {
        let repo = self.active_repo()?;
        let workdir = repo.spec.workdir.clone();
        let target = repo.diff_target.as_ref()?;

        match target {
            DiffTarget::WorkingTree { path, area } => {
                let status = match &repo.status {
                    Loadable::Ready(s) => s,
                    _ => return None,
                };
                let entries = match area {
                    DiffArea::Unstaged => status.unstaged.as_slice(),
                    DiffArea::Staged => status.staged.as_slice(),
                };
                let is_deleted = entries
                    .iter()
                    .any(|e| e.kind == FileStatusKind::Deleted && &e.path == path);
                if !is_deleted {
                    return None;
                }
                Some(if path.is_absolute() {
                    path.clone()
                } else {
                    workdir.join(path)
                })
            }
            DiffTarget::Commit {
                commit_id,
                path: Some(path),
            } => {
                let details = match &repo.commit_details {
                    Loadable::Ready(d) => d,
                    _ => return None,
                };
                if &details.id != commit_id {
                    return None;
                }
                let is_deleted = details
                    .files
                    .iter()
                    .any(|f| f.kind == FileStatusKind::Deleted && &f.path == path);
                if !is_deleted {
                    return None;
                }
                Some(workdir.join(path))
            }
            _ => None,
        }
    }

    pub(in super::super) fn ensure_preview_loading(&mut self, path: std::path::PathBuf) {
        let should_reset = match self.worktree_preview_path.as_ref() {
            Some(p) => p != &path,
            None => true,
        };
        if should_reset {
            self.worktree_preview_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
            self.worktree_preview_path = Some(path);
            self.worktree_preview = Loadable::Loading;
            self.worktree_preview_segments_cache_path = None;
            self.worktree_preview_segments_cache.clear();
        } else if matches!(
            self.worktree_preview,
            Loadable::NotLoaded | Loadable::Error(_)
        ) {
            self.worktree_preview = Loadable::Loading;
        }
    }

    pub(in super::super) fn ensure_worktree_preview_loaded(
        &mut self,
        path: std::path::PathBuf,
        cx: &mut gpui::Context<Self>,
    ) {
        let should_reload = match self.worktree_preview_path.as_ref() {
            Some(p) => p != &path,
            None => true,
        } || matches!(
            self.worktree_preview,
            Loadable::Error(_) | Loadable::NotLoaded
        );
        if !should_reload {
            return;
        }

        self.worktree_preview_path = Some(path.clone());
        self.worktree_preview = Loadable::Loading;
        self.worktree_preview_segments_cache_path = None;
        self.worktree_preview_segments_cache.clear();
        self.worktree_preview_scroll
            .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);

        cx.spawn(async move |view, cx| {
            const MAX_BYTES: u64 = 2 * 1024 * 1024;
            let path_for_task = path.clone();
            let task = cx.background_executor().spawn(async move {
                let meta = std::fs::metadata(&path_for_task).map_err(|e| e.to_string())?;
                if meta.is_dir() {
                    return Err("Selected path is a directory. Select a file inside to preview, or stage the directory to add its contents.".to_string());
                }
                if meta.len() > MAX_BYTES {
                    return Err(format!(
                        "File is too large to preview ({} bytes).",
                        meta.len()
                    ));
                }

                let bytes = std::fs::read(&path_for_task).map_err(|e| e.to_string())?;
                let text = String::from_utf8(bytes).map_err(|_| {
                    "File is not valid UTF-8; binary preview is not supported.".to_string()
                })?;

                let lines = text.lines().map(|s| s.to_string()).collect::<Vec<_>>();
                Ok::<Arc<Vec<String>>, String>(Arc::new(lines))
            });

            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                if this.worktree_preview_path.as_ref() != Some(&path) {
                    return;
                }
                this.worktree_preview_scroll
                    .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
                this.worktree_preview = match result {
                    Ok(lines) => Loadable::Ready(lines),
                    Err(e) => Loadable::Error(e),
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub(in super::super) fn try_populate_worktree_preview_from_diff_file(&mut self) {
        let Some((abs_path, preview_result)) = (|| {
            let repo = self.active_repo()?;
            let path_from_target = match repo.diff_target.as_ref()? {
                DiffTarget::WorkingTree { path, .. } => Some(path),
                DiffTarget::Commit {
                    path: Some(path), ..
                } => Some(path),
                _ => None,
            }?;

            let abs_path = if path_from_target.is_absolute() {
                path_from_target.clone()
            } else {
                repo.spec.workdir.join(path_from_target)
            };

            let prefer_old = match repo.diff_target.as_ref()? {
                DiffTarget::WorkingTree { path, area } => match &repo.status {
                    Loadable::Ready(status) => {
                        let entries = match area {
                            DiffArea::Unstaged => status.unstaged.as_slice(),
                            DiffArea::Staged => status.staged.as_slice(),
                        };
                        entries
                            .iter()
                            .any(|e| e.kind == FileStatusKind::Deleted && &e.path == path)
                    }
                    _ => false,
                },
                DiffTarget::Commit {
                    commit_id,
                    path: Some(path),
                } => match &repo.commit_details {
                    Loadable::Ready(details) if &details.id == commit_id => details
                        .files
                        .iter()
                        .any(|f| f.kind == FileStatusKind::Deleted && &f.path == path),
                    _ => false,
                },
                _ => false,
            };

            let mut diff_file_error: Option<String> = None;
            let mut preview_result: Option<Result<Arc<Vec<String>>, String>> = match &repo.diff_file
            {
                Loadable::NotLoaded | Loadable::Loading => None,
                Loadable::Error(e) => {
                    diff_file_error = Some(e.clone());
                    None
                }
                Loadable::Ready(file) => file.as_ref().and_then(|file| {
                    let text = if prefer_old {
                        file.old.as_deref()
                    } else {
                        file.new.as_deref()
                    };
                    text.map(|text| {
                        let lines = text.lines().map(|s| s.to_string()).collect::<Vec<_>>();
                        Ok(Arc::new(lines))
                    })
                }),
            };

            if preview_result.is_none() {
                match &repo.diff {
                    Loadable::Ready(diff) => {
                        let annotated = annotate_unified(diff);
                        if prefer_old {
                            if let Some((_abs_path, lines)) = build_deleted_file_preview_from_diff(
                                &annotated,
                                &repo.spec.workdir,
                                repo.diff_target.as_ref(),
                            ) {
                                preview_result = Some(Ok(Arc::new(lines)));
                            }
                        } else if let Some((_abs_path, lines)) = build_new_file_preview_from_diff(
                            &annotated,
                            &repo.spec.workdir,
                            repo.diff_target.as_ref(),
                        ) {
                            preview_result = Some(Ok(Arc::new(lines)));
                        } else if let Some(e) = diff_file_error {
                            preview_result = Some(Err(e));
                        } else {
                            preview_result =
                                Some(Err("No text preview available for this file.".to_string()));
                        }
                    }
                    Loadable::Error(e) => preview_result = Some(Err(e.clone())),
                    Loadable::NotLoaded | Loadable::Loading => {}
                }
            }

            Some((abs_path, preview_result))
        })() else {
            return;
        };

        if matches!(self.worktree_preview, Loadable::Ready(_))
            && self.worktree_preview_path.as_ref() == Some(&abs_path)
        {
            return;
        }

        let Some(preview_result) = preview_result else {
            return;
        };

        match preview_result {
            Ok(lines) => {
                self.worktree_preview_scroll
                    .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
                self.worktree_preview_path = Some(abs_path);
                self.worktree_preview = Loadable::Ready(lines);
                self.worktree_preview_segments_cache_path = None;
                self.worktree_preview_segments_cache.clear();
            }
            Err(e) => {
                if self.worktree_preview_path.as_ref() != Some(&abs_path)
                    || matches!(
                        self.worktree_preview,
                        Loadable::NotLoaded | Loadable::Loading
                    )
                {
                    self.worktree_preview_scroll
                        .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
                    self.worktree_preview_path = Some(abs_path);
                    self.worktree_preview = Loadable::Error(e);
                    self.worktree_preview_segments_cache_path = None;
                    self.worktree_preview_segments_cache.clear();
                }
            }
        }
    }

    pub(in super::super) fn touch_diff_text_layout_cache(
        &mut self,
        key: u64,
        layout: Option<ShapedLine>,
    ) {
        let epoch = self.diff_text_layout_cache_epoch;
        match layout {
            Some(layout) => {
                self.diff_text_layout_cache.insert(
                    key,
                    DiffTextLayoutCacheEntry {
                        layout,
                        last_used_epoch: epoch,
                    },
                );
            }
            None => {
                if let Some(entry) = self.diff_text_layout_cache.get_mut(&key) {
                    entry.last_used_epoch = epoch;
                }
            }
        }

        self.prune_diff_text_layout_cache();
    }

    fn prune_diff_text_layout_cache(&mut self) {
        if self.diff_text_layout_cache.len()
            <= DIFF_TEXT_LAYOUT_CACHE_MAX_ENTRIES + DIFF_TEXT_LAYOUT_CACHE_PRUNE_OVERAGE
        {
            return;
        }

        let over_by = self
            .diff_text_layout_cache
            .len()
            .saturating_sub(DIFF_TEXT_LAYOUT_CACHE_MAX_ENTRIES);
        if over_by == 0 {
            return;
        }

        let mut by_age: Vec<(u64, u64)> = self
            .diff_text_layout_cache
            .iter()
            .map(|(k, v)| (*k, v.last_used_epoch))
            .collect();
        by_age.sort_by_key(|(_, last_used)| *last_used);

        for (key, _) in by_age.into_iter().take(over_by) {
            self.diff_text_layout_cache.remove(&key);
        }
    }

    pub(in super::super) fn diff_text_segments_cache_get(
        &self,
        key: usize,
    ) -> Option<&CachedDiffStyledText> {
        self.diff_text_segments_cache
            .get(key)
            .and_then(Option::as_ref)
    }

    pub(in super::super) fn diff_text_segments_cache_set(
        &mut self,
        key: usize,
        value: CachedDiffStyledText,
    ) -> &CachedDiffStyledText {
        if self.diff_text_segments_cache.len() <= key {
            self.diff_text_segments_cache.resize_with(key + 1, || None);
        }
        self.diff_text_segments_cache[key] = Some(value);
        self.diff_text_segments_cache[key]
            .as_ref()
            .expect("just set")
    }

    pub(in super::super) fn is_file_diff_view_active(&self) -> bool {
        let Some(repo) = self.active_repo() else {
            return false;
        };
        self.file_diff_cache_repo_id == Some(repo.id)
            && self.file_diff_cache_rev == repo.diff_file_rev
            && self.file_diff_cache_target == repo.diff_target
            && self.file_diff_cache_path.is_some()
    }

    pub(in super::super) fn is_file_image_diff_view_active(&self) -> bool {
        let Some(repo) = self.active_repo() else {
            return false;
        };
        self.file_image_diff_cache_repo_id == Some(repo.id)
            && self.file_image_diff_cache_rev == repo.diff_file_rev
            && self.file_image_diff_cache_target == repo.diff_target
            && self.file_image_diff_cache_path.is_some()
            && (self.file_image_diff_cache_old.is_some()
                || self.file_image_diff_cache_new.is_some())
    }

    pub(in super::super) fn consume_suppress_click_after_drag(&mut self) -> bool {
        if self.diff_suppress_clicks_remaining > 0 {
            self.diff_suppress_clicks_remaining =
                self.diff_suppress_clicks_remaining.saturating_sub(1);
            return true;
        }
        false
    }

    fn diff_text_normalized_selection(&self) -> Option<(DiffTextPos, DiffTextPos)> {
        let a = self.diff_text_anchor?;
        let b = self.diff_text_head?;
        Some(if a.cmp_key() <= b.cmp_key() {
            (a, b)
        } else {
            (b, a)
        })
    }

    pub(in super::super) fn diff_text_selection_color(&self) -> gpui::Rgba {
        with_alpha(
            self.theme.colors.accent,
            if self.theme.is_dark { 0.28 } else { 0.18 },
        )
    }

    pub(in super::super) fn set_diff_text_hitbox(
        &mut self,
        visible_ix: usize,
        region: DiffTextRegion,
        hitbox: DiffTextHitbox,
    ) {
        self.diff_text_hitboxes.insert((visible_ix, region), hitbox);
    }

    fn diff_text_pos_from_hitbox(
        &self,
        visible_ix: usize,
        region: DiffTextRegion,
        position: Point<Pixels>,
    ) -> Option<DiffTextPos> {
        let hitbox = self.diff_text_hitboxes.get(&(visible_ix, region))?;
        let layout = &self.diff_text_layout_cache.get(&hitbox.layout_key)?.layout;
        let local = hitbox.bounds.localize(&position)?;
        let x = local.x.max(px(0.0));
        let offset = layout
            .closest_index_for_x(x)
            .min(layout.len())
            .min(hitbox.text_len);
        Some(DiffTextPos {
            visible_ix,
            region,
            offset,
        })
    }

    fn diff_text_pos_for_mouse(&self, position: Point<Pixels>) -> Option<DiffTextPos> {
        if self.diff_text_hitboxes.is_empty() {
            return None;
        }

        let restrict_region = self
            .diff_text_selecting
            .then_some(self.diff_text_anchor)
            .flatten()
            .map(|p| p.region)
            .filter(|r| matches!(r, DiffTextRegion::SplitLeft | DiffTextRegion::SplitRight));

        for ((visible_ix, region), hitbox) in &self.diff_text_hitboxes {
            if restrict_region.is_some_and(|restrict| restrict != *region) {
                continue;
            }
            if hitbox.bounds.contains(&position) {
                return self.diff_text_pos_from_hitbox(*visible_ix, *region, position);
            }
        }

        let mut best: Option<((usize, DiffTextRegion), Pixels)> = None;
        for (key, hitbox) in &self.diff_text_hitboxes {
            if restrict_region.is_some_and(|restrict| restrict != key.1) {
                continue;
            }
            let dy = if position.y < hitbox.bounds.top() {
                hitbox.bounds.top() - position.y
            } else if position.y > hitbox.bounds.bottom() {
                position.y - hitbox.bounds.bottom()
            } else {
                px(0.0)
            };
            let dx = if position.x < hitbox.bounds.left() {
                hitbox.bounds.left() - position.x
            } else if position.x > hitbox.bounds.right() {
                position.x - hitbox.bounds.right()
            } else {
                px(0.0)
            };
            let score = dy + dx;
            if best.is_none() || score < best.unwrap().1 {
                best = Some((*key, score));
            }
        }
        let ((visible_ix, region), _) = best?;
        self.diff_text_pos_from_hitbox(visible_ix, region, position)
    }

    pub(in super::super) fn begin_diff_text_selection(
        &mut self,
        visible_ix: usize,
        region: DiffTextRegion,
        position: Point<Pixels>,
    ) {
        let Some(pos) = self.diff_text_pos_from_hitbox(visible_ix, region, position) else {
            return;
        };
        self.diff_text_selecting = true;
        self.diff_text_anchor = Some(pos);
        self.diff_text_head = Some(pos);
        self.diff_suppress_clicks_remaining = 0;
    }

    pub(in super::super) fn update_diff_text_selection_from_mouse(
        &mut self,
        position: Point<Pixels>,
    ) {
        if !self.diff_text_selecting {
            return;
        }
        let Some(pos) = self.diff_text_pos_for_mouse(position) else {
            return;
        };
        if self.diff_text_head != Some(pos) {
            self.diff_text_head = Some(pos);
            if self
                .diff_text_normalized_selection()
                .is_some_and(|(a, b)| a != b)
            {
                self.diff_suppress_clicks_remaining = 1;
            }
        }
    }

    pub(in super::super) fn end_diff_text_selection(&mut self) {
        self.diff_text_selecting = false;
    }

    pub(in super::super) fn diff_text_has_selection(&self) -> bool {
        self.diff_text_normalized_selection()
            .is_some_and(|(a, b)| a != b)
    }

    pub(in super::super) fn diff_text_local_selection_range(
        &self,
        visible_ix: usize,
        region: DiffTextRegion,
        text_len: usize,
    ) -> Option<Range<usize>> {
        let (start, end) = self.diff_text_normalized_selection()?;
        if start == end {
            return None;
        }
        if visible_ix < start.visible_ix || visible_ix > end.visible_ix {
            return None;
        }

        let split_region = (self.diff_view == DiffViewMode::Split
            && start.region == end.region
            && matches!(
                start.region,
                DiffTextRegion::SplitLeft | DiffTextRegion::SplitRight
            ))
        .then_some(start.region);
        if split_region.is_some_and(|r| r != region) {
            return None;
        }

        let region_order = region.order();
        let start_order = start.region.order();
        let end_order = end.region.order();

        let mut a = 0usize;
        let mut b = text_len;

        if start.visible_ix == end.visible_ix && visible_ix == start.visible_ix {
            if region_order < start_order || region_order > end_order {
                return None;
            }
            if region == start.region {
                a = start.offset.min(text_len);
            }
            if region == end.region {
                b = end.offset.min(text_len);
            }
        } else if visible_ix == start.visible_ix {
            if region_order < start_order {
                return None;
            }
            if region == start.region {
                a = start.offset.min(text_len);
            }
        } else if visible_ix == end.visible_ix {
            if region_order > end_order {
                return None;
            }
            if region == end.region {
                b = end.offset.min(text_len);
            }
        }

        if a >= b {
            return None;
        }
        Some(a..b)
    }

    pub(in super::super) fn diff_text_line_for_region(
        &self,
        visible_ix: usize,
        region: DiffTextRegion,
    ) -> SharedString {
        let fallback = SharedString::default();
        let expand_tabs = |s: &str| -> SharedString {
            if !s.contains('\t') {
                return s.to_string().into();
            }
            let mut out = String::with_capacity(s.len());
            for ch in s.chars() {
                match ch {
                    '\t' => out.push_str("    "),
                    _ => out.push(ch),
                }
            }
            out.into()
        };

        if self.is_file_preview_active() {
            if region != DiffTextRegion::Inline {
                return fallback;
            }
            let Loadable::Ready(lines) = &self.worktree_preview else {
                return fallback;
            };
            return lines
                .get(visible_ix)
                .map(|l| expand_tabs(l))
                .unwrap_or(fallback);
        }

        let Some(&mapped_ix) = self.diff_visible_indices.get(visible_ix) else {
            return fallback;
        };

        if self.diff_view == DiffViewMode::Inline {
            if region != DiffTextRegion::Inline {
                return fallback;
            }
            if self.is_file_diff_view_active() {
                if let Some(styled) = self.diff_text_segments_cache_get(mapped_ix) {
                    return styled.text.clone();
                }
                return self
                    .file_diff_inline_cache
                    .get(mapped_ix)
                    .map(|l| expand_tabs(diff_content_text(l)))
                    .unwrap_or(fallback);
            }

            if let Some(styled) = self.diff_text_segments_cache_get(mapped_ix) {
                return styled.text.clone();
            }
            let Some(line) = self.diff_cache.get(mapped_ix) else {
                return fallback;
            };
            let display = if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
                parse_unified_hunk_header_for_display(line.text.as_ref())
                    .map(|p| {
                        let heading = p.heading.unwrap_or_default();
                        if heading.is_empty() {
                            format!("{} {}", p.old, p.new)
                        } else {
                            format!("{} {}  {heading}", p.old, p.new)
                        }
                    })
                    .unwrap_or_else(|| line.text.clone())
            } else if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                && line.text.starts_with("diff --git ")
            {
                parse_diff_git_header_path(line.text.as_ref()).unwrap_or_else(|| line.text.clone())
            } else {
                line.text.clone()
            };
            return expand_tabs(display.as_str());
        }

        match region {
            DiffTextRegion::SplitLeft | DiffTextRegion::SplitRight => {}
            DiffTextRegion::Inline => return fallback,
        }

        if self.is_file_diff_view_active() {
            let key = match region {
                DiffTextRegion::SplitLeft => mapped_ix * 2,
                DiffTextRegion::SplitRight => mapped_ix * 2 + 1,
                DiffTextRegion::Inline => unreachable!(),
            };
            if let Some(styled) = self.diff_text_segments_cache_get(key) {
                return styled.text.clone();
            }
            let Some(row) = self.file_diff_cache_rows.get(mapped_ix) else {
                return fallback;
            };
            let text = match region {
                DiffTextRegion::SplitLeft => row.old.as_deref().unwrap_or(""),
                DiffTextRegion::SplitRight => row.new.as_deref().unwrap_or(""),
                DiffTextRegion::Inline => unreachable!(),
            };
            return expand_tabs(text);
        }

        let Some(split_row) = self.diff_split_cache.get(mapped_ix) else {
            return fallback;
        };
        match split_row {
            PatchSplitRow::Raw { src_ix, click_kind } => {
                let Some(line) = self.diff_cache.get(*src_ix) else {
                    return fallback;
                };
                let display = match click_kind {
                    DiffClickKind::HunkHeader => {
                        parse_unified_hunk_header_for_display(line.text.as_ref())
                            .map(|p| {
                                let heading = p.heading.unwrap_or_default();
                                if heading.is_empty() {
                                    format!("{} {}", p.old, p.new)
                                } else {
                                    format!("{} {}  {heading}", p.old, p.new)
                                }
                            })
                            .unwrap_or_else(|| line.text.clone())
                    }
                    DiffClickKind::FileHeader => parse_diff_git_header_path(line.text.as_ref())
                        .unwrap_or_else(|| line.text.clone()),
                    DiffClickKind::Line => line.text.clone(),
                };
                expand_tabs(display.as_str())
            }
            PatchSplitRow::Aligned { row, .. } => {
                let text = match region {
                    DiffTextRegion::SplitLeft => row.old.as_deref().unwrap_or(""),
                    DiffTextRegion::SplitRight => row.new.as_deref().unwrap_or(""),
                    DiffTextRegion::Inline => unreachable!(),
                };
                expand_tabs(text)
            }
        }
    }

    fn diff_text_combined_offset(&self, pos: DiffTextPos, left_len: usize) -> usize {
        match self.diff_view {
            DiffViewMode::Inline => pos.offset,
            DiffViewMode::Split => match pos.region {
                DiffTextRegion::SplitLeft => pos.offset,
                DiffTextRegion::SplitRight => left_len.saturating_add(1).saturating_add(pos.offset),
                DiffTextRegion::Inline => pos.offset,
            },
        }
    }

    fn selected_diff_text_string(&self) -> Option<String> {
        let (start, end) = self.diff_text_normalized_selection()?;
        if start == end {
            return None;
        }

        let force_inline = self.is_file_preview_active();

        let mut out = String::new();
        for visible_ix in start.visible_ix..=end.visible_ix {
            if force_inline || self.diff_view == DiffViewMode::Inline {
                let text = self.diff_text_line_for_region(visible_ix, DiffTextRegion::Inline);
                let line_len = text.len();
                let a = if visible_ix == start.visible_ix {
                    start.offset.min(line_len)
                } else {
                    0
                };
                let b = if visible_ix == end.visible_ix {
                    end.offset.min(line_len)
                } else {
                    line_len
                };
                if !out.is_empty() {
                    out.push('\n');
                }
                if a < b {
                    out.push_str(&text[a..b]);
                }
                continue;
            }

            let split_region = (start.region == end.region
                && matches!(
                    start.region,
                    DiffTextRegion::SplitLeft | DiffTextRegion::SplitRight
                ))
            .then_some(start.region);

            if let Some(region) = split_region {
                let text = self.diff_text_line_for_region(visible_ix, region);
                let line_len = text.len();
                let a = if visible_ix == start.visible_ix {
                    start.offset.min(line_len)
                } else {
                    0
                };
                let b = if visible_ix == end.visible_ix {
                    end.offset.min(line_len)
                } else {
                    line_len
                };
                if !out.is_empty() {
                    out.push('\n');
                }
                if a < b {
                    out.push_str(&text[a..b]);
                }
            } else {
                let left = self.diff_text_line_for_region(visible_ix, DiffTextRegion::SplitLeft);
                let right = self.diff_text_line_for_region(visible_ix, DiffTextRegion::SplitRight);
                let combined = format!("{}\t{}", left.as_ref(), right.as_ref());
                let left_len = left.len();
                let combined_len = combined.len();

                let a = if visible_ix == start.visible_ix {
                    self.diff_text_combined_offset(start, left_len)
                        .min(combined_len)
                } else {
                    0
                };
                let b = if visible_ix == end.visible_ix {
                    self.diff_text_combined_offset(end, left_len)
                        .min(combined_len)
                } else {
                    combined_len
                };

                if !out.is_empty() {
                    out.push('\n');
                }
                if a < b {
                    out.push_str(&combined[a..b]);
                }
            }
        }

        if out.is_empty() { None } else { Some(out) }
    }

    pub(in super::super) fn copy_selected_diff_text_to_clipboard(
        &mut self,
        cx: &mut gpui::Context<Self>,
    ) {
        let Some(text) = self.selected_diff_text_string() else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
    }

    pub(in super::super) fn copy_diff_text_selection_or_region_line_to_clipboard(
        &mut self,
        visible_ix: usize,
        region: DiffTextRegion,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.diff_text_has_selection() {
            self.copy_selected_diff_text_to_clipboard(cx);
            return;
        }
        let text = self.diff_text_line_for_region(visible_ix, region);
        if text.is_empty() {
            return;
        }
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.to_string()));
    }

    pub(in super::super) fn open_diff_editor_context_menu(
        &mut self,
        visible_ix: usize,
        region: DiffTextRegion,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let Some(repo) = self.active_repo() else {
            return;
        };
        let repo_id = repo.id;
        let workdir = repo.spec.workdir.clone();

        let (area, allow_apply) = match repo.diff_target.as_ref() {
            Some(DiffTarget::WorkingTree { area, .. }) => (*area, true),
            _ => (DiffArea::Unstaged, false),
        };

        let copy_text = self.selected_diff_text_string().or_else(|| {
            let text = self.diff_text_line_for_region(visible_ix, region);
            (!text.is_empty()).then_some(text.to_string())
        });

        let list_len = self.diff_visible_indices.len();
        let clicked_visible_ix = if list_len == 0 {
            visible_ix
        } else {
            visible_ix.min(list_len - 1)
        };

        let text_selection = context_menu_selection_range_from_diff_text(
            self.diff_text_normalized_selection(),
            self.diff_view,
            clicked_visible_ix,
            region,
        );

        if list_len > 0 && text_selection.is_none() {
            let existing = self
                .diff_selection_range
                .map(|(a, b)| (a.min(b), a.max(b)))
                .filter(|(a, b)| clicked_visible_ix >= *a && clicked_visible_ix <= *b);
            if existing.is_none() {
                self.diff_selection_anchor = Some(clicked_visible_ix);
                self.diff_selection_range = Some((clicked_visible_ix, clicked_visible_ix));
            }
        }

        struct FileDiffSrcLookup {
            file_rel: std::path::PathBuf,
            add_by_new_line: std::collections::HashMap<u32, usize>,
            remove_by_old_line: std::collections::HashMap<u32, usize>,
            context_by_old_line: std::collections::HashMap<u32, usize>,
        }

        let file_diff_lookup = if self.is_file_diff_view_active() {
            self.file_diff_cache_path.as_ref().map(|abs| {
                let rel = abs.strip_prefix(&workdir).unwrap_or(abs);
                let file_rel = rel.to_path_buf();
                // Git diffs use forward slashes even on Windows.
                let rel_str = file_rel.to_string_lossy().replace('\\', "/");

                let mut add_by_new_line: std::collections::HashMap<u32, usize> =
                    std::collections::HashMap::new();
                let mut remove_by_old_line: std::collections::HashMap<u32, usize> =
                    std::collections::HashMap::new();
                let mut context_by_old_line: std::collections::HashMap<u32, usize> =
                    std::collections::HashMap::new();

                for (ix, line) in self.diff_cache.iter().enumerate() {
                    if self.diff_file_for_src_ix.get(ix).and_then(|p| p.as_deref())
                        != Some(rel_str.as_str())
                    {
                        continue;
                    }
                    match line.kind {
                        gitgpui_core::domain::DiffLineKind::Add => {
                            if let Some(n) = line.new_line {
                                add_by_new_line.insert(n, ix);
                            }
                        }
                        gitgpui_core::domain::DiffLineKind::Remove => {
                            if let Some(o) = line.old_line {
                                remove_by_old_line.insert(o, ix);
                            }
                        }
                        gitgpui_core::domain::DiffLineKind::Context => {
                            if let Some(o) = line.old_line {
                                context_by_old_line.insert(o, ix);
                            }
                        }
                        gitgpui_core::domain::DiffLineKind::Header
                        | gitgpui_core::domain::DiffLineKind::Hunk => {}
                    }
                }

                FileDiffSrcLookup {
                    file_rel,
                    add_by_new_line,
                    remove_by_old_line,
                    context_by_old_line,
                }
            })
        } else {
            None
        };

        let src_ixs_for_visible_ix = |visible_ix: usize| -> Vec<usize> {
            if let Some(lookup) = file_diff_lookup.as_ref() {
                let Some(&mapped_ix) = self.diff_visible_indices.get(visible_ix) else {
                    return Vec::new();
                };
                match self.diff_view {
                    DiffViewMode::Inline => {
                        let Some(line) = self.file_diff_inline_cache.get(mapped_ix) else {
                            return Vec::new();
                        };
                        match line.kind {
                            gitgpui_core::domain::DiffLineKind::Add => line
                                .new_line
                                .and_then(|n| lookup.add_by_new_line.get(&n).copied())
                                .into_iter()
                                .collect(),
                            gitgpui_core::domain::DiffLineKind::Remove => line
                                .old_line
                                .and_then(|o| lookup.remove_by_old_line.get(&o).copied())
                                .into_iter()
                                .collect(),
                            gitgpui_core::domain::DiffLineKind::Context => line
                                .old_line
                                .and_then(|o| lookup.context_by_old_line.get(&o).copied())
                                .into_iter()
                                .collect(),
                            gitgpui_core::domain::DiffLineKind::Header
                            | gitgpui_core::domain::DiffLineKind::Hunk => Vec::new(),
                        }
                    }
                    DiffViewMode::Split => {
                        let Some(row) = self.file_diff_cache_rows.get(mapped_ix) else {
                            return Vec::new();
                        };
                        match row.kind {
                            gitgpui_core::file_diff::FileDiffRowKind::Context => row
                                .old_line
                                .and_then(|o| lookup.context_by_old_line.get(&o).copied())
                                .into_iter()
                                .collect(),
                            gitgpui_core::file_diff::FileDiffRowKind::Add => row
                                .new_line
                                .and_then(|n| lookup.add_by_new_line.get(&n).copied())
                                .into_iter()
                                .collect(),
                            gitgpui_core::file_diff::FileDiffRowKind::Remove => row
                                .old_line
                                .and_then(|o| lookup.remove_by_old_line.get(&o).copied())
                                .into_iter()
                                .collect(),
                            gitgpui_core::file_diff::FileDiffRowKind::Modify => {
                                let mut out = Vec::new();
                                if let Some(o) = row.old_line
                                    && let Some(ix) = lookup.remove_by_old_line.get(&o).copied()
                                {
                                    out.push(ix);
                                }
                                if let Some(n) = row.new_line
                                    && let Some(ix) = lookup.add_by_new_line.get(&n).copied()
                                    && !out.contains(&ix)
                                {
                                    out.push(ix);
                                }
                                out
                            }
                        }
                    }
                }
            } else {
                self.diff_src_ixs_for_visible_ix(visible_ix)
            }
        };

        let clicked_src_ix = src_ixs_for_visible_ix(clicked_visible_ix)
            .into_iter()
            .next();
        let hunk_src_ix = clicked_src_ix.and_then(|src_ix| self.diff_enclosing_hunk_src_ix(src_ix));

        let path = hunk_src_ix
            .or(clicked_src_ix)
            .and_then(|ix| self.diff_file_for_src_ix.get(ix))
            .and_then(|p| p.as_deref())
            .map(std::path::PathBuf::from);
        let path = path.or_else(|| file_diff_lookup.as_ref().map(|l| l.file_rel.clone()));

        let allow_patch_actions = allow_apply && !self.is_file_preview_active();

        let selection = text_selection
            .or_else(|| self.diff_selection_range.map(|(a, b)| (a.min(b), a.max(b))))
            .or_else(|| (list_len > 0).then_some((clicked_visible_ix, clicked_visible_ix)))
            .map(|(a, b)| {
                if list_len == 0 {
                    (0, 0)
                } else {
                    (a.min(list_len - 1), b.min(list_len - 1))
                }
            });

        let (hunks_count, hunk_patch, lines_count, lines_patch) =
            if allow_patch_actions && let Some((sel_a, sel_b)) = selection {
                let mut selected_src_ixs: std::collections::HashSet<usize> =
                    std::collections::HashSet::new();
                let mut selected_change_src_ixs: std::collections::HashSet<usize> =
                    std::collections::HashSet::new();

                for vix in sel_a..=sel_b {
                    for src_ix in src_ixs_for_visible_ix(vix) {
                        let Some(line) = self.diff_cache.get(src_ix) else {
                            continue;
                        };
                        selected_src_ixs.insert(src_ix);
                        if matches!(
                            line.kind,
                            gitgpui_core::domain::DiffLineKind::Add
                                | gitgpui_core::domain::DiffLineKind::Remove
                        ) {
                            selected_change_src_ixs.insert(src_ix);
                        }
                    }
                }

                let mut selected_hunks: Vec<usize> = selected_src_ixs
                    .into_iter()
                    .filter_map(|ix| self.diff_enclosing_hunk_src_ix(ix))
                    .collect();
                selected_hunks.sort_unstable();
                selected_hunks.dedup();

                let hunk_patch = build_unified_patch_for_hunks(&self.diff_cache, &selected_hunks);
                let hunks_count = hunk_patch
                    .as_ref()
                    .map(|_| selected_hunks.len())
                    .unwrap_or(0);

                let lines_patch = build_unified_patch_for_selected_lines_across_hunks(
                    &self.diff_cache,
                    &selected_change_src_ixs,
                );
                let lines_count = lines_patch
                    .as_ref()
                    .map(|_| selected_change_src_ixs.len())
                    .unwrap_or(0);

                (hunks_count, hunk_patch, lines_count, lines_patch)
            } else {
                (0, None, 0, None)
            };

        self.open_popover_at(
            PopoverKind::DiffEditorMenu {
                repo_id,
                area,
                path,
                hunk_patch,
                hunks_count,
                lines_patch,
                lines_count,
                copy_text,
            },
            anchor,
            window,
            cx,
        );
    }

    fn diff_src_ixs_for_visible_ix(&self, visible_ix: usize) -> Vec<usize> {
        if self.is_file_diff_view_active() {
            return Vec::new();
        }
        let Some(&mapped_ix) = self.diff_visible_indices.get(visible_ix) else {
            return Vec::new();
        };

        match self.diff_view {
            DiffViewMode::Inline => vec![mapped_ix],
            DiffViewMode::Split => {
                let Some(row) = self.diff_split_cache.get(mapped_ix) else {
                    return Vec::new();
                };
                match row {
                    PatchSplitRow::Raw { src_ix, .. } => vec![*src_ix],
                    PatchSplitRow::Aligned {
                        old_src_ix,
                        new_src_ix,
                        ..
                    } => {
                        let mut out = Vec::new();
                        if let Some(ix) = old_src_ix {
                            out.push(*ix);
                        }
                        if let Some(ix) = new_src_ix {
                            if !out.contains(ix) {
                                out.push(*ix);
                            }
                        }
                        out
                    }
                }
            }
        }
    }

    fn diff_enclosing_hunk_src_ix(&self, src_ix: usize) -> Option<usize> {
        enclosing_hunk_src_ix(&self.diff_cache, src_ix)
    }

    pub(in super::super) fn select_all_diff_text(&mut self) {
        if self.is_file_preview_active() {
            let Some(count) = self.worktree_preview_line_count() else {
                return;
            };
            if count == 0 {
                return;
            }
            let end_visible_ix = count - 1;
            let end_text = self.diff_text_line_for_region(end_visible_ix, DiffTextRegion::Inline);

            self.diff_text_selecting = false;
            self.diff_text_anchor = Some(DiffTextPos {
                visible_ix: 0,
                region: DiffTextRegion::Inline,
                offset: 0,
            });
            self.diff_text_head = Some(DiffTextPos {
                visible_ix: end_visible_ix,
                region: DiffTextRegion::Inline,
                offset: end_text.len(),
            });
            return;
        }

        if self.diff_visible_indices.is_empty() {
            return;
        }

        let start_region = match self.diff_view {
            DiffViewMode::Inline => DiffTextRegion::Inline,
            DiffViewMode::Split => self
                .diff_text_head
                .or(self.diff_text_anchor)
                .map(|p| p.region)
                .filter(|r| matches!(r, DiffTextRegion::SplitLeft | DiffTextRegion::SplitRight))
                .unwrap_or(DiffTextRegion::SplitLeft),
        };

        let end_visible_ix = self.diff_visible_indices.len() - 1;
        let end_region = start_region;
        let end_text = self.diff_text_line_for_region(end_visible_ix, end_region);

        self.diff_text_selecting = false;
        self.diff_text_anchor = Some(DiffTextPos {
            visible_ix: 0,
            region: start_region,
            offset: 0,
        });
        self.diff_text_head = Some(DiffTextPos {
            visible_ix: end_visible_ix,
            region: end_region,
            offset: end_text.len(),
        });
    }

    fn select_diff_text_rows_range(
        &mut self,
        start_visible_ix: usize,
        end_visible_ix: usize,
        region: DiffTextRegion,
    ) {
        let list_len = self.diff_visible_indices.len();
        if list_len == 0 {
            return;
        }

        let a = start_visible_ix.min(list_len - 1);
        let b = end_visible_ix.min(list_len - 1);
        let (a, b) = if a <= b { (a, b) } else { (b, a) };

        let region = match self.diff_view {
            DiffViewMode::Inline => DiffTextRegion::Inline,
            DiffViewMode::Split => match region {
                DiffTextRegion::SplitRight => DiffTextRegion::SplitRight,
                _ => DiffTextRegion::SplitLeft,
            },
        };
        let start_region = region;
        let end_region = region;

        let end_text = self.diff_text_line_for_region(b, end_region);

        self.diff_text_selecting = false;
        self.diff_text_anchor = Some(DiffTextPos {
            visible_ix: a,
            region: start_region,
            offset: 0,
        });
        self.diff_text_head = Some(DiffTextPos {
            visible_ix: b,
            region: end_region,
            offset: end_text.len(),
        });

        // Double-click produces two click events; suppress both.
        self.diff_suppress_clicks_remaining = 2;
    }

    pub(in super::super) fn double_click_select_diff_text(
        &mut self,
        visible_ix: usize,
        region: DiffTextRegion,
        kind: DiffClickKind,
    ) {
        if self.is_file_preview_active() {
            let Some(count) = self.worktree_preview_line_count() else {
                return;
            };
            if count == 0 {
                return;
            }
            let visible_ix = visible_ix.min(count - 1);
            let end_text = self.diff_text_line_for_region(visible_ix, DiffTextRegion::Inline);
            self.diff_text_selecting = false;
            self.diff_text_anchor = Some(DiffTextPos {
                visible_ix,
                region: DiffTextRegion::Inline,
                offset: 0,
            });
            self.diff_text_head = Some(DiffTextPos {
                visible_ix,
                region: DiffTextRegion::Inline,
                offset: end_text.len(),
            });

            // Double-click produces two click events; suppress both.
            self.diff_suppress_clicks_remaining = 2;
            return;
        }

        let list_len = self.diff_visible_indices.len();
        if list_len == 0 {
            return;
        }
        let visible_ix = visible_ix.min(list_len - 1);

        // File-diff view doesn't have file/hunk header blocks; treat as row selection.
        if self.is_file_diff_view_active() {
            self.select_diff_text_rows_range(visible_ix, visible_ix, region);
            return;
        }

        let end = match self.diff_view {
            DiffViewMode::Inline => match kind {
                DiffClickKind::Line => visible_ix,
                DiffClickKind::HunkHeader => self
                    .diff_next_boundary_visible_ix(visible_ix, |src_ix| {
                        let line = &self.diff_cache[src_ix];
                        matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk)
                            || (matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                                && line.text.starts_with("diff --git "))
                    })
                    .unwrap_or(list_len - 1),
                DiffClickKind::FileHeader => self
                    .diff_next_boundary_visible_ix(visible_ix, |src_ix| {
                        let line = &self.diff_cache[src_ix];
                        matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                            && line.text.starts_with("diff --git ")
                    })
                    .unwrap_or(list_len - 1),
            },
            DiffViewMode::Split => match kind {
                DiffClickKind::Line => visible_ix,
                DiffClickKind::HunkHeader => self
                    .split_next_boundary_visible_ix(visible_ix, |row| {
                        matches!(
                            row,
                            PatchSplitRow::Raw {
                                click_kind: DiffClickKind::HunkHeader | DiffClickKind::FileHeader,
                                ..
                            }
                        )
                    })
                    .unwrap_or(list_len - 1),
                DiffClickKind::FileHeader => self
                    .split_next_boundary_visible_ix(visible_ix, |row| {
                        matches!(
                            row,
                            PatchSplitRow::Raw {
                                click_kind: DiffClickKind::FileHeader,
                                ..
                            }
                        )
                    })
                    .unwrap_or(list_len - 1),
            },
        };

        self.select_diff_text_rows_range(visible_ix, end, region);
    }

    fn split_next_boundary_visible_ix(
        &self,
        from_visible_ix: usize,
        is_boundary: impl Fn(&PatchSplitRow) -> bool,
    ) -> Option<usize> {
        let from_visible_ix =
            from_visible_ix.min(self.diff_visible_indices.len().saturating_sub(1));
        for visible_ix in (from_visible_ix + 1)..self.diff_visible_indices.len() {
            let row_ix = *self.diff_visible_indices.get(visible_ix)?;
            let row = self.diff_split_cache.get(row_ix)?;
            if is_boundary(row) {
                return Some(visible_ix.saturating_sub(1));
            }
        }
        None
    }

    fn diff_next_boundary_visible_ix(
        &self,
        from_visible_ix: usize,
        is_boundary: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        let from_visible_ix =
            from_visible_ix.min(self.diff_visible_indices.len().saturating_sub(1));
        for visible_ix in (from_visible_ix + 1)..self.diff_visible_indices.len() {
            let src_ix = *self.diff_visible_indices.get(visible_ix)?;
            if is_boundary(src_ix) {
                return Some(visible_ix.saturating_sub(1));
            }
        }
        None
    }

    pub(in super::super) fn history_visible_columns(&self) -> (bool, bool) {
        // Prefer keeping commit message visible. Hide SHA first, then date.
        let mut available = self.last_window_size.width;
        available -= px(280.0);
        available -= px(420.0);
        available -= px(64.0);
        if available <= px(0.0) {
            return (false, false);
        }

        let min_message = px(220.0);

        // Always show Branch + Graph; Message is flex.
        let fixed_base = self.history_col_branch + self.history_col_graph;

        // Show both by default.
        let mut show_date = true;
        let mut show_sha = true;
        let mut fixed = fixed_base + self.history_col_date + self.history_col_sha;

        if available - fixed < min_message {
            show_sha = false;
            fixed -= self.history_col_sha;
        }
        if available - fixed < min_message {
            show_date = false;
            show_sha = false;
        }

        (show_date, show_sha)
    }

    fn ensure_diff_hunk_picker_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Entity<zed::TextInput> {
        let theme = self.theme;
        let input = self.diff_hunk_picker_search_input.get_or_insert_with(|| {
            cx.new(|cx| {
                zed::TextInput::new(
                    zed::TextInputOptions {
                        placeholder: "Filter hunks".into(),
                        multiline: false,
                        read_only: false,
                        chromeless: false,
                        soft_wrap: false,
                    },
                    window,
                    cx,
                )
            })
        });
        input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text("", cx);
        });
        let focus_handle = input.read_with(cx, |input, _| input.focus_handle());
        window.focus(&focus_handle);
        input.clone()
    }
}

impl MainPaneView {
    pub(in super::super) fn ensure_file_diff_cache(&mut self) {
        struct Rebuild {
            repo_id: RepoId,
            diff_file_rev: u64,
            diff_target: Option<DiffTarget>,
            file_path: Option<std::path::PathBuf>,
            rows: Vec<FileDiffRow>,
            inline_rows: Vec<AnnotatedDiffLine>,
            inline_word_highlights: Vec<Option<Vec<Range<usize>>>>,
            split_word_highlights_old: Vec<Option<Vec<Range<usize>>>>,
            split_word_highlights_new: Vec<Option<Vec<Range<usize>>>>,
        }

        enum Action {
            Clear,
            Noop,
            Reset {
                repo_id: RepoId,
                diff_file_rev: u64,
                diff_target: Option<DiffTarget>,
            },
            Rebuild(Rebuild),
        }

        let action = (|| {
            let Some(repo) = self.active_repo() else {
                return Action::Clear;
            };

            if !Self::is_file_diff_target(repo.diff_target.as_ref()) {
                return Action::Clear;
            }

            if self.file_diff_cache_repo_id == Some(repo.id)
                && self.file_diff_cache_rev == repo.diff_file_rev
                && self.file_diff_cache_target.as_ref() == repo.diff_target.as_ref()
            {
                return Action::Noop;
            }

            let repo_id = repo.id;
            let diff_file_rev = repo.diff_file_rev;
            let diff_target = repo.diff_target.clone();

            let Loadable::Ready(file_opt) = &repo.diff_file else {
                return Action::Reset {
                    repo_id,
                    diff_file_rev,
                    diff_target,
                };
            };
            let Some(file) = file_opt.as_ref() else {
                return Action::Reset {
                    repo_id,
                    diff_file_rev,
                    diff_target,
                };
            };

            let old_text = file.old.as_deref().unwrap_or("");
            let new_text = file.new.as_deref().unwrap_or("");
            let rows = gitgpui_core::file_diff::side_by_side_rows(old_text, new_text);

            // Store the file path for syntax highlighting.
            let workdir = &repo.spec.workdir;
            let file_path = Some(if file.path.is_absolute() {
                file.path.clone()
            } else {
                workdir.join(&file.path)
            });

            // Precompute word highlights and inline rows.
            let mut split_word_highlights_old: Vec<Option<Vec<Range<usize>>>> =
                vec![None; rows.len()];
            let mut split_word_highlights_new: Vec<Option<Vec<Range<usize>>>> =
                vec![None; rows.len()];
            for (row_ix, row) in rows.iter().enumerate() {
                if matches!(row.kind, gitgpui_core::file_diff::FileDiffRowKind::Modify) {
                    let old = row.old.as_deref().unwrap_or("");
                    let new = row.new.as_deref().unwrap_or("");
                    let (old_ranges, new_ranges) = capped_word_diff_ranges(old, new);
                    if !old_ranges.is_empty() {
                        split_word_highlights_old[row_ix] = Some(old_ranges);
                    }
                    if !new_ranges.is_empty() {
                        split_word_highlights_new[row_ix] = Some(new_ranges);
                    }
                }
            }

            let mut inline_rows: Vec<AnnotatedDiffLine> = Vec::new();
            let mut inline_word_highlights: Vec<Option<Vec<Range<usize>>>> = Vec::new();
            for row in &rows {
                use gitgpui_core::file_diff::FileDiffRowKind as K;
                match row.kind {
                    K::Context => {
                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Context,
                            text: format!(" {}", row.old.as_deref().unwrap_or("")),
                            old_line: row.old_line,
                            new_line: row.new_line,
                        });
                        inline_word_highlights.push(None);
                    }
                    K::Add => {
                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Add,
                            text: format!("+{}", row.new.as_deref().unwrap_or("")),
                            old_line: None,
                            new_line: row.new_line,
                        });
                        inline_word_highlights.push(None);
                    }
                    K::Remove => {
                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Remove,
                            text: format!("-{}", row.old.as_deref().unwrap_or("")),
                            old_line: row.old_line,
                            new_line: None,
                        });
                        inline_word_highlights.push(None);
                    }
                    K::Modify => {
                        let old = row.old.as_deref().unwrap_or("");
                        let new = row.new.as_deref().unwrap_or("");
                        let (old_ranges, new_ranges) = capped_word_diff_ranges(old, new);

                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Remove,
                            text: format!("-{}", old),
                            old_line: row.old_line,
                            new_line: None,
                        });
                        inline_word_highlights.push((!old_ranges.is_empty()).then_some(old_ranges));

                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Add,
                            text: format!("+{}", new),
                            old_line: None,
                            new_line: row.new_line,
                        });
                        inline_word_highlights.push((!new_ranges.is_empty()).then_some(new_ranges));
                    }
                }
            }

            Action::Rebuild(Rebuild {
                repo_id,
                diff_file_rev,
                diff_target,
                file_path,
                rows,
                inline_rows,
                inline_word_highlights,
                split_word_highlights_old,
                split_word_highlights_new,
            })
        })();

        match action {
            Action::Noop => {}
            Action::Clear => {
                self.file_diff_cache_repo_id = None;
                self.file_diff_cache_target = None;
                self.file_diff_cache_rev = 0;
                self.file_diff_cache_path = None;
                self.file_diff_cache_rows.clear();
                self.file_diff_inline_cache.clear();
                self.file_diff_inline_word_highlights.clear();
                self.file_diff_split_word_highlights_old.clear();
                self.file_diff_split_word_highlights_new.clear();
            }
            Action::Reset {
                repo_id,
                diff_file_rev,
                diff_target,
            } => {
                self.file_diff_cache_repo_id = Some(repo_id);
                self.file_diff_cache_rev = diff_file_rev;
                self.file_diff_cache_target = diff_target;
                self.file_diff_cache_path = None;
                self.file_diff_cache_rows.clear();
                self.file_diff_inline_cache.clear();
                self.file_diff_inline_word_highlights.clear();
                self.file_diff_split_word_highlights_old.clear();
                self.file_diff_split_word_highlights_new.clear();
            }
            Action::Rebuild(rebuild) => {
                self.file_diff_cache_repo_id = Some(rebuild.repo_id);
                self.file_diff_cache_rev = rebuild.diff_file_rev;
                self.file_diff_cache_target = rebuild.diff_target;
                self.file_diff_cache_path = rebuild.file_path;
                self.file_diff_cache_rows = rebuild.rows;
                self.file_diff_inline_cache = rebuild.inline_rows;
                self.file_diff_inline_word_highlights = rebuild.inline_word_highlights;
                self.file_diff_split_word_highlights_old = rebuild.split_word_highlights_old;
                self.file_diff_split_word_highlights_new = rebuild.split_word_highlights_new;

                // Reset the segment cache to avoid mixing patch/file indices.
                self.diff_text_segments_cache.clear();
            }
        }
    }

    fn image_format_for_path(path: &std::path::Path) -> Option<gpui::ImageFormat> {
        let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
        match ext.as_str() {
            "png" => Some(gpui::ImageFormat::Png),
            "jpg" | "jpeg" => Some(gpui::ImageFormat::Jpeg),
            "gif" => Some(gpui::ImageFormat::Gif),
            "webp" => Some(gpui::ImageFormat::Webp),
            "bmp" => Some(gpui::ImageFormat::Bmp),
            "svg" => Some(gpui::ImageFormat::Svg),
            "tif" | "tiff" => Some(gpui::ImageFormat::Tiff),
            _ => None,
        }
    }

    pub(in super::super) fn ensure_file_image_diff_cache(&mut self) {
        struct Rebuild {
            repo_id: RepoId,
            diff_file_rev: u64,
            diff_target: Option<DiffTarget>,
            file_path: Option<std::path::PathBuf>,
            old: Option<Arc<gpui::Image>>,
            new: Option<Arc<gpui::Image>>,
        }

        enum Action {
            Clear,
            Noop,
            Reset {
                repo_id: RepoId,
                diff_file_rev: u64,
                diff_target: Option<DiffTarget>,
            },
            Rebuild(Rebuild),
        }

        let action = (|| {
            let Some(repo) = self.active_repo() else {
                return Action::Clear;
            };

            if !Self::is_file_diff_target(repo.diff_target.as_ref()) {
                return Action::Clear;
            }

            if self.file_image_diff_cache_repo_id == Some(repo.id)
                && self.file_image_diff_cache_rev == repo.diff_file_rev
                && self.file_image_diff_cache_target.as_ref() == repo.diff_target.as_ref()
            {
                return Action::Noop;
            }

            let repo_id = repo.id;
            let diff_file_rev = repo.diff_file_rev;
            let diff_target = repo.diff_target.clone();

            let Loadable::Ready(file_opt) = &repo.diff_file_image else {
                return Action::Reset {
                    repo_id,
                    diff_file_rev,
                    diff_target,
                };
            };
            let Some(file) = file_opt.as_ref() else {
                return Action::Reset {
                    repo_id,
                    diff_file_rev,
                    diff_target,
                };
            };

            let format = Self::image_format_for_path(&file.path);
            let old = file.old.as_ref().and_then(|bytes| {
                format.map(|format| Arc::new(gpui::Image::from_bytes(format, bytes.clone())))
            });
            let new = file.new.as_ref().and_then(|bytes| {
                format.map(|format| Arc::new(gpui::Image::from_bytes(format, bytes.clone())))
            });

            let workdir = &repo.spec.workdir;
            let file_path = Some(if file.path.is_absolute() {
                file.path.clone()
            } else {
                workdir.join(&file.path)
            });

            Action::Rebuild(Rebuild {
                repo_id,
                diff_file_rev,
                diff_target,
                file_path,
                old,
                new,
            })
        })();

        match action {
            Action::Noop => {}
            Action::Clear => {
                self.file_image_diff_cache_repo_id = None;
                self.file_image_diff_cache_target = None;
                self.file_image_diff_cache_rev = 0;
                self.file_image_diff_cache_path = None;
                self.file_image_diff_cache_old = None;
                self.file_image_diff_cache_new = None;
            }
            Action::Reset {
                repo_id,
                diff_file_rev,
                diff_target,
            } => {
                self.file_image_diff_cache_repo_id = Some(repo_id);
                self.file_image_diff_cache_rev = diff_file_rev;
                self.file_image_diff_cache_target = diff_target;
                self.file_image_diff_cache_path = None;
                self.file_image_diff_cache_old = None;
                self.file_image_diff_cache_new = None;
            }
            Action::Rebuild(rebuild) => {
                self.file_image_diff_cache_repo_id = Some(rebuild.repo_id);
                self.file_image_diff_cache_rev = rebuild.diff_file_rev;
                self.file_image_diff_cache_target = rebuild.diff_target;
                self.file_image_diff_cache_path = rebuild.file_path;
                self.file_image_diff_cache_old = rebuild.old;
                self.file_image_diff_cache_new = rebuild.new;
            }
        }
    }

    pub(in super::super) fn rebuild_diff_cache(&mut self) {
        self.diff_cache.clear();
        self.diff_cache_repo_id = None;
        self.diff_cache_rev = 0;
        self.diff_cache_target = None;
        self.diff_file_for_src_ix.clear();
        self.diff_split_cache.clear();
        self.diff_split_cache_len = 0;
        self.diff_visible_indices.clear();
        self.diff_visible_cache_len = 0;
        self.diff_visible_is_file_view = false;
        self.diff_scrollbar_markers_cache.clear();
        self.diff_word_highlights.clear();
        self.diff_file_stats.clear();
        self.diff_text_segments_cache.clear();
        self.diff_selection_anchor = None;
        self.diff_selection_range = None;
        self.diff_preview_is_new_file = false;
        self.diff_preview_new_file_lines = Arc::new(Vec::new());

        let (repo_id, diff_rev, diff_target, workdir, annotated) = {
            let Some(repo) = self.active_repo() else {
                return;
            };
            let workdir = repo.spec.workdir.clone();
            let annotated = match &repo.diff {
                Loadable::Ready(diff) => Some(annotate_unified(diff)),
                _ => None,
            };
            (
                repo.id,
                repo.diff_rev,
                repo.diff_target.clone(),
                workdir,
                annotated,
            )
        };

        self.diff_cache_repo_id = Some(repo_id);
        self.diff_cache_rev = diff_rev;
        self.diff_cache_target = diff_target;

        let Some(annotated) = annotated else {
            return;
        };

        self.diff_cache = annotated;
        self.diff_file_for_src_ix = compute_diff_file_for_src_ix(&self.diff_cache);
        self.diff_file_stats = compute_diff_file_stats(&self.diff_cache);
        self.rebuild_diff_word_highlights();

        if let Some((abs_path, lines)) = build_new_file_preview_from_diff(
            &self.diff_cache,
            &workdir,
            self.diff_cache_target.as_ref(),
        ) {
            self.diff_preview_is_new_file = true;
            self.diff_preview_new_file_lines = Arc::new(lines);
            self.worktree_preview_path = Some(abs_path);
            self.worktree_preview = Loadable::Ready(self.diff_preview_new_file_lines.clone());
            self.worktree_preview_segments_cache_path = None;
            self.worktree_preview_segments_cache.clear();
            self.worktree_preview_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
        }
    }

    fn rebuild_diff_word_highlights(&mut self) {
        self.diff_word_highlights.clear();
        self.diff_word_highlights
            .resize_with(self.diff_cache.len(), || None);

        let mut ix = 0usize;
        while ix < self.diff_cache.len() {
            let kind = self.diff_cache[ix].kind;
            if matches!(kind, gitgpui_core::domain::DiffLineKind::Hunk) {
                ix += 1;
                continue;
            }

            if !matches!(kind, gitgpui_core::domain::DiffLineKind::Remove) {
                ix += 1;
                continue;
            }

            let mut removed: Vec<(usize, &str)> = Vec::new();
            while ix < self.diff_cache.len()
                && matches!(
                    self.diff_cache[ix].kind,
                    gitgpui_core::domain::DiffLineKind::Remove
                )
            {
                let text = diff_content_text(&self.diff_cache[ix]);
                removed.push((ix, text));
                ix += 1;
            }

            let mut added: Vec<(usize, &str)> = Vec::new();
            while ix < self.diff_cache.len()
                && matches!(
                    self.diff_cache[ix].kind,
                    gitgpui_core::domain::DiffLineKind::Add
                )
            {
                let text = diff_content_text(&self.diff_cache[ix]);
                added.push((ix, text));
                ix += 1;
            }

            let pairs = removed.len().min(added.len());
            for i in 0..pairs {
                let (old_ix, old_text) = removed[i];
                let (new_ix, new_text) = added[i];
                let (old_ranges, new_ranges) = capped_word_diff_ranges(old_text, new_text);
                if !old_ranges.is_empty() {
                    self.diff_word_highlights[old_ix] = Some(old_ranges);
                }
                if !new_ranges.is_empty() {
                    self.diff_word_highlights[new_ix] = Some(new_ranges);
                }
            }

            for (old_ix, old_text) in removed.into_iter().skip(pairs) {
                if !old_text.is_empty() {
                    self.diff_word_highlights[old_ix] = Some(vec![0..old_text.len()]);
                }
            }
            for (new_ix, new_text) in added.into_iter().skip(pairs) {
                if !new_text.is_empty() {
                    self.diff_word_highlights[new_ix] = Some(vec![0..new_text.len()]);
                }
            }
        }
    }

    fn ensure_diff_split_cache(&mut self) {
        if self.diff_split_cache_len == self.diff_cache.len() && !self.diff_split_cache.is_empty() {
            return;
        }
        self.diff_split_cache_len = self.diff_cache.len();
        self.diff_split_cache = build_patch_split_rows(&self.diff_cache);
    }

    fn diff_scrollbar_markers_patch(&self) -> Vec<zed::ScrollbarMarker> {
        match self.diff_view {
            DiffViewMode::Inline => {
                scrollbar_markers_from_flags(self.diff_visible_indices.len(), |visible_ix| {
                    let Some(&src_ix) = self.diff_visible_indices.get(visible_ix) else {
                        return 0;
                    };
                    let Some(line) = self.diff_cache.get(src_ix) else {
                        return 0;
                    };
                    match line.kind {
                        gitgpui_core::domain::DiffLineKind::Add => 1,
                        gitgpui_core::domain::DiffLineKind::Remove => 2,
                        _ => 0,
                    }
                })
            }
            DiffViewMode::Split => {
                scrollbar_markers_from_flags(self.diff_visible_indices.len(), |visible_ix| {
                    let Some(&row_ix) = self.diff_visible_indices.get(visible_ix) else {
                        return 0;
                    };
                    let Some(row) = self.diff_split_cache.get(row_ix) else {
                        return 0;
                    };
                    match row {
                        PatchSplitRow::Aligned { row, .. } => match row.kind {
                            gitgpui_core::file_diff::FileDiffRowKind::Add => 1,
                            gitgpui_core::file_diff::FileDiffRowKind::Remove => 2,
                            gitgpui_core::file_diff::FileDiffRowKind::Modify => 3,
                            gitgpui_core::file_diff::FileDiffRowKind::Context => 0,
                        },
                        PatchSplitRow::Raw { .. } => 0,
                    }
                })
            }
        }
    }

    fn compute_diff_scrollbar_markers(&self) -> Vec<zed::ScrollbarMarker> {
        if !self.is_file_diff_view_active() {
            return self.diff_scrollbar_markers_patch();
        }

        match self.diff_view {
            DiffViewMode::Inline => {
                scrollbar_markers_from_flags(self.diff_visible_indices.len(), |visible_ix| {
                    let Some(&inline_ix) = self.diff_visible_indices.get(visible_ix) else {
                        return 0;
                    };
                    let Some(line) = self.file_diff_inline_cache.get(inline_ix) else {
                        return 0;
                    };
                    match line.kind {
                        gitgpui_core::domain::DiffLineKind::Add => 1,
                        gitgpui_core::domain::DiffLineKind::Remove => 2,
                        _ => 0,
                    }
                })
            }
            DiffViewMode::Split => {
                scrollbar_markers_from_flags(self.diff_visible_indices.len(), |visible_ix| {
                    let Some(&row_ix) = self.diff_visible_indices.get(visible_ix) else {
                        return 0;
                    };
                    let Some(row) = self.file_diff_cache_rows.get(row_ix) else {
                        return 0;
                    };
                    match row.kind {
                        gitgpui_core::file_diff::FileDiffRowKind::Add => 1,
                        gitgpui_core::file_diff::FileDiffRowKind::Remove => 2,
                        gitgpui_core::file_diff::FileDiffRowKind::Modify => 3,
                        gitgpui_core::file_diff::FileDiffRowKind::Context => 0,
                    }
                })
            }
        }
    }

    pub(in super::super) fn ensure_diff_visible_indices(&mut self) {
        let is_file_view = self.is_file_diff_view_active();
        let current_len = if is_file_view {
            match self.diff_view {
                DiffViewMode::Inline => self.file_diff_inline_cache.len(),
                DiffViewMode::Split => self.file_diff_cache_rows.len(),
            }
        } else {
            self.diff_cache.len()
        };

        if self.diff_visible_cache_len == current_len
            && self.diff_visible_view == self.diff_view
            && self.diff_visible_is_file_view == is_file_view
        {
            return;
        }

        self.diff_visible_cache_len = current_len;
        self.diff_visible_view = self.diff_view;
        self.diff_visible_is_file_view = is_file_view;

        if is_file_view {
            self.diff_visible_indices = (0..current_len).collect();
            self.diff_scrollbar_markers_cache = self.compute_diff_scrollbar_markers();
            if self.diff_search_active && !self.diff_search_query.as_ref().trim().is_empty() {
                self.diff_search_recompute_matches_for_current_view();
            }
            return;
        }

        match self.diff_view {
            DiffViewMode::Inline => {
                self.diff_visible_indices = self
                    .diff_cache
                    .iter()
                    .enumerate()
                    .filter_map(|(ix, line)| {
                        (!should_hide_unified_diff_header_line(line)).then_some(ix)
                    })
                    .collect();
            }
            DiffViewMode::Split => {
                self.ensure_diff_split_cache();

                self.diff_visible_indices = self
                    .diff_split_cache
                    .iter()
                    .enumerate()
                    .filter_map(|(ix, row)| match row {
                        PatchSplitRow::Raw { src_ix, .. } => self
                            .diff_cache
                            .get(*src_ix)
                            .is_some_and(|line| !should_hide_unified_diff_header_line(line))
                            .then_some(ix),
                        PatchSplitRow::Aligned { .. } => Some(ix),
                    })
                    .collect();
            }
        }

        self.diff_scrollbar_markers_cache = self.compute_diff_scrollbar_markers();

        if self.diff_search_active && !self.diff_search_query.as_ref().trim().is_empty() {
            self.diff_search_recompute_matches_for_current_view();
        }
    }

    pub(in super::super) fn handle_patch_row_click(
        &mut self,
        clicked_visible_ix: usize,
        kind: DiffClickKind,
        shift: bool,
    ) {
        if self.is_file_diff_view_active() {
            self.handle_file_diff_row_click(clicked_visible_ix, shift);
            return;
        }
        match self.diff_view {
            DiffViewMode::Inline => self.handle_diff_row_click(clicked_visible_ix, kind, shift),
            DiffViewMode::Split => self.handle_split_row_click(clicked_visible_ix, kind, shift),
        }
    }

    fn handle_split_row_click(
        &mut self,
        clicked_visible_ix: usize,
        kind: DiffClickKind,
        shift: bool,
    ) {
        let list_len = self.diff_visible_indices.len();
        if list_len == 0 {
            self.diff_selection_anchor = None;
            self.diff_selection_range = None;
            return;
        }

        let clicked_visible_ix = clicked_visible_ix.min(list_len - 1);

        if shift && let Some(anchor) = self.diff_selection_anchor {
            let a = anchor.min(clicked_visible_ix);
            let b = anchor.max(clicked_visible_ix);
            self.diff_selection_range = Some((a, b));
            return;
        }

        let end = match kind {
            DiffClickKind::Line => clicked_visible_ix,
            DiffClickKind::HunkHeader => self
                .split_next_boundary_visible_ix(clicked_visible_ix, |row| {
                    matches!(
                        row,
                        PatchSplitRow::Raw {
                            click_kind: DiffClickKind::HunkHeader | DiffClickKind::FileHeader,
                            ..
                        }
                    )
                })
                .unwrap_or(list_len - 1),
            DiffClickKind::FileHeader => self
                .split_next_boundary_visible_ix(clicked_visible_ix, |row| {
                    matches!(
                        row,
                        PatchSplitRow::Raw {
                            click_kind: DiffClickKind::FileHeader,
                            ..
                        }
                    )
                })
                .unwrap_or(list_len - 1),
        };

        self.diff_selection_anchor = Some(clicked_visible_ix);
        self.diff_selection_range = Some((clicked_visible_ix, end));
    }

    fn handle_diff_row_click(
        &mut self,
        clicked_visible_ix: usize,
        kind: DiffClickKind,
        shift: bool,
    ) {
        let list_len = self.diff_visible_indices.len();
        if list_len == 0 {
            self.diff_selection_anchor = None;
            self.diff_selection_range = None;
            return;
        }

        let clicked_visible_ix = clicked_visible_ix.min(list_len - 1);

        if shift && let Some(anchor) = self.diff_selection_anchor {
            let a = anchor.min(clicked_visible_ix);
            let b = anchor.max(clicked_visible_ix);
            self.diff_selection_range = Some((a, b));
            return;
        }

        let end = match kind {
            DiffClickKind::Line => clicked_visible_ix,
            DiffClickKind::HunkHeader => self
                .diff_next_boundary_visible_ix(clicked_visible_ix, |src_ix| {
                    let line = &self.diff_cache[src_ix];
                    matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk)
                        || (matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                            && line.text.starts_with("diff --git "))
                })
                .unwrap_or(list_len - 1),
            DiffClickKind::FileHeader => self
                .diff_next_boundary_visible_ix(clicked_visible_ix, |src_ix| {
                    let line = &self.diff_cache[src_ix];
                    matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                        && line.text.starts_with("diff --git ")
                })
                .unwrap_or(list_len - 1),
        };

        self.diff_selection_anchor = Some(clicked_visible_ix);
        self.diff_selection_range = Some((clicked_visible_ix, end));
    }

    fn handle_file_diff_row_click(&mut self, clicked_visible_ix: usize, shift: bool) {
        let list_len = self.diff_visible_indices.len();
        if list_len == 0 {
            self.diff_selection_anchor = None;
            self.diff_selection_range = None;
            return;
        }

        let clicked_visible_ix = clicked_visible_ix.min(list_len - 1);
        if shift && let Some(anchor) = self.diff_selection_anchor {
            let a = anchor.min(clicked_visible_ix);
            let b = anchor.max(clicked_visible_ix);
            self.diff_selection_range = Some((a, b));
            return;
        }

        self.diff_selection_anchor = Some(clicked_visible_ix);
        self.diff_selection_range = Some((clicked_visible_ix, clicked_visible_ix));
    }

    fn file_change_visible_indices(&self) -> Vec<usize> {
        if !self.is_file_diff_view_active() {
            return Vec::new();
        }
        let mut out: Vec<usize> = Vec::new();
        match self.diff_view {
            DiffViewMode::Inline => {
                let mut prev_changed = false;
                for visible_ix in 0..self.diff_visible_indices.len() {
                    let Some(&inline_ix) = self.diff_visible_indices.get(visible_ix) else {
                        continue;
                    };
                    let changed = self.file_diff_inline_cache.get(inline_ix).is_some_and(|l| {
                        matches!(
                            l.kind,
                            gitgpui_core::domain::DiffLineKind::Add
                                | gitgpui_core::domain::DiffLineKind::Remove
                        )
                    });
                    if changed && !prev_changed {
                        out.push(visible_ix);
                    }
                    prev_changed = changed;
                }
            }
            DiffViewMode::Split => {
                let mut prev_changed = false;
                for visible_ix in 0..self.diff_visible_indices.len() {
                    let Some(&row_ix) = self.diff_visible_indices.get(visible_ix) else {
                        continue;
                    };
                    let changed = self.file_diff_cache_rows.get(row_ix).is_some_and(|row| {
                        !matches!(row.kind, gitgpui_core::file_diff::FileDiffRowKind::Context)
                    });
                    if changed && !prev_changed {
                        out.push(visible_ix);
                    }
                    prev_changed = changed;
                }
            }
        }
        out
    }

    fn patch_hunk_entries(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (visible_ix, &ix) in self.diff_visible_indices.iter().enumerate() {
            match self.diff_view {
                DiffViewMode::Inline => {
                    let Some(line) = self.diff_cache.get(ix) else {
                        continue;
                    };
                    if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
                        out.push((visible_ix, ix));
                    }
                }
                DiffViewMode::Split => {
                    let Some(row) = self.diff_split_cache.get(ix) else {
                        continue;
                    };
                    if let PatchSplitRow::Raw {
                        src_ix,
                        click_kind: DiffClickKind::HunkHeader,
                    } = row
                    {
                        out.push((visible_ix, *src_ix));
                    }
                }
            }
        }
        out
    }

    pub(in super::super) fn diff_nav_entries(&self) -> Vec<usize> {
        if self.is_file_diff_view_active() {
            return self.file_change_visible_indices();
        }
        self.patch_hunk_entries()
            .into_iter()
            .map(|(visible_ix, _)| visible_ix)
            .collect()
    }

    fn conflict_nav_entries_for_split(rows: &[FileDiffRow]) -> Vec<usize> {
        let mut out = Vec::new();
        let mut in_block = false;
        for (ix, row) in rows.iter().enumerate() {
            let is_change = row.kind != gitgpui_core::file_diff::FileDiffRowKind::Context;
            if is_change && !in_block {
                out.push(ix);
                in_block = true;
            } else if !is_change {
                in_block = false;
            }
        }
        out
    }

    fn conflict_nav_entries_for_inline(rows: &[ConflictInlineRow]) -> Vec<usize> {
        let mut out = Vec::new();
        let mut in_block = false;
        for (ix, row) in rows.iter().enumerate() {
            let is_change = row.kind != gitgpui_core::domain::DiffLineKind::Context;
            if is_change && !in_block {
                out.push(ix);
                in_block = true;
            } else if !is_change {
                in_block = false;
            }
        }
        out
    }

    pub(in super::super) fn conflict_nav_entries(&self) -> Vec<usize> {
        match self.conflict_resolver.diff_mode {
            ConflictDiffMode::Split => {
                Self::conflict_nav_entries_for_split(&self.conflict_resolver.diff_rows)
            }
            ConflictDiffMode::Inline => {
                Self::conflict_nav_entries_for_inline(&self.conflict_resolver.inline_rows)
            }
        }
    }

    pub(in super::super) fn diff_nav_prev_target(
        entries: &[usize],
        current: usize,
    ) -> Option<usize> {
        entries.iter().rev().find(|&&ix| ix < current).copied()
    }

    pub(in super::super) fn diff_nav_next_target(
        entries: &[usize],
        current: usize,
    ) -> Option<usize> {
        entries.iter().find(|&&ix| ix > current).copied()
    }

    pub(in super::super) fn conflict_jump_prev(&mut self) {
        let entries = self.conflict_nav_entries();
        if entries.is_empty() {
            return;
        }

        let current = self.conflict_resolver.nav_anchor.unwrap_or(0);
        let Some(target) = Self::diff_nav_prev_target(&entries, current) else {
            return;
        };

        self.conflict_resolver_diff_scroll
            .scroll_to_item_strict(target, gpui::ScrollStrategy::Center);
        self.conflict_resolver.nav_anchor = Some(target);
    }

    pub(in super::super) fn conflict_jump_next(&mut self) {
        let entries = self.conflict_nav_entries();
        if entries.is_empty() {
            return;
        }

        let current = self.conflict_resolver.nav_anchor.unwrap_or(0);
        let Some(target) = Self::diff_nav_next_target(&entries, current) else {
            return;
        };

        self.conflict_resolver_diff_scroll
            .scroll_to_item_strict(target, gpui::ScrollStrategy::Center);
        self.conflict_resolver.nav_anchor = Some(target);
    }

    pub(in super::super) fn diff_jump_prev(&mut self) {
        let entries = self.diff_nav_entries();
        if entries.is_empty() {
            return;
        }

        let current = self.diff_selection_anchor.unwrap_or(0);
        let Some(target) = Self::diff_nav_prev_target(&entries, current) else {
            return;
        };

        self.diff_scroll
            .scroll_to_item_strict(target, gpui::ScrollStrategy::Center);
        self.diff_selection_anchor = Some(target);
        self.diff_selection_range = Some((target, target));
    }

    pub(in super::super) fn diff_jump_next(&mut self) {
        let entries = self.diff_nav_entries();
        if entries.is_empty() {
            return;
        }

        let current = self.diff_selection_anchor.unwrap_or(0);
        let Some(target) = Self::diff_nav_next_target(&entries, current) else {
            return;
        };

        self.diff_scroll
            .scroll_to_item_strict(target, gpui::ScrollStrategy::Center);
        self.diff_selection_anchor = Some(target);
        self.diff_selection_range = Some((target, target));
    }

    pub(in super::super) fn maybe_autoscroll_diff_to_first_change(&mut self) {
        if !self.diff_autoscroll_pending {
            return;
        }
        if self.diff_search_active && !self.diff_search_query.as_ref().trim().is_empty() {
            self.diff_autoscroll_pending = false;
            return;
        }
        if self.diff_visible_indices.is_empty() {
            return;
        }

        let entries = self.diff_nav_entries();
        let target = entries.first().copied().unwrap_or(0);

        self.diff_scroll
            .scroll_to_item(target, gpui::ScrollStrategy::Top);
        self.diff_selection_anchor = Some(target);
        self.diff_selection_range = Some((target, target));
        self.diff_autoscroll_pending = false;
    }

    fn active_conflict_target(
        &self,
    ) -> Option<(
        std::path::PathBuf,
        Option<gitgpui_core::domain::FileConflictKind>,
    )> {
        let repo = self.active_repo()?;
        let DiffTarget::WorkingTree { path, area } = repo.diff_target.as_ref()? else {
            return None;
        };
        if *area != DiffArea::Unstaged {
            return None;
        }
        let Loadable::Ready(status) = &repo.status else {
            return None;
        };
        let conflict = status
            .unstaged
            .iter()
            .find(|e| e.path == *path && e.kind == FileStatusKind::Conflicted)?;

        Some((path.clone(), conflict.conflict))
    }

    pub(in super::super) fn diff_search_recompute_matches(&mut self) {
        if !self.diff_search_active {
            self.diff_search_matches.clear();
            self.diff_search_match_ix = None;
            return;
        }

        if !self.is_file_preview_active() && self.active_conflict_target().is_none() {
            self.ensure_diff_visible_indices();
        }

        self.diff_search_recompute_matches_for_current_view();
    }

    fn diff_search_recompute_matches_for_current_view(&mut self) {
        self.diff_search_matches.clear();
        self.diff_search_match_ix = None;

        let query = self.diff_search_query.as_ref().trim();
        if query.is_empty() {
            return;
        }

        if self.is_file_preview_active() {
            let Loadable::Ready(lines) = &self.worktree_preview else {
                return;
            };
            for (ix, line) in lines.iter().enumerate() {
                if contains_ascii_case_insensitive(line, query) {
                    self.diff_search_matches.push(ix);
                }
            }
        } else if let Some((_path, conflict_kind)) = self.active_conflict_target() {
            let is_conflict_resolver = Self::conflict_requires_resolver(conflict_kind);

            match (is_conflict_resolver, self.diff_view) {
                (true, _) => match self.conflict_resolver.diff_mode {
                    ConflictDiffMode::Split => {
                        for (ix, row) in self.conflict_resolver.diff_rows.iter().enumerate() {
                            if row
                                .old
                                .as_deref()
                                .is_some_and(|s| contains_ascii_case_insensitive(s, query))
                                || row
                                    .new
                                    .as_deref()
                                    .is_some_and(|s| contains_ascii_case_insensitive(s, query))
                            {
                                self.diff_search_matches.push(ix);
                            }
                        }
                    }
                    ConflictDiffMode::Inline => {
                        for (ix, row) in self.conflict_resolver.inline_rows.iter().enumerate() {
                            if contains_ascii_case_insensitive(row.content.as_str(), query) {
                                self.diff_search_matches.push(ix);
                            }
                        }
                    }
                },
                (false, DiffViewMode::Split) => {
                    for (ix, row) in self.conflict_resolver.diff_rows.iter().enumerate() {
                        if row
                            .old
                            .as_deref()
                            .is_some_and(|s| contains_ascii_case_insensitive(s, query))
                            || row
                                .new
                                .as_deref()
                                .is_some_and(|s| contains_ascii_case_insensitive(s, query))
                        {
                            self.diff_search_matches.push(ix);
                        }
                    }
                }
                (false, DiffViewMode::Inline) => {
                    for (ix, row) in self.conflict_resolver.inline_rows.iter().enumerate() {
                        if contains_ascii_case_insensitive(row.content.as_str(), query) {
                            self.diff_search_matches.push(ix);
                        }
                    }
                }
            }
        } else {
            let total = self.diff_visible_indices.len();
            for visible_ix in 0..total {
                match self.diff_view {
                    DiffViewMode::Inline => {
                        let text =
                            self.diff_text_line_for_region(visible_ix, DiffTextRegion::Inline);
                        if contains_ascii_case_insensitive(text.as_ref(), query) {
                            self.diff_search_matches.push(visible_ix);
                        }
                    }
                    DiffViewMode::Split => {
                        let left =
                            self.diff_text_line_for_region(visible_ix, DiffTextRegion::SplitLeft);
                        let right =
                            self.diff_text_line_for_region(visible_ix, DiffTextRegion::SplitRight);
                        if contains_ascii_case_insensitive(left.as_ref(), query)
                            || contains_ascii_case_insensitive(right.as_ref(), query)
                        {
                            self.diff_search_matches.push(visible_ix);
                        }
                    }
                }
            }
        }

        if !self.diff_search_matches.is_empty() {
            self.diff_search_match_ix = Some(0);
            let first = self.diff_search_matches[0];
            self.diff_search_scroll_to_visible_ix(first);
        }
    }

    pub(in super::super) fn diff_search_prev_match(&mut self) {
        if !self.diff_search_active {
            return;
        }

        if self.diff_search_matches.is_empty() {
            self.diff_search_recompute_matches();
        }
        let len = self.diff_search_matches.len();
        if len == 0 {
            return;
        }

        let current = self
            .diff_search_match_ix
            .unwrap_or(0)
            .min(len.saturating_sub(1));
        let next_ix = if current == 0 { len - 1 } else { current - 1 };
        self.diff_search_match_ix = Some(next_ix);
        let target = self.diff_search_matches[next_ix];
        self.diff_search_scroll_to_visible_ix(target);
    }

    pub(in super::super) fn diff_search_next_match(&mut self) {
        if !self.diff_search_active {
            return;
        }

        if self.diff_search_matches.is_empty() {
            self.diff_search_recompute_matches();
        }
        let len = self.diff_search_matches.len();
        if len == 0 {
            return;
        }

        let current = self
            .diff_search_match_ix
            .unwrap_or(0)
            .min(len.saturating_sub(1));
        let next_ix = (current + 1) % len;
        self.diff_search_match_ix = Some(next_ix);
        let target = self.diff_search_matches[next_ix];
        self.diff_search_scroll_to_visible_ix(target);
    }

    fn diff_search_scroll_to_visible_ix(&mut self, visible_ix: usize) {
        if self.is_file_preview_active() {
            self.worktree_preview_scroll
                .scroll_to_item_strict(visible_ix, gpui::ScrollStrategy::Center);
            return;
        }

        if let Some((_path, conflict_kind)) = self.active_conflict_target() {
            if Self::conflict_requires_resolver(conflict_kind) {
                self.conflict_resolver_diff_scroll
                    .scroll_to_item_strict(visible_ix, gpui::ScrollStrategy::Center);
            } else {
                self.diff_scroll
                    .scroll_to_item_strict(visible_ix, gpui::ScrollStrategy::Center);
            }
            return;
        }

        self.diff_scroll
            .scroll_to_item_strict(visible_ix, gpui::ScrollStrategy::Center);
        self.diff_selection_anchor = Some(visible_ix);
        self.diff_selection_range = Some((visible_ix, visible_ix));
    }

    fn sync_conflict_resolver(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(repo_id) = self.active_repo_id() else {
            self.conflict_resolver = ConflictResolverUiState::default();
            return;
        };

        let Some(repo) = self.state.repos.iter().find(|r| r.id == repo_id) else {
            self.conflict_resolver = ConflictResolverUiState::default();
            return;
        };

        let Some(DiffTarget::WorkingTree { path, area }) = repo.diff_target.as_ref() else {
            self.conflict_resolver = ConflictResolverUiState::default();
            return;
        };
        if *area != DiffArea::Unstaged {
            self.conflict_resolver = ConflictResolverUiState::default();
            return;
        }

        let is_conflicted = match &repo.status {
            Loadable::Ready(status) => status.unstaged.iter().any(|e| {
                e.path == *path && e.kind == gitgpui_core::domain::FileStatusKind::Conflicted
            }),
            _ => false,
        };
        if !is_conflicted {
            self.conflict_resolver = ConflictResolverUiState::default();
            return;
        }

        let path = path.clone();

        let should_load = repo.conflict_file_path.as_ref() != Some(&path)
            && !matches!(repo.conflict_file, Loadable::Loading);
        if should_load {
            self.conflict_resolver = ConflictResolverUiState::default();
            let theme = self.theme;
            self.conflict_resolver_input.update(cx, |input, cx| {
                input.set_theme(theme, cx);
                input.set_text("", cx);
            });
            self.store.dispatch(Msg::LoadConflictFile { repo_id, path });
            return;
        }

        let Loadable::Ready(Some(file)) = &repo.conflict_file else {
            return;
        };
        if file.path != path {
            return;
        }

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        file.ours.hash(&mut hasher);
        file.theirs.hash(&mut hasher);
        file.current.hash(&mut hasher);
        let source_hash = hasher.finish();

        let needs_rebuild = self.conflict_resolver.repo_id != Some(repo_id)
            || self.conflict_resolver.path.as_ref() != Some(&path)
            || self.conflict_resolver.source_hash != Some(source_hash);

        if !needs_rebuild {
            return;
        }

        self.conflict_diff_segments_cache_split.clear();
        self.conflict_diff_segments_cache_inline.clear();

        let resolved = if let Some(cur) = file.current.as_deref() {
            let segments = conflict_resolver::parse_conflict_markers(cur);
            if conflict_resolver::conflict_count(&segments) > 0 {
                conflict_resolver::generate_resolved_text(&segments)
            } else {
                cur.to_string()
            }
        } else if let Some(ours) = file.ours.as_deref() {
            ours.to_string()
        } else if let Some(theirs) = file.theirs.as_deref() {
            theirs.to_string()
        } else {
            String::new()
        };
        let ours_text = file.ours.as_deref().unwrap_or("");
        let theirs_text = file.theirs.as_deref().unwrap_or("");
        let diff_rows = gitgpui_core::file_diff::side_by_side_rows(ours_text, theirs_text);
        let inline_rows = conflict_resolver::build_inline_rows(&diff_rows);

        let diff_mode = if self.conflict_resolver.repo_id == Some(repo_id)
            && self.conflict_resolver.path.as_ref() == Some(&path)
        {
            self.conflict_resolver.diff_mode
        } else {
            ConflictDiffMode::Split
        };
        let nav_anchor = if self.conflict_resolver.repo_id == Some(repo_id)
            && self.conflict_resolver.path.as_ref() == Some(&path)
        {
            self.conflict_resolver.nav_anchor
        } else {
            None
        };

        let theme = self.theme;
        self.conflict_resolver_input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text(resolved, cx);
        });

        self.conflict_resolver = ConflictResolverUiState {
            repo_id: Some(repo_id),
            path: Some(path),
            source_hash: Some(source_hash),
            current: file.current.clone(),
            diff_rows,
            inline_rows,
            diff_mode,
            nav_anchor,
            split_selected: std::collections::BTreeSet::new(),
            inline_selected: std::collections::BTreeSet::new(),
        };

        if self.diff_search_active && !self.diff_search_query.as_ref().trim().is_empty() {
            self.diff_search_recompute_matches();
        }
    }

    pub(in super::super) fn conflict_resolver_set_mode(
        &mut self,
        mode: ConflictDiffMode,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.conflict_resolver.diff_mode == mode {
            return;
        }
        self.conflict_resolver.diff_mode = mode;
        self.conflict_resolver.nav_anchor = None;
        self.conflict_resolver.split_selected.clear();
        self.conflict_resolver.inline_selected.clear();
        if self.diff_search_active && !self.diff_search_query.as_ref().trim().is_empty() {
            self.diff_search_recompute_matches();
        }
        cx.notify();
    }

    pub(in super::super) fn conflict_resolver_selection_is_empty(&self) -> bool {
        match self.conflict_resolver.diff_mode {
            ConflictDiffMode::Split => self.conflict_resolver.split_selected.is_empty(),
            ConflictDiffMode::Inline => self.conflict_resolver.inline_selected.is_empty(),
        }
    }

    pub(in super::super) fn conflict_resolver_clear_selection(
        &mut self,
        cx: &mut gpui::Context<Self>,
    ) {
        self.conflict_resolver.split_selected.clear();
        self.conflict_resolver.inline_selected.clear();
        cx.notify();
    }

    pub(in super::super) fn conflict_resolver_toggle_split_selected(
        &mut self,
        row_ix: usize,
        side: ConflictPickSide,
        cx: &mut gpui::Context<Self>,
    ) {
        self.conflict_resolver.nav_anchor = Some(row_ix);
        let key = (row_ix, side);
        if self.conflict_resolver.split_selected.contains(&key) {
            self.conflict_resolver.split_selected.remove(&key);
        } else {
            self.conflict_resolver.split_selected.insert(key);
        }
        cx.notify();
    }

    pub(in super::super) fn conflict_resolver_toggle_inline_selected(
        &mut self,
        ix: usize,
        cx: &mut gpui::Context<Self>,
    ) {
        self.conflict_resolver.nav_anchor = Some(ix);
        if self.conflict_resolver.inline_selected.contains(&ix) {
            self.conflict_resolver.inline_selected.remove(&ix);
        } else {
            self.conflict_resolver.inline_selected.insert(ix);
        }
        cx.notify();
    }

    pub(in super::super) fn conflict_resolver_append_selection_to_output(
        &mut self,
        cx: &mut gpui::Context<Self>,
    ) {
        let lines = match self.conflict_resolver.diff_mode {
            ConflictDiffMode::Split => conflict_resolver::collect_split_selection(
                &self.conflict_resolver.diff_rows,
                &self.conflict_resolver.split_selected,
            ),
            ConflictDiffMode::Inline => conflict_resolver::collect_inline_selection(
                &self.conflict_resolver.inline_rows,
                &self.conflict_resolver.inline_selected,
            ),
        };
        if lines.is_empty() {
            return;
        }

        let current = self
            .conflict_resolver_input
            .read_with(cx, |i, _| i.text().to_string());
        let next = conflict_resolver::append_lines_to_output(&current, &lines);
        let theme = self.theme;
        self.conflict_resolver_input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text(next, cx);
        });
    }

    pub(in super::super) fn conflict_resolver_set_output(
        &mut self,
        text: String,
        cx: &mut gpui::Context<Self>,
    ) {
        let theme = self.theme;
        self.conflict_resolver_input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text(text, cx);
        });
    }

    pub(in super::super) fn conflict_resolver_reset_output_from_markers(
        &mut self,
        cx: &mut gpui::Context<Self>,
    ) {
        let Some(current) = self.conflict_resolver.current.as_deref() else {
            return;
        };
        let segments = conflict_resolver::parse_conflict_markers(current);
        if conflict_resolver::conflict_count(&segments) == 0 {
            return;
        }
        let resolved = conflict_resolver::generate_resolved_text(&segments);
        self.conflict_resolver_set_output(resolved, cx);
    }
}

impl Render for MainPaneView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        self.last_window_size = window.window_bounds().get_bounds().size;

        let show_diff = self
            .active_repo()
            .and_then(|r| r.diff_target.as_ref())
            .is_some();
        if show_diff {
            self.diff_view(cx)
        } else {
            self.history_view(cx)
        }
    }
}

fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }

    let haystack_bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    if needle_bytes.len() > haystack_bytes.len() {
        return false;
    }

    'outer: for start in 0..=(haystack_bytes.len() - needle_bytes.len()) {
        for (offset, needle_byte) in needle_bytes.iter().copied().enumerate() {
            let haystack_byte = haystack_bytes[start + offset];
            if !haystack_byte.eq_ignore_ascii_case(&needle_byte) {
                continue 'outer;
            }
        }
        return true;
    }

    false
}
