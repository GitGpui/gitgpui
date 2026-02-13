use super::*;

pub(super) fn panel(this: &mut PopoverHost, cx: &mut gpui::Context<PopoverHost>) -> gpui::Div {
    let theme = this.theme;
    let close = cx.listener(|this, _e: &ClickEvent, _w, cx| this.close_popover(cx));

    let active_repo = this.active_repo();
    let active_repo_id = active_repo.map(|r| r.id);
    let rebase_in_progress = active_repo
        .and_then(|r| match &r.rebase_in_progress {
            Loadable::Ready(v) => Some(*v),
            _ => None,
        })
        .unwrap_or(false);

    let selected_commit = active_repo.and_then(|r| r.selected_commit.clone());
    let selected_short = selected_commit.as_ref().map(|id| {
        let sha = id.as_ref();
        sha.get(0..8).unwrap_or(sha).to_string()
    });

    let separator = || {
        div()
            .h(px(1.0))
            .w_full()
            .bg(theme.colors.border)
            .my(px(4.0))
    };

    let section_label = |id: &'static str, text: &'static str| {
        div()
            .id(id)
            .px_2()
            .pt(px(6.0))
            .pb(px(4.0))
            .text_xs()
            .text_color(theme.colors.text_muted)
            .child(text)
    };

    let entry = |id: &'static str, label: SharedString, disabled: bool| {
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .px_2()
            .py_1()
            .when(!disabled, |d| {
                d.hover(move |s| s.bg(theme.colors.hover))
                    .active(move |s| s.bg(theme.colors.active))
            })
            .when(disabled, |d| d.text_color(theme.colors.text_muted))
            .child(label)
    };

    let mut install_desktop = div()
        .id("app_menu_install_desktop")
        .debug_selector(|| "app_menu_install_desktop".to_string())
        .px_2()
        .py_1()
        .hover(move |s| s.bg(theme.colors.hover))
        .active(move |s| s.bg(theme.colors.active))
        .child("Install desktop integration");

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        install_desktop = install_desktop.on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.install_linux_desktop_integration(cx);
            this.popover = None;
            this.popover_anchor = None;
            cx.notify();
        }));
    }

    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    {
        install_desktop = install_desktop.text_color(theme.colors.text_muted);
    }

    div()
        .flex()
        .flex_col()
        .min_w(px(200.0))
        .child(section_label("app_menu_app_section", "Application"))
        .child(
            entry("app_menu_settings", "Settings…".into(), false).on_click(cx.listener(
                |this, e: &ClickEvent, _w, cx| {
                    this.popover = Some(PopoverKind::Settings);
                    this.popover_anchor = Some(e.position());
                    this.settings_date_format_open = false;
                    cx.notify();
                },
            )),
        )
        .child(separator())
        .child(section_label("app_menu_repo_section", "Repository"))
        .child(
            entry(
                "app_menu_rebase",
                "Rebase onto…".into(),
                active_repo_id.is_none() || rebase_in_progress,
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                if rebase_in_progress {
                    return;
                }
                this.open_popover_at(
                    PopoverKind::RebasePrompt { repo_id },
                    e.position(),
                    window,
                    cx,
                );
            })),
        )
        .child(
            entry(
                "app_menu_rebase_continue",
                "Rebase --continue".into(),
                active_repo_id.is_none() || !rebase_in_progress,
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                if !rebase_in_progress {
                    return;
                }
                this.store.dispatch(Msg::RebaseContinue { repo_id });
                this.popover = None;
                this.popover_anchor = None;
                cx.notify();
            })),
        )
        .child(
            entry(
                "app_menu_rebase_abort",
                "Rebase --abort".into(),
                active_repo_id.is_none() || !rebase_in_progress,
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                if !rebase_in_progress {
                    return;
                }
                this.store.dispatch(Msg::RebaseAbort { repo_id });
                this.popover = None;
                this.popover_anchor = None;
                cx.notify();
            })),
        )
        .child(separator())
        .child(section_label("app_menu_remotes_section", "Remotes"))
        .child(
            entry(
                "app_menu_add_remote",
                "Add remote…".into(),
                active_repo_id.is_none(),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                this.open_popover_at(
                    PopoverKind::RemoteAddPrompt { repo_id },
                    e.position(),
                    window,
                    cx,
                );
            })),
        )
        .child(
            entry(
                "app_menu_edit_remote_fetch_url",
                "Edit remote fetch URL…".into(),
                active_repo_id.is_none(),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                this.ensure_remote_picker_search_input(window, cx);
                this.open_popover_at(
                    PopoverKind::RemoteUrlPicker {
                        repo_id,
                        kind: RemoteUrlKind::Fetch,
                    },
                    e.position(),
                    window,
                    cx,
                );
            })),
        )
        .child(
            entry(
                "app_menu_edit_remote_push_url",
                "Edit remote push URL…".into(),
                active_repo_id.is_none(),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                this.ensure_remote_picker_search_input(window, cx);
                this.open_popover_at(
                    PopoverKind::RemoteUrlPicker {
                        repo_id,
                        kind: RemoteUrlKind::Push,
                    },
                    e.position(),
                    window,
                    cx,
                );
            })),
        )
        .child(
            entry(
                "app_menu_remove_remote",
                "Remove remote…".into(),
                active_repo_id.is_none(),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                this.ensure_remote_picker_search_input(window, cx);
                this.open_popover_at(
                    PopoverKind::RemoteRemovePicker { repo_id },
                    e.position(),
                    window,
                    cx,
                );
            })),
        )
        .child(separator())
        .child(section_label("app_menu_patches_section", "Patches"))
        .child({
            let disabled = active_repo_id.is_none() || selected_commit.is_none();
            let label: SharedString = selected_short
                .as_ref()
                .map(|s| format!("Export patch for {s}…").into())
                .unwrap_or_else(|| "Export patch…".into());
            let selected_commit = selected_commit.clone();
            let selected_short = selected_short.clone();

            entry("app_menu_export_patch", label, disabled).on_click(cx.listener(
                move |this, _e: &ClickEvent, window, cx| {
                    let Some(repo_id) = active_repo_id else {
                        return;
                    };
                    let Some(commit_id) = selected_commit.clone() else {
                        return;
                    };
                    cx.stop_propagation();
                    let view = cx.weak_entity();
                    let short = selected_short.clone().unwrap_or_else(|| {
                        let sha = commit_id.as_ref();
                        sha.get(0..8).unwrap_or(sha).to_string()
                    });
                    let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                        files: false,
                        directories: true,
                        multiple: false,
                        prompt: Some("Export patch to folder".into()),
                    });
                    window
                        .spawn(cx, async move |cx| {
                            let result = rx.await;
                            let paths = match result {
                                Ok(Ok(Some(paths))) => paths,
                                Ok(Ok(None)) => return,
                                Ok(Err(_)) | Err(_) => return,
                            };
                            let Some(folder) = paths.into_iter().next() else {
                                return;
                            };
                            let dest = folder.join(format!("commit-{short}.patch"));
                            let _ = view.update(cx, |this, cx| {
                                this.store.dispatch(Msg::ExportPatch {
                                    repo_id,
                                    commit_id: commit_id.clone(),
                                    dest,
                                });
                                cx.notify();
                            });
                        })
                        .detach();
                    this.popover = None;
                    this.popover_anchor = None;
                    cx.notify();
                },
            ))
        })
        .child(
            entry(
                "app_menu_apply_patch",
                "Apply patch…".into(),
                active_repo_id.is_none(),
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, window, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                cx.stop_propagation();
                let view = cx.weak_entity();
                let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some("Select patch file".into()),
                });
                window
                    .spawn(cx, async move |cx| {
                        let result = rx.await;
                        let paths = match result {
                            Ok(Ok(Some(paths))) => paths,
                            Ok(Ok(None)) => return,
                            Ok(Err(_)) | Err(_) => return,
                        };
                        let Some(patch) = paths.into_iter().next() else {
                            return;
                        };
                        let _ = view.update(cx, |this, cx| {
                            this.store.dispatch(Msg::ApplyPatch { repo_id, patch });
                            cx.notify();
                        });
                    })
                    .detach();
                this.popover = None;
                this.popover_anchor = None;
                cx.notify();
            })),
        )
        .child(separator())
        .child(section_label("app_menu_worktrees_section", "Worktrees"))
        .child(
            entry(
                "app_menu_add_worktree",
                "Add worktree…".into(),
                active_repo_id.is_none(),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                this.open_popover_at(
                    PopoverKind::WorktreeAddPrompt { repo_id },
                    e.position(),
                    window,
                    cx,
                );
            })),
        )
        .child(
            entry(
                "app_menu_open_worktree",
                "Open worktree…".into(),
                active_repo_id.is_none(),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                this.open_popover_at(
                    PopoverKind::WorktreeOpenPicker { repo_id },
                    e.position(),
                    window,
                    cx,
                );
            })),
        )
        .child(
            entry(
                "app_menu_remove_worktree",
                "Remove worktree…".into(),
                active_repo_id.is_none(),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                this.open_popover_at(
                    PopoverKind::WorktreeRemovePicker { repo_id },
                    e.position(),
                    window,
                    cx,
                );
            })),
        )
        .child(separator())
        .child(section_label("app_menu_submodules_section", "Submodules"))
        .child(
            entry(
                "app_menu_add_submodule",
                "Add submodule…".into(),
                active_repo_id.is_none(),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                this.open_popover_at(
                    PopoverKind::SubmoduleAddPrompt { repo_id },
                    e.position(),
                    window,
                    cx,
                );
            })),
        )
        .child(
            entry(
                "app_menu_update_submodules",
                "Update submodules".into(),
                active_repo_id.is_none(),
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                this.store.dispatch(Msg::UpdateSubmodules { repo_id });
                this.popover = None;
                this.popover_anchor = None;
                cx.notify();
            })),
        )
        .child(
            entry(
                "app_menu_open_submodule",
                "Open submodule…".into(),
                active_repo_id.is_none(),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                this.open_popover_at(
                    PopoverKind::SubmoduleOpenPicker { repo_id },
                    e.position(),
                    window,
                    cx,
                );
            })),
        )
        .child(
            entry(
                "app_menu_remove_submodule",
                "Remove submodule…".into(),
                active_repo_id.is_none(),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                let Some(repo_id) = active_repo_id else {
                    return;
                };
                this.open_popover_at(
                    PopoverKind::SubmoduleRemovePicker { repo_id },
                    e.position(),
                    window,
                    cx,
                );
            })),
        )
        .child(separator())
        .child(install_desktop)
        .child(
            div()
                .id("app_menu_quit")
                .debug_selector(|| "app_menu_quit".to_string())
                .px_2()
                .py_1()
                .hover(move |s| s.bg(theme.colors.hover))
                .active(move |s| s.bg(theme.colors.active))
                .child("Quit")
                .on_click(cx.listener(|_this, _e: &ClickEvent, _w, cx| {
                    cx.quit();
                })),
        )
        .child(
            div()
                .id("app_menu_close")
                .debug_selector(|| "app_menu_close".to_string())
                .px_2()
                .py_1()
                .hover(move |s| s.bg(theme.colors.hover))
                .active(move |s| s.bg(theme.colors.active))
                .child("Close")
                .on_click(close),
        )
}
