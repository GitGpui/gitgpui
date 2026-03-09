use super::helpers::*;
use super::*;

impl MainPaneView {
    pub(super) fn notify_fingerprint_for(state: &AppState) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        state.active_repo.hash(&mut hasher);

        if let Some(repo_id) = state.active_repo
            && let Some(repo) = state.repos.iter().find(|r| r.id == repo_id)
        {
            repo.diff_state.diff_state_rev.hash(&mut hasher);
            repo.conflict_state.conflict_rev.hash(&mut hasher);

            // Only include status changes when viewing a working tree diff.
            let status_rev = if matches!(
                repo.diff_state.diff_target,
                Some(DiffTarget::WorkingTree { .. })
            ) {
                repo.status_rev
            } else {
                0
            };
            status_rev.hash(&mut hasher);
        }

        hasher.finish()
    }

    pub(in crate::view) fn clear_diff_selection_or_exit(
        &mut self,
        repo_id: RepoId,
        cx: &mut gpui::Context<Self>,
    ) {
        match clear_diff_selection_action(self.view_mode) {
            ClearDiffSelectionAction::ClearSelection => {
                self.store.dispatch(Msg::ClearDiffSelection { repo_id });
            }
            ClearDiffSelectionAction::ExitFocusedMergetool => {
                self.set_focused_mergetool_exit_code(FOCUSED_MERGETOOL_EXIT_CANCELED);
                cx.quit();
            }
        }
    }

    pub(super) fn set_focused_mergetool_exit_code(&self, code: i32) {
        if let Some(exit_code) = &self.focused_mergetool_exit_code {
            exit_code.store(code, Ordering::SeqCst);
        }
    }

    pub(super) fn focused_mergetool_labels_or_default(&self) -> FocusedMergetoolLabels {
        self.focused_mergetool_labels
            .clone()
            .unwrap_or(FocusedMergetoolLabels {
                local: "LOCAL".to_string(),
                remote: "REMOTE".to_string(),
                base: "BASE".to_string(),
            })
    }

    pub(in crate::view) fn focused_mergetool_save_and_exit(
        &mut self,
        repo_id: RepoId,
        path: std::path::PathBuf,
        cx: &mut gpui::Context<Self>,
    ) {
        use gitcomet_core::conflict_output::{
            ConflictMarkerLabels, GenerateResolvedTextOptions, UnresolvedConflictMode,
        };

        let Some(repo) = self.state.repos.iter().find(|repo| repo.id == repo_id) else {
            self.set_focused_mergetool_exit_code(FOCUSED_MERGETOOL_EXIT_ERROR);
            cx.quit();
            return;
        };

        let labels = self.focused_mergetool_labels_or_default();
        let output = conflict_resolver::generate_resolved_text_with_options(
            &self.conflict_resolver.marker_segments,
            GenerateResolvedTextOptions {
                unresolved_mode: UnresolvedConflictMode::PreserveMarkers,
                labels: Some(ConflictMarkerLabels {
                    local: labels.local.as_str(),
                    remote: labels.remote.as_str(),
                    base: labels.base.as_str(),
                }),
            },
        );

        let full_path = repo.spec.workdir.join(&path);
        if let Some(parent) = full_path.parent().filter(|p| !p.as_os_str().is_empty())
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            eprintln!(
                "Failed to create parent directory for {}: {err}",
                full_path.display()
            );
            self.set_focused_mergetool_exit_code(FOCUSED_MERGETOOL_EXIT_ERROR);
            cx.quit();
            return;
        }

        if let Err(err) = std::fs::write(&full_path, output.as_bytes()) {
            eprintln!(
                "Failed to write merged output to {}: {err}",
                full_path.display()
            );
            self.set_focused_mergetool_exit_code(FOCUSED_MERGETOOL_EXIT_ERROR);
            cx.quit();
            return;
        }

        let total = conflict_resolver::conflict_count(&self.conflict_resolver.marker_segments);
        let resolved =
            conflict_resolver::resolved_conflict_count(&self.conflict_resolver.marker_segments);
        let exit_code = focused_mergetool_save_exit_code(total, resolved);
        self.set_focused_mergetool_exit_code(exit_code);
        cx.quit();
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::view) fn new(
        store: Arc<AppStore>,
        ui_model: Entity<AppUiModel>,
        theme: AppTheme,
        date_time_format: DateTimeFormat,
        timezone: Timezone,
        show_timezone: bool,
        history_show_author: bool,
        history_show_date: bool,
        history_show_sha: bool,
        conflict_enable_whitespace_autosolve: bool,
        conflict_enable_regex_autosolve: bool,
        conflict_enable_history_autosolve: bool,
        view_mode: GitCometViewMode,
        focused_mergetool_labels: Option<FocusedMergetoolLabels>,
        focused_mergetool_exit_code: Option<Arc<AtomicI32>>,
        root_view: WeakEntity<GitCometView>,
        tooltip_host: WeakEntity<TooltipHost>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let state = Arc::clone(&ui_model.read(cx).state);
        let initial_fingerprint = Self::notify_fingerprint_for(&state);
        let subscription = cx.observe(&ui_model, |this, model, cx| {
            let next = Arc::clone(&model.read(cx).state);
            let next_fingerprint = Self::notify_fingerprint_for(&next);
            if next_fingerprint == this.notify_fingerprint {
                this.state = next;
                return;
            }

            this.notify_fingerprint = next_fingerprint;
            this.apply_state_snapshot(next, cx);
            cx.notify();
        });

        let diff_raw_input = cx.new(|cx| {
            components::TextInput::new(
                components::TextInputOptions {
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
            let mut input = components::TextInput::new(
                components::TextInputOptions {
                    placeholder: "Resolve file contents…".into(),
                    multiline: true,
                    read_only: false,
                    chromeless: true,
                    soft_wrap: false,
                },
                window,
                cx,
            );
            input.set_suppress_right_click(true);
            input.set_line_height(Some(px(20.0)), cx);
            input
        });

        let conflict_resolver_subscription =
            cx.observe(&conflict_resolver_input, |this, input, cx| {
                let output_text = input.read(cx).text().to_string();
                let mut output_hasher = std::collections::hash_map::DefaultHasher::new();
                output_text.hash(&mut output_hasher);
                let output_hash = output_hasher.finish();

                let path = this.conflict_resolver.path.clone();
                let needs_update = this.conflict_resolved_preview_path.as_ref() != path.as_ref()
                    || this.conflict_resolved_preview_source_hash != Some(output_hash);
                if !needs_update {
                    return;
                }

                this.conflict_resolved_preview_path = path.clone();
                this.conflict_resolved_preview_source_hash = Some(output_hash);
                this.schedule_conflict_resolved_outline_recompute(path, output_hash, cx);
            });

        let diff_search_input = cx.new(|cx| {
            components::TextInput::new(
                components::TextInputOptions {
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
                this.clear_conflict_diff_query_overlay_caches();
                this.diff_search_recompute_matches();
                cx.notify();
            }
        });

        let diff_panel_focus_handle = cx.focus_handle().tab_index(0).tab_stop(false);

        let last_window_size = window.viewport_size();
        let history_view = cx.new(|cx| {
            super::HistoryView::new(
                Arc::clone(&store),
                ui_model.clone(),
                theme,
                date_time_format,
                timezone,
                show_timezone,
                history_show_author,
                history_show_date,
                history_show_sha,
                root_view.clone(),
                tooltip_host.clone(),
                last_window_size,
                window,
                cx,
            )
        });

        let mut pane = Self {
            store,
            state,
            view_mode,
            focused_mergetool_labels,
            focused_mergetool_exit_code,
            theme,
            date_time_format,
            _ui_model_subscription: subscription,
            root_view,
            tooltip_host,
            notify_fingerprint: initial_fingerprint,
            active_context_menu_invoker: None,
            last_window_size: size(px(0.0), px(0.0)),
            show_whitespace: false,
            conflict_enable_whitespace_autosolve,
            conflict_enable_regex_autosolve,
            conflict_enable_history_autosolve,
            diff_view: DiffViewMode::Split,
            svg_diff_view_mode: SvgDiffViewMode::Image,
            diff_word_wrap: false,
            diff_split_ratio: 0.5,
            diff_split_resize: None,
            diff_split_last_synced_y: px(0.0),
            diff_horizontal_min_width: px(0.0),
            diff_cache_repo_id: None,
            diff_cache_rev: 0,
            diff_cache_target: None,
            diff_cache: Vec::new(),
            diff_file_for_src_ix: Vec::new(),
            diff_language_for_src_ix: Vec::new(),
            diff_click_kinds: Vec::new(),
            diff_header_display_cache: HashMap::default(),
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
            diff_word_highlights_seq: 0,
            diff_word_highlights_inflight: None,
            diff_file_stats: Vec::new(),
            diff_text_segments_cache: Vec::new(),
            diff_selection_anchor: None,
            diff_selection_range: None,
            diff_text_selecting: false,
            diff_text_anchor: None,
            diff_text_head: None,
            diff_text_autoscroll_seq: 0,
            diff_text_autoscroll_target: None,
            diff_text_last_mouse_pos: point(px(0.0), px(0.0)),
            diff_suppress_clicks_remaining: 0,
            diff_text_hitboxes: HashMap::default(),
            diff_text_layout_cache_epoch: 0,
            diff_text_layout_cache: HashMap::default(),
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
            file_diff_cache_language: None,
            file_diff_cache_rows: Vec::new(),
            file_diff_inline_cache: Vec::new(),
            file_diff_inline_word_highlights: Vec::new(),
            file_diff_split_word_highlights_old: Vec::new(),
            file_diff_split_word_highlights_new: Vec::new(),
            file_diff_cache_seq: 0,
            file_diff_cache_inflight: None,
            file_image_diff_cache_repo_id: None,
            file_image_diff_cache_rev: 0,
            file_image_diff_cache_target: None,
            file_image_diff_cache_path: None,
            file_image_diff_cache_old: None,
            file_image_diff_cache_new: None,
            file_image_diff_cache_old_svg_path: None,
            file_image_diff_cache_new_svg_path: None,
            worktree_preview_path: None,
            worktree_preview: Loadable::NotLoaded,
            worktree_preview_segments_cache_path: None,
            worktree_preview_syntax_language: None,
            worktree_preview_segments_cache: HashMap::default(),
            diff_preview_is_new_file: false,
            diff_preview_new_file_lines: Arc::new(Vec::new()),
            conflict_resolver_input,
            _conflict_resolver_input_subscription: conflict_resolver_subscription,
            conflict_resolver: ConflictResolverUiState::default(),
            conflict_resolver_vsplit_ratio: 0.5,
            conflict_resolver_vsplit_resize: None,
            conflict_three_way_col_ratios: [1.0 / 3.0, 2.0 / 3.0],
            conflict_three_way_col_widths: [px(0.0); 3],
            conflict_hsplit_resize: None,
            conflict_diff_split_ratio: 0.5,
            conflict_diff_split_resize: None,
            conflict_diff_split_col_widths: [px(0.0); 2],
            conflict_canvas_rows_enabled: conflict_canvas_rows_enabled_from_env(),
            conflict_diff_segments_cache_split: HashMap::default(),
            conflict_diff_segments_cache_inline: HashMap::default(),
            conflict_diff_query_segments_cache_split: HashMap::default(),
            conflict_diff_query_segments_cache_inline: HashMap::default(),
            conflict_diff_query_cache_query: SharedString::default(),
            conflict_three_way_segments_cache: HashMap::default(),
            conflict_resolved_preview_path: None,
            conflict_resolved_preview_source_hash: None,
            conflict_resolved_preview_syntax_language: None,
            conflict_resolved_preview_lines: Vec::new(),
            conflict_resolved_preview_segments_cache: HashMap::default(),
            history_view,
            diff_scroll: UniformListScrollHandle::default(),
            diff_split_right_scroll: UniformListScrollHandle::default(),
            conflict_resolver_diff_scroll: UniformListScrollHandle::default(),
            conflict_resolved_preview_scroll: UniformListScrollHandle::default(),
            worktree_preview_scroll: UniformListScrollHandle::default(),
            path_display_cache: std::cell::RefCell::new(HashMap::default()),
        };

        pane.set_theme(theme, cx);
        pane.rebuild_diff_cache(cx);
        pane
    }

    pub(in crate::view) fn set_theme(&mut self, theme: AppTheme, cx: &mut gpui::Context<Self>) {
        self.theme = theme;
        self.diff_text_segments_cache.clear();
        self.worktree_preview_segments_cache_path = None;
        self.worktree_preview_segments_cache.clear();
        self.conflict_diff_segments_cache_split.clear();
        self.conflict_diff_segments_cache_inline.clear();
        self.conflict_diff_query_segments_cache_split.clear();
        self.conflict_diff_query_segments_cache_inline.clear();
        self.conflict_diff_query_cache_query = SharedString::default();
        self.conflict_resolved_preview_segments_cache.clear();
        self.diff_raw_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.diff_search_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.conflict_resolver_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        let output_syntax_highlights = build_resolved_output_syntax_highlights(
            theme,
            &self.conflict_resolved_preview_lines,
            self.conflict_resolved_preview_syntax_language,
        );
        self.conflict_resolver_input.update(cx, |input, cx| {
            input.set_highlights(output_syntax_highlights, cx);
        });
        if let Some(input) = &self.diff_hunk_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        self.history_view
            .update(cx, |view, cx| view.set_theme(theme, cx));
        cx.notify();
    }

    pub(in crate::view) fn clear_conflict_diff_query_overlay_caches(&mut self) {
        self.conflict_diff_query_segments_cache_split.clear();
        self.conflict_diff_query_segments_cache_inline.clear();
        self.conflict_diff_query_cache_query = SharedString::default();
    }

    pub(in crate::view) fn sync_conflict_diff_query_overlay_caches(&mut self, query: &str) {
        if self.conflict_diff_query_cache_query.as_ref() != query {
            self.conflict_diff_query_cache_query = query.to_string().into();
            self.conflict_diff_query_segments_cache_split.clear();
            self.conflict_diff_query_segments_cache_inline.clear();
        }
    }

    pub(in crate::view) fn clear_conflict_diff_style_caches(&mut self) {
        self.conflict_diff_segments_cache_split.clear();
        self.conflict_diff_segments_cache_inline.clear();
        self.clear_conflict_diff_query_overlay_caches();
    }

    pub(super) fn conflict_resolver_invalidate_resolved_outline(&mut self) {
        self.conflict_resolver.resolver_pending_recompute_seq = self
            .conflict_resolver
            .resolver_pending_recompute_seq
            .wrapping_add(1);
        self.conflict_resolved_preview_path = None;
        self.conflict_resolved_preview_source_hash = None;
        self.conflict_resolved_preview_syntax_language = None;
        self.conflict_resolved_preview_lines.clear();
        self.conflict_resolved_preview_segments_cache.clear();
        self.conflict_resolver.resolved_line_meta.clear();
        self.conflict_resolver
            .resolved_output_conflict_markers
            .clear();
        self.conflict_resolver
            .resolved_output_line_sources_index
            .clear();
    }

    pub(super) fn recompute_conflict_resolved_outline_and_provenance(
        &mut self,
        path: Option<&std::path::PathBuf>,
        cx: &mut gpui::Context<Self>,
    ) {
        let _perf_scope = perf::span(ConflictPerfSpan::RecomputeResolvedOutline);
        self.conflict_resolved_preview_syntax_language =
            path.and_then(|p| rows::diff_syntax_language_for_path(p.to_string_lossy().as_ref()));
        let output_text = self
            .conflict_resolver_input
            .read_with(cx, |input, _| input.text().to_string());
        self.conflict_resolved_preview_lines =
            conflict_resolver::split_output_lines_for_outline(&output_text);
        self.conflict_resolved_preview_segments_cache.clear();
        let output_syntax_highlights = build_resolved_output_syntax_highlights(
            self.theme,
            &self.conflict_resolved_preview_lines,
            self.conflict_resolved_preview_syntax_language,
        );
        self.conflict_resolver_input.update(cx, |input, cx| {
            input.set_highlights(output_syntax_highlights, cx);
        });

        // Provenance: classify each output line as A/B/C/Manual.
        let view_mode = self.conflict_resolver.view_mode;
        let (two_way_old, two_way_new) = if view_mode == ConflictResolverViewMode::TwoWayDiff {
            collect_two_way_source_lines(&self.conflict_resolver.diff_rows)
        } else {
            (Vec::new(), Vec::new())
        };
        let sources = match view_mode {
            ConflictResolverViewMode::ThreeWay => conflict_resolver::SourceLines {
                a: &self.conflict_resolver.three_way_lines.base,
                b: &self.conflict_resolver.three_way_lines.ours,
                c: &self.conflict_resolver.three_way_lines.theirs,
            },
            ConflictResolverViewMode::TwoWayDiff => conflict_resolver::SourceLines {
                a: &two_way_old,
                b: &two_way_new,
                c: &[],
            },
        };
        let mut meta = conflict_resolver::compute_resolved_line_provenance(
            &self.conflict_resolved_preview_lines,
            &sources,
        );
        apply_conflict_choice_provenance_hints(
            &mut meta,
            &self.conflict_resolver.marker_segments,
            &output_text,
            view_mode,
        );
        self.conflict_resolver.resolved_output_line_sources_index =
            conflict_resolver::build_resolved_output_line_sources_index(
                &meta,
                &self.conflict_resolved_preview_lines,
                view_mode,
            );
        self.conflict_resolver.resolved_output_conflict_markers =
            build_resolved_output_conflict_markers(
                &self.conflict_resolver.marker_segments,
                &output_text,
                self.conflict_resolved_preview_lines.len(),
            );
        self.conflict_resolver.resolved_line_meta = meta;
    }

    pub(super) fn conflict_resolver_scroll_resolved_output_to_line(
        &self,
        target_line_ix: usize,
        line_count: usize,
    ) {
        scroll_conflict_resolved_output_to_line(
            &self.conflict_resolved_preview_scroll,
            target_line_ix,
            line_count,
        );
    }

    pub(super) fn conflict_resolver_scroll_resolved_output_to_line_in_text(
        &self,
        target_line_ix: usize,
        output_text: &str,
    ) {
        let line_count = output_text.split('\n').count().max(1);
        self.conflict_resolver_scroll_resolved_output_to_line(target_line_ix, line_count);
    }

    pub(super) fn schedule_conflict_resolved_outline_recompute(
        &mut self,
        path: Option<std::path::PathBuf>,
        output_hash: u64,
        cx: &mut gpui::Context<Self>,
    ) {
        self.conflict_resolver.resolver_pending_recompute_seq = self
            .conflict_resolver
            .resolver_pending_recompute_seq
            .wrapping_add(1);
        let seq = self.conflict_resolver.resolver_pending_recompute_seq;

        cx.spawn(
            async move |view: WeakEntity<MainPaneView>, cx: &mut gpui::AsyncApp| {
                Timer::after(Duration::from_millis(CONFLICT_RESOLVED_OUTLINE_DEBOUNCE_MS)).await;
                let _ = view.update(cx, |this, cx| {
                    if this.conflict_resolver.resolver_pending_recompute_seq != seq {
                        return;
                    }
                    if this.conflict_resolved_preview_source_hash != Some(output_hash)
                        || this.conflict_resolved_preview_path.as_ref() != path.as_ref()
                    {
                        return;
                    }
                    this.recompute_conflict_resolved_outline_and_provenance(path.as_ref(), cx);

                    cx.notify();
                });
            },
        )
        .detach();
    }

    pub(in crate::view) fn set_active_context_menu_invoker(
        &mut self,
        next: Option<SharedString>,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.active_context_menu_invoker == next {
            return;
        }
        self.active_context_menu_invoker = next.clone();
        self.history_view.update(cx, |view, cx| {
            view.set_active_context_menu_invoker(next, cx)
        });
        cx.notify();
    }

    pub(in crate::view) fn set_date_time_format(
        &mut self,
        next: DateTimeFormat,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.date_time_format == next {
            return;
        }
        self.date_time_format = next;
        self.history_view
            .update(cx, |view, cx| view.set_date_time_format(next, cx));
        cx.notify();
    }

    pub(in crate::view) fn set_timezone(&mut self, next: Timezone, cx: &mut gpui::Context<Self>) {
        self.history_view
            .update(cx, |view, cx| view.set_timezone(next, cx));
        cx.notify();
    }

    pub(in crate::view) fn set_show_timezone(
        &mut self,
        enabled: bool,
        cx: &mut gpui::Context<Self>,
    ) {
        self.history_view
            .update(cx, |view, cx| view.set_show_timezone(enabled, cx));
        cx.notify();
    }

    pub(in crate::view) fn active_repo_id(&self) -> Option<RepoId> {
        self.state.active_repo
    }

    pub(in crate::view) fn active_repo(&self) -> Option<&RepoState> {
        let repo_id = self.active_repo_id()?;
        self.state.repos.iter().find(|r| r.id == repo_id)
    }

    pub(in crate::view) fn history_visible_column_preferences(
        &self,
        cx: &gpui::App,
    ) -> (bool, bool, bool) {
        self.history_view
            .read(cx)
            .history_visible_column_preferences()
    }

    pub(in crate::view) fn conflict_advanced_autosolve_settings(&self) -> (bool, bool, bool) {
        (
            self.conflict_enable_whitespace_autosolve,
            self.conflict_enable_regex_autosolve,
            self.conflict_enable_history_autosolve,
        )
    }

    pub(in crate::view) fn set_conflict_enable_whitespace_autosolve(
        &mut self,
        enabled: bool,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.conflict_enable_whitespace_autosolve == enabled {
            return;
        }
        self.conflict_enable_whitespace_autosolve = enabled;
        cx.notify();
    }

    pub(in crate::view) fn set_conflict_enable_regex_autosolve(
        &mut self,
        enabled: bool,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.conflict_enable_regex_autosolve == enabled {
            return;
        }
        self.conflict_enable_regex_autosolve = enabled;
        cx.notify();
    }

    pub(in crate::view) fn set_conflict_enable_history_autosolve(
        &mut self,
        enabled: bool,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.conflict_enable_history_autosolve == enabled {
            return;
        }
        self.conflict_enable_history_autosolve = enabled;
        cx.notify();
    }

    pub(in crate::view) fn open_popover_at(
        &mut self,
        kind: PopoverKind,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let root_view = self.root_view.clone();
        let window_handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = root_view.update(cx, |root, cx| {
                    root.open_popover_at(kind, anchor, window, cx);
                });
            });
        });
    }

    pub(in crate::view) fn activate_context_menu_invoker(
        &mut self,
        invoker: SharedString,
        cx: &mut gpui::Context<Self>,
    ) {
        let _ = self.root_view.update(cx, move |root, cx| {
            root.set_active_context_menu_invoker(Some(invoker), cx);
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::view) fn open_conflict_resolver_input_row_context_menu(
        &mut self,
        invoker: SharedString,
        line_label: SharedString,
        line_target: ResolverPickTarget,
        chunk_label: SharedString,
        chunk_target: ResolverPickTarget,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.activate_context_menu_invoker(invoker, cx);
        self.open_popover_at(
            PopoverKind::ConflictResolverInputRowMenu {
                line_label,
                line_target,
                chunk_label,
                chunk_target,
            },
            anchor,
            window,
            cx,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::view) fn open_conflict_resolver_chunk_context_menu(
        &mut self,
        invoker: SharedString,
        conflict_ix: usize,
        has_base: bool,
        is_three_way: bool,
        selected_choices: Vec<conflict_resolver::ConflictChoice>,
        output_line_ix: Option<usize>,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.activate_context_menu_invoker(invoker, cx);
        self.open_popover_at(
            PopoverKind::ConflictResolverChunkMenu {
                conflict_ix,
                has_base,
                is_three_way,
                selected_choices,
                output_line_ix,
            },
            anchor,
            window,
            cx,
        );
    }

    pub(in crate::view) fn conflict_resolver_selected_choices_for_conflict_ix(
        &self,
        conflict_ix: usize,
    ) -> Vec<conflict_resolver::ConflictChoice> {
        conflict_group_selected_choices_for_ix(
            &self.conflict_resolver.marker_segments,
            &self.conflict_resolver.conflict_region_indices,
            conflict_ix,
        )
    }

    pub(in crate::view) fn conflict_resolver_has_base_for_conflict_ix(
        &self,
        conflict_ix: usize,
    ) -> bool {
        self.conflict_resolver
            .marker_segments
            .iter()
            .filter_map(|seg| match seg {
                conflict_resolver::ConflictSegment::Block(block) => Some(block.base.is_some()),
                _ => None,
            })
            .nth(conflict_ix)
            .unwrap_or(false)
    }

    pub(in crate::view) fn open_conflict_resolver_output_context_menu(
        &mut self,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let (selected_text, cursor_offset, clicked_offset, content) =
            self.conflict_resolver_input.read_with(cx, |i, _| {
                (
                    i.selected_text(),
                    i.cursor_offset(),
                    i.offset_for_position(anchor),
                    i.text().to_string(),
                )
            });
        let context_line =
            conflict_resolver_output_context_line(&content, cursor_offset, Some(clicked_offset));

        if let Some(marker) = resolved_output_marker_for_line(
            &self.conflict_resolver.marker_segments,
            &content,
            context_line,
        ) {
            let is_three_way = self.conflict_resolver.view_mode
                == conflict_resolver::ConflictResolverViewMode::ThreeWay;
            let selected_choices =
                self.conflict_resolver_selected_choices_for_conflict_ix(marker.conflict_ix);
            let has_base = self.conflict_resolver_has_base_for_conflict_ix(marker.conflict_ix);
            let invoker: SharedString = format!(
                "resolver_output_chunk_menu_{}_{}",
                marker.conflict_ix, context_line
            )
            .into();
            self.open_conflict_resolver_chunk_context_menu(
                invoker,
                marker.conflict_ix,
                has_base,
                is_three_way,
                selected_choices,
                Some(context_line),
                anchor,
                window,
                cx,
            );
            return;
        }

        let is_three_way = self.conflict_resolver.view_mode
            == conflict_resolver::ConflictResolverViewMode::ThreeWay;

        let (has_source_a, has_source_b, has_source_c) = if is_three_way {
            (
                context_line < self.conflict_resolver.three_way_lines.base.len(),
                context_line < self.conflict_resolver.three_way_lines.ours.len(),
                context_line < self.conflict_resolver.three_way_lines.theirs.len(),
            )
        } else {
            (
                context_line < self.conflict_resolver.diff_rows.len()
                    && self
                        .conflict_resolver
                        .diff_rows
                        .get(context_line)
                        .and_then(|r| r.old.as_ref())
                        .is_some(),
                context_line < self.conflict_resolver.diff_rows.len()
                    && self
                        .conflict_resolver
                        .diff_rows
                        .get(context_line)
                        .and_then(|r| r.new.as_ref())
                        .is_some(),
                false,
            )
        };

        self.open_popover_at(
            PopoverKind::ConflictResolverOutputMenu {
                cursor_line: context_line,
                selected_text,
                has_source_a,
                has_source_b,
                has_source_c,
                is_three_way,
            },
            anchor,
            window,
            cx,
        );
    }

    pub(in crate::view) fn open_popover_at_cursor(
        &mut self,
        kind: PopoverKind,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let root_view = self.root_view.clone();
        let window_handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = root_view.update(cx, |root, cx| {
                    root.open_popover_at(kind, root.last_mouse_pos, window, cx);
                });
            });
        });
    }

    pub(in crate::view) fn clear_status_multi_selection(
        &mut self,
        repo_id: RepoId,
        cx: &mut gpui::Context<Self>,
    ) {
        let _ = self.root_view.update(cx, |root, cx| {
            root.details_pane.update(cx, |pane, cx| {
                pane.status_multi_selection.remove(&repo_id);
                cx.notify();
            });
        });
    }

    pub(in crate::view) fn scroll_status_list_to_ix(
        &mut self,
        area: DiffArea,
        ix: usize,
        cx: &mut gpui::Context<Self>,
    ) {
        let _ = self.root_view.update(cx, |root, cx| {
            root.details_pane
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

    pub(in crate::view) fn set_tooltip_text_if_changed(
        &mut self,
        next: Option<SharedString>,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let _ = self
            .tooltip_host
            .update(cx, |host, cx| host.set_tooltip_text_if_changed(next, cx));
        false
    }

    pub(in crate::view) fn clear_tooltip_if_matches(
        &mut self,
        tooltip: &SharedString,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let tooltip = tooltip.clone();
        let _ = self
            .tooltip_host
            .update(cx, |host, cx| host.clear_tooltip_if_matches(&tooltip, cx));
        false
    }

    pub(super) fn apply_state_snapshot(
        &mut self,
        next: Arc<AppState>,
        cx: &mut gpui::Context<Self>,
    ) {
        let prev_active_repo_id = self.state.active_repo;
        let prev_diff_target = self
            .active_repo()
            .and_then(|r| r.diff_state.diff_target.as_ref())
            .cloned();

        let next_repo_id = next.active_repo;
        let next_repo = next_repo_id.and_then(|id| next.repos.iter().find(|r| r.id == id));
        let next_diff_target = next_repo
            .and_then(|r| r.diff_state.diff_target.as_ref())
            .cloned();
        let next_diff_rev = next_repo.map(|r| r.diff_state.diff_rev).unwrap_or(0);

        if prev_diff_target != next_diff_target {
            self.diff_selection_anchor = None;
            self.diff_selection_range = None;
            self.diff_autoscroll_pending = next_diff_target.is_some();
        }

        self.state = next;

        self.sync_conflict_resolver(cx);

        if prev_active_repo_id != next_repo_id {
            self.history_view.update(cx, |view, _| {
                view.history_scroll
                    .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
            });
        }

        let should_rebuild_diff_cache = self.diff_cache_repo_id != next_repo_id
            || self.diff_cache_rev != next_diff_rev
            || self.diff_cache_target != next_diff_target;
        if should_rebuild_diff_cache {
            self.rebuild_diff_cache(cx);
        }

        // History caches are now managed by HistoryView.
    }

    pub(in crate::view) fn cached_path_display(&self, path: &std::path::PathBuf) -> SharedString {
        let mut cache = self.path_display_cache.borrow_mut();
        path_display::cached_path_display(&mut cache, path)
    }

    pub(in crate::view) fn touch_diff_text_layout_cache(
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
    }

    /// Prune the layout cache if it has grown past the high-water mark.
    /// Call once per render frame (after bumping the epoch), **not** from
    /// the per-row `touch_diff_text_layout_cache` hot path.
    pub(in crate::view) fn prune_diff_text_layout_cache(&mut self) {
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

    pub(in crate::view) fn diff_text_segments_cache_get(
        &self,
        key: usize,
    ) -> Option<&CachedDiffStyledText> {
        self.diff_text_segments_cache
            .get(key)
            .and_then(Option::as_ref)
    }

    pub(in crate::view) fn file_diff_split_cache_key(
        &self,
        row_ix: usize,
        region: DiffTextRegion,
    ) -> Option<usize> {
        let base = row_ix.checked_mul(2)?;
        match region {
            DiffTextRegion::SplitLeft => Some(base),
            DiffTextRegion::SplitRight => base.checked_add(1),
            DiffTextRegion::Inline => None,
        }
    }

    pub(in crate::view) fn diff_text_segments_cache_set(
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

    pub(in crate::view) fn is_file_diff_view_active(&self) -> bool {
        let Some(repo) = self.active_repo() else {
            return false;
        };
        self.file_diff_cache_repo_id == Some(repo.id)
            && self.file_diff_cache_rev == repo.diff_state.diff_file_rev
            && self.file_diff_cache_target == repo.diff_state.diff_target
            && self.file_diff_cache_path.is_some()
    }

    pub(in crate::view) fn is_file_image_diff_view_active(&self) -> bool {
        let Some(repo) = self.active_repo() else {
            return false;
        };
        self.file_image_diff_cache_repo_id == Some(repo.id)
            && self.file_image_diff_cache_rev == repo.diff_state.diff_file_rev
            && self.file_image_diff_cache_target == repo.diff_state.diff_target
            && self.file_image_diff_cache_path.is_some()
            && (self.file_image_diff_cache_old.is_some()
                || self.file_image_diff_cache_new.is_some()
                || self.file_image_diff_cache_old_svg_path.is_some()
                || self.file_image_diff_cache_new_svg_path.is_some())
    }

    pub(in crate::view) fn consume_suppress_click_after_drag(&mut self) -> bool {
        if self.diff_suppress_clicks_remaining > 0 {
            self.diff_suppress_clicks_remaining =
                self.diff_suppress_clicks_remaining.saturating_sub(1);
            return true;
        }
        false
    }

    pub(super) fn diff_src_ixs_for_visible_ix(&self, visible_ix: usize) -> Vec<usize> {
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
                        let mut out = Vec::with_capacity(2);
                        if let Some(ix) = old_src_ix {
                            out.push(*ix);
                        }
                        if let Some(ix) = new_src_ix
                            && out.first().copied() != Some(*ix)
                        {
                            out.push(*ix);
                        }
                        out
                    }
                }
            }
        }
    }

    pub(super) fn diff_enclosing_hunk_src_ix(&self, src_ix: usize) -> Option<usize> {
        enclosing_hunk_src_ix(&self.diff_cache, src_ix)
    }

    pub(in crate::view) fn select_all_diff_text(&mut self) {
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

    pub(super) fn select_diff_text_rows_range(
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

    pub(in crate::view) fn double_click_select_diff_text(
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
                        matches!(line.kind, gitcomet_core::domain::DiffLineKind::Hunk)
                            || (matches!(line.kind, gitcomet_core::domain::DiffLineKind::Header)
                                && line.text.starts_with("diff --git "))
                    })
                    .unwrap_or(list_len - 1),
                DiffClickKind::FileHeader => self
                    .diff_next_boundary_visible_ix(visible_ix, |src_ix| {
                        let line = &self.diff_cache[src_ix];
                        matches!(line.kind, gitcomet_core::domain::DiffLineKind::Header)
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

    pub(super) fn split_next_boundary_visible_ix(
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

    pub(super) fn diff_next_boundary_visible_ix(
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

    pub(in crate::view) fn sync_diff_split_vertical_scroll(&mut self) {
        let left_handle = self.diff_scroll.0.borrow().base_handle.clone();
        let right_handle = self.diff_split_right_scroll.0.borrow().base_handle.clone();
        let left_offset = left_handle.offset();
        let right_offset = right_handle.offset();

        if left_offset.y == right_offset.y {
            self.diff_split_last_synced_y = left_offset.y;
            return;
        }

        let last_synced_y = self.diff_split_last_synced_y;
        let left_changed = left_offset.y != last_synced_y;
        let right_changed = right_offset.y != last_synced_y;

        let master_y = match (left_changed, right_changed) {
            (true, false) => left_offset.y,
            (false, true) => right_offset.y,
            // If both changed (or neither changed), prefer the left scroll (the vertical scrollbar).
            _ => left_offset.y,
        };

        left_handle.set_offset(point(left_offset.x, master_y));
        right_handle.set_offset(point(right_offset.x, master_y));
        self.diff_split_last_synced_y = master_y;
    }

    pub(in crate::view) fn main_pane_content_width(&self, cx: &mut gpui::Context<Self>) -> Pixels {
        let fallback_sidebar = px(280.0);
        let fallback_details = px(420.0);
        let (sidebar_w, details_w) = self
            .root_view
            .update(cx, |root, _cx| (root.sidebar_width, root.details_width))
            .unwrap_or((fallback_sidebar, fallback_details));

        let handles_w = px(PANE_RESIZE_HANDLE_PX) * 2.0;
        (self.last_window_size.width - sidebar_w - details_w - handles_w).max(px(0.0))
    }
}
